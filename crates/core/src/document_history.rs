//! Retained private readings and explicitly linked document revisions. No original copies or I/O.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentLink {
    pub document_id: i64,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadingSummary {
    pub id: i64,
    pub reading_sha256: String,
    pub read_at: i64,
    pub engine: String,
    pub status: complyph_extract::Status,
    pub text_bytes: usize,
    pub units: usize,
    pub ocr: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionReview {
    pub id: i64,
    pub created_at: i64,
    /// None means a historical review whose exact reading was not recorded.
    pub reading_id: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub id: i64,
    pub document_id: i64,
    pub name: String,
    pub document_sha256: String,
    pub captured_at: i64,
    pub readings: Vec<ReadingSummary>,
    pub reviews: Vec<VersionReview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentHistory {
    pub group_id: i64,
    pub title: String,
    pub documents: Vec<DocumentLink>,
    pub versions: Vec<DocumentVersion>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetainedReading {
    pub id: i64,
    pub version_id: i64,
    pub document_id: i64,
    pub name: String,
    pub reading_sha256: String,
    pub saved: SavedExtraction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionReference {
    pub version_id: i64,
    pub reading_id: Option<i64>,
    pub reading_sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionLinkRequest {
    pub document_id: i64,
    pub revised_document_id: i64,
    pub expected_group_id: i64,
    pub expected_revision_group_id: i64,
}

pub(super) fn migrate(tx: &Connection) -> Result<()> {
    tx.execute_batch("
      CREATE TABLE document_groups(id INTEGER PRIMARY KEY AUTOINCREMENT,title TEXT NOT NULL,created_at INTEGER NOT NULL);
      CREATE TABLE document_group_members(document_id INTEGER PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,group_id INTEGER NOT NULL REFERENCES document_groups(id));
      CREATE INDEX group_documents ON document_group_members(group_id);
      CREATE TABLE document_versions(id INTEGER PRIMARY KEY AUTOINCREMENT,document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,name TEXT NOT NULL,sha256 TEXT NOT NULL,captured_at INTEGER NOT NULL,UNIQUE(document_id,sha256));
      CREATE TABLE document_readings(id INTEGER PRIMARY KEY AUTOINCREMENT,version_id INTEGER NOT NULL REFERENCES document_versions(id) ON DELETE CASCADE,sha256 TEXT NOT NULL,read_at INTEGER NOT NULL,engine TEXT NOT NULL,status TEXT NOT NULL,text_bytes INTEGER NOT NULL,units INTEGER NOT NULL,ocr INTEGER NOT NULL,value TEXT NOT NULL,UNIQUE(version_id,sha256));
      CREATE TABLE review_version_links(review_id INTEGER PRIMARY KEY REFERENCES review_runs(id) ON DELETE CASCADE,version_id INTEGER NOT NULL REFERENCES document_versions(id) ON DELETE CASCADE,reading_id INTEGER REFERENCES document_readings(id) ON DELETE CASCADE);
      CREATE INDEX version_reviews ON review_version_links(version_id,review_id);
      CREATE INDEX IF NOT EXISTS review_runs_by_document ON review_runs(document_id,id);
      CREATE TABLE document_link_events(id INTEGER PRIMARY KEY AUTOINCREMENT,document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,previous_group INTEGER NOT NULL,new_group INTEGER NOT NULL,created_at INTEGER NOT NULL);
      CREATE TRIGGER version_immutable BEFORE UPDATE ON document_versions BEGIN SELECT RAISE(ABORT,'Retained document versions are immutable'); END;
      CREATE TRIGGER reading_immutable BEFORE UPDATE ON document_readings BEGIN SELECT RAISE(ABORT,'Retained document readings are immutable'); END;
      CREATE TRIGGER review_version_immutable BEFORE UPDATE ON review_version_links BEGIN SELECT RAISE(ABORT,'A review stays bound to its original reading'); END;
      CREATE TRIGGER link_history_immutable BEFORE UPDATE ON document_link_events BEGIN SELECT RAISE(ABORT,'Revision link history is append-only'); END;
      CREATE TRIGGER check_review_version BEFORE INSERT ON review_version_links
        WHEN (SELECT document_id FROM review_runs WHERE id=NEW.review_id)!=(SELECT document_id FROM document_versions WHERE id=NEW.version_id)
          OR (NEW.reading_id IS NOT NULL AND (SELECT version_id FROM document_readings WHERE id=NEW.reading_id)!=NEW.version_id)
        BEGIN SELECT RAISE(ABORT,'Review, document version and reading do not match'); END;
    ")?;
    let documents = tx
        .prepare("SELECT id,name,path,kind,bytes,added_at,modified_at FROM documents ORDER BY id")?
        .query_map([], document_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for document in &documents {
        ensure_group(tx, document)?;
        let cached: Option<String> = tx
            .query_row(
                "SELECT value FROM document_extractions WHERE document_id=?1",
                [document.id],
                |r| r.get(0),
            )
            .optional()?;
        // Preserve malformed/unsupported legacy cache bytes as-is; never invent a reading.
        if let Some(value) = cached
            && value.len() <= complyph_extract::MAX_REPORT_BYTES as usize + 1024
            && let Ok(saved) = serde_json::from_str::<SavedExtraction>(&value)
            && valid_reading(document, &saved).is_ok()
        {
            retain_reading(tx, document, &saved)?;
        }
    }
    let mut statement =
        tx.prepare("SELECT id,document_id,created_at,value FROM review_runs ORDER BY id")?;
    let rows = statement.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (id, document_id, created_at, value) = row?;
        if let Ok(review) = serde_json::from_str::<complyph_checks::Review>(&value) {
            let saved = SavedReview {
                id,
                document_id,
                created_at,
                review,
            };
            bind_review(tx, &saved, None)?;
        }
    }
    tx.pragma_update(None, "user_version", 6)?;
    Ok(())
}

pub(super) fn ensure_group(conn: &Connection, document: &Document) -> Result<i64> {
    if let Some(id) = conn
        .query_row(
            "SELECT group_id FROM document_group_members WHERE document_id=?1",
            [document.id],
            |r| r.get(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO document_groups(title,created_at) VALUES(?1,?2)",
        params![document.name, now()],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO document_group_members(document_id,group_id) VALUES(?1,?2)",
        params![document.id, id],
    )?;
    Ok(id)
}
pub(crate) fn group_id(conn: &Connection, document_id: i64) -> Result<i64> {
    conn.query_row(
        "SELECT group_id FROM document_group_members WHERE document_id=?1",
        [document_id],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| {
        WorkspaceError::InvalidDocument("This document entry is no longer available.".into())
    })
}
fn version(
    conn: &Connection,
    document_id: i64,
    hash: &str,
    name: &str,
    captured_at: i64,
) -> Result<i64> {
    conn.execute("INSERT OR IGNORE INTO document_versions(document_id,name,sha256,captured_at) VALUES(?1,?2,?3,?4)",params![document_id,name,hash,captured_at])?;
    Ok(conn.query_row(
        "SELECT id FROM document_versions WHERE document_id=?1 AND sha256=?2",
        params![document_id, hash],
        |r| r.get(0),
    )?)
}
fn valid_reading(document: &Document, saved: &SavedExtraction) -> Result<()> {
    if saved.sha256.len() != 64
        || !saved.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || saved.report.format != document.kind.format()
        || saved.report.text_bytes() > complyph_extract::MAX_TEXT_BYTES
        || saved.report.units.len() > complyph_extract::MAX_UNITS
        || (saved.report.status == complyph_extract::Status::Ready
            && saved.report.units.iter().all(|u| u.text.trim().is_empty()))
        || serde_json::to_vec(&saved.report)?.len() > complyph_extract::MAX_REPORT_BYTES as usize
    {
        return Err(WorkspaceError::InvalidDocument(
            "The reading cannot be retained: its file hash, format or size is invalid.".into(),
        ));
    }
    Ok(())
}
pub fn reading_sha256(report: &complyph_extract::Report) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(report)?)))
}
pub(super) fn retain_reading(
    conn: &Connection,
    document: &Document,
    saved: &SavedExtraction,
) -> Result<(i64, i64)> {
    valid_reading(document, saved)?;
    let version_id = version(
        conn,
        document.id,
        &saved.sha256,
        &document.name,
        saved.read_at,
    )?;
    let hash = reading_sha256(&saved.report)?;
    let status = serde_json::to_string(&saved.report.status)?;
    conn.execute("INSERT OR IGNORE INTO document_readings(version_id,sha256,read_at,engine,status,text_bytes,units,ocr,value) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![version_id,hash,saved.read_at,saved.report.engine,status,saved.report.text_bytes() as i64,saved.report.units.len() as i64,saved.report.ocr.is_some(),serde_json::to_string(saved)?])?;
    let reading_id = conn.query_row(
        "SELECT id FROM document_readings WHERE version_id=?1 AND sha256=?2",
        params![version_id, hash],
        |r| r.get(0),
    )?;
    Ok((version_id, reading_id))
}
pub(super) fn bind_review(
    conn: &Connection,
    review: &SavedReview,
    reading: Option<&SavedExtraction>,
) -> Result<()> {
    let document = conn.query_row(
        "SELECT id,name,path,kind,bytes,added_at,modified_at FROM documents WHERE id=?1",
        [review.document_id],
        document_from_row,
    )?;
    let (version_id, reading_id) = if let Some(reading) = reading {
        if reading.sha256 != review.review.document_sha256
            || reading.report.ocr != review.review.ocr
        {
            return Err(WorkspaceError::InvalidReview(
                "The review does not match the selected document reading.".into(),
            ));
        }
        let (version_id, reading_id) = retain_reading(conn, &document, reading)?;
        (version_id, Some(reading_id))
    } else {
        // Historical reviews carry a file hash, but do not prove which OCR/reader run was used.
        (
            version(
                conn,
                document.id,
                &review.review.document_sha256,
                &document.name,
                review.created_at,
            )?,
            None,
        )
    };
    conn.execute(
        "INSERT INTO review_version_links(review_id,version_id,reading_id) VALUES(?1,?2,?3)",
        params![review.id, version_id, reading_id],
    )?;
    Ok(())
}
pub(super) fn remove_empty_groups(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM document_groups WHERE NOT EXISTS(SELECT 1 FROM document_group_members m WHERE m.group_id=document_groups.id)",[])?;
    Ok(())
}

pub(crate) fn retained_reading(conn: &Connection, id: i64) -> Result<RetainedReading> {
    let (version_id,document_id,name,file_hash,hash,value):(i64,i64,String,String,String,String)=conn.query_row(
        "SELECT v.id,v.document_id,v.name,v.sha256,r.sha256,r.value FROM document_readings r JOIN document_versions v ON v.id=r.version_id WHERE r.id=?1",
        [id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
    if value.len() > complyph_extract::MAX_REPORT_BYTES as usize + 1024 {
        return Err(WorkspaceError::InvalidDocument(
            "The retained reading exceeds its size limit.".into(),
        ));
    }
    let saved: SavedExtraction = serde_json::from_str(&value)?;
    if saved.sha256 != file_hash || reading_sha256(&saved.report)? != hash {
        return Err(WorkspaceError::InvalidDocument("This retained reading failed its integrity check. It cannot be compared or used for passage navigation.".into()));
    }
    Ok(RetainedReading {
        id,
        version_id,
        document_id,
        name,
        reading_sha256: hash,
        saved,
    })
}
pub(crate) fn review_reading_id(conn: &Connection, review_id: i64) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT reading_id FROM review_version_links WHERE review_id=?1",
            [review_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}
pub(crate) fn review_reference(
    conn: &Connection,
    review_id: i64,
) -> Result<Option<VersionReference>> {
    Ok(conn.query_row("SELECT l.version_id,l.reading_id,r.sha256 FROM review_version_links l LEFT JOIN document_readings r ON r.id=l.reading_id WHERE l.review_id=?1",[review_id],|r|Ok(VersionReference{version_id:r.get(0)?,reading_id:r.get(1)?,reading_sha256:r.get(2)?})).optional()?)
}

impl Workspace {
    pub fn review_counts(&self) -> Result<std::collections::BTreeMap<i64, u64>> {
        self.connection
            .prepare("SELECT document_id,COUNT(*) FROM review_runs GROUP BY document_id")?
            .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))?
            .collect::<std::result::Result<_, _>>()
            .map_err(Into::into)
    }
    pub fn document_history(&self, document_id: i64) -> Result<DocumentHistory> {
        let tx = self.connection.unchecked_transaction()?;
        let group_id = group_id(&tx, document_id)?;
        let title = tx.query_row(
            "SELECT title FROM document_groups WHERE id=?1",
            [group_id],
            |r| r.get(0),
        )?;
        let documents=tx.prepare("SELECT d.id,d.name FROM document_group_members m JOIN documents d ON d.id=m.document_id WHERE m.group_id=?1 ORDER BY d.added_at,d.id")?
            .query_map([group_id],|r|Ok(DocumentLink{document_id:r.get(0)?,name:r.get(1)?}))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        let mut versions=tx.prepare("SELECT v.id,v.document_id,v.name,v.sha256,v.captured_at FROM document_versions v JOIN document_group_members m ON m.document_id=v.document_id WHERE m.group_id=?1 ORDER BY v.id")?
            .query_map([group_id],|r|Ok(DocumentVersion{id:r.get(0)?,document_id:r.get(1)?,name:r.get(2)?,document_sha256:r.get(3)?,captured_at:r.get(4)?,readings:vec![],reviews:vec![]}))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        for version in &mut versions {
            let rows=tx.prepare("SELECT id,sha256,read_at,engine,status,text_bytes,units,ocr FROM document_readings WHERE version_id=?1 ORDER BY id")?
                .query_map([version.id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?,r.get::<_,bool>(7)?)))?
                .collect::<std::result::Result<Vec<_>,_>>()?;
            for (id, hash, read_at, engine, status, text_bytes, units, ocr) in rows {
                version.readings.push(ReadingSummary {
                    id,
                    reading_sha256: hash,
                    read_at,
                    engine,
                    status: serde_json::from_str(&status)?,
                    text_bytes: text_bytes as usize,
                    units: units as usize,
                    ocr,
                });
            }
            version.reviews=tx.prepare("SELECT r.id,r.created_at,l.reading_id FROM review_version_links l JOIN review_runs r ON r.id=l.review_id WHERE l.version_id=?1 ORDER BY r.id")?
                .query_map([version.id],|r|Ok(VersionReview{id:r.get(0)?,created_at:r.get(1)?,reading_id:r.get(2)?}))?
                .collect::<std::result::Result<Vec<_>,_>>()?;
        }
        tx.commit()?;
        Ok(DocumentHistory {
            group_id,
            title,
            documents,
            versions,
        })
    }
    /// Explicitly link one standalone file's complete history. Never merge two multi-file histories.
    pub fn link_document_revision(&self, request: &RevisionLinkRequest) -> Result<()> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.connection,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let base = group_id(&tx, request.document_id)?;
        let revised = group_id(&tx, request.revised_document_id)?;
        if base != request.expected_group_id || revised != request.expected_revision_group_id {
            return Err(WorkspaceError::RevisionConflict);
        }
        if base != revised {
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM document_group_members WHERE group_id=?1",
                [revised],
                |r| r.get(0),
            )?;
            if count != 1 {
                return Err(WorkspaceError::InvalidDocument("This file already belongs to another multi-file history. Unlink that file first; histories are never merged automatically.".into()));
            }
            tx.execute(
                "UPDATE document_group_members SET group_id=?1 WHERE document_id=?2",
                params![base, request.revised_document_id],
            )?;
            tx.execute("INSERT INTO document_link_events(document_id,previous_group,new_group,created_at) VALUES(?1,?2,?3,?4)",params![request.revised_document_id,revised,base,now()])?;
            remove_empty_groups(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Correct a mistaken link without changing readings, findings, notes or originals.
    pub fn unlink_document_revision(&self, document_id: i64, expected_group_id: i64) -> Result<()> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.connection,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let old = group_id(&tx, document_id)?;
        if old != expected_group_id {
            return Err(WorkspaceError::RevisionConflict);
        }
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM document_group_members WHERE group_id=?1",
            [old],
            |r| r.get(0),
        )?;
        if count > 1 {
            let name: String = tx.query_row(
                "SELECT name FROM documents WHERE id=?1",
                [document_id],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO document_groups(title,created_at) VALUES(?1,?2)",
                params![name, now()],
            )?;
            let new = tx.last_insert_rowid();
            tx.execute(
                "UPDATE document_group_members SET group_id=?1 WHERE document_id=?2",
                params![new, document_id],
            )?;
            tx.execute("INSERT INTO document_link_events(document_id,previous_group,new_group,created_at) VALUES(?1,?2,?3,?4)",params![document_id,old,new,now()])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn retained_reading(&self, id: i64) -> Result<RetainedReading> {
        retained_reading(&self.connection, id)
    }
    pub fn reading_for_review(&self, review_id: i64) -> Result<Option<RetainedReading>> {
        let tx = self.connection.unchecked_transaction()?;
        let reading = review_reading_id(&tx, review_id)?
            .map(|id| retained_reading(&tx, id))
            .transpose()?;
        if let Some(reading) = &reading {
            let review = review_workflow::capture(&tx, review_id)?;
            if review.saved.document_id != reading.document_id
                || review.saved.review.document_sha256 != reading.saved.sha256
            {
                return Err(WorkspaceError::InvalidReview(
                    "The saved review no longer matches its retained document version.".into(),
                ));
            }
        }
        tx.commit()?;
        Ok(reading)
    }
}

#[cfg(test)]
pub(crate) fn remove_schema_for_legacy_test(conn: &Connection) {
    conn.execute_batch("DROP TABLE review_version_links; DROP TABLE document_readings; DROP TABLE document_versions; DROP TABLE document_link_events; DROP TABLE document_group_members; DROP TABLE document_groups;").unwrap();
}
