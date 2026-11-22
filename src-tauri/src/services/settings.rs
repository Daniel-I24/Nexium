use std::path::Path;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
};

pub const DEFAULT_SPEECH_MODEL: &str = "small";
pub const DEFAULT_SPEECH_IDLE_MINUTES: u32 = 10;
/// Carpetas autorizadas a la vez, como mucho.
pub const MAX_FOLDERS: usize = 20;

/// Ruta para mostrar y guardar: sin el prefijo `\\?\` que añade Windows al resolverla.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub speech_model: String,
    pub speech_idle_minutes: u32,
    pub weather_city: String,
    /// Carpetas donde Nexium puede buscar y leer. Solo se añaden con el selector del sistema o con el permiso
    /// de la conversación (`add_folder`), nunca con una ruta escrita por la interfaz.
    pub authorized_folders: Vec<String>,
    /// Nombre del micrófono elegido. `None` es automático: el primer micrófono real.
    pub microphone: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            speech_model: DEFAULT_SPEECH_MODEL.into(),
            speech_idle_minutes: DEFAULT_SPEECH_IDLE_MINUTES,
            weather_city: String::new(),
            authorized_folders: Vec::new(),
            microphone: None,
        }
    }
}

impl Settings {
    pub fn validate(&self, known_speech_models: &[&str]) -> AppResult<()> {
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

    /// Devuelve `true` solo la primera vez que se pide con esa clave, y lo deja anotado.
    /// Sirve para avisos que se muestran una única vez, como el de la bandeja.
    pub fn first_time(&self, key: &str) -> AppResult<bool> {
        let inserted = self.db.lock()?.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, 'shown')",
            params![format!("once:{key}")],
        )?;
        Ok(inserted == 1)
    }

    /// Guarda los ajustes que se editan en la pantalla. Las carpetas no se tocan: tienen sus propias acciones.
    pub fn save_editable(&self, edited: &Settings) -> AppResult<()> {
        let mut settings = edited.clone();
        settings.authorized_folders = self.get()?.authorized_folders;
        self.save(&settings)
    }

    /// Autoriza una carpeta que existe. Devuelve la lista resultante, sin duplicados.
    pub fn add_folder(&self, path: &Path) -> AppResult<Vec<String>> {
        let resolved = path.canonicalize().map_err(|_| {
            AppError::Validation("Esa carpeta no existe o no se puede abrir.".into())
        })?;
        if !resolved.is_dir() {
            return Err(AppError::Validation(
                "Solo se pueden autorizar carpetas.".into(),
            ));
        }
        let folder = display_path(&resolved);
        let mut settings = self.get()?;
        if !settings.authorized_folders.contains(&folder) {
            if settings.authorized_folders.len() >= MAX_FOLDERS {
                return Err(AppError::Validation(format!(
                    "Puedes autorizar como mucho {MAX_FOLDERS} carpetas."
                )));
            }
            settings.authorized_folders.push(folder);
            self.save(&settings)?;
        }
        Ok(settings.authorized_folders)
    }

    /// Retira el permiso de una carpeta. No toca la carpeta ni su contenido.
    pub fn remove_folder(&self, folder: &str) -> AppResult<Vec<String>> {
        let mut settings = self.get()?;
        settings.authorized_folders.retain(|item| item != folder);
        self.save(&settings)?;
        Ok(settings.authorized_folders)
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
        // El límite diario editable ya no existe: el valor guardado se ignora.
        assert_eq!(stored, Settings::default());
    }

    #[test]
    fn the_voice_idle_period_is_validated_and_defaults_for_old_settings() {
        let mut settings = Settings::default();
        let models = ["small"];
        assert!(settings.validate(&models).is_ok());
        settings.speech_idle_minutes = 0;
        assert!(settings.validate(&models).is_err());
        settings.speech_idle_minutes = 241;
        assert!(settings.validate(&models).is_err());
        let stored: Settings = serde_json::from_str(r#"{"speechModel":"small"}"#).unwrap();
        assert_eq!(stored.speech_idle_minutes, DEFAULT_SPEECH_IDLE_MINUTES);
    }

    #[test]
    fn a_one_time_notice_is_only_first_once() {
        let service = service();
        assert!(service.first_time("tray").unwrap());
        assert!(!service.first_time("tray").unwrap());
        assert!(service.first_time("otro").unwrap());
        service.save(&Settings::default()).unwrap();
        assert!(!service.first_time("tray").unwrap());
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
    fn folders_are_added_once_and_only_if_they_exist() {
        let service = service();
        let dir = tempfile::tempdir().unwrap();
        let folders = service.add_folder(dir.path()).unwrap();
        assert_eq!(folders.len(), 1);
        assert!(!folders[0].starts_with(r"\\?\"));
        assert_eq!(service.add_folder(dir.path()).unwrap().len(), 1);
        assert!(service.add_folder(&dir.path().join("no-existe")).is_err());
        let file = dir.path().join("archivo.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(service.add_folder(&file).is_err());
        assert!(service.remove_folder(&folders[0]).unwrap().is_empty());
    }

    #[test]
    fn saving_the_screen_never_changes_the_folders() {
        let service = service();
        let dir = tempfile::tempdir().unwrap();
        service.add_folder(dir.path()).unwrap();
        let edited = Settings {
            authorized_folders: vec![r"C:\".into()],
            weather_city: "Cali".into(),
            ..Settings::default()
        };
        service.save_editable(&edited).unwrap();
        let saved = service.get().unwrap();
        assert_eq!(saved.weather_city, "Cali");
        assert_eq!(saved.authorized_folders.len(), 1);
        assert_ne!(saved.authorized_folders[0], r"C:\");
    }
}
