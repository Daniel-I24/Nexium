use std::{collections::HashSet, path::PathBuf};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::{AppError, AppResult},
};

pub const DEFAULT_PROVIDER: &str = "gemini";
pub const DEFAULT_INTERPRET_MODEL: &str = "gemini-3.5-flash-lite";
pub const DEFAULT_VISION_MODEL: &str = "gemini-3.6-flash";
pub const DEFAULT_SPEECH_MODEL: &str = "small";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub ai_provider: String,
    pub interpret_model: String,
    pub vision_model: String,
    pub daily_ai_limit: u32,
    pub speech_model: String,
    pub weather_city: String,
    pub authorized_folders: Vec<String>,
    pub folders_confirmed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_provider: DEFAULT_PROVIDER.into(),
            interpret_model: DEFAULT_INTERPRET_MODEL.into(),
            vision_model: DEFAULT_VISION_MODEL.into(),
            daily_ai_limit: 200,
            speech_model: DEFAULT_SPEECH_MODEL.into(),
            weather_city: String::new(),
            authorized_folders: Vec::new(),
            folders_confirmed: false,
        }
    }
}

impl Settings {
    /// Un identificador de modelo solo admite caracteres de identificador: nunca llega a una URL.
    fn valid_model(model: &str) -> bool {
        !model.is_empty() && model.len() <= 80 && model.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c))
    }

    pub fn validate(&self, known_folders: &[String], known_speech_models: &[&str]) -> AppResult<()> {
        if !Self::valid_model(&self.interpret_model) || !Self::valid_model(&self.vision_model) {
            return Err(AppError::Validation("El nombre de un modelo de IA no es válido.".into()));
        }
        if self.ai_provider != DEFAULT_PROVIDER {
            return Err(AppError::Validation("Ese proveedor de IA no está disponible.".into()));
        }
        if !(1..=10_000).contains(&self.daily_ai_limit) {
            return Err(AppError::Validation("El límite diario debe estar entre 1 y 10 000.".into()));
        }
        if !known_speech_models.contains(&self.speech_model.as_str()) {
            return Err(AppError::Validation("Ese modelo de voz no existe.".into()));
        }
        if self.weather_city.chars().count() > 100 {
            return Err(AppError::Validation("El nombre de la ciudad es demasiado largo.".into()));
        }
        let known: HashSet<&String> = known_folders.iter().collect();
        if self.authorized_folders.iter().any(|folder| !known.contains(folder)) {
            return Err(AppError::Validation("Solo se pueden autorizar las carpetas que ofrece Nexium.".into()));
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
        let stored: Option<String> = self.db.lock()?.query_row("SELECT value FROM settings WHERE key = 'app'", [], |row| row.get(0)).optional()?;
        Ok(stored.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default())
    }

    pub fn save(&self, settings: &Settings) -> AppResult<()> {
        let json = serde_json::to_string(settings).map_err(|e| AppError::Operation(e.to_string()))?;
        self.db.lock()?.execute("INSERT INTO settings (key, value) VALUES ('app', ?1) ON CONFLICT(key) DO UPDATE SET value = ?1", params![json])?;
        Ok(())
    }

    pub fn authorized_paths(&self) -> AppResult<Vec<PathBuf>> {
        Ok(self.get()?.authorized_folders.into_iter().map(PathBuf::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> SettingsService {
        SettingsService::new(Database::in_memory().unwrap())
    }

    #[test]
    fn defaults_keep_separate_models() {
        let settings = service().get().unwrap();
        assert_eq!(settings.ai_provider, "gemini");
        assert_ne!(settings.interpret_model, settings.vision_model);
    }

    #[test]
    fn saved_models_persist() {
        let service = service();
        let mut settings = service.get().unwrap();
        settings.vision_model = "gemini-3.5-flash-lite".into();
        service.save(&settings).unwrap();
        assert_eq!(service.get().unwrap().vision_model, "gemini-3.5-flash-lite");
    }

    #[test]
    fn rejects_models_with_url_characters() {
        let settings = Settings { interpret_model: "../secret?x=1".into(), ..Settings::default() };
        assert!(settings.validate(&[], &["small"]).is_err());
    }

    #[test]
    fn rejects_folders_nexium_did_not_offer() {
        let settings = Settings { authorized_folders: vec!["C:\\".into()], ..Settings::default() };
        assert!(settings.validate(&["C:\\Users\\a\\Documents".into()], &["small"]).is_err());
    }
}
