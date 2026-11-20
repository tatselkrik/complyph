//! Explicit local, nonrecursive intake. No parser, review, OCR or network is invoked.
use super::*;
use std::{collections::HashMap, time::Instant};

pub const QUIET_PERIOD: Duration = Duration::from_secs(10);
const BATCH: usize = 8;
pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch("CREATE TABLE folder_watch(id INTEGER PRIMARY KEY CHECK(id=1),root TEXT NOT NULL,enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),generation INTEGER NOT NULL);
        CREATE TABLE folder_intake(path TEXT PRIMARY KEY,signature TEXT NOT NULL,sha256 TEXT NOT NULL,document_id INTEGER REFERENCES documents(id) ON DELETE SET NULL);
        CREATE INDEX folder_intake_by_hash ON folder_intake(sha256); PRAGMA user_version=5;")?;
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration {
    pub root: PathBuf,
    pub enabled: bool,
    pub generation: i64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scan {
    pub added: Vec<i64>,
    pub updated: usize,
    pub duplicates: usize,
    pub waiting: usize,
    pub too_large: usize,
    pub ignored: usize,
}
impl Workspace {
    pub fn watched_folder(&self) -> Result<Option<Configuration>> {
        self.connection
            .query_row(
                "SELECT root,enabled,generation FROM folder_watch WHERE id=1 AND root<>''",
                [],
                |r| {
                    Ok(Configuration {
                        root: PathBuf::from(r.get::<_, String>(0)?),
                        enabled: r.get(1)?,
                        generation: r.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }
    pub fn configure_watched_folder(&self, root: &Path, enabled: bool) -> Result<Configuration> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() || !local_path(&root) {
            return Err(WorkspaceError::InvalidDocument(
                "Choose a local folder on this PC, not a network location.".into(),
            ));
        }
        if let Some(database) = self.connection.path() {
            let workspace = Path::new(database)
                .parent()
                .and_then(Path::parent)
                .and_then(|p| fs::canonicalize(p).ok());
            if workspace.is_some_and(|p| root.starts_with(&p) || p.starts_with(&root)) {
                return Err(WorkspaceError::InvalidDocument("Choose a document intake folder outside the ComplyPH workspace and its parent folders.".into()));
            }
        }
        self.connection.execute("INSERT INTO folder_watch VALUES(1,?1,?2,1) ON CONFLICT(id) DO UPDATE SET root=excluded.root,enabled=excluded.enabled,generation=folder_watch.generation+1",params![root.to_str().ok_or_else(||WorkspaceError::InvalidDocument("This folder path is unsupported.".into()))?,enabled])?;
        self.watched_folder()?.ok_or_else(|| {
            WorkspaceError::InvalidDocument("The watched folder was not saved.".into())
        })
    }
    pub fn forget_watched_folder(&self) -> Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        tx.execute(
            "UPDATE folder_watch SET root='',enabled=0,generation=generation+1",
            [],
        )?;
        tx.execute("DELETE FROM folder_intake", [])?;
        tx.commit()?;
        Ok(())
    }
    pub fn set_folder_watch_enabled(&self, enabled: bool) -> Result<()> {
        self.connection.execute(
            "UPDATE folder_watch SET enabled=?1,generation=generation+1 WHERE id=1",
            [enabled],
        )?;
        Ok(())
    }
}

fn local_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let drive = match path.components().next() {
            Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
                std::path::Prefix::Disk(d) | std::path::Prefix::VerbatimDisk(d) => d,
                _ => return false,
            },
            _ => return false,
        };
        let root = [drive as u16, 58, 92, 0];
        // SAFETY: NUL-terminated drive root; no document is opened.
        matches!(
            unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root.as_ptr()) },
            2 | 3 | 6
        )
    }
    #[cfg(not(windows))]
    {
        path.is_absolute()
    }
}

