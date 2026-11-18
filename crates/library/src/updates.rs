//! Approved public URL watches and immutable downloaded copies. No HTTP or private-store dependency.
use super::*;
use url::Url;

pub const MAX_DOWNLOAD_BYTES: usize = 32 * 1024 * 1024;
type PreviousCheck = (String, Option<String>, Option<String>, Option<String>);

pub fn approved_update_url(value: &str) -> Result<String> {
    let url = Url::parse(value.trim())
        .map_err(|_| LibraryError::InvalidImport("Enter a complete official HTTPS URL.".into()))?;
    let host = url.host_str().unwrap_or_default();
    let agencies = [
        "dole.gov.ph",
        "senate.gov.ph",
        "privacy.gov.ph",
        "bir.gov.ph",
        "sss.gov.ph",
        "philhealth.gov.ph",
        "pagibigfund.gov.ph",
        "officialgazette.gov.ph",
    ];
    if value.len() > 2048
        || value.chars().any(char::is_whitespace)
        || url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !agencies
            .iter()
            .any(|agency| host == *agency || host.ends_with(&format!(".{agency}")))
    {
        return Err(LibraryError::InvalidImport("Use an HTTPS publication URL on a supported Philippine agency domain, without credentials, fragments or a custom port.".into()));
    }
    Ok(url.to_string())
}

pub fn same_origin(first: &str, second: &str) -> bool {
    match (Url::parse(first), Url::parse(second)) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => false,
    }
}

pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch("CREATE TABLE source_update_watches(
        source_id INTEGER PRIMARY KEY REFERENCES sources(id), url TEXT NOT NULL, enabled INTEGER NOT NULL CHECK(enabled IN(0,1)),
        approved_at INTEGER NOT NULL, last_attempt INTEGER, last_success INTEGER, last_status TEXT NOT NULL DEFAULT 'Never checked',
        last_error TEXT, last_hash TEXT, etag TEXT, last_modified TEXT, lease_token TEXT, lease_until INTEGER);
      CREATE TABLE source_update_checks(id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL REFERENCES sources(id),
        url TEXT NOT NULL, checked_at INTEGER NOT NULL, outcome TEXT NOT NULL, content_sha256 TEXT, detail TEXT NOT NULL);
      CREATE TABLE downloaded_source_changes(id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL REFERENCES sources(id),
        url TEXT NOT NULL, resolved_url TEXT NOT NULL, downloaded_at INTEGER NOT NULL, content_sha256 TEXT NOT NULL,
        baseline_sha256 TEXT NOT NULL, raw_bytes BLOB NOT NULL, mime TEXT NOT NULL,
        reviewed_at INTEGER, review_note TEXT, UNIQUE(source_id,url,content_sha256));
      CREATE INDEX changes_pending ON downloaded_source_changes(source_id,reviewed_at);
      PRAGMA user_version=4;")?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateWatch {
    pub source_id: i64,
    pub url: String,
    pub enabled: bool,
    pub approved_at: i64,
    pub last_attempt: Option<i64>,
    pub last_success: Option<i64>,
    pub last_status: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateTarget {
    pub source_id: i64,
    pub url: String,
    pub reference_sha256: String,
    pub previous_sha256: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub lease_token: String,
}

