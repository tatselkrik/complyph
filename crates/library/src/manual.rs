//! Local reference intake. URLs are citations only; this module never fetches them.
use super::*;
use chrono::{NaiveDate, Utc};
use url::Url;

const MAX_RAW_BYTES: usize = 32 * 1024 * 1024;
const MAX_READING_BYTES: usize = 6 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualVerification {
    #[default]
    AwaitingVerification,
    IdentityChecked,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManualSourceInput {
    pub title: String,
    pub instrument: String,
    pub issuer: String,
    pub official_url: String,
    pub domains: Vec<String>,
    #[serde(default)]
    pub issued_on: Option<String>,
    #[serde(default)]
    pub effective_from: Option<String>,
    #[serde(default)]
    pub effectivity_note: String,
    #[serde(default)]
    pub applicability_note: String,
    #[serde(default)]
    pub verification: ManualVerification,
    #[serde(default)]
    pub verification_note: String,
}

/// Prepared by the desktop's bounded parser. Never includes the selected file path.
pub struct ManualSourceFile {
    pub raw: Vec<u8>,
    pub reading_text: String,
    pub mime: String,
    pub pages: Option<u64>,
    pub extraction_note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImportOutcome {
    pub source_id: i64,
    pub added: bool,
    pub new_version: bool,
    pub metadata: SourceMetadata,
}

fn invalid(message: impl Into<String>) -> LibraryError {
    LibraryError::InvalidImport(message.into())
}

fn field(value: &str, name: &str, maximum: usize, required: bool) -> Result<String> {
    let value = value.trim();
    if (required && value.is_empty())
        || value.len() > maximum
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(invalid(format!(
            "Enter a valid {name} (up to {maximum} bytes)."
        )));
    }
    Ok(value.into())
}

fn date(value: &Option<String>, label: &str) -> Result<Option<String>> {
    match value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(value) => {
            let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
                invalid(format!(
                    "{label} must be a real date in YYYY-MM-DD format, or blank."
                ))
            })?;
            if parsed.format("%Y-%m-%d").to_string() != value {
                return Err(invalid(format!("{label} must use YYYY-MM-DD format.")));
            }
            Ok(Some(value.into()))
        }
    }
}

impl ManualSourceInput {
    /// Validate before starting a parser, and again inside the catalog write path.
    pub fn validated(&self) -> Result<Self> {
        let mut input = self.clone();
        input.title = field(&input.title, "title", 300, true)?;
        input.instrument = field(
            &input.instrument,
            "instrument or reference number",
            200,
            true,
        )?;
        input.issuer = field(&input.issuer, "issuing agency", 200, true)?;
        input.effectivity_note = field(&input.effectivity_note, "effectivity note", 4000, false)?;
        input.applicability_note = field(&input.applicability_note, "scope note", 4000, false)?;
        input.verification_note =
            field(&input.verification_note, "verification note", 4000, false)?;
        let citation = field(&input.official_url, "official URL", 2048, true)?;
        if citation.chars().any(char::is_whitespace) {
            return Err(invalid(
                "The official URL cannot contain spaces or line breaks.",
            ));
        }
        let url =
            Url::parse(&citation).map_err(|_| invalid("Enter a complete HTTPS official URL."))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(invalid(
                "Enter an HTTPS official URL without embedded credentials.",
            ));
        }
        input.official_url = url.to_string();
        let domains = starter_manifest()?.domains;
        if input.domains.is_empty()
            || input
                .domains
                .iter()
                .any(|id| !domains.iter().any(|d| &d.id == id))
        {
            return Err(invalid("Choose at least one supported compliance domain."));
        }
        input.domains.sort();
        input.domains.dedup();
        input.issued_on = date(&input.issued_on, "Issue date")?;
        input.effective_from = date(&input.effective_from, "Effective date")?;
        if input.verification == ManualVerification::IdentityChecked
            && input.verification_note.is_empty()
        {
            return Err(invalid(
                "Record what you compared with the official publication before marking identity checked.",
            ));
        }
        Ok(input)
    }
}

