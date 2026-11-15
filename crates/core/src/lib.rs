//! Private, local workspace data. This crate has no networking dependencies.

use directories::BaseDirs;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("Local storage could not be accessed: {0}")]
    Io(#[from] std::io::Error),
    #[error("The local workspace database could not be read: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("The saved workspace settings could not be read: {0}")]
    Settings(#[from] serde_json::Error),
    #[error("{0}")]
    InvalidDocument(String),
    #[error(
        "This workspace uses a newer database version. Open it with a newer LaborLens PH release."
    )]
    NewerDatabase,
    #[error("The operating system did not provide a local application-data directory.")]
    MissingDataDirectory,
}

pub type Result<T> = std::result::Result<T, WorkspaceError>;

#[derive(Clone, Debug)]
pub struct WorkspacePaths {
    pub root: PathBuf,
    pub private_dir: PathBuf,
    pub database: PathBuf,
    pub library_dir: PathBuf,
}

impl WorkspacePaths {
    pub fn default_location() -> Result<Self> {
        let base = BaseDirs::new().ok_or(WorkspaceError::MissingDataDirectory)?;
        Ok(Self::under(base.data_local_dir().join("LaborLens PH")))
    }

    pub fn under(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let private_dir = root.join("private");
        Self {
            database: private_dir.join("workspace.sqlite3"),
            library_dir: root.join("library"),
            root,
            private_dir,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Pdf,
    Docx,
}

impl DocumentKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Docx => "DOCX",
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "docx" => Some(Self::Docx),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub id: i64,
    pub name: String,
    pub path: PathBuf,
    pub kind: DocumentKind,
    pub bytes: u64,
    pub added_at: i64,
    pub modified_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub remember_page: bool,
    pub last_page: String,
    pub show_full_paths: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            remember_page: true,
            last_page: "review".to_owned(),
            show_full_paths: false,
        }
    }
}

pub struct Workspace {
    connection: Connection,
}

