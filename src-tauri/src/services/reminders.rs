use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Row};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    models::Reminder,
};

const MAX_CONTENT: usize = 300;
/// Un recordatorio se avisa si su hora pasó hace como mucho esto (el equipo pudo estar suspendido unos
/// minutos). Si pasó más, Nexium estaba cerrado a esa hora: queda vencido, sin aviso tardío.
pub const NOTIFY_GRACE: chrono::Duration = chrono::Duration::minutes(5);

/// Estados guardados. Pendiente: su hora no ha llegado. Avisado: la notificación se envió a su hora.
/// Vencido: la hora pasó sin que se pudiera avisar.
pub const PENDING: &str = "pending";
pub const NOTIFIED: &str = "notified";
pub const MISSED: &str = "missed";

/// Resultado de una revisión: los que hay que avisar ahora y cuántos acaban de quedar vencidos.
#[derive(Debug, Default)]
pub struct DueReminders {
    pub to_notify: Vec<Reminder>,
    pub missed: usize,
}

impl DueReminders {
    pub fn changed(&self) -> bool {
        !self.to_notify.is_empty() || self.missed > 0
    }
}

#[derive(Clone)]
pub struct ReminderService {
    db: Database,
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<Reminder> {
    Ok(Reminder {
        id: row.get(0)?,
        content: row.get(1)?,
        due_at: row.get(2)?,
        status: row.get(3)?,
    })
}

fn validate(content: &str, due_at: &str, now: DateTime<Utc>) -> AppResult<(String, String)> {
    let content = content.trim();
    if content.is_empty() || content.chars().count() > MAX_CONTENT {
        return Err(AppError::Validation(
            "El recordatorio debe tener entre 1 y 300 caracteres.".into(),
        ));
    }
    let due = DateTime::parse_from_rfc3339(due_at)
        .map_err(|_| AppError::Validation("La fecha del recordatorio no es válida.".into()))?;
    if due.with_timezone(&Utc) <= now {
        return Err(AppError::Validation(
            "La fecha del recordatorio ya pasó.".into(),
        ));
    }
    Ok((content.to_owned(), due.with_timezone(&Utc).to_rfc3339()))
}

impl ReminderService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create(&self, content: &str, due_at: &str, now: DateTime<Utc>) -> AppResult<Reminder> {
        let (content, due_at) = validate(content, due_at, now)?;
        let connection = self.db.lock()?;
        connection.execute(
            "INSERT INTO reminders (content, due_at, status, created_at) VALUES (?1, ?2, 'pending', ?3)",
            params![content, due_at, Local::now().to_rfc3339()],
        )?;
        Ok(Reminder {
            id: connection.last_insert_rowid(),
            content,
            due_at,
            status: PENDING.into(),
        })
    }

    pub fn update(
        &self,
        id: i64,
        content: &str,
        due_at: &str,
        now: DateTime<Utc>,
    ) -> AppResult<Reminder> {
        let (content, due_at) = validate(content, due_at, now)?;
        let changed = self.db.lock()?.execute(
            "UPDATE reminders SET content = ?1, due_at = ?2, status = 'pending' WHERE id = ?3",
            params![content, due_at, id],
        )?;
        if changed == 0 {
            return Err(AppError::Validation(
                "Ese recordatorio ya no existe.".into(),
            ));
        }
        Ok(Reminder {
            id,
            content,
            due_at,
            status: PENDING.into(),
        })
    }

