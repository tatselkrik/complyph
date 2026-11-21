//! Append-only public-library review history and explicit version/rule activation.
use super::*;
use chrono::{NaiveDate, Utc};

pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch("
      CREATE TABLE active_source_versions(source_id INTEGER PRIMARY KEY REFERENCES sources(id),version_id INTEGER NOT NULL REFERENCES source_versions(id));
      INSERT INTO active_source_versions SELECT s.id,(SELECT v.id FROM source_versions v JOIN source_snapshots p ON p.version_id=v.id WHERE v.source_id=s.id ORDER BY v.retrieved_at DESC,v.id DESC LIMIT 1) FROM sources s WHERE EXISTS(SELECT 1 FROM source_versions v JOIN source_snapshots p ON p.version_id=v.id WHERE v.source_id=s.id);
      CREATE TABLE source_events(id INTEGER PRIMARY KEY,source_id INTEGER NOT NULL REFERENCES sources(id),content_sha256 TEXT NOT NULL,kind TEXT NOT NULL,data_json TEXT NOT NULL,actor TEXT NOT NULL,basis TEXT NOT NULL,review_on TEXT NOT NULL,recorded_at INTEGER NOT NULL);
      CREATE INDEX source_events_latest ON source_events(source_id,content_sha256,kind,id);
      CREATE TABLE candidate_readings(change_id INTEGER PRIMARY KEY REFERENCES downloaded_source_changes(id),reading_text TEXT NOT NULL,text_sha256 TEXT NOT NULL,extraction TEXT NOT NULL,pages INTEGER,prepared_at INTEGER NOT NULL);
      CREATE TABLE candidate_decisions(id INTEGER PRIMARY KEY,change_id INTEGER NOT NULL REFERENCES downloaded_source_changes(id),decision TEXT NOT NULL CHECK(decision IN('dismissed','adopted')),base_sha256 TEXT NOT NULL,actor TEXT NOT NULL,basis TEXT NOT NULL,review_on TEXT NOT NULL,recorded_at INTEGER NOT NULL);
      CREATE TABLE rule_profiles(rule_id TEXT PRIMARY KEY,profile_json TEXT NOT NULL,fingerprint TEXT NOT NULL);
      CREATE TABLE rule_revalidations(id INTEGER PRIMARY KEY,rule_id TEXT NOT NULL REFERENCES rule_profiles(rule_id),token TEXT NOT NULL,decision TEXT NOT NULL CHECK(decision IN('unchanged_profile','update_required')),actor TEXT NOT NULL,basis TEXT NOT NULL,review_on TEXT NOT NULL,test_reference TEXT NOT NULL,recorded_at INTEGER NOT NULL);
      CREATE TABLE rule_hold_resolutions(update_assessment_id INTEGER PRIMARY KEY REFERENCES rule_revalidations(id),resolution_assessment_id INTEGER NOT NULL REFERENCES rule_revalidations(id));
    ")?;
    Ok(())
}
#[cfg(test)]
pub(super) fn remove_new_tables_for_legacy_fixture(connection: &Connection) {
    connection.execute_batch("DROP TABLE rule_hold_resolutions;DROP TABLE rule_revalidations;DROP TABLE rule_profiles;DROP TABLE candidate_readings;DROP TABLE candidate_decisions;DROP TABLE source_events;DROP TABLE active_source_versions;DROP TABLE publication_link_reviews;DROP TABLE publication_links;DROP TABLE publication_versions;DROP TABLE publication_pages;").unwrap();
}