pub enum UpdateReply {
    Download {
        resolved_url: String,
        mime: String,
        etag: Option<String>,
        last_modified: Option<String>,
        bytes: Vec<u8>,
    },
    NotModified {
        resolved_url: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckReceipt {
    pub source_id: i64,
    pub checked_at: i64,
    pub outcome: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadedChange {
    pub id: i64,
    pub source_id: i64,
    pub url: String,
    pub resolved_url: String,
    pub downloaded_at: i64,
    pub content_sha256: String,
    pub baseline_sha256: String,
    pub bytes: u64,
    pub mime: String,
    pub reviewed_at: Option<i64>,
    pub review_note: Option<String>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn header(value: Option<String>, max: usize) -> Option<String> {
    value.filter(|v| {
        !v.is_empty() && v.len() <= max && v.is_ascii() && !v.chars().any(char::is_control)
    })
}

pub fn download_mime(bytes: &[u8], claimed: &str) -> Result<&'static str> {
    if bytes.is_empty() || bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err(LibraryError::InvalidImport(
            "The response was empty or exceeded 32 MiB.".into(),
        ));
    }
    if bytes.starts_with(b"%PDF-") {
        return Ok("application/pdf");
    }
    if bytes.starts_with(b"PK\x03\x04")
        && claimed.split(';').next()
            == Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    {
        return Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.document");
    }
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).to_ascii_lowercase();
    if claimed.starts_with("text/html")
        && (prefix.contains("<html") || prefix.contains("<!doctype html"))
    {
        return Ok("text/html");
    }
    if claimed.starts_with("text/plain")
        && !bytes.contains(&0)
        && std::str::from_utf8(bytes).is_ok()
    {
        return Ok("text/plain");
    }
    Err(LibraryError::InvalidImport(
        "The server did not return a supported PDF, DOCX, HTML or plain-text reference.".into(),
    ))
}

impl LibraryCatalog {
    pub fn disable_update_watch(&self, source_id: i64) -> Result<()> {
        self.connection.execute("UPDATE source_update_watches SET enabled=0,lease_token=NULL,lease_until=NULL WHERE source_id=?1",[source_id])?;
        Ok(())
    }
    pub fn set_update_watch(&self, source_id: i64, url: &str, enabled: bool) -> Result<()> {
        self.snapshot(source_id)?;
        let url = approved_update_url(url)?;
        self.connection.execute("INSERT INTO source_update_watches(source_id,url,enabled,approved_at) VALUES(?1,?2,?3,?4)
          ON CONFLICT(source_id) DO UPDATE SET url=excluded.url,enabled=excluded.enabled,approved_at=excluded.approved_at,
          last_attempt=CASE WHEN url=excluded.url THEN last_attempt END,
          last_success=CASE WHEN url=excluded.url THEN last_success END,
          last_status=CASE WHEN url=excluded.url THEN last_status ELSE 'Never checked' END,
          last_error=NULL,last_hash=CASE WHEN url=excluded.url THEN last_hash END,
          etag=CASE WHEN url=excluded.url THEN etag END,last_modified=CASE WHEN url=excluded.url THEN last_modified END,
          lease_token=NULL,lease_until=NULL", params![source_id,url,enabled,now()])?;
        Ok(())
    }

    pub fn update_watches(&self) -> Result<Vec<UpdateWatch>> {
        let mut stmt = self.connection.prepare("SELECT source_id,url,enabled,approved_at,last_attempt,last_success,last_status,last_error FROM source_update_watches ORDER BY source_id")?;
        Ok(stmt
            .query_map([], |r| {
                Ok(UpdateWatch {
                    source_id: r.get(0)?,
                    url: r.get(1)?,
                    enabled: r.get(2)?,
                    approved_at: r.get(3)?,
                    last_attempt: r.get(4)?,
                    last_success: r.get(5)?,
                    last_status: r.get(6)?,
                    last_error: r.get(7)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn due_source_ids(&self, due_only: bool) -> Result<Vec<i64>> {
        let mut stmt = self.connection.prepare("SELECT source_id FROM source_update_watches WHERE enabled=1
          AND (lease_until IS NULL OR lease_until<?1) AND (?2=0 OR last_attempt IS NULL OR last_attempt<=?1-86400 OR last_status='checking')
          ORDER BY COALESCE(last_attempt,0),source_id LIMIT 20")?;
        Ok(stmt
            .query_map(params![now(), due_only], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn claim_source_check(
        &mut self,
        source_id: i64,
        due_only: bool,
    ) -> Result<Option<UpdateTarget>> {
        let snapshot = self.snapshot(source_id)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<PreviousCheck> = tx.query_row(
            "SELECT url,last_hash,etag,last_modified FROM source_update_watches WHERE source_id=?1 AND enabled=1
             AND (lease_until IS NULL OR lease_until<?2) AND (?3=0 OR last_attempt IS NULL OR last_attempt<=?2-86400 OR last_status='checking')",
            params![source_id,now(),due_only],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((url, previous_sha256, etag, last_modified)) = previous else {
            return Ok(None);
        };
        approved_update_url(&url)?;
        if let Some(hash) = &previous_sha256
            && hash != &snapshot.metadata.raw_sha256
        {
            let raw: Vec<u8> = tx.query_row("SELECT raw_bytes FROM downloaded_source_changes WHERE source_id=?1 AND url=?2 AND content_sha256=?3",
                params![source_id,url,hash], |r|r.get(0))?;
            if sha256(&raw) != *hash {
                return Err(LibraryError::Integrity("downloaded source history".into()));
            }
        }
        let token = sha256(
            format!(
                "{}-{source_id}-{}-{}",
                std::process::id(),
                now(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            )
            .as_bytes(),
        );
        tx.execute("UPDATE source_update_watches SET last_attempt=?1,lease_until=?1+90,lease_token=?2,last_status='checking',last_error=NULL WHERE source_id=?3",params![now(),token,source_id])?;
        tx.commit()?;
        Ok(Some(UpdateTarget {
            source_id,
            url,
            reference_sha256: snapshot.metadata.raw_sha256,
            etag: header(etag, 512),
            last_modified: header(last_modified, 128),
            previous_sha256,
            lease_token: token,
        }))
    }

    pub fn finish_source_check(
        &mut self,
        target: &UpdateTarget,
        reply: UpdateReply,
    ) -> Result<CheckReceipt> {
        let checked_at = now();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = tx.query_row("SELECT lease_token FROM source_update_watches WHERE source_id=?1 AND enabled=1 AND url=?2",params![target.source_id,target.url],|r|r.get(0)).optional()?.flatten();
        if current.as_deref() != Some(&target.lease_token) {
            return Err(LibraryError::InvalidImport(
                "The source approval changed while checking; the response was discarded.".into(),
            ));
        }
        let mut hash = None;
        let mut validators = None;
        let (outcome, detail, success) = match reply {
            UpdateReply::Download {
                resolved_url,
                mime,
                etag,
                last_modified,
                bytes,
            } => {
                let valid = approved_update_url(&resolved_url).and_then(|_| {
                    if !same_origin(&target.url, &resolved_url) {
                        return Err(LibraryError::InvalidImport(
                            "A redirect left the approved origin.".into(),
                        ));
                    }
                    download_mime(&bytes, &mime)
                });
                match valid {
                    Err(error) => ("failed", error.to_string(), false),
                    Ok(mime) => {
                        let digest = sha256(&bytes);
                        let changed = digest
                            != target
                                .previous_sha256
                                .as_deref()
                                .unwrap_or(&target.reference_sha256);
                        if digest != target.reference_sha256 {
                            tx.execute("INSERT OR IGNORE INTO downloaded_source_changes(source_id,url,resolved_url,downloaded_at,content_sha256,baseline_sha256,raw_bytes,mime)
                              VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![target.source_id,target.url,resolved_url,checked_at,digest,target.reference_sha256,bytes,mime])?;
                        }
                        validators = Some((header(etag, 512), header(last_modified, 128)));
                        hash = Some(digest);
                        if changed {
                            ("changed","New bytes saved locally for review. The reference and checking rules were preserved.".into(),true)
                        } else {
                            ("unchanged","No byte change since the last saved comparison. Existing review flags remain visible.".into(),true)
                        }
                    }
                }
            }
            UpdateReply::NotModified { resolved_url } => {
                if target.previous_sha256.is_some()
                    && (target.etag.is_some() || target.last_modified.is_some())
                    && same_origin(&target.url, &resolved_url)
                    && approved_update_url(&resolved_url).is_ok()
                {
                    ("unchanged","The approved server returned 304; the previous preserved copy is retained.".into(),true)
                } else {
                    ("failed","An unsolicited or invalid 304 response could not establish source currency.".into(),false)
                }
            }
            UpdateReply::Failed { message } => (
                "failed",
                message
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(600)
                    .collect(),
                false,
            ),
        };
        tx.execute("INSERT INTO source_update_checks(source_id,url,checked_at,outcome,content_sha256,detail) VALUES(?1,?2,?3,?4,?5,?6)",params![target.source_id,target.url,checked_at,outcome,hash,detail])?;
        tx.execute("UPDATE source_update_watches SET last_success=CASE WHEN ?1 THEN ?2 ELSE last_success END,
            last_status=?3,last_error=CASE WHEN ?1 THEN NULL ELSE ?4 END,last_hash=COALESCE(?5,last_hash),
            lease_token=NULL,lease_until=NULL WHERE source_id=?6",params![success,checked_at,outcome,detail,hash,target.source_id])?;
        if let Some((etag, last_modified)) = validators {
            tx.execute(
                "UPDATE source_update_watches SET etag=?1,last_modified=?2 WHERE source_id=?3",
                params![etag, last_modified, target.source_id],
            )?;
        }
        tx.commit()?;
        Ok(CheckReceipt {
            source_id: target.source_id,
            checked_at,
            outcome: outcome.into(),
            detail,
        })
    }

    pub fn downloaded_changes(&self, source_id: i64) -> Result<Vec<DownloadedChange>> {
        let mut stmt = self.connection.prepare("SELECT id,source_id,url,resolved_url,downloaded_at,content_sha256,baseline_sha256,length(raw_bytes),mime,reviewed_at,review_note FROM downloaded_source_changes WHERE source_id=?1 ORDER BY id DESC")?;
        Ok(stmt
            .query_map([source_id], |r| {
                Ok(DownloadedChange {
                    id: r.get(0)?,
                    source_id: r.get(1)?,
                    url: r.get(2)?,
                    resolved_url: r.get(3)?,
                    downloaded_at: r.get(4)?,
                    content_sha256: r.get(5)?,
                    baseline_sha256: r.get(6)?,
                    bytes: r.get::<_, i64>(7)? as u64,
                    mime: r.get(8)?,
                    reviewed_at: r.get(9)?,
                    review_note: r.get(10)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn export_downloaded_change(&self, id: i64, output: &Path) -> Result<()> {
        let (raw, expected): (Vec<u8>, String) = self.connection.query_row(
            "SELECT raw_bytes,content_sha256 FROM downloaded_source_changes WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if sha256(&raw) != expected {
            return Err(LibraryError::Integrity("downloaded source copy".into()));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        file.write_all(&raw)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn mark_download_reviewed(&self, id: i64, note: &str) -> Result<()> {
        let note = note.trim();
        if note.is_empty() || note.len() > 4000 {
            return Err(LibraryError::InvalidImport(
                "Record a review note (up to 4,000 bytes).".into(),
            ));
        }
        let changed = self.connection.execute("UPDATE downloaded_source_changes SET reviewed_at=?1,review_note=?2 WHERE id=?3 AND reviewed_at IS NULL",params![now(),note,id])?;
        if changed != 1 {
            return Err(LibraryError::InvalidImport(
                "This download has already been reviewed or is unavailable.".into(),
            ));
        }
        Ok(())
    }

    pub fn pending_update_count(&self) -> Result<u64> {
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM downloaded_source_changes WHERE reviewed_at IS NULL",
            [],
            |r| r.get::<_, i64>(0),
        )? as u64)
    }

    pub fn reference_has_downloaded_changes(&self, key: &str) -> Result<bool> {
        Ok(self.connection.query_row("SELECT EXISTS(SELECT 1 FROM downloaded_source_changes c JOIN sources s ON s.id=c.source_id WHERE s.stable_key=?1)",[key],|r|r.get(0))?)
    }

    pub fn last_successful_source_check(&self) -> Result<Option<i64>> {
        Ok(self.connection.query_row(
            "SELECT MAX(last_success) FROM source_update_watches",
            [],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, LibraryCatalog, i64, String) {
        let temp = tempfile::tempdir().unwrap();
        let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        let source = catalog
            .sources()
            .unwrap()
            .into_iter()
            .find(|s| s.metadata.key == "labor-code-reference")
            .unwrap();
        (temp, catalog, source.id, source.metadata.source_url)
    }
    fn download(url: &str, body: &[u8]) -> UpdateReply {
        UpdateReply::Download {
            resolved_url: url.into(),
            mime: "application/pdf".into(),
            etag: Some("\"public-version\"".into()),
            last_modified: None,
            bytes: body.to_vec(),
        }
    }
    #[test]
    fn official_urls_need_explicit_approval_and_cannot_smuggle_credentials_or_local_targets() {
        for url in [
            "http://privacy.gov.ph/",
            "https://127.0.0.1/",
            "https://privacy.gov.ph.evil.example/",
            "https://evilprivacy.gov.ph/",
            "https://privacy.gov.ph:444/",
            "https://name:secret@privacy.gov.ph/",
            "https://privacy.gov.ph/#fragment",
            "file:///private/workspace.sqlite3",
            "https://www.facebook.com/example",
        ] {
            assert!(approved_update_url(url).is_err(), "{url}");
        }
        assert!(approved_update_url("https://bir-cdn.bir.gov.ph/BIR/pdf/test.pdf").is_ok());
        let (_temp, catalog, id, url) = setup();
        assert!(catalog.due_source_ids(false).unwrap().is_empty());
        assert_eq!(catalog.summary().unwrap().approved_update_count, 0);
        catalog.set_update_watch(id, &url, true).unwrap();
        assert_eq!(catalog.due_source_ids(true).unwrap(), vec![id]);
        catalog.set_update_watch(id, &url, false).unwrap();
        assert!(catalog.due_source_ids(false).unwrap().is_empty());
        assert_eq!(catalog.update_watches().unwrap().len(), 1);
    }
    #[test]
    fn changed_unchanged_failed_and_reverted_downloads_preserve_rules_private_data_and_every_copy()
    {
        let (temp, mut catalog, id, url) = setup();
        let private = temp.path().join("private");
        fs::create_dir(&private).unwrap();
        fs::write(private.join("workspace.sqlite3"), b"private sentinel").unwrap();
        let old = catalog.snapshot(id).unwrap();
        let original_hash = old.metadata.raw_sha256.clone();
        catalog.set_update_watch(id, &url, true).unwrap();
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert!(catalog.claim_source_check(id, false).unwrap().is_none());
        assert_eq!(
            catalog
                .finish_source_check(
                    &target,
                    UpdateReply::Download {
                        resolved_url: url.clone(),
                        mime: old.metadata.raw_mime.clone(),
                        etag: Some("\"original\"".into()),
                        last_modified: None,
                        bytes: old.raw.clone()
                    }
                )
                .unwrap()
                .outcome,
            "unchanged"
        );
        assert!(catalog.due_source_ids(true).unwrap().is_empty());
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        let first = b"%PDF-1.7\nFictional changed public reference A";
        assert_eq!(
            catalog
                .finish_source_check(&target, download(&url, first))
                .unwrap()
                .outcome,
            "changed"
        );
        assert_eq!(catalog.pending_update_count().unwrap(), 1);
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert!(target.etag.is_some());
        assert_eq!(
            catalog
                .finish_source_check(
                    &target,
                    UpdateReply::NotModified {
                        resolved_url: url.clone()
                    }
                )
                .unwrap()
                .outcome,
            "unchanged"
        );
        assert_eq!(catalog.pending_update_count().unwrap(), 1);
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert_eq!(
            catalog
                .finish_source_check(&target, download(&url, first))
                .unwrap()
                .outcome,
            "unchanged"
        );
        let last_success = catalog.last_successful_source_check().unwrap();
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert_eq!(
            catalog
                .finish_source_check(
                    &target,
                    UpdateReply::Failed {
                        message: "HTTP 503".into()
                    }
                )
                .unwrap()
                .outcome,
            "failed"
        );
        assert_eq!(
            catalog.last_successful_source_check().unwrap(),
            last_success
        );
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        let second = b"%PDF-1.7\nFictional changed public reference B";
        assert_eq!(
            catalog
                .finish_source_check(&target, download(&url, second))
                .unwrap()
                .outcome,
            "changed"
        );
        assert_eq!(catalog.downloaded_changes(id).unwrap().len(), 2);
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert_eq!(
            catalog
                .finish_source_check(
                    &target,
                    UpdateReply::Download {
                        resolved_url: url.clone(),
                        mime: old.metadata.raw_mime.clone(),
                        etag: Some("\"original\"".into()),
                        last_modified: None,
                        bytes: old.raw.clone()
                    }
                )
                .unwrap()
                .outcome,
            "changed"
        );
        assert_eq!(
            catalog.snapshot(id).unwrap().metadata.raw_sha256,
            original_hash
        );
        assert_eq!(catalog.summary().unwrap().source_count, 19);
        assert_eq!(catalog.summary().unwrap().verified_source_count, 14);
        assert!(
            catalog
                .reference_has_downloaded_changes("labor-code-reference")
                .unwrap()
        );
        let change = catalog.downloaded_changes(id).unwrap().remove(0);
        let export = temp.path().join("changed.pdf");
        catalog
            .export_downloaded_change(change.id, &export)
            .unwrap();
        assert_eq!(fs::read(&export).unwrap(), second);
        assert!(
            catalog
                .export_downloaded_change(change.id, &export)
                .is_err()
        );
        assert!(catalog.mark_download_reviewed(change.id, "").is_err());
        catalog
            .mark_download_reviewed(
                change.id,
                "Compared the fictional fixture; no real law was evaluated.",
            )
            .unwrap();
        assert!(
            catalog
                .mark_download_reviewed(change.id, "replace audit note")
                .is_err()
        );
        assert_eq!(catalog.pending_update_count().unwrap(), 1);
        drop(catalog);
        let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        assert_eq!(catalog.downloaded_changes(id).unwrap().len(), 2);
        assert_eq!(
            catalog.snapshot(id).unwrap().metadata.raw_sha256,
            original_hash
        );
        assert_eq!(
            fs::read(private.join("workspace.sqlite3")).unwrap(),
            b"private sentinel"
        );
        catalog
            .connection
            .execute(
                "UPDATE downloaded_source_changes SET raw_bytes=X'00' WHERE id=?1",
                [change.id],
            )
            .unwrap();
        assert!(
            catalog
                .export_downloaded_change(change.id, &temp.path().join("tampered.pdf"))
                .is_err()
        );
    }
    #[test]
    fn revoked_approval_stale_lease_unexpected_304_and_bad_downloads_never_replace_a_reference() {
        let (_temp, mut catalog, id, url) = setup();
        let hash = catalog.snapshot(id).unwrap().metadata.raw_sha256;
        catalog.set_update_watch(id, &url, true).unwrap();
        let old = catalog.claim_source_check(id, false).unwrap().unwrap();
        catalog.set_update_watch(id, &url, false).unwrap();
        assert!(
            catalog
                .finish_source_check(&old, download(&url, b"%PDF-1.7 bad"))
                .is_err()
        );
        catalog.set_update_watch(id, &url, true).unwrap();
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert_eq!(
            catalog
                .finish_source_check(
                    &target,
                    UpdateReply::NotModified {
                        resolved_url: url.clone()
                    }
                )
                .unwrap()
                .outcome,
            "failed"
        );
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert_eq!(
            catalog
                .finish_source_check(
                    &target,
                    download("https://privacy.gov.ph/", b"%PDF-1.7 different origin")
                )
                .unwrap()
                .outcome,
            "failed"
        );
        let target = catalog.claim_source_check(id, false).unwrap().unwrap();
        assert_eq!(
            catalog
                .finish_source_check(&target, download(&url, b"MZ executable"))
                .unwrap()
                .outcome,
            "failed"
        );
        assert_eq!(catalog.pending_update_count().unwrap(), 0);
        assert_eq!(catalog.last_successful_source_check().unwrap(), None);
        assert_eq!(catalog.snapshot(id).unwrap().metadata.raw_sha256, hash);
        assert!(download_mime(&vec![0; MAX_DOWNLOAD_BYTES + 1], "application/pdf").is_err());
    }
    #[test]
    fn schema_three_catalogs_retain_their_snapshots_when_update_tables_are_added() {
        let (temp, catalog, id, _) = setup();
        let snapshot = catalog.snapshot(id).unwrap();
        catalog.connection.execute_batch("DROP TABLE source_update_checks;DROP TABLE source_update_watches;DROP TABLE downloaded_source_changes;PRAGMA user_version=3;").unwrap();
        drop(catalog);
        let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        assert_eq!(catalog.summary().unwrap().source_count, 19);
        assert_eq!(
            catalog.snapshot(id).unwrap().metadata.raw_sha256,
            snapshot.metadata.raw_sha256
        );
        assert!(catalog.update_watches().unwrap().is_empty());
    }
}

#[cfg(test)]
mod schedule_tests {
    use super::*;
    #[test]
    fn interrupted_checks_retry_after_the_lease_and_reject_late_results() {
        let temp = tempfile::tempdir().unwrap();
        let mut catalog = LibraryCatalog::open(temp.path()).unwrap();
        let source = catalog.sources().unwrap().remove(0);
        catalog
            .set_update_watch(source.id, &source.metadata.source_url, true)
            .unwrap();
        let old = catalog
            .claim_source_check(source.id, true)
            .unwrap()
            .unwrap();
        assert!(catalog.due_source_ids(true).unwrap().is_empty());
        catalog
            .connection
            .execute(
                "UPDATE source_update_watches SET lease_until=0 WHERE source_id=?1",
                [source.id],
            )
            .unwrap();
        assert_eq!(catalog.due_source_ids(true).unwrap(), vec![source.id]);
        let new = catalog
            .claim_source_check(source.id, true)
            .unwrap()
            .unwrap();
        assert_ne!(old.lease_token, new.lease_token);
        assert!(
            catalog
                .finish_source_check(
                    &old,
                    UpdateReply::Failed {
                        message: "late result".into()
                    }
                )
                .is_err()
        );
        catalog
            .finish_source_check(
                &new,
                UpdateReply::Failed {
                    message: "offline".into(),
                },
            )
            .unwrap();
        assert!(catalog.due_source_ids(true).unwrap().is_empty());
        assert!(
            catalog
                .claim_source_check(source.id, false)
                .unwrap()
                .is_some()
        );
    }
}
