//! Shared location discovery only. No document or database access.

use directories::BaseDirs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LocationError {
    #[error("No local application-data directory is available.")]
    Unavailable,
    #[error(
        "Both ComplyPH and LaborLens PH data folders exist. Use --data-dir to select the existing workspace explicitly; no data has been moved."
    )]
    Ambiguous,
}

pub fn default_data_root() -> Result<PathBuf, LocationError> {
    let base = BaseDirs::new().ok_or(LocationError::Unavailable)?;
    data_root_under(base.data_local_dir())
}

/// Reuse an existing LaborLens PH directory without moving live SQLite files.
pub fn data_root_under(base: &Path) -> Result<PathBuf, LocationError> {
    let current = base.join("ComplyPH");
    let legacy = base.join("LaborLens PH");
    match (current.exists(), legacy.exists()) {
        (true, true) => Err(LocationError::Ambiguous),
        (false, true) => Ok(legacy),
        _ => Ok(current),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn brand_change_reuses_legacy_data_without_copying_or_modifying_it() {
        let temp = tempfile::TempDir::new().unwrap();
        assert_eq!(
            data_root_under(temp.path()).unwrap(),
            temp.path().join("ComplyPH")
        );
        let legacy = temp.path().join("LaborLens PH");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("preserve"), b"existing workspace").unwrap();
        assert_eq!(data_root_under(temp.path()).unwrap(), legacy);
        assert_eq!(
            fs::read(legacy.join("preserve")).unwrap(),
            b"existing workspace"
        );
        assert!(!temp.path().join("ComplyPH").exists());
    }

    #[test]
    fn two_existing_roots_require_explicit_selection() {
        let temp = tempfile::TempDir::new().unwrap();
        fs::create_dir(temp.path().join("LaborLens PH")).unwrap();
        fs::create_dir(temp.path().join("ComplyPH")).unwrap();
        assert!(matches!(
            data_root_under(temp.path()),
            Err(LocationError::Ambiguous)
        ));
    }
}
