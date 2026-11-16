//! Private, local workspace data. This crate has no networking dependencies.

pub mod migration;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const SCHEMA_VERSION: i32 = 3;

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
    #[error("This workspace uses a newer database version. Open it with a newer ComplyPH release.")]
    NewerDatabase,
    #[error("The operating system did not provide a local application-data directory.")]
    MissingDataDirectory,
    #[error("{0}")]
    Location(#[from] complyph_paths::LocationError),
}

pub type Result<T> = std::result::Result<T, WorkspaceError>;

pub fn default_workspace_target() -> Result<PathBuf> {
    Ok(complyph_paths::storage_plan()?.preferred)
}

#[derive(Clone, Debug)]
pub struct WorkspacePaths {
    pub root: PathBuf,
    pub private_dir: PathBuf,
    pub database: PathBuf,
    pub library_dir: PathBuf,
}

impl WorkspacePaths {
    pub fn default_location() -> Result<Self> {
        migration::prepare(&complyph_paths::storage_plan()?)
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
    pub fn format(self) -> complyph_extract::Format {
        match self {
            Self::Pdf => complyph_extract::Format::Pdf,
            Self::Docx => complyph_extract::Format::Docx,
        }
    }
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedExtraction {
    pub sha256: String,
    pub read_at: i64,
    pub report: complyph_extract::Report,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedReview {
    pub id: i64,
    pub document_id: i64,
    pub created_at: i64,
    pub review: complyph_checks::Review,
}

pub struct DocumentSnapshot {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub modified_at: Option<i64>,
}

/// Read a bounded snapshot locally. Hashes and private bytes never go to the updater.
pub fn document_snapshot(path: &Path) -> Result<DocumentSnapshot> {
    let file = fs::File::open(path)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > complyph_extract::MAX_INPUT_BYTES as u64 {
        return Err(WorkspaceError::InvalidDocument(
            "Choose a PDF or DOCX file of 32 MiB or less.".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(complyph_extract::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let after = path.metadata()?;
    if bytes.len() > complyph_extract::MAX_INPUT_BYTES
        || bytes.len() as u64 != before.len()
        || after.len() != before.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(WorkspaceError::InvalidDocument(
            "The document changed while being read. Save it and try again.".into(),
        ));
    }
    Ok(DocumentSnapshot {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes,
        modified_at: after.modified().ok().and_then(unix_seconds),
    })
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
        complyph_paths::secure_directory(&paths.root)?;
        complyph_paths::secure_directory(&paths.private_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&paths.private_dir, fs::Permissions::from_mode(0o700))?;
        }
        let mut connection = Connection::open(&paths.database)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        // Serialize schema inspection with migrations, including concurrent first opens.
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let version: i32 = transaction.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(WorkspaceError::NewerDatabase);
        }
        if version == 0 {
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
        }
        if version < 2 {
            transaction.execute_batch(
                "CREATE TABLE document_extractions (
                    document_id INTEGER PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
                    value TEXT NOT NULL
                );
                PRAGMA user_version = 2;",
            )?;
        }
        if version < 3 {
            transaction.execute_batch(
                "CREATE TABLE review_runs (
                id INTEGER PRIMARY KEY,
                document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                created_at INTEGER NOT NULL,
                value TEXT NOT NULL
            ); PRAGMA user_version = 3;",
            )?;
        }
        transaction.commit()?;
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
    /// Text extraction is a separate, explicit local operation.
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

    pub fn extraction(&self, id: i64) -> Result<Option<SavedExtraction>> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM document_extractions WHERE document_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(Into::into)
    }

    /// Call only after confirming that the original still matches the parsed snapshot.
    pub fn save_extraction(
        &self,
        document: &Document,
        snapshot: &DocumentSnapshot,
        report: complyph_extract::Report,
    ) -> Result<SavedExtraction> {
        let saved = SavedExtraction {
            sha256: snapshot.sha256.clone(),
            read_at: now(),
            report,
        };
        let value = serde_json::to_string(&saved)?;
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE documents SET bytes = ?1, modified_at = ?2 WHERE id = ?3 AND path = ?4",
            params![
                snapshot.bytes.len() as i64,
                snapshot.modified_at,
                document.id,
                document.path.to_str()
            ],
        )?;
        transaction.execute(
            "INSERT INTO document_extractions(document_id, value) VALUES (?1, ?2)
            ON CONFLICT(document_id) DO UPDATE SET value = excluded.value",
            params![document.id, value],
        )?;
        transaction.commit()?;
        Ok(saved)
    }

