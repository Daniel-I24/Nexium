use chrono::Local;
use rusqlite::{params, OptionalExtension};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    models::{Category, HistoryEntry, InteractionStatus, PartSummary},
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

/// Estado, categoría y resumen de una petición con varias partes. Pendiente si alguna espera un permiso o una
/// confirmación; fallida solo si fallaron todas; si no, completada, y el resumen dice cuál falló.
pub fn combine_parts(parts: &[PartSummary]) -> (Category, InteractionStatus, String) {
    let all = |status: InteractionStatus| parts.iter().all(|part| part.status == status);
    let status = if parts
        .iter()
        .any(|part| part.status == InteractionStatus::Pending)
    {
        InteractionStatus::Pending
    } else if all(InteractionStatus::Failed) {
        InteractionStatus::Failed
    } else if all(InteractionStatus::Cancelled) {
        InteractionStatus::Cancelled
    } else {
        InteractionStatus::Completed
    };
    let category = match parts.first() {
        Some(first) if parts.iter().all(|part| part.category == first.category) => first.category,
        _ => Category::General,
    };
    // Sin números delante: el historial muestra el texto sin formato, y «1. » parecería una lista.
    let summary = parts
        .iter()
        .map(|part| {
            let marker = match part.status {
                InteractionStatus::Failed => " (falló)",
                InteractionStatus::Pending => " (pendiente)",
                InteractionStatus::Cancelled => " (cancelada)",
                InteractionStatus::Completed => "",
            };
            format!("{}{marker}", part.summary)
        })
        .collect::<Vec<_>>()
        .join(" · ");
    (category, status, summary)
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

    /// Una petición con varias solicitudes: una sola entrada, con el estado y el resultado de cada parte.
    pub fn record_parts(&self, request: &str, parts: &[PartSummary]) -> AppResult<(i64, String)> {
        let (category, status, summary) = combine_parts(parts);
        let (id, created_at) = self.record(request, category, status, &summary)?;
        self.db.lock()?.execute(
            "UPDATE history SET parts = ?1 WHERE id = ?2",
            params![serde_json::to_string(parts).unwrap_or_default(), id],
        )?;
        Ok((id, created_at))
    }

    /// Una entrada que ya existía (una solicitud que esperaba un permiso) pasa a tener varias partes.
    pub fn set_parts(&self, id: i64, parts: &[PartSummary]) -> AppResult<Option<(String, String)>> {
        let (category, status, summary) = combine_parts(parts);
        let Some(found) = self.update(id, category, status, &summary)? else {
            return Ok(None);
        };
        self.db.lock()?.execute(
            "UPDATE history SET parts = ?1 WHERE id = ?2",
            params![serde_json::to_string(parts).unwrap_or_default(), id],
        )?;
        Ok(Some(found))
    }

    /// Lleva una parte a su nuevo estado (tras un permiso o una confirmación) y recalcula la entrada. Devuelve
    /// la solicitud completa y su fecha, o `None` si la entrada o la parte ya no existen.
    pub fn update_part(
        &self,
        id: i64,
        index: usize,
        part: PartSummary,
    ) -> AppResult<Option<(String, String)>> {
        let connection = self.db.lock()?;
        let stored: Option<(String, String, Option<String>)> = connection
            .query_row(
                "SELECT request, created_at, parts FROM history WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((request, created_at, Some(json))) = stored else {
            return Ok(None);
        };
        let mut parts: Vec<PartSummary> = serde_json::from_str(&json).unwrap_or_default();
        let Some(slot) = parts.get_mut(index) else {
            return Ok(None);
        };
        // El fragmento de la petición no cambia: solo su estado y su resultado.
        *slot = PartSummary {
            request: std::mem::take(&mut slot.request),
            ..part
        };
        let (category, status, summary) = combine_parts(&parts);
        connection.execute(
            "UPDATE history SET category = ?1, status = ?2, response = ?3, parts = ?4 WHERE id = ?5",
            params![
                category.as_str(),
                status.as_str(),
                summary,
                serde_json::to_string(&parts).unwrap_or_default(),
                id
            ],
        )?;
        Ok(Some((request, created_at)))
    }

    pub fn list(&self, status: Option<InteractionStatus>) -> AppResult<Vec<HistoryEntry>> {
        let connection = self.db.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, request, created_at, category, status, response, parts FROM history
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
                parts: row
                    .get::<_, Option<String>>(6)?
                    .and_then(|json| serde_json::from_str(&json).ok()),
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

    fn part(request: &str, status: InteractionStatus) -> PartSummary {
        PartSummary {
            request: request.into(),
            category: Category::System,
            status,
            summary: format!("resultado de {request}"),
        }
    }

    #[test]
    fn a_request_with_several_parts_is_one_entry_with_each_result() {
        let history = service();
        let (id, _) = history
            .record_parts(
                "¿Qué hora es y cuánta batería tengo?",
                &[
                    part("hora", InteractionStatus::Completed),
                    part("batería", InteractionStatus::Pending),
                ],
            )
            .unwrap();
        let entry = &history.list(None).unwrap()[0];
        assert_eq!(entry.status, InteractionStatus::Pending);
        assert_eq!(
            entry.response,
            "resultado de hora · resultado de batería (pendiente)"
        );
        // Completar una parte conserva su fragmento y recalcula la entrada.
        history
            .update_part(id, 1, part("otro texto", InteractionStatus::Completed))
            .unwrap()
            .unwrap();
        let entry = &history.list(None).unwrap()[0];
        assert_eq!(entry.status, InteractionStatus::Completed);
        assert_eq!(entry.parts.as_ref().unwrap()[1].request, "batería");
        assert!(history
            .update_part(id, 7, part("x", InteractionStatus::Completed))
            .unwrap()
            .is_none());
    }

    #[test]
    fn the_status_of_several_parts() {
        use InteractionStatus::{Cancelled, Completed, Failed, Pending};
        let status = |statuses: &[InteractionStatus]| {
            let parts: Vec<_> = statuses.iter().map(|status| part("p", *status)).collect();
            combine_parts(&parts).1
        };
        assert_eq!(status(&[Completed, Pending]), Pending);
        assert_eq!(status(&[Completed, Failed]), Completed);
        assert_eq!(status(&[Failed, Failed]), Failed);
        assert_eq!(status(&[Cancelled, Cancelled]), Cancelled);
        assert_eq!(status(&[Cancelled, Completed]), Completed);
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
