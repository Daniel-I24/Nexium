use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

const MIGRATIONS: &[&str] = &[r"
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE reminders (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        content TEXT NOT NULL,
        due_at TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        created_at TEXT NOT NULL
    );
    CREATE INDEX reminders_due ON reminders (status, due_at);
    CREATE TABLE history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        request TEXT NOT NULL,
        created_at TEXT NOT NULL,
        category TEXT NOT NULL,
        status TEXT NOT NULL,
        response TEXT NOT NULL
    );
    CREATE TABLE ai_usage (
        day TEXT PRIMARY KEY,
        requests INTEGER NOT NULL DEFAULT 0,
        tokens INTEGER NOT NULL DEFAULT 0
    );
"];

/// Conexión única compartida: las consultas son breves y locales.
#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        Self::prepare(Connection::open(path)?)
    }

    pub fn in_memory() -> AppResult<Self> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(mut connection: Connection) -> AppResult<Self> {
        connection.pragma_update(None, "journal_mode", "WAL")?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        for (index, migration) in MIGRATIONS.iter().enumerate().skip(usize::try_from(version).unwrap_or(0)) {
            let transaction = connection.transaction()?;
            transaction.execute_batch(migration)?;
            transaction.pragma_update(None, "user_version", i64::try_from(index + 1).unwrap_or(i64::MAX))?;
            transaction.commit()?;
        }
        Ok(Self { connection: Arc::new(Mutex::new(connection)) })
    }

    pub fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| AppError::Operation("La base de datos local quedó bloqueada.".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_survives_reopening_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexium.db");
        {
            let db = Database::open(&path).unwrap();
            db.lock().unwrap().execute("INSERT INTO settings VALUES ('a', 'b')", []).unwrap();
        }
        let db = Database::open(&path).unwrap();
        let value: String = db.lock().unwrap().query_row("SELECT value FROM settings WHERE key='a'", [], |r| r.get(0)).unwrap();
        assert_eq!(value, "b");
    }

    #[test]
    fn reopening_does_not_repeat_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexium.db");
        Database::open(&path).unwrap();
        assert!(Database::open(&path).is_ok());
    }
}