    /// Removes the reference and cached text. The original document is untouched.
    pub fn remove_document(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM documents WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn save_review(
        &self,
        document_id: i64,
        review: complyph_checks::Review,
    ) -> Result<SavedReview> {
        let saved = SavedReview {
            id: 0,
            document_id,
            created_at: now(),
            review,
        };
        self.connection.execute(
            "INSERT INTO review_runs(document_id,created_at,value) VALUES (?1,?2,?3)",
            params![
                document_id,
                saved.created_at,
                serde_json::to_string(&saved.review)?
            ],
        )?;
        Ok(SavedReview {
            id: self.connection.last_insert_rowid(),
            ..saved
        })
    }
    pub fn reviews(&self) -> Result<Vec<SavedReview>> {
        let mut statement = self
            .connection
            .prepare("SELECT id,document_id,created_at,value FROM review_runs ORDER BY id DESC")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|r| {
            let (id, document_id, created_at, value) = r?;
            Ok(SavedReview {
                id,
                document_id,
                created_at,
                review: serde_json::from_str(&value)?,
            })
        })
        .collect()
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

    #[test]
    fn schema_one_upgrade_preserves_records_preferences_and_caches_text_privately() {
        let temp = tempfile::TempDir::new().unwrap();
        let paths = WorkspacePaths::under(temp.path().join("data"));
        fs::create_dir_all(&paths.private_dir).unwrap();
        let original = temp.path().join("Synthetic.pdf");
        let bytes = include_bytes!("../../../examples/Synthetic employment.pdf");
        fs::write(&original, bytes).unwrap();
        let db = Connection::open(&paths.database).unwrap();
        db.execute_batch("CREATE TABLE documents(id INTEGER PRIMARY KEY,path TEXT UNIQUE,name TEXT,kind TEXT,bytes INTEGER,added_at INTEGER,modified_at INTEGER);
            CREATE TABLE preferences(id INTEGER PRIMARY KEY,value TEXT); PRAGMA user_version=1;").unwrap();
        db.execute(
            "INSERT INTO documents VALUES(7,?1,'Synthetic.pdf','PDF',?2,42,NULL)",
            params![original.to_str(), bytes.len() as i64],
        )
        .unwrap();
        let preferences = Preferences {
            last_page: "documents".into(),
            show_full_paths: true,
            ..Default::default()
        };
        db.execute(
            "INSERT INTO preferences VALUES(1,?1)",
            [serde_json::to_string(&preferences).unwrap()],
        )
        .unwrap();
        drop(db);
        let workspace = Workspace::open(&paths).unwrap();
        let document = workspace.documents().unwrap().remove(0);
        assert_eq!(document.id, 7);
        assert_eq!(document.added_at, 42);
        assert_eq!(workspace.preferences().unwrap(), preferences);
        assert!(workspace.extraction(7).unwrap().is_none());
        let snapshot = document_snapshot(&original).unwrap();
        let report = complyph_extract::extract(complyph_extract::Format::Pdf, &snapshot.bytes);
        let saved = workspace
            .save_extraction(&document, &snapshot, report)
            .unwrap();
        drop(workspace);
        let workspace = Workspace::open(&paths).unwrap();
        assert_eq!(
            workspace.extraction(7).unwrap().unwrap().sha256,
            saved.sha256
        );
        assert!(
            workspace.extraction(7).unwrap().unwrap().report.units[1]
                .text
                .contains("PAGE-TWO-TERMS")
        );
        assert!(!paths.library_dir.exists());
        workspace.remove_document(7).unwrap();
        assert!(workspace.extraction(7).unwrap().is_none());
        assert_eq!(fs::read(&original).unwrap(), bytes);
    }

    #[test]
    fn simultaneous_private_migrations_do_not_duplicate_or_lose_entries() {
        let temp = tempfile::TempDir::new().unwrap();
        let paths = WorkspacePaths::under(temp.path().join("data"));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let paths = paths.clone();
                std::thread::spawn(move || Workspace::open(&paths).unwrap().documents().unwrap())
            })
            .collect();
        for handle in handles {
            assert!(handle.join().unwrap().is_empty());
        }
    }

    #[test]
    fn renamed_app_reopens_legacy_documents_and_preferences() {
        let temp = tempfile::TempDir::new().unwrap();
        let legacy = WorkspacePaths::under(temp.path().join("LaborLens PH"));
        let document = temp.path().join("Original.pdf");
        fs::write(&document, b"synthetic original").unwrap();
        let workspace = Workspace::open(&legacy).unwrap();
        workspace.register_document(&document).unwrap();
        let preferences = Preferences {
            show_full_paths: true,
            last_page: "documents".into(),
            ..Preferences::default()
        };
        workspace.save_preferences(&preferences).unwrap();
        drop(workspace);
        let discovered =
            WorkspacePaths::under(complyph_paths::data_root_under(temp.path()).unwrap());
        let reopened = Workspace::open(&discovered).unwrap();
        assert_eq!(reopened.documents().unwrap().len(), 1);
        assert_eq!(reopened.preferences().unwrap(), preferences);
        assert_eq!(fs::read(document).unwrap(), b"synthetic original");
        assert!(!temp.path().join("ComplyPH").exists());
    }
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
    #[test]
    fn reports_persist_evidence_and_cascade_only_after_removing_reference() {
        let temp = tempfile::tempdir().unwrap();
        let paths = WorkspacePaths::under(temp.path().join("workspace"));
        let original = temp.path().join("synthetic.pdf");
        fs::write(&original, b"original unchanged").unwrap();
        let workspace = Workspace::open(&paths).unwrap();
        let document = workspace.register_document(&original).unwrap();
        let report = complyph_extract::Report::failure(
            complyph_extract::Format::Pdf,
            complyph_extract::Status::NeedsOcr,
            "synthetic scan",
        );
        let review = complyph_checks::review(
            &report,
            "synthetic-hash",
            complyph_checks::Context::default(),
        );
        let saved = workspace.save_review(document.id, review).unwrap();
        drop(workspace);
        let workspace = Workspace::open(&paths).unwrap();
        let reports = workspace.reviews().unwrap();
        assert_eq!(reports[0].id, saved.id);
        assert_eq!(reports[0].review.document_sha256, "synthetic-hash");
        assert_eq!(
            reports[0].review.findings[0].outcome,
            complyph_checks::Outcome::NeedsContext
        );
        workspace.remove_document(document.id).unwrap();
        assert!(workspace.reviews().unwrap().is_empty());
        assert_eq!(fs::read(original).unwrap(), b"original unchanged");
    }
}