impl LibraryCatalog {
    /// Append an immutable public snapshot. Existing bundled sources cannot be replaced.
    /// The same manual URL adds a version; identical bytes retain their original metadata.
    pub fn import_manual(
        &mut self,
        input: &ManualSourceInput,
        file: ManualSourceFile,
    ) -> Result<ImportOutcome> {
        let input = input.validated()?;
        let extension = match file.mime.as_str() {
            "application/pdf" if file.raw.starts_with(b"%PDF-") => "pdf",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                if file.raw.starts_with(b"PK") =>
            {
                "docx"
            }
            _ => return Err(invalid("Choose a readable PDF or DOCX legal reference.")),
        };
        if file.raw.is_empty()
            || file.raw.len() > MAX_RAW_BYTES
            || file.reading_text.trim().is_empty()
            || file.reading_text.len() > MAX_READING_BYTES
            || file.extraction_note.len() > 16_000
        {
            return Err(invalid(
                "The reference exceeds the import limits or has no readable text. Nothing was saved.",
            ));
        }
        let hash = sha256(&file.raw);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(i64, Option<String>)> = transaction
            .query_row(
                "SELECT id,stable_key FROM sources WHERE source_url=?1",
                [&input.official_url],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (source_id, key, new_version) = if let Some((id, Some(key))) = existing {
            let previous: Option<String> = transaction.query_row(
                "SELECT p.metadata_json FROM source_versions v JOIN source_snapshots p ON p.version_id=v.id
                 WHERE v.source_id=?1 ORDER BY v.retrieved_at DESC,v.id DESC LIMIT 1", [id], |r| r.get(0)).optional()?;
            let previous: SourceMetadata = previous.map(|v| serde_json::from_str(&v)).transpose()?
                .ok_or_else(|| invalid("This URL belongs to an existing preserved source. It cannot be replaced by manual intake."))?;
            if !key.starts_with("manual-") || !previous.provenance.is_manual() {
                return Err(invalid(
                    "This official URL is already in the preserved collection. Manual import cannot replace a bundled source.",
                ));
            }
            let identical: Option<String> = transaction.query_row(
                "SELECT p.metadata_json FROM source_versions v JOIN source_snapshots p ON p.version_id=v.id
                 WHERE v.source_id=?1 AND v.content_sha256=?2", params![id, hash], |r| r.get(0)).optional()?;
            if let Some(metadata) = identical {
                return Ok(ImportOutcome {
                    source_id: id,
                    added: false,
                    new_version: false,
                    metadata: serde_json::from_str(&metadata)?,
                });
            }
            (id, key, true)
        } else if existing.is_some() {
            return Err(invalid(
                "This URL belongs to a legacy reference and cannot be replaced by manual intake.",
            ));
        } else {
            let key = format!("manual-{}", sha256(input.official_url.as_bytes()));
            transaction.execute(
                "INSERT INTO sources(stable_key,title,issuer,source_url,verification_status)
                VALUES(?1,?2,?3,?4,'unverified')",
                params![key, input.title, input.issuer, input.official_url],
            )?;
            (transaction.last_insert_rowid(), key, false)
        };
        let now = Utc::now();
        let checked = input.verification == ManualVerification::IdentityChecked;
        let provenance = if checked {
            Provenance::ManualIdentityChecked
        } else {
            Provenance::ManualPending
        };
        let metadata = SourceMetadata {
            provenance,
            official_reference_url: Some(input.official_url.clone()),
            key,
            title: input.title,
            instrument: input.instrument,
            issuer: input.issuer,
            domains: input.domains,
            source_url: input.official_url,
            resolved_url: String::new(),
            acquired_at_utc: now.to_rfc3339(),
            acquired_at_unix: now.timestamp(),
            verified_at_utc: if checked {
                now.to_rfc3339()
            } else {
                String::new()
            },
            verified_at_unix: if checked { now.timestamp() } else { 0 },
            raw_path: format!("manual/{hash}.{extension}"),
            raw_mime: file.mime,
            raw_bytes: file.raw.len() as u64,
            raw_sha256: hash,
            text_path: format!("manual/{}.txt", sha256(file.reading_text.as_bytes())),
            text_sha256: sha256(file.reading_text.as_bytes()),
            extraction: file.extraction_note,
            pages: file.pages,
            issued_on: input.issued_on,
            effective_from: input.effective_from,
            effectivity_note: input.effectivity_note,
            applicability_note: if input.applicability_note.is_empty() {
                "Reference only. Adding a source does not add or validate automated checks.".into()
            } else {
                input.applicability_note
            },
            verification_scope: format!(
                "{} No online request or agency-byte verification was performed by ComplyPH. {}",
                if checked {
                    "User recorded an identity comparison with the official publication."
                } else {
                    "Awaiting independent verification; the official URL is a user-provided citation."
                },
                input.verification_note
            ),
            acquisition_headers: Default::default(),
        };
        validate_payload(&metadata, &file.raw, &file.reading_text)?;
        transaction.execute(
            "UPDATE sources SET title=?1,issuer=?2,verification_status=?3 WHERE id=?4",
            params![
                metadata.title,
                metadata.issuer,
                provenance.database_status(),
                source_id
            ],
        )?;
        transaction.execute("DELETE FROM source_domains WHERE source_id=?1", [source_id])?;
        for domain in &metadata.domains {
            transaction.execute(
                "INSERT INTO source_domains(source_id,domain_id) VALUES(?1,?2)",
                params![source_id, domain],
            )?;
        }
        let unix_date = |value: &Option<String>| {
            value
                .as_ref()
                .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|d| d.and_utc().timestamp())
        };
        transaction.execute("INSERT INTO source_versions(source_id,content_sha256,local_path,retrieved_at,published_at,effective_from)
            VALUES(?1,?2,?3,?4,?5,?6)", params![source_id, metadata.raw_sha256,
            format!("sqlite:source_snapshots/{}", metadata.raw_sha256), now.timestamp(), unix_date(&metadata.issued_on), unix_date(&metadata.effective_from)])?;
        let version_id = transaction.last_insert_rowid();
        transaction.execute("INSERT INTO source_snapshots(version_id,raw_bytes,reading_text,metadata_json,verified_at)
            VALUES(?1,?2,?3,?4,?5)", params![version_id, file.raw, file.reading_text, serde_json::to_string(&metadata)?, metadata.verified_at_unix])?;
        transaction.execute("INSERT INTO active_source_versions(source_id,version_id) VALUES(?1,?2) ON CONFLICT(source_id) DO UPDATE SET version_id=excluded.version_id",params![source_id,version_id])?;
        transaction.commit()?;
        Ok(ImportOutcome {
            source_id,
            added: true,
            new_version,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ManualSourceInput {
        ManualSourceInput {
            title: "Fictional legal reference".into(),
            instrument: "TEST 2026-01".into(),
            issuer: "Synthetic agency".into(),
            official_url: "https://example.gov.ph/test/reference.pdf".into(),
            domains: vec!["ph.labor".into()],
            ..Default::default()
        }
    }

    fn file(suffix: &str) -> ManualSourceFile {
        ManualSourceFile {
            raw: format!("%PDF-1.7\nSynthetic fixture {suffix}").into_bytes(),
            reading_text: format!("Fictional legal reference {suffix}"),
            mime: "application/pdf".into(),
            pages: Some(1),
            extraction_note: "Synthetic parser fixture; not a legal source.".into(),
        }
    }

    #[test]
    fn urls_dates_and_identity_claims_are_validated_before_writes() {
        for url in [
            "file:///secret.pdf",
            "javascript:alert(1)",
            "https://user:secret@example.gov.ph/",
            "https://",
            "https://example.gov.ph/bad url",
        ] {
            let mut value = input();
            value.official_url = url.into();
            assert!(value.validated().is_err(), "{url}");
        }
        let mut value = input();
        value.issued_on = Some("2025-02-29".into());
        assert!(value.validated().is_err());
        value.issued_on = Some("2024-02-29".into());
        assert!(value.validated().is_ok());
        value.issued_on = Some("2026-1-1".into());
        assert!(value.validated().is_err());
        value.issued_on = None;
        value.domains = vec!["unknown".into()];
        assert!(value.validated().is_err());
        value.domains = vec!["ph.labor".into()];
        value.verification = ManualVerification::IdentityChecked;
        assert!(value.validated().is_err());
        value.verification_note = "Compared with the official publication.".into();
        assert!(value.validated().is_ok());
    }

    #[test]
    fn manual_versions_keep_history_without_promoting_verification_or_overwriting_the_bundle() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("private")).unwrap();
        fs::write(
            temp.path().join("private/workspace.sqlite3"),
            b"private sentinel",
        )
        .unwrap();
        let mut catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        let bundled_date = catalog.summary().unwrap().bundled_verified_at;
        let first = catalog.import_manual(&input(), file("first")).unwrap();
        assert!(first.added && !first.new_version);
        assert_eq!(first.metadata.provenance, Provenance::ManualPending);
        assert_eq!(first.metadata.verified_at_unix, 0);
        assert!(first.metadata.resolved_url.is_empty());
        let mut checked = input();
        checked.verification = ManualVerification::IdentityChecked;
        checked.verification_note =
            "Compared title, identifier and complete text with the publication.".into();
        let duplicate = catalog.import_manual(&checked, file("first")).unwrap();
        assert!(!duplicate.added);
        assert_eq!(duplicate.metadata.provenance, Provenance::ManualPending);
        let second = catalog.import_manual(&checked, file("second")).unwrap();
        assert!(second.added && second.new_version);
        assert_eq!(second.source_id, first.source_id);
        let summary = catalog.summary().unwrap();
        assert_eq!(summary.source_count, 20);
        assert_eq!(summary.verified_source_count, 14);
        assert_eq!(summary.identity_checked_upload_count, 3);
        assert_eq!(summary.provisional_source_count, 2);
        assert_eq!(summary.user_checked_source_count, 1);
        assert_eq!(summary.awaiting_verification_count, 0);
        assert_eq!(summary.bundled_verified_at, bundled_date);
        let original = catalog
            .snapshot_for_review(
                &first.metadata.key,
                &first.metadata.raw_sha256,
                &first.metadata.text_sha256,
            )
            .unwrap();
        assert!(original.text.ends_with("first"));
        let mut collision = input();
        collision.official_url = catalog
            .snapshot_by_key("labor-code-reference")
            .unwrap()
            .metadata
            .source_url;
        assert!(
            catalog
                .import_manual(&collision, file("collision"))
                .is_err()
        );
        let mut malformed = file("invalid");
        malformed.reading_text.clear();
        assert!(catalog.import_manual(&input(), malformed).is_err());
        drop(catalog);
        let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
        assert!(
            catalog
                .snapshot(first.source_id)
                .unwrap()
                .text
                .ends_with("second")
        );
        assert_eq!(
            fs::read(temp.path().join("private/workspace.sqlite3")).unwrap(),
            b"private sentinel"
        );
        let db = Connection::open(temp.path().join("library/catalog.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM source_versions", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            21
        );
        db.execute("UPDATE source_snapshots SET reading_text='tampered' WHERE version_id=(SELECT MAX(id) FROM source_versions)", []).unwrap();
        drop(catalog);
        assert!(LibraryCatalog::open(&temp.path().join("library")).is_err());
    }
}
