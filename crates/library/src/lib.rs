//! Public legal-source catalog only. No dependency on private workspace data.
//! Downloading and legal-rule activation are not part of this foundation.

use directories::BaseDirs;
use rusqlite::Connection;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("The local legal library could not be accessed: {0}")]
    Io(#[from] std::io::Error),
    #[error("The local legal catalog could not be read: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("This legal catalog requires a newer LaborLens PH release.")]
    NewerDatabase,
    #[error("No local application-data directory is available.")]
    MissingDataDirectory,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct LibrarySummary {
    pub source_count: u64,
    pub verified_source_count: u64,
    pub last_successful_check: Option<i64>,
    pub automatic_updates_available: bool,
}

pub struct LibraryCatalog {
    connection: Connection,
}

pub fn default_library_directory() -> Result<PathBuf, LibraryError> {
    let base = BaseDirs::new().ok_or(LibraryError::MissingDataDirectory)?;
    Ok(base.data_local_dir().join("LaborLens PH").join("library"))
}

impl LibraryCatalog {
    /// Accepts only the public-library directory, never private workspace paths.
    pub fn open(directory: &Path) -> Result<Self, LibraryError> {
        fs::create_dir_all(directory)?;
        let mut connection = Connection::open(directory.join("catalog.sqlite3"))?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let version: i32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return Err(LibraryError::NewerDatabase);
        }
        if version == 0 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE sources (
                    id INTEGER PRIMARY KEY,
                    title TEXT NOT NULL,
                    issuer TEXT NOT NULL,
                    source_url TEXT NOT NULL UNIQUE,
                    verification_status TEXT NOT NULL DEFAULT 'unverified'
                      CHECK(verification_status IN ('unverified', 'verified', 'needs_review'))
                 );
                 CREATE TABLE source_versions (
                    id INTEGER PRIMARY KEY,
                    source_id INTEGER NOT NULL REFERENCES sources(id),
                    content_sha256 TEXT NOT NULL,
                    local_path TEXT NOT NULL,
                    retrieved_at INTEGER NOT NULL,
                    published_at INTEGER,
                    effective_from INTEGER,
                    UNIQUE(source_id, content_sha256)
                 );
                 PRAGMA user_version = 1;",
            )?;
            transaction.commit()?;
        }
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { connection })
    }

    pub fn summary(&self) -> Result<LibrarySummary, LibraryError> {
        let (source_count, verified_source_count) = self.connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(verification_status = 'verified'), 0) FROM sources",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )?;
        Ok(LibrarySummary {
            source_count: source_count as u64,
            verified_source_count: verified_source_count as u64,
            // No downloader exists yet: never fabricate a successful check.
            last_successful_check: None,
            automatic_updates_available: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_library_does_not_claim_sources_or_successful_updates() {
        let temp = tempfile::TempDir::new().unwrap();
        let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        let summary = catalog.summary().unwrap();
        assert_eq!(summary.source_count, 0);
        assert_eq!(summary.verified_source_count, 0);
        assert_eq!(summary.last_successful_check, None);
        assert!(!summary.automatic_updates_available);
    }

    #[test]
    fn catalog_initialization_does_not_access_private_workspace() {
        let temp = tempfile::TempDir::new().unwrap();
        let private = temp.path().join("private");
        fs::create_dir(&private).unwrap();
        let sentinel = private.join("workspace.sqlite3");
        fs::write(&sentinel, b"private sentinel, not a database").unwrap();
        let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        assert_eq!(catalog.summary().unwrap().source_count, 0);
        assert_eq!(
            fs::read(sentinel).unwrap(),
            b"private sentinel, not a database"
        );
    }
}
