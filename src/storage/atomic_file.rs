use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::domain::AppError;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| AppError::FileSystem {
        operation: "locate parent directory for",
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"),
    })?;
    fs::create_dir_all(parent).map_err(|source| fs_error("create directory", parent, source))?;
    let temporary = path.with_extension("tmp");
    let mut file =
        fs::File::create(&temporary).map_err(|source| fs_error("create", &temporary, source))?;
    file.write_all(bytes)
        .map_err(|source| fs_error("write", &temporary, source))?;
    file.sync_all()
        .map_err(|source| fs_error("flush", &temporary, source))?;
    if path.exists() {
        let backup = path.with_extension("bak");
        let _ = fs::copy(path, backup);
    }
    fs::rename(&temporary, path).map_err(|source| fs_error("replace", path, source))
}

pub fn quarantine_corrupt(path: &Path) -> Result<Option<PathBuf>, AppError> {
    if !path.exists() {
        return Ok(None);
    }
    let quarantine = path.with_extension("corrupt");
    fs::rename(path, &quarantine).map_err(|source| fs_error("quarantine", path, source))?;
    Ok(Some(quarantine))
}

fn fs_error(operation: &'static str, path: &Path, source: std::io::Error) -> AppError {
    AppError::FileSystem {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
