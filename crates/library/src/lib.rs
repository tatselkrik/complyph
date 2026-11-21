//! Public official-source snapshots only. No private-workspace or HTTP dependency.
mod bundled;
mod comparison;
mod discovery;
mod maintenance;
mod manual;
mod updates;

pub use comparison::*;
pub use discovery::*;
pub use maintenance::*;

pub use manual::{ImportOutcome, ManualSourceFile, ManualSourceInput, ManualVerification};
pub use updates::{
    CheckReceipt, DownloadedChange, MAX_DOWNLOAD_BYTES, UpdateReply, UpdateTarget, UpdateWatch,
    approved_update_url, download_mime, same_origin,
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const MANIFEST: &str = include_str!("../../../library/starter/manifest.json");
const SCHEMA_VERSION: i32 = 5;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("The local reference library could not be accessed: {0}")]
    Io(#[from] std::io::Error),
    #[error("The local catalog could not be read: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Source metadata could not be read: {0}")]
    Metadata(#[from] serde_json::Error),
    #[error("{0}")]
    Location(#[from] complyph_paths::LocationError),
    #[error("This catalog requires a newer ComplyPH release.")]
    NewerDatabase,
    #[error("Source integrity check failed: {0}. The snapshot cannot be shown as verified.")]
    Integrity(String),
    #[error("No local snapshot is available for this source.")]
    MissingSnapshot,
    #[error("{0}")]
    InvalidImport(String),
}
type Result<T> = std::result::Result<T, LibraryError>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Domain {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CoverageGap {
    pub domain: String,
    pub instrument: String,
    pub source_url: String,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    #[default]
    AgencyDownload,
    UploadedIdentity,
    ThirdParty,
    ManualPending,
    ManualIdentityChecked,
    ReviewedDownload,
}

impl Provenance {
    pub fn label(self) -> &'static str {
        match self {
            Self::AgencyDownload => "Verified agency download",
            Self::UploadedIdentity => "Identity checked · imported copy",
            Self::ThirdParty => "Provisional · third-party copy",
            Self::ManualPending => "Awaiting verification · manual import",
            Self::ManualIdentityChecked => "Identity checked · user reviewed",
            Self::ReviewedDownload => "Official download · reviewed adoption",
        }
    }

    fn database_status(self) -> &'static str {
        match self {
            Self::AgencyDownload => "verified",
            Self::UploadedIdentity | Self::ManualIdentityChecked | Self::ReviewedDownload => {
                "needs_review"
            }
            Self::ThirdParty | Self::ManualPending => "unverified",
        }
    }

    pub fn is_manual(self) -> bool {
        matches!(self, Self::ManualPending | Self::ManualIdentityChecked)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExternalReference {
    pub title: String,
    pub url: String,
    pub note: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceMetadata {
    #[serde(default)]
    pub provenance: Provenance,
    #[serde(default)]
    pub official_reference_url: Option<String>,
    pub key: String,
    pub title: String,
    pub instrument: String,
    pub issuer: String,
    pub domains: Vec<String>,
    pub source_url: String,
    pub resolved_url: String,
    pub acquired_at_utc: String,
    pub acquired_at_unix: i64,
    pub verified_at_utc: String,
    pub verified_at_unix: i64,
    pub raw_path: String,
    pub raw_mime: String,
    pub raw_bytes: u64,
    pub raw_sha256: String,
    pub text_path: String,
    pub text_sha256: String,
    pub extraction: String,
    pub pages: Option<u64>,
    pub issued_on: Option<String>,
    pub effective_from: Option<String>,
    pub effectivity_note: String,
    pub applicability_note: String,
    pub verification_scope: String,
    pub acquisition_headers: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Deserialize)]
pub struct StarterManifest {
    pub schema_version: u32,
    pub bundle_id: String,
    pub verified_at_utc: String,
    pub verified_at_unix: i64,
    pub verification_scope: String,
    pub domains: Vec<Domain>,
    pub coverage_gaps: Vec<CoverageGap>,
    pub sources: Vec<SourceMetadata>,
    #[serde(default)]
    pub external_references: Vec<ExternalReference>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DomainSummary {
    pub id: String,
    pub label: String,
    pub source_count: u64,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct LibrarySummary {
    pub source_count: u64,
    pub verified_source_count: u64,
    pub identity_checked_upload_count: u64,
    pub provisional_source_count: u64,
    pub manually_imported_source_count: u64,
    pub awaiting_verification_count: u64,
    pub user_checked_source_count: u64,
    pub reviewed_adoption_count: u64,
    pub approved_update_count: u64,
    pub pending_update_count: u64,
    pub bundled_verified_at: Option<i64>,
    pub last_successful_check: Option<i64>,
    pub automatic_updates_available: bool,
    pub domains: Vec<DomainSummary>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SourceRecord {
    pub id: i64,
    pub metadata: SourceMetadata,
    pub identity_review: Option<SourceIdentityDecision>,
}
pub struct SourceSnapshot {
    pub metadata: SourceMetadata,
    pub raw: Vec<u8>,
    pub text: String,
}
pub struct LibraryCatalog {
    connection: Connection,
}

pub fn starter_manifest() -> Result<StarterManifest> {
    Ok(serde_json::from_str(MANIFEST)?)
}
pub fn default_library_directory() -> Result<PathBuf> {
    Ok(complyph_paths::default_data_root()?.join("library"))
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn validate_payload(metadata: &SourceMetadata, raw: &[u8], text: &str) -> Result<()> {
    if raw.len() as u64 != metadata.raw_bytes
        || sha256(raw) != metadata.raw_sha256
        || sha256(text.as_bytes()) != metadata.text_sha256
    {
        return Err(LibraryError::Integrity(metadata.instrument.clone()));
    }
    Ok(())
}

impl LibraryCatalog {
    /// Only the public-library directory is passed in. Bundled data is local.
    pub fn open(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let mut connection = Connection::open(directory.join("catalog.sqlite3"))?;
        // First import writes a substantial local bundle; allow another process to finish.
        connection.busy_timeout(Duration::from_secs(30))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i32 = transaction.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(LibraryError::NewerDatabase);
        }
        if version == 0 {
            transaction.execute_batch(
                "CREATE TABLE sources (
                    id INTEGER PRIMARY KEY, title TEXT NOT NULL, issuer TEXT NOT NULL,
                    source_url TEXT NOT NULL UNIQUE,
                    verification_status TEXT NOT NULL DEFAULT 'unverified'
                      CHECK(verification_status IN ('unverified','verified','needs_review'))
                 );
                 CREATE TABLE source_versions (
                    id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL REFERENCES sources(id),
                    content_sha256 TEXT NOT NULL, local_path TEXT NOT NULL, retrieved_at INTEGER NOT NULL,
                    published_at INTEGER, effective_from INTEGER, UNIQUE(source_id,content_sha256)
                 );"
            )?;
        }
        if version < 2 {
            transaction.execute_batch(
                "ALTER TABLE sources ADD COLUMN stable_key TEXT;
                 CREATE UNIQUE INDEX sources_by_stable_key ON sources(stable_key);
                 CREATE TABLE domains(id TEXT PRIMARY KEY,label TEXT NOT NULL);
                 CREATE TABLE source_domains(
                    source_id INTEGER NOT NULL REFERENCES sources(id),
                    domain_id TEXT NOT NULL REFERENCES domains(id),PRIMARY KEY(source_id,domain_id)
                 );
                 CREATE TABLE source_snapshots(
                    version_id INTEGER PRIMARY KEY REFERENCES source_versions(id),
                    raw_bytes BLOB NOT NULL,reading_text TEXT NOT NULL,
                    metadata_json TEXT NOT NULL,verified_at INTEGER NOT NULL
                 );
                 CREATE TABLE imported_bundles(
                    id TEXT PRIMARY KEY,manifest_sha256 TEXT NOT NULL,imported_at INTEGER NOT NULL
                 );
                 PRAGMA user_version=2;",
            )?;
        }
        // Schema 3 adds manual-import provenance to snapshot metadata. Older apps
        // must refuse it rather than misread the verification status.
        if version < 3 {
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        if version < 4 {
            updates::migrate(&transaction)?;
        }
        if version < 5 {
            maintenance::migrate(&transaction)?;
            discovery::migrate(&transaction)?;
        }
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
        let mut catalog = Self { connection };
        catalog.import_starter()?;
        catalog.validate_saved_snapshots()?;
        Ok(catalog)
    }

    fn import_starter(&mut self) -> Result<()> {
        let manifest = starter_manifest()?;
        if manifest.schema_version != 1 || manifest.sources.len() != bundled::PAYLOADS.len() {
            return Err(LibraryError::Integrity("bundled manifest".into()));
        }
        for metadata in &manifest.sources {
            let payload = bundled::PAYLOADS
                .iter()
                .find(|p| p.key == metadata.key)
                .ok_or_else(|| LibraryError::Integrity(metadata.key.clone()))?;
            validate_payload(metadata, payload.raw, payload.text)?;
            if metadata.domains.is_empty()
                || metadata
                    .domains
                    .iter()
                    .any(|id| !manifest.domains.iter().any(|d| &d.id == id))
            {
                return Err(LibraryError::Integrity(metadata.key.clone()));
            }
        }
        let manifest_hash = sha256(MANIFEST.as_bytes());
        let existing: Option<String> = self
            .connection
            .query_row(
                "SELECT manifest_sha256 FROM imported_bundles WHERE id=?1",
                [&manifest.bundle_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing != manifest_hash {
                return Err(LibraryError::Integrity("bundle identity changed".into()));
            }
            return Ok(());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Another opener may have installed this bundle while we validated payloads
        // or waited for the write lock. Recheck under that lock before binding BLOBs.
        let installed: Option<String> = transaction
            .query_row(
                "SELECT manifest_sha256 FROM imported_bundles WHERE id=?1",
                [&manifest.bundle_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(installed) = installed {
            if installed != manifest_hash {
                return Err(LibraryError::Integrity("bundle identity changed".into()));
            }
            transaction.commit()?;
            return Ok(());
        }
        for domain in &manifest.domains {
            transaction.execute("INSERT INTO domains(id,label) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET label=excluded.label", params![domain.id,domain.label])?;
        }
        for metadata in &manifest.sources {
            let payload = bundled::PAYLOADS
                .iter()
                .find(|p| p.key == metadata.key)
                .ok_or_else(|| LibraryError::Integrity(metadata.key.clone()))?;
            transaction.execute(
                "INSERT INTO sources(stable_key,title,issuer,source_url,verification_status)
                 VALUES(?1,?2,?3,?4,?5) ON CONFLICT(stable_key) DO NOTHING",
                params![
                    metadata.key,
                    metadata.title,
                    metadata.issuer,
                    metadata.source_url,
                    metadata.provenance.database_status()
                ],
            )?;
            let source_id: i64 = transaction.query_row(
                "SELECT id FROM sources WHERE stable_key=?1",
                [&metadata.key],
                |r| r.get(0),
            )?;
            for domain in &metadata.domains {
                transaction.execute(
                    "INSERT OR IGNORE INTO source_domains(source_id,domain_id) VALUES(?1,?2)",
                    params![source_id, domain],
                )?;
            }
            transaction.execute(
                "INSERT OR IGNORE INTO source_versions(source_id,content_sha256,local_path,retrieved_at)
                 VALUES(?1,?2,?3,?4)",
                params![source_id,metadata.raw_sha256,format!("sqlite:source_snapshots/{}",metadata.raw_sha256),metadata.acquired_at_unix],
            )?;
            let version_id: i64 = transaction.query_row(
                "SELECT id FROM source_versions WHERE source_id=?1 AND content_sha256=?2",
                params![source_id, metadata.raw_sha256],
                |r| r.get(0),
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO source_snapshots(version_id,raw_bytes,reading_text,metadata_json,verified_at)
                 VALUES(?1,?2,?3,?4,?5)",
                params![version_id,payload.raw,payload.text,serde_json::to_string(metadata)?,metadata.verified_at_unix],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO active_source_versions(source_id,version_id) VALUES(?1,?2)",
                params![source_id, version_id],
            )?;
        }
        let imported_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        transaction.execute(
            "INSERT OR IGNORE INTO imported_bundles(id,manifest_sha256,imported_at) VALUES(?1,?2,?3)",
            params![manifest.bundle_id, manifest_hash, imported_at],
        )?;
        let stored_hash: String = transaction.query_row(
            "SELECT manifest_sha256 FROM imported_bundles WHERE id=?1",
            [&manifest.bundle_id],
            |row| row.get(0),
        )?;
        if stored_hash != manifest_hash {
            return Err(LibraryError::Integrity("bundle identity changed".into()));
        }
        transaction.commit()?;
        Ok(())
    }

    fn validate_saved_snapshots(&self) -> Result<()> {
        let mut statement = self.connection.prepare(
            "SELECT p.metadata_json,p.raw_bytes,p.reading_text FROM source_snapshots p
             JOIN source_versions v ON v.id=p.version_id JOIN sources s ON s.id=v.source_id",
        )?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (metadata, raw, text) = row?;
            validate_payload(&serde_json::from_str(&metadata)?, &raw, &text)?;
        }
        Ok(())
    }

    pub fn summary(&self) -> Result<LibrarySummary> {
        let source_count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM sources", [], |r| r.get(0))?;
        let verified_source_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM sources s WHERE verification_status='verified'
             AND EXISTS(SELECT 1 FROM source_versions v JOIN source_snapshots p ON p.version_id=v.id WHERE v.source_id=s.id)",
            [],|r|r.get(0),
        )?;
        let bundled_verified_at = self.connection.query_row(
            "SELECT MAX(p.verified_at) FROM source_snapshots p JOIN source_versions v ON v.id=p.version_id
             JOIN sources s ON s.id=v.source_id WHERE s.stable_key NOT LIKE 'manual-%'",
            [],
            |r| r.get(0),
        )?;
        let records = self.sources()?;
        let count = |provenance| {
            records
                .iter()
                .filter(|s| s.metadata.provenance == provenance)
                .count() as u64
        };
        let identity_checked_upload_count = count(Provenance::UploadedIdentity);
        let provisional_source_count = count(Provenance::ThirdParty);
        let awaiting_verification_count = count(Provenance::ManualPending);
        let user_checked_source_count = count(Provenance::ManualIdentityChecked);
        let mut statement = self.connection.prepare(
            "SELECT d.id,d.label,COUNT(s.id) FROM domains d
             LEFT JOIN source_domains sd ON sd.domain_id=d.id
             LEFT JOIN sources s ON s.id=sd.source_id
             GROUP BY d.id,d.label ORDER BY d.rowid",
        )?;
        let domains = statement
            .query_map([], |r| {
                Ok(DomainSummary {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    source_count: r.get::<_, i64>(2)? as u64,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(LibrarySummary {
            source_count: source_count as u64,
            verified_source_count: verified_source_count as u64,
            identity_checked_upload_count,
            provisional_source_count,
            manually_imported_source_count: awaiting_verification_count + user_checked_source_count,
            awaiting_verification_count,
            user_checked_source_count,
            reviewed_adoption_count: count(Provenance::ReviewedDownload),
            bundled_verified_at,
            approved_update_count: self
                .update_watches()?
                .iter()
                .filter(|watch| watch.enabled)
                .count() as u64,
            pending_update_count: self.pending_update_count()?,
            last_successful_check: self.last_successful_source_check()?,
            automatic_updates_available: cfg!(any(windows, target_os = "linux")),
            domains,
        })
    }

    pub fn sources(&self) -> Result<Vec<SourceRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT s.id,p.metadata_json FROM sources s JOIN active_source_versions a ON a.source_id=s.id
             JOIN source_versions v ON v.id=a.version_id AND v.source_id=s.id
             JOIN source_snapshots p ON p.version_id=v.id ORDER BY s.id"
        )?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.map(|r| {
            let (id, metadata) = r?;
            let mut metadata = serde_json::from_str(&metadata)?;
            self.apply_annotations(id, &mut metadata)?;
            let identity_review = self.source_identity_decision(id, &metadata.raw_sha256)?;
            Ok(SourceRecord {
                id,
                metadata,
                identity_review,
            })
        })
        .collect()
    }

    pub fn snapshot(&self, source_id: i64) -> Result<SourceSnapshot> {
        let (metadata, raw, text): (String, Vec<u8>, String) = self
            .connection
            .query_row(
                "SELECT p.metadata_json,p.raw_bytes,p.reading_text FROM active_source_versions a
             JOIN source_versions v ON v.id=a.version_id AND v.source_id=a.source_id
             JOIN source_snapshots p ON p.version_id=v.id WHERE a.source_id=?1",
                [source_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or(LibraryError::MissingSnapshot)?;
        let metadata = serde_json::from_str(&metadata)?;
        validate_payload(&metadata, &raw, &text)?;
        Ok(SourceSnapshot {
            metadata,
            raw,
            text,
        })
    }

    pub fn snapshot_by_key(&self, key: &str) -> Result<SourceSnapshot> {
        let id: i64 = self.connection.query_row(
            "SELECT id FROM sources WHERE stable_key=?1",
            [key],
            |r| r.get(0),
        )?;
        self.snapshot(id)
    }

    /// Read the exact historical source used by a report; never substitute the latest version.
    pub fn snapshot_for_review(
        &self,
        key: &str,
        raw_sha256: &str,
        text_sha256: &str,
    ) -> Result<SourceSnapshot> {
        let (metadata,raw,text):(String,Vec<u8>,String)=self.connection.query_row(
            "SELECT p.metadata_json,p.raw_bytes,p.reading_text FROM sources s JOIN source_versions v ON v.source_id=s.id JOIN source_snapshots p ON p.version_id=v.id WHERE s.stable_key=?1 AND v.content_sha256=?2",
            params![key,raw_sha256],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or(LibraryError::MissingSnapshot)?;
        let metadata: SourceMetadata = serde_json::from_str(&metadata)?;
        validate_payload(&metadata, &raw, &text)?;
        if metadata.key != key
            || metadata.raw_sha256 != raw_sha256
            || metadata.text_sha256 != text_sha256
        {
            return Err(LibraryError::Integrity(
                "The report's exact saved source/text version is unavailable".into(),
            ));
        }
        Ok(SourceSnapshot {
            metadata,
            raw,
            text,
        })
    }

    /// Export a public original only, without overwriting existing files.
    pub fn export_source(&self, source_id: i64, output: &Path) -> Result<()> {
        let snapshot = self.snapshot(source_id)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        file.write_all(&snapshot.raw)?;
        file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_first_opens_share_one_complete_bundle() {
        let temp = tempfile::TempDir::new().unwrap();
        let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers = (0..2)
            .map(|_| {
                let gate = gate.clone();
                let path = temp.path().to_path_buf();
                std::thread::spawn(move || {
                    gate.wait();
                    LibraryCatalog::open(&path)
                        .unwrap()
                        .summary()
                        .unwrap()
                        .verified_source_count
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            assert_eq!(worker.join().unwrap(), 14);
        }
        assert_eq!(
            LibraryCatalog::open(temp.path())
                .unwrap()
                .summary()
                .unwrap()
                .source_count,
            19
        );
    }

    #[test]
    fn starter_has_real_snapshots_for_seven_domains_without_claiming_runtime_updates() {
        let temp = tempfile::TempDir::new().unwrap();
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        let summary = catalog.summary().unwrap();
        assert_eq!(summary.source_count, 19);
        assert_eq!(summary.verified_source_count, 14);
        assert_eq!(summary.domains.len(), 7);
        assert!(summary.domains.iter().all(|d| d.source_count > 0));
        assert!(summary.bundled_verified_at.is_some());
        assert_eq!(summary.last_successful_check, None);
        assert_eq!(
            summary.automatic_updates_available,
            cfg!(any(windows, target_os = "linux"))
        );
        let source = catalog.sources().unwrap().remove(0);
        let snapshot = catalog.snapshot(source.id).unwrap();
        assert!(snapshot.text.contains("442"));
        assert_eq!(sha256(&snapshot.raw), source.metadata.raw_sha256);
    }
    #[test]
    fn imports_are_idempotent_and_do_not_touch_private_data() {
        let temp = tempfile::TempDir::new().unwrap();
        let private = temp.path().join("private");
        fs::create_dir(&private).unwrap();
        fs::write(private.join("workspace.sqlite3"), b"private sentinel").unwrap();
        let public = temp.path().join("library");
        drop(LibraryCatalog::open(&public).unwrap());
        let catalog = LibraryCatalog::open(&public).unwrap();
        assert_eq!(catalog.summary().unwrap().source_count, 19);
        let versions: i64 = catalog
            .connection
            .query_row("SELECT COUNT(*) FROM source_versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(versions, 19);
        assert_eq!(
            fs::read(private.join("workspace.sqlite3")).unwrap(),
            b"private sentinel"
        );
    }
    #[test]
    fn version_one_entries_survive_migration() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Connection::open(temp.path().join("catalog.sqlite3")).unwrap();
        db.execute_batch("CREATE TABLE sources(id INTEGER PRIMARY KEY,title TEXT NOT NULL,issuer TEXT NOT NULL,source_url TEXT NOT NULL UNIQUE,verification_status TEXT NOT NULL DEFAULT 'unverified');CREATE TABLE source_versions(id INTEGER PRIMARY KEY,source_id INTEGER NOT NULL REFERENCES sources(id),content_sha256 TEXT NOT NULL,local_path TEXT NOT NULL,retrieved_at INTEGER NOT NULL,published_at INTEGER,effective_from INTEGER,UNIQUE(source_id,content_sha256));INSERT INTO sources VALUES(99,'Existing reference','Existing issuer','https://example.gov.ph/reference','unverified');INSERT INTO source_versions VALUES(99,99,'legacy-hash','original-location',123,NULL,NULL);PRAGMA user_version=1;").unwrap();
        drop(db);
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        assert_eq!(catalog.summary().unwrap().source_count, 20);
        let old: String = catalog
            .connection
            .query_row(
                "SELECT local_path FROM source_versions WHERE id=99",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old, "original-location");
    }
    #[test]
    fn corrupted_snapshot_is_rejected() {
        let temp = tempfile::TempDir::new().unwrap();
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        catalog.connection.execute("UPDATE source_snapshots SET raw_bytes=x'00' WHERE version_id=(SELECT MIN(version_id) FROM source_snapshots)",[]).unwrap();
        drop(catalog);
        assert!(matches!(
            LibraryCatalog::open(temp.path()),
            Err(LibraryError::Integrity(_))
        ));
    }
    #[test]
    fn exporting_preserves_bytes_and_refuses_overwrite() {
        let temp = tempfile::TempDir::new().unwrap();
        let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        let source = catalog.sources().unwrap().remove(0);
        let output = temp.path().join("official-source.html");
        catalog.export_source(source.id, &output).unwrap();
        assert_eq!(
            sha256(&fs::read(&output).unwrap()),
            source.metadata.raw_sha256
        );
        assert!(catalog.export_source(source.id, &output).is_err());
        assert_eq!(
            sha256(&fs::read(&output).unwrap()),
            source.metadata.raw_sha256
        );
    }
    #[test]
    fn newer_schema_is_not_modified() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Connection::open(temp.path().join("catalog.sqlite3")).unwrap();
        db.execute_batch("PRAGMA user_version=99;").unwrap();
        drop(db);
        assert!(matches!(
            LibraryCatalog::open(temp.path()),
            Err(LibraryError::NewerDatabase)
        ));
        let db = Connection::open(temp.path().join("catalog.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            99
        );
    }
    #[test]
    fn provenance_is_not_promoted_by_adding_uploaded_copies() {
        let temp = tempfile::tempdir().unwrap();
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        let summary = catalog.summary().unwrap();
        assert_eq!(summary.source_count, 19);
        assert_eq!(summary.verified_source_count, 14);
        assert_eq!(summary.identity_checked_upload_count, 3);
        assert_eq!(summary.provisional_source_count, 2);
        let upload = catalog.snapshot_by_key("dole-252-25-upload").unwrap();
        assert_eq!(upload.metadata.provenance, Provenance::UploadedIdentity);
        assert_eq!(upload.metadata.pages, Some(110));
        assert_eq!(
            catalog
                .snapshot_by_key("pagibig-499-upload")
                .unwrap()
                .metadata
                .provenance,
            Provenance::ThirdParty
        );
        assert!(
            starter_manifest().unwrap().external_references[0]
                .title
                .contains("PwC")
        );
        catalog.connection.execute("UPDATE source_snapshots SET reading_text='tampered' WHERE version_id=(SELECT v.id FROM source_versions v JOIN sources s ON s.id=v.source_id WHERE s.stable_key='pagibig-499-upload')",[]).unwrap();
        drop(catalog);
        assert!(LibraryCatalog::open(temp.path()).is_err());
    }
    #[test]
    fn review_citations_open_the_exact_snapshot_even_when_a_newer_version_exists() {
        let temp = tempfile::tempdir().unwrap();
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        let old = catalog.snapshot_by_key("labor-code-reference").unwrap();
        let mut new = old.metadata.clone();
        let raw = b"synthetic newer source";
        let text = "Synthetic later source; deliberately no old provisions.";
        new.raw_bytes = raw.len() as u64;
        new.raw_sha256 = sha256(raw);
        new.text_sha256 = sha256(text.as_bytes());
        let id: i64 = catalog
            .connection
            .query_row(
                "SELECT id FROM sources WHERE stable_key='labor-code-reference'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        catalog.connection.execute("INSERT INTO source_versions(source_id,content_sha256,local_path,retrieved_at) VALUES (?1,?2,'synthetic',?3)",params![id,new.raw_sha256,new.acquired_at_unix+10000]).unwrap();
        let version = catalog.connection.last_insert_rowid();
        catalog.connection.execute("INSERT INTO source_snapshots(version_id,raw_bytes,reading_text,metadata_json,verified_at) VALUES (?1,?2,?3,?4,?5)",params![version,raw,text,serde_json::to_string(&new).unwrap(),new.verified_at_unix]).unwrap();
        // Merely preserving newer bytes must not make them active.
        assert_eq!(
            catalog.snapshot(id).unwrap().metadata.raw_sha256,
            old.metadata.raw_sha256
        );
        catalog.restore_preserved_version(id,&new.raw_sha256,&AdoptionReview{
            reviewer:LibraryReviewer{actor:"Synthetic reviewer".into(),review_on:"2026-11-21".into(),basis:"Fictional version used to verify historical report citations; not a real legal adoption.".into()},
            expected_active_sha256:old.metadata.raw_sha256.clone(),identity_confirmed:true,original_and_reading_compared:true,effectivity_and_scope_reviewed:true,
        }).unwrap();
        assert_eq!(
            catalog
                .snapshot_by_key("labor-code-reference")
                .unwrap()
                .text,
            text
        );
        let exact = catalog
            .snapshot_for_review(
                &old.metadata.key,
                &old.metadata.raw_sha256,
                &old.metadata.text_sha256,
            )
            .unwrap();
        assert_eq!(exact.text, old.text);
        assert_eq!(exact.raw, old.raw);
        assert!(
            catalog
                .snapshot_for_review(
                    &old.metadata.key,
                    &old.metadata.raw_sha256,
                    "incorrect-text-hash"
                )
                .is_err()
        );
        assert!(
            catalog
                .snapshot_for_review(
                    &old.metadata.key,
                    "missing-version",
                    &old.metadata.text_sha256
                )
                .is_err()
        );
    }
    #[test]
    fn editorial_bundle_upgrade_retains_previous_snapshots_and_bundle_identity() {
        let temp = tempfile::tempdir().unwrap();
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        let source = catalog.snapshot_by_key("labor-code-reference").unwrap();
        catalog
            .connection
            .execute("DELETE FROM imported_bundles", [])
            .unwrap();
        catalog
            .connection
            .execute(
                "INSERT INTO imported_bundles(id,manifest_sha256,imported_at) VALUES (?1,?2,1)",
                params![
                    "complyph-starter-2026-11-17-v2",
                    "92d6979422a8778cbf343f8a1f00b9f272d289c5613697fab5909f68bb82b386"
                ],
            )
            .unwrap();
        drop(catalog);
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        assert_eq!(catalog.summary().unwrap().source_count, 19);
        assert_eq!(
            catalog
                .connection
                .query_row("SELECT COUNT(*) FROM imported_bundles", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            catalog
                .connection
                .query_row(
                    "SELECT manifest_sha256 FROM imported_bundles WHERE id=?1",
                    ["complyph-starter-2026-11-17-v2"],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "92d6979422a8778cbf343f8a1f00b9f272d289c5613697fab5909f68bb82b386"
        );
        let restored = catalog
            .snapshot_for_review(
                &source.metadata.key,
                &source.metadata.raw_sha256,
                &source.metadata.text_sha256,
            )
            .unwrap();
        assert_eq!(restored.raw, source.raw);
        assert_eq!(restored.text, source.text);
    }
}
