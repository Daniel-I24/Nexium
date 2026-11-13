use std::{collections::HashSet, path::PathBuf};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
};

pub const DEFAULT_SPEECH_MODEL: &str = "small";
pub const DEFAULT_SPEECH_IDLE_MINUTES: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub daily_ai_limit: u32,
    pub speech_model: String,
    pub speech_idle_minutes: u32,
    pub weather_city: String,
    pub authorized_folders: Vec<String>,
    pub folders_confirmed: bool,
    /// Nombre del micrófono elegido. `None` es automático: el primer micrófono real.
    pub microphone: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            daily_ai_limit: 200,
            speech_model: DEFAULT_SPEECH_MODEL.into(),
            speech_idle_minutes: DEFAULT_SPEECH_IDLE_MINUTES,
            weather_city: String::new(),
            authorized_folders: Vec::new(),
            folders_confirmed: false,
            microphone: None,
        }
    }
}

impl Settings {
    pub fn validate(
        &self,
        known_folders: &[String],
        known_speech_models: &[&str],
    ) -> AppResult<()> {
        if !(1..=10_000).contains(&self.daily_ai_limit) {
            return Err(AppError::Validation(
                "El límite diario debe estar entre 1 y 10 000.".into(),
            ));
        }
        if !known_speech_models.contains(&self.speech_model.as_str()) {
            return Err(AppError::Validation("Ese modelo de voz no existe.".into()));
        }
        if !(1..=240).contains(&self.speech_idle_minutes) {
            return Err(AppError::Validation(
                "El tiempo para liberar el modelo de voz debe estar entre 1 y 240 minutos.".into(),
            ));
        }
        if self
            .microphone
            .as_ref()
            .is_some_and(|name| name.trim().is_empty() || name.chars().count() > 200)
        {
            return Err(AppError::Validation(
                "El nombre del micrófono no es válido.".into(),
            ));
        }
        if self.weather_city.chars().count() > 100 {
            return Err(AppError::Validation(
                "El nombre de la ciudad es demasiado largo.".into(),
            ));
        }
        let known: HashSet<&String> = known_folders.iter().collect();
        if self
            .authorized_folders
            .iter()
            .any(|folder| !known.contains(folder))
        {
            return Err(AppError::Validation(
                "Solo se pueden autorizar las carpetas que ofrece Nexium.".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SettingsService {
    db: Database,
}

impl SettingsService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn get(&self) -> AppResult<Settings> {
        let stored: Option<String> = self
            .db
            .lock()?
            .query_row("SELECT value FROM settings WHERE key = 'app'", [], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(stored
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default())
    }

    pub fn save(&self, settings: &Settings) -> AppResult<()> {
        let json =
            serde_json::to_string(settings).map_err(|e| AppError::Operation(e.to_string()))?;
        self.db.lock()?.execute("INSERT INTO settings (key, value) VALUES ('app', ?1) ON CONFLICT(key) DO UPDATE SET value = ?1", params![json])?;
        Ok(())
    }

    /// Identificador anónimo y aleatorio de esta instalación, creado la primera vez. Solo sirve para limitar el uso.
    pub fn install_id(&self) -> AppResult<String> {
        let db = self.db.lock()?;
        db.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('install_id', ?1)",
            params![Uuid::new_v4().to_string()],
        )?;
        Ok(db.query_row(
            "SELECT value FROM settings WHERE key = 'install_id'",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn authorized_paths(&self) -> AppResult<Vec<PathBuf>> {
        Ok(self
            .get()?
            .authorized_folders
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> SettingsService {
        SettingsService::new(Database::in_memory().unwrap())
    }

    #[test]
    fn the_installation_id_is_anonymous_and_stable() {
        let service = service();
        let id = service.install_id().unwrap();
        assert_eq!(Uuid::parse_str(&id).unwrap().get_version_num(), 4);
        assert_eq!(service.install_id().unwrap(), id);
        service.save(&Settings::default()).unwrap();
        assert_eq!(service.install_id().unwrap(), id);
    }

    #[test]
    fn settings_saved_by_older_versions_still_load() {
        let stored: Settings = serde_json::from_str(
            r#"{"aiProvider":"gemini","interpretModel":"x","visionModel":"y","dailyAiLimit":7}"#,
        )
        .unwrap();
        assert_eq!(stored.daily_ai_limit, 7);
    }

    #[test]
    fn the_voice_idle_period_is_validated_and_defaults_for_old_settings() {
        let mut settings = Settings::default();
        let models = ["small"];
        assert!(settings.validate(&[], &models).is_ok());
        settings.speech_idle_minutes = 0;
        assert!(settings.validate(&[], &models).is_err());
        settings.speech_idle_minutes = 241;
        assert!(settings.validate(&[], &models).is_err());
        let stored: Settings = serde_json::from_str(r#"{"speechModel":"small"}"#).unwrap();
        assert_eq!(stored.speech_idle_minutes, DEFAULT_SPEECH_IDLE_MINUTES);
    }

    #[test]
    fn saved_settings_persist() {
        let service = service();
        let mut settings = service.get().unwrap();
        settings.weather_city = "Medellín".into();
        service.save(&settings).unwrap();
        assert_eq!(service.get().unwrap().weather_city, "Medellín");
    }

    #[test]
    fn rejects_folders_nexium_did_not_offer() {
        let settings = Settings {
            authorized_folders: vec!["C:\\".into()],
            ..Settings::default()
        };
        assert!(settings
            .validate(&["C:\\Users\\a\\Documents".into()], &["small"])
            .is_err());
    }
}