fn signature(metadata: &fs::Metadata) -> Option<String> {
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(format!(
        "{}:{}:{}",
        metadata.len(),
        modified.as_secs(),
        modified.subsec_nanos()
    ))
}
fn completed(kind: DocumentKind, bytes: &[u8]) -> bool {
    match kind {
        DocumentKind::Pdf => {
            bytes.starts_with(b"%PDF-")
                && bytes[bytes.len().saturating_sub(2048)..]
                    .windows(5)
                    .any(|w| w == b"%%EOF")
        }
        DocumentKind::Docx => {
            bytes.starts_with(b"PK\x03\x04")
                && bytes[bytes.len().saturating_sub(65_557)..]
                    .windows(4)
                    .any(|w| w == b"PK\x05\x06")
        }
    }
}
fn eligible(path: &Path, metadata: &fs::Metadata) -> bool {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || DocumentKind::from_path(path).is_none()
    {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    !name.starts_with('.')
        && !name.starts_with("~$")
        && !name.contains(".tmp.")
        && !name.contains(".partial.")
}
fn read_lock(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    options.open(path)
}
struct Observation {
    signature: String,
    since: Instant,
}
#[derive(Default)]
pub struct Scanner {
    generation: Option<i64>,
    cursor: usize,
    observed: HashMap<PathBuf, Observation>,
}
impl Scanner {
    /// Poll on a background thread while the desktop is open. Time is injected for deterministic tests.
    pub fn poll(&mut self, workspace: &Workspace, now: Instant) -> Result<Scan> {
        let mut outcome = Scan::default();
        let Some(config) = workspace.watched_folder()?.filter(|c| c.enabled) else {
            self.observed.clear();
            return Ok(outcome);
        };
        if self.generation != Some(config.generation) {
            self.observed.clear();
            self.generation = Some(config.generation);
            self.cursor = 0;
        }
        if fs::symlink_metadata(&config.root)?.file_type().is_symlink()
            || fs::canonicalize(&config.root)? != config.root
        {
            return Err(WorkspaceError::InvalidDocument("The watched folder moved or became a link. Choose it again before resuming intake.".into()));
        }
        let mut paths = Vec::new();
        let mut present = std::collections::HashSet::new();
        for (index, entry) in fs::read_dir(&config.root)?.enumerate() {
            if index >= 10_000 {
                return Err(WorkspaceError::InvalidDocument(
                    "This folder has too many entries. Choose a smaller intake folder.".into(),
                ));
            }
            let Ok(entry) = entry else {
                outcome.ignored += 1;
                continue;
            };
            let path = entry.path();
            present.insert(path.clone());
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                outcome.waiting += 1;
                continue;
            };
            if !eligible(&path, &metadata) {
                outcome.ignored += 1;
                continue;
            }
            if metadata.len() > complyph_extract::MAX_INPUT_BYTES as u64 {
                outcome.too_large += 1;
                continue;
            }
            let Some(sig) = signature(&metadata) else {
                outcome.waiting += 1;
                continue;
            };
            let old: Option<String> = workspace
                .connection
                .query_row(
                    "SELECT signature FROM folder_intake WHERE path=?1",
                    [path.to_str()],
                    |r| r.get(0),
                )
                .optional()?;
            if old.as_deref() == Some(&sig) {
                continue;
            }
            let observation = self
                .observed
                .entry(path.clone())
                .or_insert_with(|| Observation {
                    signature: sig.clone(),
                    since: now,
                });
            if observation.signature != sig {
                observation.signature = sig.clone();
                observation.since = now;
            }
            if now.saturating_duration_since(observation.since) < QUIET_PERIOD {
                outcome.waiting += 1;
                continue;
            }
            paths.push((path, sig));
        }
        paths.sort_by(|a, b| a.0.cmp(&b.0));
        self.observed.retain(|p, _| present.contains(p));
        let count = paths.len();
        let batch = if count == 0 {
            Vec::new()
        } else {
            let offset = self.cursor % count;
            self.cursor = (offset + BATCH) % count;
            paths
                .iter()
                .cycle()
                .skip(offset)
                .take(BATCH.min(count))
                .cloned()
                .collect::<Vec<_>>()
        };
        for (path, sig) in batch {
            let Ok(_guard) = read_lock(&path) else {
                outcome.waiting += 1;
                continue;
            };
            let Ok(snapshot) = document_snapshot(&path) else {
                outcome.waiting += 1;
                continue;
            };
            if signature(&fs::metadata(&path)?) != Some(sig.clone())
                || !completed(
                    DocumentKind::from_path(&path).expect("eligible kind"),
                    &snapshot.bytes,
                )
            {
                outcome.waiting += 1;
                continue;
            }
            let tx = rusqlite::Transaction::new_unchecked(
                &workspace.connection,
                rusqlite::TransactionBehavior::Immediate,
            )?;
            if workspace.watched_folder()?.as_ref() != Some(&config) {
                return Ok(outcome);
            }
            let canonical = fs::canonicalize(&path)?;
            if canonical.parent() != Some(config.root.as_path())
                || !eligible(&path, &fs::symlink_metadata(&path)?)
            {
                outcome.ignored += 1;
                continue;
            }
            let candidates = {
                let mut statement=tx.prepare("SELECT DISTINCT d.id,d.path FROM documents d LEFT JOIN folder_intake i ON i.document_id=d.id LEFT JOIN document_extractions e ON e.document_id=d.id WHERE i.sha256=?1 OR json_extract(e.value,'$.sha256')=?1")?;
                statement
                    .query_map([&snapshot.sha256], |r| {
                        Ok((r.get::<_, i64>(0)?, PathBuf::from(r.get::<_, String>(1)?)))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            };
            let duplicate = candidates.into_iter().find(|(_, p)| {
                *p != canonical
                    && local_path(p)
                    && document_snapshot(p).is_ok_and(|s| s.sha256 == snapshot.sha256)
            });
            let existed: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE path=?1)",
                [canonical.to_str()],
                |r| r.get(0),
            )?;
            let id = if let Some((id, _)) = duplicate {
                outcome.duplicates += 1;
                id
            } else {
                let document = workspace.register_document(&canonical)?;
                if existed {
                    outcome.updated += 1;
                } else {
                    outcome.added.push(document.id);
                }
                document.id
            };
            tx.execute("INSERT INTO folder_intake(path,signature,sha256,document_id) VALUES(?1,?2,?3,?4) ON CONFLICT(path) DO UPDATE SET signature=excluded.signature,sha256=excluded.sha256,document_id=excluded.document_id",params![path.to_str(),sig,snapshot.sha256,id])?;
            tx.commit()?;
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_writes_completion_duplicates_restart_pause_and_removal_are_respected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("intake");
        fs::create_dir(&root).unwrap();
        let paths = WorkspacePaths::under(temp.path().join("data"));
        let workspace = Workspace::open(&paths).unwrap();
        assert!(
            workspace
                .configure_watched_folder(&paths.root, true)
                .is_err()
        );
        workspace.configure_watched_folder(&root, true).unwrap();
        let file = root.join("Fictional.pdf");
        let bytes = include_bytes!("../../../examples/Synthetic employment.pdf");
        fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
        let now = Instant::now();
        let mut scanner = Scanner::default();
        assert_eq!(scanner.poll(&workspace, now).unwrap().waiting, 1);
        assert!(
            scanner
                .poll(&workspace, now + QUIET_PERIOD)
                .unwrap()
                .added
                .is_empty()
        );
        fs::write(&file, bytes).unwrap();
        scanner.poll(&workspace, now + QUIET_PERIOD).unwrap();
        let added = scanner.poll(&workspace, now + QUIET_PERIOD * 2).unwrap();
        assert_eq!(added.added.len(), 1);
        fs::write(root.join("Duplicate.pdf"), bytes).unwrap();
        scanner.poll(&workspace, now + QUIET_PERIOD * 3).unwrap();
        assert_eq!(
            scanner
                .poll(&workspace, now + QUIET_PERIOD * 4)
                .unwrap()
                .duplicates,
            1
        );
        assert_eq!(workspace.documents().unwrap().len(), 1);
        assert!(
            Scanner::default()
                .poll(&workspace, now)
                .unwrap()
                .added
                .is_empty()
        );
        workspace.set_folder_watch_enabled(false).unwrap();
        fs::write(root.join("Paused.pdf"), bytes).unwrap();
        assert_eq!(
            scanner
                .poll(&workspace, now + QUIET_PERIOD * 10)
                .unwrap()
                .waiting,
            0
        );
        workspace.remove_document(added.added[0]).unwrap();
        workspace.set_folder_watch_enabled(true).unwrap();
        scanner.poll(&workspace, now + QUIET_PERIOD * 11).unwrap();
        // Unchanged paths explicitly removed by the user stay removed; newly arrived paths can be imported.
        let next = scanner.poll(&workspace, now + QUIET_PERIOD * 12).unwrap();
        assert_eq!(next.added.len(), 1);
        assert_eq!(fs::read(&file).unwrap(), bytes);
        assert!(workspace.reviews().unwrap().is_empty());
    }
    #[test]
    fn links_temporary_names_subfolders_and_oversize_files_are_never_intake_candidates() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("intake");
        fs::create_dir(&root).unwrap();
        let workspace = Workspace::open(&WorkspacePaths::under(temp.path().join("data"))).unwrap();
        workspace.configure_watched_folder(&root, true).unwrap();
        fs::write(root.join("~$lock.docx"), b"PK\x03\x04").unwrap();
        fs::create_dir(root.join("Nested")).unwrap();
        fs::write(
            root.join("Nested/Hidden.pdf"),
            include_bytes!("../../../examples/Synthetic employment.pdf"),
        )
        .unwrap();
        fs::File::create(root.join("Huge.pdf"))
            .unwrap()
            .set_len(complyph_extract::MAX_INPUT_BYTES as u64 + 1)
            .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("Nested/Hidden.pdf"), root.join("Link.pdf")).unwrap();
        let mut scanner = Scanner::default();
        let now = Instant::now();
        let scan = scanner.poll(&workspace, now).unwrap();
        assert_eq!(scan.too_large, 1);
        scanner.poll(&workspace, now + QUIET_PERIOD).unwrap();
        assert!(workspace.documents().unwrap().is_empty());
    }
}