impl Workspace {
    pub fn open(paths: &WorkspacePaths) -> Result<Self> {
        fs::create_dir_all(&paths.private_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&paths.private_dir, fs::Permissions::from_mode(0o700))?;
        }
        let mut connection = Connection::open(&paths.database)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let version: i32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(WorkspaceError::NewerDatabase);
        }
        if version == 0 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE documents (
                    id INTEGER PRIMARY KEY,
                    path TEXT NOT NULL UNIQUE,
                    name TEXT NOT NULL,
                    kind TEXT NOT NULL CHECK(kind IN ('PDF', 'DOCX')),
                    bytes INTEGER NOT NULL CHECK(bytes >= 0),
                    added_at INTEGER NOT NULL,
                    modified_at INTEGER
                );
                CREATE TABLE preferences (
                    id INTEGER PRIMARY KEY CHECK(id = 1),
                    value TEXT NOT NULL
                );
                CREATE INDEX documents_by_added ON documents(added_at DESC, id DESC);
                PRAGMA user_version = 1;",
            )?;
            transaction.commit()?;
        }
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&paths.database, fs::Permissions::from_mode(0o600))?;
        }
        Ok(Self { connection })
    }

    /// Register a reference to an existing file. Never copy, alter, or upload it.
    /// Extraction and checking are intentionally separate, later milestones.
    pub fn register_document(&self, path: &Path) -> Result<Document> {
        let kind = DocumentKind::from_path(path).ok_or_else(|| {
            WorkspaceError::InvalidDocument("Choose a PDF or DOCX document.".to_owned())
        })?;
        let canonical = path.canonicalize()?;
        let metadata = canonical.metadata()?;
        if !metadata.is_file() {
            return Err(WorkspaceError::InvalidDocument(
                "Choose a document file, rather than a folder.".to_owned(),
            ));
        }
        let path_text = canonical.to_str().ok_or_else(|| {
            WorkspaceError::InvalidDocument("This file path is not valid Unicode.".to_owned())
        })?;
        let name = path.file_name().and_then(|s| s.to_str()).ok_or_else(|| {
            WorkspaceError::InvalidDocument("The file name is not valid Unicode.".to_owned())
        })?;
        let bytes = i64::try_from(metadata.len()).map_err(|_| {
            WorkspaceError::InvalidDocument("This file is too large to register.".to_owned())
        })?;
        let modified_at = metadata.modified().ok().and_then(unix_seconds);
        self.connection.execute(
            "INSERT INTO documents (path, name, kind, bytes, added_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(path) DO UPDATE SET
                name = excluded.name, kind = excluded.kind,
                bytes = excluded.bytes, modified_at = excluded.modified_at",
            params![path_text, name, kind.label(), bytes, now(), modified_at],
        )?;
        self.connection
            .query_row(
                "SELECT id, name, path, kind, bytes, added_at, modified_at
                 FROM documents WHERE path = ?1",
                [path_text],
                document_from_row,
            )
            .map_err(Into::into)
    }

    pub fn documents(&self) -> Result<Vec<Document>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, path, kind, bytes, added_at, modified_at
             FROM documents ORDER BY added_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], document_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Removes only the workspace reference. The original document is untouched.
    pub fn remove_document(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM documents WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn preferences(&self) -> Result<Preferences> {
        let value: Option<String> = self
            .connection
            .query_row("SELECT value FROM preferences WHERE id = 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map(|value| value.unwrap_or_default())
            .map_err(Into::into)
    }

    pub fn save_preferences(&self, preferences: &Preferences) -> Result<()> {
        let value = serde_json::to_string(preferences)?;
        self.connection.execute(
            "INSERT INTO preferences (id, value) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET value = excluded.value",
            [value],
        )?;
        Ok(())
    }
}

fn document_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
    let kind: String = row.get(3)?;
    let kind = match kind.as_str() {
        "PDF" => DocumentKind::Pdf,
        "DOCX" => DocumentKind::Docx,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(Document {
        id: row.get(0)?,
        name: row.get(1)?,
        path: PathBuf::from(row.get::<_, String>(2)?),
        kind,
        bytes: row.get::<_, i64>(4)? as u64,
        added_at: row.get(5)?,
        modified_at: row.get(6)?,
    })
}

pub fn now() -> i64 {
    unix_seconds(SystemTime::now()).unwrap_or(0)
}

fn unix_seconds(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn document_references_and_preferences_survive_reopening() {
        let temp = TempDir::new().unwrap();
        let paths = WorkspacePaths::under(temp.path().join("data"));
        let original = temp.path().join("Synthetic contract.DOCX");
        fs::write(&original, b"synthetic fixture; extraction is not performed").unwrap();
        let expected = Preferences {
            show_full_paths: true,
            last_page: "documents".into(),
            ..Default::default()
        };
        {
            let workspace = Workspace::open(&paths).unwrap();
            let document = workspace.register_document(&original).unwrap();
            assert_eq!(document.kind, DocumentKind::Docx);
            workspace.save_preferences(&expected).unwrap();
        }
        let workspace = Workspace::open(&paths).unwrap();
        assert_eq!(workspace.documents().unwrap().len(), 1);
        assert_eq!(workspace.preferences().unwrap(), expected);
        assert_eq!(
            fs::read(&original).unwrap(),
            b"synthetic fixture; extraction is not performed"
        );
    }

    #[test]
    fn registering_the_same_path_refreshes_metadata_without_duplicates() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::open(&WorkspacePaths::under(temp.path().join("data"))).unwrap();
        let original = temp.path().join("Synthetic.pdf");
        fs::write(&original, b"one").unwrap();
        let first = workspace.register_document(&original).unwrap();
        fs::write(&original, b"a larger fixture").unwrap();
        let second = workspace.register_document(&original).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(first.added_at, second.added_at);
        assert_eq!(second.bytes, 16);
        assert_eq!(workspace.documents().unwrap().len(), 1);
    }

    #[test]
    fn unsupported_or_missing_documents_do_not_create_records() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::open(&WorkspacePaths::under(temp.path().join("data"))).unwrap();
        let original = temp.path().join("Synthetic.txt");
        fs::write(&original, b"fixture").unwrap();
        assert!(workspace.register_document(&original).is_err());
        assert!(
            workspace
                .register_document(&temp.path().join("missing.pdf"))
                .is_err()
        );
        let folder = temp.path().join("folder.pdf");
        fs::create_dir(&folder).unwrap();
        assert!(workspace.register_document(&folder).is_err());
        assert!(workspace.documents().unwrap().is_empty());
    }

    #[test]
    fn removing_a_record_preserves_the_original_file() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::open(&WorkspacePaths::under(temp.path().join("data"))).unwrap();
        let original = temp.path().join("Synthetic.pdf");
        fs::write(&original, b"keep this fixture").unwrap();
        let document = workspace.register_document(&original).unwrap();
        workspace.remove_document(document.id).unwrap();
        assert!(workspace.documents().unwrap().is_empty());
        assert_eq!(fs::read(&original).unwrap(), b"keep this fixture");
    }

    #[test]
    fn newer_databases_are_rejected_without_modification() {
        let temp = TempDir::new().unwrap();
        let paths = WorkspacePaths::under(temp.path());
        drop(Workspace::open(&paths).unwrap());
        let connection = Connection::open(&paths.database).unwrap();
        connection.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(
            Workspace::open(&paths),
            Err(WorkspaceError::NewerDatabase)
        ));
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            99
        );
    }

    #[test]
    fn malformed_saved_preferences_are_reported_instead_of_overwritten() {
        let temp = TempDir::new().unwrap();
        let paths = WorkspacePaths::under(temp.path());
        let workspace = Workspace::open(&paths).unwrap();
        workspace
            .connection
            .execute("INSERT INTO preferences VALUES (1, 'broken')", [])
            .unwrap();
        assert!(matches!(
            workspace.preferences(),
            Err(WorkspaceError::Settings(_))
        ));
        let stored: String = workspace
            .connection
            .query_row("SELECT value FROM preferences", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, "broken");
    }
}
