//! Workspace location and owner-only directory permissions. No database/content reads.

mod permissions;
pub use permissions::secure_directory;

use directories::BaseDirs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LocationError {
    #[error("No application-data directory is available.")]
    Unavailable,
    #[error(
        "More than one existing workspace was found. Choose the existing folder in ComplyPH; no data has been replaced."
    )]
    Ambiguous,
    #[error(
        "An older workspace needs migration. Open the ComplyPH desktop first; the public updater does not read or migrate private data."
    )]
    MigrationRequired,
    #[error("The workspace directory could not be protected or accessed: {0}")]
    Access(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
pub struct StoragePlan {
    pub preferred: PathBuf,
    pub legacy_candidates: Vec<PathBuf>,
    pub machine_wide: bool,
}

pub fn has_workspace(root: &Path) -> bool {
    root.join("private/workspace.sqlite3").is_file()
        || root.join("library/catalog.sqlite3").is_file()
}

pub fn storage_plan() -> Result<StoragePlan, LocationError> {
    let base = BaseDirs::new().ok_or(LocationError::Unavailable)?;
    #[cfg(windows)]
    {
        let program_data = std::env::var_os("ProgramData").ok_or(LocationError::Unavailable)?;
        let mut candidates = vec![
            base.data_local_dir().join("ComplyPH"),
            base.data_local_dir().join("LaborLens PH"),
        ];
        if let Some(public) = std::env::var_os("PUBLIC") {
            candidates.extend([
                PathBuf::from(&public).join("ComplyPH"),
                PathBuf::from(public).join("LaborLens PH"),
            ]);
        }
        Ok(StoragePlan {
            preferred: PathBuf::from(program_data).join("ComplyPH"),
            legacy_candidates: candidates,
            machine_wide: true,
        })
    }
    #[cfg(not(windows))]
    {
        Ok(StoragePlan {
            preferred: data_root_under(base.data_local_dir())?,
            legacy_candidates: vec![],
            machine_wide: false,
        })
    }
}

/// Public updater entry point: never migrates or opens private databases.
pub fn default_data_root() -> Result<PathBuf, LocationError> {
    let plan = storage_plan()?;
    if plan.machine_wide {
        if !has_workspace(&plan.preferred)
            && plan
                .legacy_candidates
                .iter()
                .any(|path| has_workspace(path))
        {
            return Err(LocationError::MigrationRequired);
        }
        secure_directory(&plan.preferred)?;
    }
    Ok(plan.preferred)
}

pub fn data_root_under(base: &Path) -> Result<PathBuf, LocationError> {
    let current = base.join("ComplyPH");
    let legacy = base.join("LaborLens PH");
    match (current.exists(), legacy.exists()) {
        (true, true) => Err(LocationError::Ambiguous),
        (false, true) => Ok(legacy),
        _ => Ok(current),
    }
}

/// A legacy desktop must be closed before cross-database migration.
#[cfg(not(windows))]
pub fn another_desktop_running() -> std::io::Result<bool> {
    Ok(false)
}
#[cfg(windows)]
pub fn another_desktop_running() -> std::io::Result<bool> {
    use windows_sys::Win32::{Foundation::*, System::Diagnostics::ToolHelp::*};
    // SAFETY: process enumeration uses initialized buffers; the snapshot is closed on every path.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut next = Process32FirstW(snapshot, &mut entry);
        while next != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
            if entry.th32ProcessID != std::process::id()
                && matches!(
                    name.as_str(),
                    "complyph.exe" | "laborlensph.exe" | "laborlens ph.exe"
                )
            {
                found = true;
                break;
            }
            next = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_discovery_does_not_copy_or_change_data() {
        let temp = tempfile::TempDir::new().unwrap();
        assert_eq!(
            data_root_under(temp.path()).unwrap(),
            temp.path().join("ComplyPH")
        );
        let legacy = temp.path().join("LaborLens PH");
        std::fs::create_dir(&legacy).unwrap();
        std::fs::write(legacy.join("preserve"), b"fixture").unwrap();
        assert_eq!(data_root_under(temp.path()).unwrap(), legacy);
        assert_eq!(std::fs::read(legacy.join("preserve")).unwrap(), b"fixture");
    }
    #[test]
    fn ambiguous_roots_are_preserved() {
        let temp = tempfile::TempDir::new().unwrap();
        for name in ["ComplyPH", "LaborLens PH"] {
            std::fs::create_dir(temp.path().join(name)).unwrap();
        }
        assert!(matches!(
            data_root_under(temp.path()),
            Err(LocationError::Ambiguous)
        ));
    }
    #[test]
    fn empty_directories_are_not_migration_candidates() {
        let temp = tempfile::TempDir::new().unwrap();
        assert!(!has_workspace(temp.path()));
        std::fs::create_dir(temp.path().join("private")).unwrap();
        std::fs::write(temp.path().join("private/workspace.sqlite3"), b"fixture").unwrap();
        assert!(has_workspace(temp.path()));
    }
}
