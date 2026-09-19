use std::path::PathBuf;

use directories::ProjectDirs;

use crate::domain::AppError;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppError> {
        let dirs = ProjectDirs::from("dev", "Andromeda", "Andromeda Client")
            .ok_or(AppError::MissingDataDirectory)?;
        let data = dirs.data_dir().to_path_buf();
        Ok(Self {
            config: dirs.config_dir().to_path_buf(),
            cache: dirs.cache_dir().to_path_buf(),
            logs: data.join("logs"),
            data,
        })
    }

    #[must_use]
    pub fn isolated(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            config: root.join("config"),
            data: root.join("data"),
            cache: root.join("cache"),
            logs: root.join("logs"),
        }
    }
}
