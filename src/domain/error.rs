use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Configuration,
    FileSystem,
    Internal,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("could not determine a platform data directory")]
    MissingDataDirectory,
    #[error("could not {operation} {path}: {source}")]
    FileSystem {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("configuration at {path} is invalid: {source}")]
    InvalidConfiguration {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl AppError {
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        match self {
            Self::MissingDataDirectory | Self::FileSystem { .. } => ErrorCategory::FileSystem,
            Self::InvalidConfiguration { .. } => ErrorCategory::Configuration,
        }
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        matches!(self, Self::FileSystem { .. })
    }
}