fn invalid(message: impl Into<String>) -> LibraryError {
    LibraryError::InvalidImport(message.into())
}
fn text_field(value: &str, name: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > max
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(invalid(format!(
            "Record {name} (1–{max} bytes, without control characters)."
        )));
    }
    Ok(value.into())
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryReviewer {
    pub actor: String,
    pub review_on: String,
    pub basis: String,
}
impl LibraryReviewer {
    pub fn validated(&self) -> Result<Self> {
        let date = NaiveDate::parse_from_str(&self.review_on, "%Y-%m-%d")
            .map_err(|_| invalid("Use a real review date in YYYY-MM-DD format."))?;
        if date.format("%Y-%m-%d").to_string() != self.review_on
            || date > chrono::Local::now().date_naive()
        {
            return Err(invalid(
                "The review date must use YYYY-MM-DD and cannot be in the future.",
            ));
        }
        Ok(Self {
            actor: text_field(&self.actor, "the reviewer name or identifier", 200)?,
            review_on: self.review_on.clone(),
            basis: text_field(&self.basis, "the review basis", 4000)?,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceIdentityDecision {
    IdentityChecked,
    AwaitingVerification,
    Rejected,
}
impl SourceIdentityDecision {
    pub fn label(self) -> &'static str {
        match self {
            Self::IdentityChecked => "Identity checked by reviewer",
            Self::AwaitingVerification => "Awaiting identity verification",
            Self::Rejected => "Rejected / unsuitable source",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryEvent {
    pub id: i64,
    pub source_id: i64,
    pub content_sha256: String,
    pub kind: String,
    pub data: serde_json::Value,
    pub reviewer: LibraryReviewer,
    pub recorded_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreservedVersion {
    pub version_id: i64,
    pub source_id: i64,
    pub active: bool,
    pub metadata: SourceMetadata,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedCandidate {
    pub change: DownloadedChange,
    pub text: String,
    pub text_sha256: String,
    pub extraction: String,
    pub pages: Option<u64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionReview {
    pub reviewer: LibraryReviewer,
    pub expected_active_sha256: String,
    pub identity_confirmed: bool,
    pub original_and_reading_compared: bool,
    pub effectivity_and_scope_reviewed: bool,
}
impl AdoptionReview {
    fn validated(&self) -> Result<Self> {
        let mut value = self.clone();
        value.reviewer = value.reviewer.validated()?;
        if !valid_hash(&value.expected_active_sha256)
            || !value.identity_confirmed
            || !value.original_and_reading_compared
            || !value.effectivity_and_scope_reviewed
        {
            return Err(invalid(
                "Confirm the publication identity, compare the reading with the original, review effectivity/scope and identify the current version before adoption.",
            ));
        }
        Ok(value)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvisionBoundary {
    pub title: String,
    pub start: String,
    pub end: String,
    pub text_sha256: String,
}
impl ProvisionBoundary {
    pub fn text<'a>(&self, text: &'a str) -> Result<&'a str> {
        if self.start.is_empty() || self.end.is_empty() || !valid_hash(&self.text_sha256) {
            return Err(invalid("Invalid compiled provision boundary."));
        }
        let mut matches = text.match_indices(&self.start);
        let (start, _) = matches
            .next()
            .ok_or_else(|| invalid("The supporting provision is missing."))?;
        if matches.next().is_some() {
            return Err(invalid("The supporting provision is ambiguous."));
        }
        let end = text[start + self.start.len()..]
            .find(&self.end)
            .map(|n| start + self.start.len() + n)
            .ok_or_else(|| invalid("The complete supporting provision ending is missing."))?;
        let section = &text[start..end];
        if sha256(section.as_bytes()) != self.text_sha256 {
            return Err(invalid(
                "Supporting provision text changed; a newly implemented and tested rule profile is required.",
            ));
        }
        Ok(section)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleDependency {
    pub rule_id: String,
    pub title: String,
    pub source_key: String,
    pub raw_sha256: String,
    pub text_sha256: String,
    pub engine: String,
    pub pack_sha256: String,
    pub provisions: Vec<ProvisionBoundary>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleReadiness {
    Current,
    RevalidationRequired,
    Revalidated,
    UpdateRequired,
}
impl RuleReadiness {
    pub fn label(self) -> &'static str {
        match self {
            Self::Current => "Current compiled profile",
            Self::RevalidationRequired => "Revalidation required",
            Self::Revalidated => "Unchanged profile revalidated",
            Self::UpdateRequired => "Updated rule implementation required",
        }
    }
    pub fn usable(self) -> bool {
        matches!(self, Self::Current | Self::Revalidated)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleLibraryState {
    pub dependency: RuleDependency,
    pub active_sha256: String,
    pub active_text_sha256: String,
    pub token: String,
    pub status: RuleReadiness,
    pub matching_provisions: bool,
    pub pending_downloads: usize,
    pub identity_ready: bool,
    pub period_ready: bool,
    pub source_effective_from: Option<String>,
    pub update_hold_id: Option<i64>,
    pub reason: String,
    pub review_id: Option<i64>,
    pub reviewer: Option<LibraryReviewer>,
    pub test_reference: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleRevalidation {
    pub expected_token: String,
    pub unchanged_profile: bool,
    pub applicability_and_amendments_reviewed: bool,
    pub test_reference: String,
    pub reviewer: LibraryReviewer,
    #[serde(default)]
    pub supersedes_update_assessment: Option<i64>,
}

impl LibraryCatalog {
    fn event(
        &self,
        id: i64,
        hash: &str,
        kind: &str,
        data: &impl Serialize,
        reviewer: &LibraryReviewer,
    ) -> Result<i64> {
        self.connection.execute("INSERT INTO source_events(source_id,content_sha256,kind,data_json,actor,basis,review_on,recorded_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,hash,kind,serde_json::to_string(data)?,reviewer.actor,reviewer.basis,reviewer.review_on,Utc::now().timestamp()])?;
        Ok(self.connection.last_insert_rowid())
    }
    fn latest_event(&self, id: i64, hash: &str, kind: &str) -> Result<Option<(i64, String)>> {
        Ok(self.connection.query_row("SELECT id,data_json FROM source_events WHERE source_id=?1 AND content_sha256=?2 AND kind=?3 ORDER BY id DESC LIMIT 1",params![id,hash,kind],|r|Ok((r.get(0)?,r.get(1)?))).optional()?)
    }
    pub fn source_history(&self, id: i64) -> Result<Vec<LibraryEvent>> {
        let mut stmt=self.connection.prepare("SELECT id,content_sha256,kind,data_json,actor,basis,review_on,recorded_at FROM source_events WHERE source_id=?1 ORDER BY id DESC")?;
        let rows = stmt.query_map([id], |r| {
            Ok((
                r.get(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get(7)?,
            ))
        })?;
        rows.map(|r| {
            let (id_event, hash, kind, data, actor, basis, review_on, recorded_at) = r?;
            Ok(LibraryEvent {
                id: id_event,
                source_id: id,
                content_sha256: hash,
                kind,
                data: serde_json::from_str(&data)?,
                reviewer: LibraryReviewer {
                    actor,
                    basis,
                    review_on,
                },
                recorded_at,
            })
        })
        .collect()
    }
    pub fn source_identity_decision(
        &self,
        id: i64,
        hash: &str,
    ) -> Result<Option<SourceIdentityDecision>> {
        self.latest_event(id, hash, "identity")?
            .map(|(_, json)| serde_json::from_str(&json).map_err(Into::into))
            .transpose()
    }
    pub fn edit_source_metadata(
        &self,
        id: i64,
        expected_hash: &str,
        input: &ManualSourceInput,
        reviewer: &LibraryReviewer,
    ) -> Result<i64> {
        let reviewer = reviewer.validated()?;
        let input = input.validated()?;
        if input.verification != ManualVerification::AwaitingVerification {
            return Err(invalid(
                "Use the separate source identity review to change verification status.",
            ));
        }
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let current = self.snapshot(id)?;
        if current.metadata.raw_sha256 != expected_hash {
            return Err(invalid(
                "The active reference changed. Reopen its metadata before saving.",
            ));
        }
        let event = self.event(id, expected_hash, "metadata", &input, &reviewer)?;
        tx.execute("DELETE FROM source_domains WHERE source_id=?1", [id])?;
        for domain in &input.domains {
            tx.execute(
                "INSERT INTO source_domains(source_id,domain_id) VALUES(?1,?2)",
                params![id, domain],
            )?;
        }
        tx.commit()?;
        Ok(event)
    }
    pub fn review_source_identity(
        &self,
        id: i64,
        expected_hash: &str,
        decision: SourceIdentityDecision,
        reviewer: &LibraryReviewer,
    ) -> Result<i64> {
        let reviewer = reviewer.validated()?;
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        if self.snapshot(id)?.metadata.raw_sha256 != expected_hash {
            return Err(invalid(
                "The active reference changed. Review its current original first.",
            ));
        }
        let event = self.event(id, expected_hash, "identity", &decision, &reviewer)?;
        tx.commit()?;
        Ok(event)
    }
    pub(super) fn apply_annotations(&self, id: i64, metadata: &mut SourceMetadata) -> Result<()> {
        if let Some((_, json)) = self.latest_event(id, &metadata.raw_sha256, "metadata")? {
            let details: ManualSourceInput = serde_json::from_str(&json)?;
            metadata.title = details.title;
            metadata.instrument = details.instrument;
            metadata.issuer = details.issuer;
            metadata.domains = details.domains;
            metadata.official_reference_url = Some(details.official_url);
            metadata.issued_on = details.issued_on;
            metadata.effective_from = details.effective_from;
            metadata.effectivity_note = details.effectivity_note;
            metadata.applicability_note = details.applicability_note;
        }
        if let Some((event_id, json)) = self.latest_event(id, &metadata.raw_sha256, "identity")? {
            let decision: SourceIdentityDecision = serde_json::from_str(&json)?;
            if metadata.provenance.is_manual() {
                metadata.provenance = if decision == SourceIdentityDecision::IdentityChecked {
                    Provenance::ManualIdentityChecked
                } else {
                    Provenance::ManualPending
                };
                if decision == SourceIdentityDecision::IdentityChecked {
                    let at: i64 = self.connection.query_row(
                        "SELECT recorded_at FROM source_events WHERE id=?1",
                        [event_id],
                        |r| r.get(0),
                    )?;
                    metadata.verified_at_unix = at;
                    metadata.verified_at_utc = chrono::DateTime::from_timestamp(at, 0)
                        .ok_or_else(|| invalid("Invalid source identity-review timestamp."))?
                        .to_rfc3339();
                } else {
                    metadata.verified_at_unix = 0;
                    metadata.verified_at_utc.clear();
                }
            }
        }
        Ok(())
    }
    pub fn display_metadata(&self, id: i64) -> Result<SourceMetadata> {
        let mut metadata = self.snapshot(id)?.metadata;
        self.apply_annotations(id, &mut metadata)?;
        Ok(metadata)
    }
    pub fn preserved_versions(&self, id: i64) -> Result<Vec<PreservedVersion>> {
        let mut stmt=self.connection.prepare("SELECT v.id,v.id=a.version_id,p.metadata_json FROM source_versions v JOIN source_snapshots p ON p.version_id=v.id JOIN active_source_versions a ON a.source_id=v.source_id WHERE v.source_id=?1 ORDER BY v.retrieved_at DESC,v.id DESC")?;
        let rows = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?)))?;
        rows.map(|r| {
            let (version_id, active, json) = r?;
            Ok(PreservedVersion {
                version_id,
                source_id: id,
                active,
                metadata: serde_json::from_str(&json)?,
            })
        })
        .collect()
    }
    pub fn downloaded_candidate_bytes(
        &self,
        change_id: i64,
    ) -> Result<(DownloadedChange, Vec<u8>)> {
        let source_id: i64 = self.connection.query_row(
            "SELECT source_id FROM downloaded_source_changes WHERE id=?1",
            [change_id],
            |r| r.get(0),
        )?;
        let change = self
            .downloaded_changes(source_id)?
            .into_iter()
            .find(|c| c.id == change_id)
            .ok_or(LibraryError::MissingSnapshot)?;
        let raw: Vec<u8> = self.connection.query_row(
            "SELECT raw_bytes FROM downloaded_source_changes WHERE id=?1",
            [change_id],
            |r| r.get(0),
        )?;
        if sha256(&raw) != change.content_sha256 || raw.len() as u64 != change.bytes {
            return Err(LibraryError::Integrity("downloaded candidate".into()));
        }
        Ok((change, raw))
    }
    pub fn prepare_candidate_reading(
        &self,
        change_id: i64,
        file: ManualSourceFile,
    ) -> Result<PreparedCandidate> {
        let (change, raw) = self.downloaded_candidate_bytes(change_id)?;
        if raw != file.raw
            || file.mime != change.mime
            || file.reading_text.trim().is_empty()
            || file.reading_text.len() > MAX_REFERENCE_TEXT_BYTES
            || file.extraction_note.len() > 16_000
        {
            return Err(invalid(
                "The prepared reading does not match the preserved public candidate or exceeds its limits.",
            ));
        }
        let pages = file
            .pages
            .map(i64::try_from)
            .transpose()
            .map_err(|_| invalid("Invalid candidate page count."))?;
        self.connection.execute("INSERT OR IGNORE INTO candidate_readings(change_id,reading_text,text_sha256,extraction,pages,prepared_at) VALUES(?1,?2,?3,?4,?5,?6)",params![change_id,file.reading_text,sha256(file.reading_text.as_bytes()),file.extraction_note,pages,Utc::now().timestamp()])?;
        self.prepared_candidate(change_id)
    }
    pub fn prepared_candidate(&self, change_id: i64) -> Result<PreparedCandidate> {
        let (change, _) = self.downloaded_candidate_bytes(change_id)?;
        let(text,text_sha256,extraction,pages):(String,String,String,Option<i64>)=self.connection.query_row("SELECT reading_text,text_sha256,extraction,pages FROM candidate_readings WHERE change_id=?1",[change_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?.ok_or_else(||invalid("Read this candidate locally before comparing or adopting it."))?;
        if sha256(text.as_bytes()) != text_sha256 {
            return Err(LibraryError::Integrity("candidate reading".into()));
        }
        Ok(PreparedCandidate {
            change,
            text,
            text_sha256,
            extraction,
            pages: pages
                .map(u64::try_from)
                .transpose()
                .map_err(|_| invalid("Invalid candidate page count."))?,
        })
    }
    pub fn compare_candidate(&self, change_id: i64) -> Result<ReferenceComparison> {
        let candidate = self.prepared_candidate(change_id)?;
        let current = self.snapshot(candidate.change.source_id)?;
        compare_readings(
            &current.metadata.raw_sha256,
            &current.text,
            &candidate.change.content_sha256,
            &candidate.text,
        )
    }
    pub fn compare_versions(
        &self,
        id: i64,
        left: &str,
        right: &str,
    ) -> Result<ReferenceComparison> {
        let key = self.snapshot(id)?.metadata.key;
        let versions = self.preserved_versions(id)?;
        let read = |hash: &str| -> Result<SourceSnapshot> {
            let m = &versions
                .iter()
                .find(|v| v.metadata.raw_sha256 == hash)
                .ok_or(LibraryError::MissingSnapshot)?
                .metadata;
            self.snapshot_for_review(&key, hash, &m.text_sha256)
        };
        let left = read(left)?;
        let right = read(right)?;
        compare_readings(
            &left.metadata.raw_sha256,
            &left.text,
            &right.metadata.raw_sha256,
            &right.text,
        )
    }
    fn record_candidate_decision(
        &self,
        id: i64,
        decision: &str,
        base: &str,
        reviewer: &LibraryReviewer,
    ) -> Result<()> {
        self.connection.execute("INSERT INTO candidate_decisions(change_id,decision,base_sha256,actor,basis,review_on,recorded_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,decision,base,reviewer.actor,reviewer.basis,reviewer.review_on,Utc::now().timestamp()])?;
        Ok(())
    }
    pub fn dismiss_candidate(
        &self,
        change_id: i64,
        expected_active: &str,
        reviewer: &LibraryReviewer,
    ) -> Result<()> {
        let reviewer = reviewer.validated()?;
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let (change, _) = self.downloaded_candidate_bytes(change_id)?;
        if self.snapshot(change.source_id)?.metadata.raw_sha256 != expected_active {
            return Err(invalid(
                "The active source changed; compare the current versions first.",
            ));
        }
        self.record_candidate_decision(change_id, "dismissed", expected_active, &reviewer)?;
        self.event(
            change.source_id,
            expected_active,
            "candidate_dismissed",
            &change,
            &reviewer,
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn adopt_candidate(
        &self,
        change_id: i64,
        review: &AdoptionReview,
    ) -> Result<SourceMetadata> {
        let review = review.validated()?;
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let candidate = self.prepared_candidate(change_id)?;
        let (_, raw) = self.downloaded_candidate_bytes(change_id)?;
        let old = self.snapshot(candidate.change.source_id)?;
        if old.metadata.raw_sha256 != review.expected_active_sha256 {
            return Err(invalid(
                "The active source changed. Compare again before adopting.",
            ));
        }
        let mut metadata = old.metadata;
        let date = Utc::now();
        metadata.provenance = Provenance::ReviewedDownload;
        metadata.source_url = candidate.change.url.clone();
        metadata.official_reference_url = Some(candidate.change.url.clone());
        metadata.resolved_url = candidate.change.resolved_url.clone();
        metadata.acquired_at_unix = candidate.change.downloaded_at;
        metadata.acquired_at_utc =
            chrono::DateTime::from_timestamp(candidate.change.downloaded_at, 0)
                .ok_or_else(|| invalid("Invalid download date."))?
                .to_rfc3339();
        metadata.verified_at_unix = date.timestamp();
        metadata.verified_at_utc = date.to_rfc3339();
        metadata.raw_path = format!("adopted/{}", candidate.change.content_sha256);
        metadata.raw_sha256 = candidate.change.content_sha256.clone();
        metadata.raw_bytes = raw.len() as u64;
        metadata.raw_mime = candidate.change.mime;
        metadata.text_path = format!("adopted/{}.txt", candidate.text_sha256);
        metadata.text_sha256 = candidate.text_sha256;
        metadata.extraction = candidate.extraction;
        metadata.pages = candidate.pages;
        metadata.issued_on = None;
        metadata.effective_from = None;
        metadata.effectivity_note="Reviewed adoption. Publication/effective dates are not inferred from byte changes; record assessed dates in source metadata.".into();
        metadata.verification_scope = format!(
            "Official HTTPS download, locally compared and deliberately adopted by {} on {}. {} This attestation is not independent legal certification.",
            review.reviewer.actor, review.reviewer.review_on, review.reviewer.basis
        );
        metadata.acquisition_headers.clear();
        validate_payload(&metadata, &raw, &candidate.text)?;
        tx.execute("INSERT OR IGNORE INTO source_versions(source_id,content_sha256,local_path,retrieved_at) VALUES(?1,?2,?3,?4)",params![candidate.change.source_id,metadata.raw_sha256,format!("sqlite:source_snapshots/{}",metadata.raw_sha256),date.timestamp()])?;
        let version_id: i64 = tx.query_row(
            "SELECT id FROM source_versions WHERE source_id=?1 AND content_sha256=?2",
            params![candidate.change.source_id, metadata.raw_sha256],
            |r| r.get(0),
        )?;
        tx.execute("INSERT OR IGNORE INTO source_snapshots(version_id,raw_bytes,reading_text,metadata_json,verified_at) VALUES(?1,?2,?3,?4,?5)",params![version_id,raw,candidate.text,serde_json::to_string(&metadata)?,date.timestamp()])?;
        let saved =
            self.snapshot_for_review(&metadata.key, &metadata.raw_sha256, &metadata.text_sha256)?;
        self.activate_version(candidate.change.source_id, version_id, &saved.metadata)?;
        self.record_candidate_decision(
            change_id,
            "adopted",
            &review.expected_active_sha256,
            &review.reviewer,
        )?;
        self.event(
            candidate.change.source_id,
            &metadata.raw_sha256,
            "adoption",
            &review,
            &review.reviewer,
        )?;
        tx.commit()?;
        Ok(saved.metadata)
    }
    fn activate_version(&self, id: i64, version_id: i64, metadata: &SourceMetadata) -> Result<()> {
        self.connection.execute(
            "UPDATE active_source_versions SET version_id=?1 WHERE source_id=?2",
            params![version_id, id],
        )?;
        self.connection.execute(
            "UPDATE sources SET verification_status=?1 WHERE id=?2",
            params![metadata.provenance.database_status(), id],
        )?;
        self.connection
            .execute("DELETE FROM source_domains WHERE source_id=?1", [id])?;
        let mut effective = metadata.clone();
        self.apply_annotations(id, &mut effective)?;
        for domain in &effective.domains {
            self.connection.execute(
                "INSERT INTO source_domains(source_id,domain_id) VALUES(?1,?2)",
                params![id, domain],
            )?;
        }
        Ok(())
    }
    pub fn restore_preserved_version(
        &self,
        id: i64,
        hash: &str,
        review: &AdoptionReview,
    ) -> Result<()> {
        let review = review.validated()?;
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let current = self.snapshot(id)?;
        if current.metadata.raw_sha256 != review.expected_active_sha256 {
            return Err(invalid(
                "The active source changed. Reopen version history.",
            ));
        }
        let version = self
            .preserved_versions(id)?
            .into_iter()
            .find(|v| v.metadata.raw_sha256 == hash)
            .ok_or(LibraryError::MissingSnapshot)?;
        self.snapshot_for_review(&version.metadata.key, hash, &version.metadata.text_sha256)?;
        self.activate_version(id, version.version_id, &version.metadata)?;
        self.event(id, hash, "version_restored", &review, &review.reviewer)?;
        tx.commit()?;
        Ok(())
    }
    pub fn register_rule_profiles(&self, profiles: &[RuleDependency]) -> Result<()> {
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let mut checked = std::collections::BTreeMap::new();
        for profile in profiles {
            if profile.rule_id.is_empty()
                || profile.provisions.is_empty()
                || !valid_hash(&profile.pack_sha256)
            {
                return Err(invalid("Invalid compiled checking profile."));
            }
            let key = (
                &profile.source_key,
                &profile.raw_sha256,
                &profile.text_sha256,
            );
            let original = match checked.entry(key) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(self.snapshot_for_review(
                        &profile.source_key,
                        &profile.raw_sha256,
                        &profile.text_sha256,
                    )?)
                }
            };
            for provision in &profile.provisions {
                provision.text(&original.text)?;
            }
            let json = serde_json::to_string(profile)?;
            tx.execute("INSERT INTO rule_profiles(rule_id,profile_json,fingerprint) VALUES(?1,?2,?3) ON CONFLICT(rule_id) DO UPDATE SET profile_json=excluded.profile_json,fingerprint=excluded.fingerprint WHERE profile_json<>excluded.profile_json OR fingerprint<>excluded.fingerprint",params![profile.rule_id,json,sha256(json.as_bytes())])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn rule_library_states(&self) -> Result<Vec<RuleLibraryState>> {
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Deferred)?;
        let mut stmt = self
            .connection
            .prepare("SELECT rule_id FROM rule_profiles ORDER BY rule_id")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut snapshots = std::collections::BTreeMap::new();
        let states = ids
            .iter()
            .map(|id| self.rule_library_state_with_cache(id, &mut snapshots))
            .collect::<Result<Vec<_>>>()?;
        drop(stmt);
        tx.commit()?;
        Ok(states)
    }
    pub fn rule_library_state(&self, rule_id: &str) -> Result<RuleLibraryState> {
        self.rule_library_state_with_cache(rule_id, &mut std::collections::BTreeMap::new())
    }
    fn rule_library_state_with_cache(
        &self,
        rule_id: &str,
        cache: &mut std::collections::BTreeMap<i64, SourceSnapshot>,
    ) -> Result<RuleLibraryState> {
        let (json, fingerprint): (String, String) = self.connection.query_row(
            "SELECT profile_json,fingerprint FROM rule_profiles WHERE rule_id=?1",
            [rule_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if sha256(json.as_bytes()) != fingerprint {
            return Err(LibraryError::Integrity(
                "rule dependency registration".into(),
            ));
        }
        let profile: RuleDependency = serde_json::from_str(&json)?;
        let id: i64 = self.connection.query_row(
            "SELECT id FROM sources WHERE stable_key=?1",
            [&profile.source_key],
            |r| r.get(0),
        )?;
        let active = match cache.entry(id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => entry.insert(self.snapshot(id)?),
        };
        let metadata_event = self.latest_event(id, &active.metadata.raw_sha256, "metadata")?;
        let identity_event = self.latest_event(id, &active.metadata.raw_sha256, "identity")?;
        let rejected = if let Some((_, json)) = &identity_event {
            serde_json::from_str::<SourceIdentityDecision>(json)?
                != SourceIdentityDecision::IdentityChecked
        } else {
            false
        };
        let mut candidates = Vec::new();
        let mut pending = 0;
        for change in self.downloaded_changes(id)? {
            let decision:Option<(i64,String)>=self.connection.query_row("SELECT id,decision FROM candidate_decisions WHERE change_id=?1 ORDER BY id DESC LIMIT 1",[change.id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if decision.is_none() {
                pending += 1;
            }
            candidates.push((change.id, change.content_sha256, decision));
        }
        let activation_event:Option<i64>=self.connection.query_row("SELECT MAX(id) FROM source_events WHERE source_id=?1 AND kind IN('adoption','version_restored')",[id],|r|r.get(0))?;
        let token = sha256(
            serde_json::to_vec(&(
                fingerprint,
                &active.metadata.raw_sha256,
                &active.metadata.text_sha256,
                metadata_event.as_ref().map(|e| e.0),
                identity_event.as_ref().map(|e| e.0),
                activation_event,
                candidates,
            ))?
            .as_slice(),
        );
        let matching = profile
            .provisions
            .iter()
            .all(|p| p.text(&active.text).is_ok());
        let mut annotated = active.metadata.clone();
        self.apply_annotations(id, &mut annotated)?;
        let period_ready = annotated.effective_from.as_deref().is_none_or(|date| {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .is_ok_and(|date| date <= chrono::Local::now().date_naive())
        });
        let mut state = RuleLibraryState {
            dependency: profile,
            active_sha256: active.metadata.raw_sha256.clone(),
            active_text_sha256: active.metadata.text_sha256.clone(),
            token: token.clone(),
            status: RuleReadiness::RevalidationRequired,
            matching_provisions: matching,
            pending_downloads: pending,
            identity_ready: !rejected,
            period_ready,
            source_effective_from: annotated.effective_from,
            update_hold_id: None,
            reason: String::new(),
            review_id: None,
            reviewer: None,
            test_reference: None,
        };
        if pending > 0 {
            state.reason = "A source download awaits an adoption or dismissal decision.".into();
        } else if rejected {
            state.reason = "The current source identity is unconfirmed or rejected.".into();
        } else if !period_ready {
            state.reason="The recorded source effective date is future or invalid. Preserve it for reference and review effectivity before activating dependent checks.".into();
        } else if !matching {
            state.status = RuleReadiness::UpdateRequired;
            state.reason="The complete supporting text differs from this compiled rule. An updated and tested rule implementation is required.".into();
        } else {
            let assessment:Option<(i64,String,String,String,String,String)>=self.connection.query_row("SELECT id,decision,actor,basis,review_on,test_reference FROM rule_revalidations WHERE rule_id=?1 AND token=?2 ORDER BY id DESC LIMIT 1",params![rule_id,token],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
            if let Some((id, decision, actor, basis, review_on, test)) = assessment {
                state.review_id = Some(id);
                state.reviewer = Some(LibraryReviewer {
                    actor,
                    basis,
                    review_on,
                });
                state.test_reference = Some(test);
                state.status = if decision == "unchanged_profile" {
                    RuleReadiness::Revalidated
                } else {
                    RuleReadiness::UpdateRequired
                };
                state.reason=if state.status.usable(){"Reviewer confirmed the unchanged compiled profile against this exact source and metadata state."}else{"Reviewer identified a legal/rule change; the compiled rule remains held."}.into();
            } else if state.active_sha256 == state.dependency.raw_sha256
                && state.active_text_sha256 == state.dependency.text_sha256
                && metadata_event.is_none()
                && activation_event.is_none()
            {
                state.status = RuleReadiness::Current;
                state.reason = "The active reference matches the tested compiled profile.".into();
            } else {
                state.reason="The active reference or assessed metadata changed. Revalidate the unchanged profile before checking.".into();
            }
        }
        // A reviewed legal-change hold survives unrelated source/metadata edits,
        // rollback and later app releases. Clearing it is a separate explicit
        // acknowledgement; changing the token alone cannot reactivate a rule.
        let hold:Option<(i64,String,String,String,String)>=self.connection.query_row(
            "SELECT u.id,u.actor,u.basis,u.review_on,u.test_reference FROM rule_revalidations u WHERE u.rule_id=?1 AND u.decision='update_required' AND NOT EXISTS(SELECT 1 FROM rule_hold_resolutions r WHERE r.update_assessment_id=u.id) ORDER BY u.id DESC LIMIT 1",
            [rule_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        if let Some((id, actor, basis, review_on, test)) = hold {
            state.update_hold_id = Some(id);
            state.review_id = Some(id);
            state.reviewer = Some(LibraryReviewer {
                actor,
                basis,
                review_on,
            });
            state.test_reference = Some(test);
            state.status = RuleReadiness::UpdateRequired;
            state.reason = format!(
                "Reviewed legal-change hold #{id} remains open. Resolve the legal/rule change before reactivation; unrelated source edits or an app upgrade do not clear it."
            );
        }
        Ok(state)
    }
    pub fn revalidate_rule(&self, rule_id: &str, review: &RuleRevalidation) -> Result<i64> {
        let reviewer = review.reviewer.validated()?;
        let test = text_field(
            &review.test_reference,
            "the test/evaluation reference",
            1000,
        )?;
        if !review.applicability_and_amendments_reviewed {
            return Err(invalid(
                "Review amendments, applicability and exceptions before recording a rule assessment.",
            ));
        }
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let state = self.rule_library_state(rule_id)?;
        if state.token != review.expected_token {
            return Err(invalid(
                "The source, metadata or candidate decisions changed. Reopen the rule assessment.",
            ));
        }
        if review.unchanged_profile
            && (!state.matching_provisions
                || state.pending_downloads > 0
                || !state.identity_ready
                || !state.period_ready)
        {
            return Err(invalid(
                "Resolve pending source decisions and identity review; changed supporting text requires a new tested rule implementation.",
            ));
        }
        if review.unchanged_profile && state.update_hold_id != review.supersedes_update_assessment {
            return Err(invalid(
                "Explicitly acknowledge the current legal-change hold before resolving it. Reopen the rule if its assessment ID changed.",
            ));
        }
        if !review.unchanged_profile && review.supersedes_update_assessment.is_some() {
            return Err(invalid(
                "An update-required assessment cannot resolve a legal-change hold.",
            ));
        }
        tx.execute("INSERT INTO rule_revalidations(rule_id,token,decision,actor,basis,review_on,test_reference,recorded_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![rule_id,state.token,if review.unchanged_profile{"unchanged_profile"}else{"update_required"},reviewer.actor,reviewer.basis,reviewer.review_on,test,Utc::now().timestamp()])?;
        let id = tx.last_insert_rowid();
        if review.unchanged_profile
            && let Some(through) = state.update_hold_id
        {
            tx.execute("INSERT INTO rule_hold_resolutions(update_assessment_id,resolution_assessment_id) SELECT u.id,?1 FROM rule_revalidations u WHERE u.rule_id=?2 AND u.decision='update_required' AND u.id<=?3 AND NOT EXISTS(SELECT 1 FROM rule_hold_resolutions r WHERE r.update_assessment_id=u.id)",params![id,rule_id,through])?;
        }
        let source_id: i64 = tx.query_row(
            "SELECT id FROM sources WHERE stable_key=?1",
            [&state.dependency.source_key],
            |r| r.get(0),
        )?;
        self.event(source_id,&state.active_sha256,"rule_revalidation",&serde_json::json!({"rule_id":rule_id,"assessment_id":id,"state_token":state.token,"unchanged_profile":review.unchanged_profile,"test_reference":test,"supersedes_update_assessment":review.supersedes_update_assessment}),&reviewer)?;
        tx.commit()?;
        Ok(id)
    }
}
