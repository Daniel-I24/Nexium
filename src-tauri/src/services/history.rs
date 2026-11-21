use chrono::Local;
use rusqlite::params;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    models::{Category, HistoryEntry, InteractionStatus},
    providers::ai::schema::plain_text,
};

const MAX_ENTRIES: i64 = 500;
/// Un fallo que la interfaz registra tiene el mismo límite que una solicitud y su causa, uno algo mayor.
pub const MAX_FAILURE_REQUEST: usize = 500;
pub const MAX_FAILURE_CAUSE: usize = 600;

#[derive(Clone)]
pub struct HistoryService {
    db: Database,
}

impl HistoryService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Solo texto: las capturas, el audio y las claves no llegan a esta capa.
    pub fn record(
        &self,
        request: &str,
        category: Category,
        status: InteractionStatus,
        response: &str,
    ) -> AppResult<(i64, String)> {
        let created_at = Local::now().to_rfc3339();
        let connection = self.db.lock()?;
        connection.execute(
            "INSERT INTO history (request, created_at, category, status, response) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![request, created_at, category.as_str(), status.as_str(), response],
        )?;
        let id = connection.last_insert_rowid();
        connection.execute(
            "DELETE FROM history WHERE id <= ?1",
            params![id - MAX_ENTRIES],
        )?;
        Ok((id, created_at))
    }

    pub fn complete(&self, id: i64, response: &str) -> AppResult<()> {
        self.db.lock()?.execute(
            "UPDATE history SET status = ?1, response = ?2 WHERE id = ?3",
            params![InteractionStatus::Completed.as_str(), response, id],
        )?;
        Ok(())
    }

    /// Lleva una entrada a su nuevo estado. Devuelve su solicitud original y su fecha, o `None` si ya no existe
    /// (por ejemplo, si se borró el historial mientras tanto).
    pub fn update(
        &self,
        id: i64,
        category: Category,
        status: InteractionStatus,
        response: &str,
    ) -> AppResult<Option<(String, String)>> {
        let connection = self.db.lock()?;
        let changed = connection.execute(
            "UPDATE history SET category = ?1, status = ?2, response = ?3 WHERE id = ?4",
            params![category.as_str(), status.as_str(), response, id],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        Ok(Some(connection.query_row(
            "SELECT request, created_at FROM history WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?))
    }

    pub fn list(&self, status: Option<InteractionStatus>) -> AppResult<Vec<HistoryEntry>> {
        let connection = self.db.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, request, created_at, category, status, response FROM history
             WHERE (?1 IS NULL OR status = ?1) ORDER BY id DESC",
        )?;
        let rows = statement.query_map(params![status.map(InteractionStatus::as_str)], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                request: row.get(1)?,
                created_at: row.get(2)?,
                category: Category::parse(&row.get::<_, String>(3)?),
                status: InteractionStatus::parse(&row.get::<_, String>(4)?),
                // Las entradas de versiones anteriores pueden traer Markdown de la IA: se limpia al mostrarlas,
                // sin tocar la base de datos.
                response: plain_text(&row.get::<_, String>(5)?),
            })
        })?;
        rows.collect::<Result<_, _>>().map_err(AppError::from)
    }

    /// Un fallo que solo vio la interfaz (micrófono, transcripción, conexión con el backend) también queda registrado.
    pub fn record_failure(&self, request: &str, cause: &str) -> AppResult<(i64, String)> {
        let request = request.trim();
        let cause = cause.trim();
        if request.is_empty() || cause.is_empty() {
            return Err(AppError::Validation(
                "Falta la solicitud o la causa del fallo.".into(),
            ));
        }
        if request.chars().count() > MAX_FAILURE_REQUEST
            || cause.chars().count() > MAX_FAILURE_CAUSE
        {
            return Err(AppError::Validation(
                "El fallo es demasiado largo para el historial.".into(),
            ));
        }
        self.record(request, Category::General, InteractionStatus::Failed, cause)
    }

    pub fn delete(&self, id: i64) -> AppResult<()> {
        self.db
            .lock()?
            .execute("DELETE FROM history WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear(&self) -> AppResult<()> {
        self.db.lock()?.execute("DELETE FROM history", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> HistoryService {
        HistoryService::new(Database::in_memory().unwrap())
    }

    #[test]
    fn records_and_filters_by_status() {
        let history = service();
        history
            .record("a", Category::Time, InteractionStatus::Completed, "ok")
            .unwrap();
        history
            .record("b", Category::Weather, InteractionStatus::Failed, "sin red")
            .unwrap();
        assert_eq!(history.list(None).unwrap().len(), 2);
        let failed = history.list(Some(InteractionStatus::Failed)).unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].request, "b");
    }

    #[test]
    fn pending_entries_can_be_completed() {
        let history = service();
        let (id, _) = history
            .record(
                "a",
                Category::Reminders,
                InteractionStatus::Pending,
                "aclaración",
            )
            .unwrap();
        history.complete(id, "guardado").unwrap();
        assert_eq!(
            history.list(None).unwrap()[0].status,
            InteractionStatus::Completed
        );
    }

    #[test]
    fn deletes_one_or_all() {
        let history = service();
        let (id, _) = history
            .record("a", Category::Time, InteractionStatus::Completed, "ok")
            .unwrap();
        history
            .record("b", Category::Time, InteractionStatus::Completed, "ok")
            .unwrap();
        history.delete(id).unwrap();
        assert_eq!(history.list(None).unwrap().len(), 1);
        history.clear().unwrap();
        assert!(history.list(None).unwrap().is_empty());
    }

    #[test]
    fn old_entries_are_shown_without_markdown_and_the_database_keeps_them() {
        let history = service();
        let stored = "**Error 0x80070002**: falta `configuracion.ini`

- Reinstala el programa";
        history
            .record(
                "pantalla",
                Category::Screen,
                InteractionStatus::Completed,
                stored,
            )
            .unwrap();
        assert_eq!(
            history.list(None).unwrap()[0].response,
            "Error 0x80070002: falta configuracion.ini Reinstala el programa"
        );
        let raw: String = history
            .db
            .lock()
            .unwrap()
            .query_row("SELECT response FROM history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(raw, stored);
    }

    #[test]
    fn failures_seen_only_by_the_interface_are_recorded_as_failed() {
        let history = service();
        history
            .record_failure("Solicitud por voz", "El micrófono no captó ningún sonido.")
            .unwrap();
        let entry = &history.list(Some(InteractionStatus::Failed)).unwrap()[0];
        assert_eq!(entry.request, "Solicitud por voz");
        assert_eq!(entry.response, "El micrófono no captó ningún sonido.");
        assert!(history.record_failure(" ", "causa").is_err());
        assert!(history.record_failure("a", "").is_err());
        assert!(history
            .record_failure(&"a".repeat(MAX_FAILURE_REQUEST + 1), "causa")
            .is_err());
    }

    #[test]
    fn keeps_a_bounded_number_of_entries() {
        let history = service();
        for _ in 0..(MAX_ENTRIES + 5) {
            history
                .record("a", Category::Time, InteractionStatus::Completed, "ok")
                .unwrap();
        }
        assert_eq!(history.list(None).unwrap().len(), MAX_ENTRIES as usize);
    }
}