    pub fn delete(&self, id: i64) -> AppResult<()> {
        self.db
            .lock()?
            .execute("DELETE FROM reminders WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list(&self) -> AppResult<Vec<Reminder>> {
        let connection = self.db.lock()?;
        let mut statement = connection
            .prepare("SELECT id, content, due_at, status FROM reminders ORDER BY due_at")?;
        let rows = statement.query_map([], from_row)?;
        rows.collect::<Result<_, _>>().map_err(AppError::from)
    }

    /// Los recordatorios cuya hora llegó. Los que pasaron hace más de `NOTIFY_GRACE` quedan vencidos aquí; los
    /// demás se devuelven, todavía pendientes, para avisarlos y marcarlos con `mark_notified`.
    pub fn take_due(&self, now: DateTime<Utc>) -> AppResult<DueReminders> {
        let connection = self.db.lock()?;
        let missed = connection.execute(
            "UPDATE reminders SET status = 'missed' WHERE status = 'pending' AND due_at < ?1",
            params![(now - NOTIFY_GRACE).to_rfc3339()],
        )?;
        let mut statement = connection.prepare("SELECT id, content, due_at, status FROM reminders WHERE status = 'pending' AND due_at <= ?1 ORDER BY due_at")?;
        let rows = statement.query_map(params![now.to_rfc3339()], from_row)?;
        Ok(DueReminders {
            to_notify: rows.collect::<Result<_, _>>()?,
            missed,
        })
    }

    /// Avisado si la notificación salió; vencido si el sistema no la aceptó.
    pub fn mark_notified(&self, id: i64, sent: bool) -> AppResult<()> {
        self.db.lock()?.execute(
            "UPDATE reminders SET status = ?1 WHERE id = ?2 AND status = 'pending'",
            params![if sent { NOTIFIED } else { MISSED }, id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 5, 12, 0, 0).unwrap()
    }

    fn at(hours: i64) -> String {
        (now() + Duration::hours(hours)).to_rfc3339()
    }

    fn service() -> ReminderService {
        ReminderService::new(Database::in_memory().unwrap())
    }

    #[test]
    fn creates_lists_updates_and_deletes() {
        let reminders = service();
        let created = reminders.create(" Entregar ", &at(2), now()).unwrap();
        assert_eq!(created.content, "Entregar");
        reminders
            .update(created.id, "Entregar ya", &at(3), now())
            .unwrap();
        assert_eq!(reminders.list().unwrap()[0].content, "Entregar ya");
        reminders.delete(created.id).unwrap();
        assert!(reminders.list().unwrap().is_empty());
    }

    #[test]
    fn rejects_past_dates_and_bad_input() {
        let reminders = service();
        assert!(reminders.create("a", &at(-1), now()).is_err());
        assert!(reminders.create("", &at(1), now()).is_err());
        assert!(reminders.create("a", "mañana", now()).is_err());
        assert!(reminders.create(&"x".repeat(301), &at(1), now()).is_err());
    }

    #[test]
    fn updating_a_missing_reminder_fails() {
        assert!(service().update(99, "a", &at(1), now()).is_err());
    }

    fn status_of(reminders: &ReminderService, content: &str) -> String {
        reminders
            .list()
            .unwrap()
            .into_iter()
            .find(|reminder| reminder.content == content)
            .unwrap()
            .status
    }

    #[test]
    fn a_reminder_on_time_is_notified_only_once() {
        let reminders = service();
        let created = reminders.create("pronto", &at(1), now()).unwrap();
        reminders.create("tarde", &at(5), now()).unwrap();
        let on_time = now() + Duration::hours(1) + Duration::seconds(20);
        let due = reminders.take_due(on_time).unwrap().to_notify;
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].content, "pronto");
        reminders.mark_notified(created.id, true).unwrap();
        assert!(!reminders.take_due(on_time).unwrap().changed());
        assert_eq!(status_of(&reminders, "pronto"), NOTIFIED);
        assert_eq!(status_of(&reminders, "tarde"), PENDING);
    }

    #[test]
    fn a_reminder_whose_time_passed_while_closed_is_missed_without_a_late_notice() {
        let reminders = service();
        reminders.create("ayer", &at(1), now()).unwrap();
        let reopened = now() + Duration::hours(1) + NOTIFY_GRACE + Duration::seconds(1);
        let checked = reminders.take_due(reopened).unwrap();
        assert!(checked.to_notify.is_empty());
        assert_eq!(checked.missed, 1);
        assert_eq!(status_of(&reminders, "ayer"), MISSED);
    }

    #[test]
    fn a_notice_the_system_rejects_is_missed() {
        let reminders = service();
        let created = reminders.create("sin aviso", &at(1), now()).unwrap();
        reminders.mark_notified(created.id, false).unwrap();
        assert_eq!(status_of(&reminders, "sin aviso"), MISSED);
    }

    #[test]
    fn editing_a_notified_or_missed_reminder_makes_it_pending_again() {
        let reminders = service();
        let created = reminders.create("otra vez", &at(1), now()).unwrap();
        reminders.mark_notified(created.id, true).unwrap();
        reminders
            .update(created.id, "otra vez", &at(2), now())
            .unwrap();
        assert_eq!(status_of(&reminders, "otra vez"), PENDING);
    }

    #[test]
    fn comparison_works_across_time_zones() {
        let reminders = service();
        reminders
            .create("zona", "2026-11-05T09:00:00-05:00", now())
            .unwrap();
        assert!(!reminders
            .take_due(Utc.with_ymd_and_hms(2026, 11, 5, 13, 59, 0).unwrap())
            .unwrap()
            .changed());
        assert_eq!(
            reminders
                .take_due(Utc.with_ymd_and_hms(2026, 11, 5, 14, 0, 1).unwrap())
                .unwrap()
                .to_notify
                .len(),
            1
        );
    }
}
