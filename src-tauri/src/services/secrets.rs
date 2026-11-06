use std::sync::Mutex;

use keyring::Entry;
use serde::Serialize;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "io.github.daniel-i24.nexium";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeySource {
    SecureStore,
    Session,
    DevEnv,
    None,
}

/// Almacén de la clave. El valor nunca sale hacia la interfaz ni aparece en `Debug`.
pub trait SecretBackend: Send + Sync {
    fn read(&self, account: &str) -> Result<Option<String>, String>;
    fn write(&self, account: &str, value: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

pub struct KeyringBackend;

impl SecretBackend for KeyringBackend {
    fn read(&self, account: &str) -> Result<Option<String>, String> {
        let entry = Entry::new(SERVICE, account).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn write(&self, account: &str, value: &str) -> Result<(), String> {
        Entry::new(SERVICE, account).and_then(|entry| entry.set_password(value)).map_err(|e| e.to_string())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let entry = Entry::new(SERVICE, account).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

pub struct SecretStore {
    backend: Box<dyn SecretBackend>,
    session: Mutex<Option<(String, String)>>,
}

impl SecretStore {
    pub fn new(backend: Box<dyn SecretBackend>) -> Self {
        Self { backend, session: Mutex::new(None) }
    }

    /// Si el sistema no tiene almacén seguro, la clave solo vive en memoria durante la sesión.
    pub fn save(&self, account: &str, value: &str) -> AppResult<KeySource> {
        let value = value.trim();
        if value.is_empty() || value.len() > 512 || value.chars().any(char::is_whitespace) {
            return Err(AppError::Validation("La clave tiene un formato no válido.".into()));
        }
        match self.backend.write(account, value) {
            Ok(()) => {
                self.clear_session();
                Ok(KeySource::SecureStore)
            }
            Err(_) => {
                *self.session.lock().map_err(lock_error)? = Some((account.to_owned(), value.to_owned()));
                Ok(KeySource::Session)
            }
        }
    }

    pub fn delete(&self, account: &str) -> AppResult<()> {
        self.clear_session();
        self.backend.delete(account).map_err(|_| AppError::Operation("No se pudo eliminar la clave del almacén seguro.".into()))
    }

    pub fn get(&self, account: &str, dev_env_var: &str) -> Option<String> {
        self.lookup(account, dev_env_var).map(|(value, _)| value)
    }

    pub fn source(&self, account: &str, dev_env_var: &str) -> KeySource {
        self.lookup(account, dev_env_var).map_or(KeySource::None, |(_, source)| source)
    }

    pub fn secure_store_available(&self, account: &str) -> bool {
        self.backend.read(account).is_ok()
    }

    fn lookup(&self, account: &str, dev_env_var: &str) -> Option<(String, KeySource)> {
        if let Ok(guard) = self.session.lock() {
            if let Some((stored_account, value)) = guard.as_ref() {
                if stored_account == account {
                    return Some((value.clone(), KeySource::Session));
                }
            }
        }
        if let Ok(Some(value)) = self.backend.read(account) {
            return Some((value, KeySource::SecureStore));
        }
        dev_env_key(dev_env_var).map(|value| (value, KeySource::DevEnv))
    }

    fn clear_session(&self) {
        if let Ok(mut guard) = self.session.lock() {
            *guard = None;
        }
    }
}

fn lock_error<T>(_: T) -> AppError {
    AppError::Operation("El almacén de claves quedó bloqueado.".into())
}

/// Solo existe en compilaciones de desarrollo: la distribución nunca lee `.env`.
#[cfg(debug_assertions)]
fn dev_env_key(variable: &str) -> Option<String> {
    let _ = dotenvy::from_filename("../.env");
    let _ = dotenvy::dotenv();
    std::env::var(variable).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(not(debug_assertions))]
fn dev_env_key(_variable: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemoryBackend {
        values: Mutex<HashMap<String, String>>,
        broken: bool,
    }

    impl SecretBackend for MemoryBackend {
        fn read(&self, account: &str) -> Result<Option<String>, String> {
            if self.broken {
                return Err("sin almacén".into());
            }
            Ok(self.values.lock().unwrap().get(account).cloned())
        }
        fn write(&self, account: &str, value: &str) -> Result<(), String> {
            if self.broken {
                return Err("sin almacén".into());
            }
            self.values.lock().unwrap().insert(account.into(), value.into());
            Ok(())
        }
        fn delete(&self, account: &str) -> Result<(), String> {
            self.values.lock().unwrap().remove(account);
            Ok(())
        }
    }

    const NO_ENV: &str = "NEXIUM_TEST_UNSET_VARIABLE";

    #[test]
    fn stores_and_deletes_in_secure_backend() {
        let store = SecretStore::new(Box::<MemoryBackend>::default());
        assert_eq!(store.save("gemini", " abc ").unwrap(), KeySource::SecureStore);
        assert_eq!(store.get("gemini", NO_ENV).as_deref(), Some("abc"));
        store.delete("gemini").unwrap();
        assert_eq!(store.source("gemini", NO_ENV), KeySource::None);
    }

    #[test]
    fn falls_back_to_session_without_persisting() {
        let store = SecretStore::new(Box::new(MemoryBackend { broken: true, ..Default::default() }));
        assert_eq!(store.save("gemini", "abc").unwrap(), KeySource::Session);
        assert_eq!(store.source("gemini", NO_ENV), KeySource::Session);
        assert!(!store.secure_store_available("gemini"));
    }

    #[test]
    fn rejects_malformed_keys() {
        let store = SecretStore::new(Box::<MemoryBackend>::default());
        assert!(store.save("gemini", "").is_err());
        assert!(store.save("gemini", "dos palabras").is_err());
    }
}
