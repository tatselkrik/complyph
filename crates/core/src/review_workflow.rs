//! Append-only, local human review records. Automated review JSON is preserved.
use super::*;

pub const MAX_NOTE_BYTES: usize = 8_000;
pub const MAX_REVIEWER_CHARS: usize = 120;

pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch(
        "CREATE TABLE review_actions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        review_id INTEGER NOT NULL REFERENCES review_runs(id) ON DELETE CASCADE,
        review_fingerprint TEXT NOT NULL,
        rule_id TEXT CHECK(rule_id IS NULL OR length(rule_id)>0),
        created_at INTEGER NOT NULL,
        reviewer TEXT NOT NULL CHECK(length(trim(reviewer))>0 AND length(reviewer)<=120),
        note TEXT NOT NULL CHECK(length(trim(note))>0 AND length(note)<=8000),
        previous_status TEXT CHECK(previous_status IN ('unresolved','acknowledged','resolved')),
        new_status TEXT CHECK(new_status IN ('unresolved','acknowledged','resolved')),
        CHECK((previous_status IS NULL AND new_status IS NULL) OR
              (rule_id IS NOT NULL AND previous_status IS NOT NULL AND new_status IS NOT NULL))
    );
    CREATE INDEX review_actions_by_target ON review_actions(review_id,rule_id,id);
    CREATE TRIGGER review_actions_immutable BEFORE UPDATE ON review_actions
        BEGIN SELECT RAISE(ABORT,'Review history is append-only'); END;
    CREATE TRIGGER tracked_reviews_immutable BEFORE UPDATE ON review_runs
        WHEN EXISTS(SELECT 1 FROM review_actions WHERE review_id=OLD.id)
        BEGIN SELECT RAISE(ABORT,'The original tracked review is immutable'); END;
    PRAGMA user_version=4;",
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    #[default]
    Unresolved,
    Acknowledged,
    Resolved,
}
impl ReviewStatus {
    pub const ALL: [Self; 3] = [Self::Unresolved, Self::Acknowledged, Self::Resolved];
    pub fn label(self) -> &'static str {
        match self {
            Self::Unresolved => "Unresolved",
            Self::Acknowledged => "Acknowledged",
            Self::Resolved => "Resolved",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Unresolved => "unresolved",
            Self::Acknowledged => "acknowledged",
            Self::Resolved => "resolved",
        }
    }
    fn from_key(key: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.key() == key)
            .ok_or_else(|| {
                WorkspaceError::InvalidReview("The saved follow-up status is unsupported.".into())
            })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewActionRequest {
    pub review_fingerprint: String,
    /// None records a document-review note for this exact saved snapshot.
    pub rule_id: Option<String>,
    pub expected_revision: i64,
    pub reviewer: String,
    pub note: String,
    pub status: Option<ReviewStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewEvent {
    pub id: i64,
    pub review_id: i64,
    pub rule_id: Option<String>,
    pub created_at: i64,
    /// Self-reported name; this is not authenticated identity or a signature.
    pub reviewer: String,
    pub note: String,
    pub previous_status: Option<ReviewStatus>,
    pub new_status: Option<ReviewStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewReadingContext {
    pub engine: String,
    pub summary: String,
    pub warnings: Vec<String>,
    pub reading: Option<complyph_extract::ReadingInfo>,
}
/// Only small immutable metadata is reused; findings/actions are captured fresh.
#[derive(Default)]
pub(super) struct ReadingContextCache {
    data_version: std::cell::Cell<i64>,
    entries: std::cell::RefCell<Vec<(i64, String, ReviewReadingContext)>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewWorkflow {
    pub saved: SavedReview,
    pub document_name: String,
    pub review_fingerprint: String,
    pub captured_at: i64,
    pub events: Vec<ReviewEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_version: Option<crate::document_history::VersionReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading_context: Option<ReviewReadingContext>,
}
impl ReviewWorkflow {
    pub fn revision(&self, rule_id: Option<&str>) -> i64 {
        self.events
            .iter()
            .rev()
            .find(|e| e.rule_id.as_deref() == rule_id)
            .map_or(0, |e| e.id)
    }
    pub fn status(&self, rule_id: &str) -> ReviewStatus {
        self.events
            .iter()
            .rev()
            .filter(|e| e.rule_id.as_deref() == Some(rule_id))
            .find_map(|e| e.new_status)
            .unwrap_or_default()
    }
    pub fn last_event_id(&self) -> i64 {
        self.events.last().map_or(0, |e| e.id)
    }
}

fn original(connection: &Connection, id: i64) -> Result<(SavedReview, String, String)> {
    let row:Option<(i64,i64,String,String)>=connection.query_row(
        "SELECT r.document_id,r.created_at,r.value,d.name FROM review_runs r JOIN documents d ON d.id=r.document_id WHERE r.id=?1",
        [id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    let (document_id, created_at, value, name) = row.ok_or_else(|| {
        WorkspaceError::InvalidReview("This saved review is no longer available.".into())
    })?;
    let mut fingerprint = Sha256::new();
    for number in [id, document_id, created_at] {
        fingerprint.update(number.to_be_bytes());
    }
    fingerprint.update(value.as_bytes());
    let saved = SavedReview {
        id,
        document_id,
        created_at,
        review: serde_json::from_str(&value)?,
    };
    Ok((saved, format!("{:x}", fingerprint.finalize()), name))
}

fn events(connection: &Connection, id: i64, fingerprint: &str) -> Result<Vec<ReviewEvent>> {
    let mut statement=connection.prepare("SELECT id,review_fingerprint,rule_id,created_at,reviewer,note,previous_status,new_status FROM review_actions WHERE review_id=?1 ORDER BY id")?;
    let rows = statement.query_map([id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;
    rows.map(|row| {
        let (event_id,original_hash,rule_id,created_at,reviewer,note,previous,new)=row?;
        if original_hash!=fingerprint {return Err(WorkspaceError::InvalidReview("The original review no longer matches its recorded tracking history. Restore the preserved snapshot before continuing.".into()));}
        Ok(ReviewEvent{id:event_id,review_id:id,rule_id,created_at,reviewer,note,
            previous_status:previous.as_deref().map(ReviewStatus::from_key).transpose()?,
            new_status:new.as_deref().map(ReviewStatus::from_key).transpose()?})
    }).collect()
}

pub(crate) fn capture(connection: &Connection, id: i64) -> Result<ReviewWorkflow> {
    capture_with_context_cache(connection, id, None)
}
fn capture_with_context_cache(
    connection: &Connection,
    id: i64,
    cache: Option<&ReadingContextCache>,
) -> Result<ReviewWorkflow> {
    if let Some(cache) = cache {
        // An external writer must not hide a changed/corrupt retained reading behind
        // metadata validated earlier in this session. Our own reading rows are immutable.
        let version = connection.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        if version != cache.data_version.get() {
            cache.entries.borrow_mut().clear();
            cache.data_version.set(version);
        }
    }
    let (saved, review_fingerprint, document_name) = original(connection, id)?;
    let events = events(connection, id, &review_fingerprint)?;
    let document_version = crate::document_history::review_reference(connection, id)?;
    let reading_context = if let Some((reading_id, hash)) = document_version
        .as_ref()
        .and_then(|v| v.reading_id.zip(v.reading_sha256.as_ref()))
    {
        if let Some(context) = cache.and_then(|c| {
            c.entries
                .borrow()
                .iter()
                .find(|(id, h, _)| *id == reading_id && h == hash)
                .map(|(_, _, c)| c.clone())
        }) {
            Some(context)
        } else {
            let r = crate::document_history::retained_reading(connection, reading_id)?;
            let context = ReviewReadingContext {
                engine: r.saved.report.engine,
                summary: r.saved.report.summary,
                warnings: r.saved.report.warnings,
                reading: r.saved.report.reading,
            };
            if let Some(cache) = cache {
                let mut cache = cache.entries.borrow_mut();
                if cache.len() >= 8 {
                    cache.clear();
                }
                cache.push((reading_id, hash.clone(), context.clone()));
            }
            Some(context)
        }
    } else {
        None
    };
    Ok(ReviewWorkflow {
        saved,
        document_name,
        review_fingerprint,
        captured_at: now(),
        events,
        document_version,
        reading_context,
    })
}

impl Workspace {
    /// One SQLite read transaction captures original findings and current history together.
    pub fn review_workflow(&self, id: i64) -> Result<ReviewWorkflow> {
        let tx = self.connection.unchecked_transaction()?;
        let workflow = capture_with_context_cache(&tx, id, Some(&self.reading_context_cache))?;
        tx.commit()?;
        Ok(workflow)
    }

    /// An explicit reviewer action. A stale window cannot overwrite another reviewer.
    pub fn append_review_action(
        &self,
        id: i64,
        request: &ReviewActionRequest,
    ) -> Result<ReviewEvent> {
        let reviewer = request.reviewer.trim();
        let normalized = request.note.replace("\r\n", "\n").replace('\r', "\n");
        let note = normalized.trim();
        if reviewer.is_empty()
            || reviewer.chars().count() > MAX_REVIEWER_CHARS
            || reviewer.chars().any(char::is_control)
        {
            return Err(WorkspaceError::InvalidReview(
                "Enter a reviewer name of up to 120 characters, without line breaks.".into(),
            ));
        }
        if note.is_empty()
            || note.len() > MAX_NOTE_BYTES
            || note
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(WorkspaceError::InvalidReview("Enter a review note of up to 8,000 UTF-8 bytes, without unsupported control characters.".into()));
        }
        if request.expected_revision < 0 || (request.rule_id.is_none() && request.status.is_some())
        {
            return Err(WorkspaceError::InvalidReview(
                "Status changes must identify a finding in this saved review.".into(),
            ));
        }
        let tx = rusqlite::Transaction::new_unchecked(
            &self.connection,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let (saved, fingerprint, _) = original(&tx, id)?;
        if fingerprint != request.review_fingerprint {
            return Err(WorkspaceError::ReviewConflict);
        }
        if let Some(rule) = &request.rule_id
            && saved
                .review
                .findings
                .iter()
                .filter(|f| f.rule_id == *rule)
                .count()
                != 1
        {
            return Err(WorkspaceError::InvalidReview(
                "Choose an unambiguous finding from this saved review.".into(),
            ));
        }
        let current_events = events(&tx, id, &fingerprint)?;
        let target: Vec<_> = current_events
            .iter()
            .filter(|e| e.rule_id == request.rule_id)
            .collect();
        if target.last().map_or(0, |e| e.id) != request.expected_revision {
            return Err(WorkspaceError::ReviewConflict);
        }
        let current = target
            .iter()
            .rev()
            .find_map(|e| e.new_status)
            .unwrap_or_default();
        let new_status = request.status.filter(|s| *s != current);
        let previous_status = new_status.map(|_| current);
        let created_at = now();
        tx.execute("INSERT INTO review_actions(review_id,review_fingerprint,rule_id,created_at,reviewer,note,previous_status,new_status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![id,fingerprint,request.rule_id,created_at,reviewer,note,previous_status.map(ReviewStatus::key),new_status.map(ReviewStatus::key)])?;
        let event = ReviewEvent {
            id: tx.last_insert_rowid(),
            review_id: id,
            rule_id: request.rule_id.clone(),
            created_at,
            reviewer: reviewer.into(),
            note: note.into(),
            previous_status,
            new_status,
        };
        tx.commit()?;
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, WorkspacePaths, Workspace, SavedReview) {
        let temp = tempfile::tempdir().unwrap();
        let paths = WorkspacePaths::under(temp.path().join("workspace"));
        let workspace = Workspace::open(&paths).unwrap();
        let original = temp.path().join("Fictional document.pdf");
        fs::write(&original, b"synthetic unchanged original").unwrap();
        let document = workspace.register_document(&original).unwrap();
        let report = complyph_extract::Report::failure(
            complyph_extract::Format::Pdf,
            complyph_extract::Status::Unreadable,
            "synthetic",
        );
        let saved = workspace
            .save_review(
                document.id,
                complyph_checks::review(&report, "synthetic-document-hash", Default::default()),
            )
            .unwrap();
        (temp, paths, workspace, saved)
    }
    fn action(
        state: &ReviewWorkflow,
        rule: Option<&str>,
        status: Option<ReviewStatus>,
        note: &str,
    ) -> ReviewActionRequest {
        ReviewActionRequest {
            review_fingerprint: state.review_fingerprint.clone(),
            rule_id: rule.map(str::to_owned),
            expected_revision: state.revision(rule),
            reviewer: "José Example".into(),
            note: note.into(),
            status,
        }
    }
    #[test]
    fn notes_status_transitions_and_corrections_preserve_original_and_survive_reopening() {
        let (temp, paths, workspace, saved) = setup();
        let original: String = workspace
            .connection
            .query_row(
                "SELECT value FROM review_runs WHERE id=?1",
                [saved.id],
                |r| r.get(0),
            )
            .unwrap();
        let first = workspace.review_workflow(saved.id).unwrap();
        let rule = first.saved.review.findings[0].rule_id.clone();
        assert_eq!(first.status(&rule), ReviewStatus::Unresolved);
        workspace
            .append_review_action(
                saved.id,
                &action(
                    &first,
                    None,
                    None,
                    "Document review note: fictional ₱ amount.",
                ),
            )
            .unwrap();
        let request = action(
            &first,
            Some(&rule),
            Some(ReviewStatus::Acknowledged),
            "Acknowledged for manual follow-up.",
        );
        workspace.append_review_action(saved.id, &request).unwrap();
        let next = workspace.review_workflow(saved.id).unwrap();
        workspace
            .append_review_action(
                saved.id,
                &action(
                    &next,
                    Some(&rule),
                    Some(ReviewStatus::Resolved),
                    "Fictional follow-up complete; original finding retained.",
                ),
            )
            .unwrap();
        let next = workspace.review_workflow(saved.id).unwrap();
        workspace
            .append_review_action(
                saved.id,
                &action(
                    &next,
                    Some(&rule),
                    None,
                    "Correction added without changing the prior note.",
                ),
            )
            .unwrap();
        assert!(
            workspace
                .connection
                .execute("UPDATE review_actions SET note='changed'", [])
                .is_err()
        );
        assert!(
            workspace
                .connection
                .execute("UPDATE review_runs SET value='{}' WHERE id=?1", [saved.id])
                .is_err()
        );
        drop(workspace);
        let workspace = Workspace::open(&paths).unwrap();
        let state = workspace.review_workflow(saved.id).unwrap();
        assert_eq!(state.events.len(), 4);
        assert_eq!(state.status(&rule), ReviewStatus::Resolved);
        assert_eq!(
            state.events[1].previous_status,
            Some(ReviewStatus::Unresolved)
        );
        assert_eq!(
            state.events[2].previous_status,
            Some(ReviewStatus::Acknowledged)
        );
        assert_eq!(state.events[3].new_status, None);
        assert!(state.events.iter().all(|e| e.reviewer == "José Example"));
        let after: String = workspace
            .connection
            .query_row(
                "SELECT value FROM review_runs WHERE id=?1",
                [saved.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(original, after);
        let fresh = workspace
            .save_review(saved.document_id, saved.review.clone())
            .unwrap();
        assert_eq!(
            workspace.review_workflow(fresh.id).unwrap().status(&rule),
            ReviewStatus::Unresolved
        );
        assert_eq!(
            fs::read(temp.path().join("Fictional document.pdf")).unwrap(),
            b"synthetic unchanged original"
        );
        workspace.remove_document(saved.document_id).unwrap();
        assert_eq!(
            workspace
                .connection
                .query_row("SELECT COUNT(*) FROM review_actions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn stale_concurrent_actions_and_reused_review_ids_require_the_original_fingerprint() {
        let (_temp, paths, workspace, saved) = setup();
        let state = workspace.review_workflow(saved.id).unwrap();
        let other = Workspace::open(&paths).unwrap();
        let request = action(&state, None, None, "First document note.");
        workspace.append_review_action(saved.id, &request).unwrap();
        assert!(matches!(
            other.append_review_action(saved.id, &request),
            Err(WorkspaceError::ReviewConflict)
        ));
        assert_eq!(workspace.review_workflow(saved.id).unwrap().events.len(), 1);
        let mut latest = workspace.review_workflow(saved.id).unwrap();
        let rule = latest.saved.review.findings[0].rule_id.clone();
        workspace
            .append_review_action(
                saved.id,
                &action(
                    &latest,
                    Some(&rule),
                    Some(ReviewStatus::Resolved),
                    "Resolve with a recorded reason.",
                ),
            )
            .unwrap();
        latest = workspace.review_workflow(saved.id).unwrap();
        workspace
            .append_review_action(
                saved.id,
                &action(
                    &latest,
                    Some(&rule),
                    Some(ReviewStatus::Unresolved),
                    "Reopened after additional review.",
                ),
            )
            .unwrap();
        assert_eq!(
            workspace.review_workflow(saved.id).unwrap().status(&rule),
            ReviewStatus::Unresolved
        );
        workspace.remove_document(saved.document_id).unwrap();
        let path = paths.root.join("different.pdf");
        fs::write(&path, b"different fictional document").unwrap();
        let doc = workspace.register_document(&path).unwrap();
        let mut review = saved.review;
        review.document_sha256 = "different-document-hash".into();
        let fresh = workspace.save_review(doc.id, review).unwrap();
        assert_eq!(fresh.id, saved.id);
        assert!(matches!(
            workspace.append_review_action(fresh.id, &request),
            Err(WorkspaceError::ReviewConflict)
        ));
    }
    #[test]
    fn invalid_targets_empty_notes_identities_and_unknown_fields_never_write_history() {
        let (_temp, _paths, workspace, saved) = setup();
        let state = workspace.review_workflow(saved.id).unwrap();
        for field in 0..7 {
            let mut request = action(&state, None, None, "Valid fictional note.");
            match field {
                0 => request.reviewer = "  ".into(),
                1 => request.reviewer = "Two\nLines".into(),
                2 => request.note = " ".into(),
                3 => request.note = "x".repeat(MAX_NOTE_BYTES + 1),
                4 => request.rule_id = Some("not-a-saved-finding".into()),
                5 => request.status = Some(ReviewStatus::Resolved),
                _ => request.note = "hidden\0control".into(),
            }
            assert!(workspace.append_review_action(saved.id, &request).is_err());
        }
        assert!(serde_json::from_value::<ReviewActionRequest>(serde_json::json!({"review_fingerprint":"x","rule_id":null,"expected_revision":0,"reviewer":"A","note":"N","status":null,"upload":true})).is_err());
        assert!(
            workspace
                .review_workflow(saved.id)
                .unwrap()
                .events
                .is_empty()
        );
    }
    #[test]
    fn schema_three_upgrade_keeps_original_review_bytes_and_starts_with_empty_tracking() {
        let (_temp, paths, workspace, saved) = setup();
        let before: String = workspace
            .connection
            .query_row(
                "SELECT value FROM review_runs WHERE id=?1",
                [saved.id],
                |r| r.get(0),
            )
            .unwrap();
        crate::document_history::remove_schema_for_legacy_test(&workspace.connection);
        workspace.connection.execute_batch("DROP TABLE folder_intake; DROP TABLE folder_watch; DROP TRIGGER tracked_reviews_immutable; DROP TABLE review_actions; PRAGMA user_version=3;").unwrap();
        drop(workspace);
        let upgraded = Workspace::open(&paths).unwrap();
        let after: String = upgraded
            .connection
            .query_row(
                "SELECT value FROM review_runs WHERE id=?1",
                [saved.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            upgraded
                .connection
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            6
        );
        assert!(
            upgraded
                .review_workflow(saved.id)
                .unwrap()
                .events
                .is_empty()
        );
    }
}
