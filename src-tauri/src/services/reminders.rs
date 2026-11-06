use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Row};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    models::Reminder,
};

const MAX_CONTENT: usize = 300;

#[derive(Clone)]
pub struct ReminderService {
    db: Database,
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<Reminder> {
    Ok(Reminder { id: row.get(0)?, content: row.get(1)?, due_at: row.get(2)?, status: row.get(3)? })
}

fn validate(content: &str, due_at: &str, now: DateTime<Utc>) -> AppResult<(String, String)> {
    let content = content.trim();
    if content.is_empty() || content.chars().count() > MAX_CONTENT {
        return Err(AppError::Validation("El recordatorio debe tener entre 1 y 300 caracteres.".into()));
    }
    let due = DateTime::parse_from_rfc3339(due_at).map_err(|_| AppError::Validation("La fecha del recordatorio no es válida.".into()))?;
    if due.with_timezone(&Utc) <= now {
        return Err(AppError::Validation("La fecha del recordatorio ya pasó.".into()));
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
        Ok(Reminder { id: connection.last_insert_rowid(), content, due_at, status: "pending".into() })
    }

    pub fn update(&self, id: i64, content: &str, due_at: &str, now: DateTime<Utc>) -> AppResult<Reminder> {
        let (content, due_at) = validate(content, due_at, now)?;
        let changed = self.db.lock()?.execute(
            "UPDATE reminders SET content = ?1, due_at = ?2, status = 'pending' WHERE id = ?3",
            params![content, due_at, id],
        )?;
        if changed == 0 {
            return Err(AppError::Validation("Ese recordatorio ya no existe.".into()));
        }
        Ok(Reminder { id, content, due_at, status: "pending".into() })
    }

    pub fn delete(&self, id: i64) -> AppResult<()> {
        self.db.lock()?.execute("DELETE FROM reminders WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list(&self) -> AppResult<Vec<Reminder>> {
        let connection = self.db.lock()?;
        let mut statement = connection.prepare("SELECT id, content, due_at, status FROM reminders ORDER BY due_at")?;
        let rows = statement.query_map([], from_row)?;
        rows.collect::<Result<_, _>>().map_err(AppError::from)
    }

    /// Reserva los recordatorios vencidos: cada uno se notifica una sola vez.
    pub fn take_due(&self, now: DateTime<Utc>) -> AppResult<Vec<Reminder>> {
        let connection = self.db.lock()?;
        let due: Vec<Reminder> = {
            let mut statement = connection.prepare("SELECT id, content, due_at, status FROM reminders WHERE status = 'pending' AND due_at <= ?1 ORDER BY due_at")?;
            let rows = statement.query_map(params![now.to_rfc3339()], from_row)?;
            rows.collect::<Result<_, _>>()?
        };
        for reminder in &due {
            connection.execute("UPDATE reminders SET status = 'notified' WHERE id = ?1", params![reminder.id])?;
        }
        Ok(due)
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
        reminders.update(created.id, "Entregar ya", &at(3), now()).unwrap();
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

    #[test]
    fn due_reminders_are_notified_only_once() {
        let reminders = service();
        reminders.create("pronto", &at(1), now()).unwrap();
        reminders.create("tarde", &at(5), now()).unwrap();
        let later = now() + Duration::hours(2);
        let due = reminders.take_due(later).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].content, "pronto");
        assert!(reminders.take_due(later).unwrap().is_empty());
    }

    #[test]
    fn comparison_works_across_time_zones() {
        let reminders = service();
        reminders.create("zona", "2026-11-05T09:00:00-05:00", now()).unwrap();
        assert!(reminders.take_due(Utc.with_ymd_and_hms(2026, 11, 5, 13, 59, 0).unwrap()).unwrap().is_empty());
        assert_eq!(reminders.take_due(Utc.with_ymd_and_hms(2026, 11, 5, 14, 0, 1).unwrap()).unwrap().len(), 1);
    }
}
