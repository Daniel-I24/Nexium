use chrono::Local;
use rusqlite::{params, OptionalExtension};

use crate::{database::Database, error::AppResult};

/// Contador local de uso de IA. No sustituye a la cuota del proveedor: evita gastarla sin darse cuenta.
#[derive(Clone)]
pub struct UsageService {
    db: Database,
}

impl UsageService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn today() -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }

    pub fn today_totals(&self) -> AppResult<(u32, u64)> {
        let totals = self
            .db
            .lock()?
            .query_row("SELECT requests, tokens FROM ai_usage WHERE day = ?1", params![Self::today()], |row| {
                Ok((row.get::<_, u32>(0)?, u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0)))
            })
            .optional()?;
        Ok(totals.unwrap_or((0, 0)))
    }

    pub fn record(&self, tokens: u64) -> AppResult<()> {
        self.db.lock()?.execute(
            "INSERT INTO ai_usage (day, requests, tokens) VALUES (?1, 1, ?2)
             ON CONFLICT(day) DO UPDATE SET requests = requests + 1, tokens = tokens + ?2",
            params![Self::today(), i64::try_from(tokens).unwrap_or(i64::MAX)],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_requests_and_tokens() {
        let usage = UsageService::new(Database::in_memory().unwrap());
        usage.record(10).unwrap();
        usage.record(5).unwrap();
        assert_eq!(usage.today_totals().unwrap(), (2, 15));
    }
}
