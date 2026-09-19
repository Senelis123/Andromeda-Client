use std::{fs, path::PathBuf};
use crate::{domain::AppError, storage::{atomic_write, quarantine_corrupt}};
use super::AppSettings;

#[derive(Debug)]
pub struct LoadOutcome { pub settings: AppSettings, pub recovery_notice: Option<String> }

#[derive(Debug, Clone)]
pub struct SettingsRepository { path: PathBuf }

impl SettingsRepository {
    #[must_use] pub fn new(path: PathBuf) -> Self { Self { path } }
    pub fn load(&self) -> Result<LoadOutcome, AppError> {
        if !self.path.exists() { return Ok(LoadOutcome { settings: AppSettings::default(), recovery_notice: None }); }
        let bytes = fs::read(&self.path).map_err(|source| AppError::FileSystem { operation: "read", path: self.path.clone(), source })?;
        match serde_json::from_slice::<AppSettings>(&bytes) {
            Ok(settings) if settings.validate().is_ok() => Ok(LoadOutcome { settings, recovery_notice: None }),
            Ok(_) => self.recover("Settings validation failed"),
            Err(error) => { tracing::warn!(path = %self.path.display(), %error, "settings are corrupt"); self.recover("Settings were corrupt") }
        }
    }
    pub fn save(&self, settings: &AppSettings) -> Result<(), AppError> {
        settings.validate().map_err(|message| AppError::InvalidConfiguration { path: self.path.clone(), source: serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidInput, message)) })?;
        let bytes = serde_json::to_vec_pretty(settings).map_err(|source| AppError::InvalidConfiguration { path: self.path.clone(), source })?;
        atomic_write(&self.path, &bytes)
    }
    fn recover(&self, message: &str) -> Result<LoadOutcome, AppError> {
        let location = quarantine_corrupt(&self.path)?;
        Ok(LoadOutcome { settings: AppSettings::default(), recovery_notice: Some(format!("{message}. Safe defaults are active; the original was preserved at {}.", location.map_or_else(|| self.path.display().to_string(), |p| p.display().to_string()))) })
    }
}
