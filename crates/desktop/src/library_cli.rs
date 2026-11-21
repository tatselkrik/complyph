//! Offline diagnostic entry point used to verify installed maintenance workflows.
use complyph_library::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{ffi::OsString, io::Read, path::Path, sync::atomic::AtomicBool};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    State {
        source_id: Option<i64>,
    },
    Metadata {
        source_id: i64,
        expected_sha256: String,
        details: ManualSourceInput,
        reviewer: LibraryReviewer,
    },
    Identity {
        source_id: i64,
        expected_sha256: String,
        decision: SourceIdentityDecision,
        reviewer: LibraryReviewer,
    },
    Prepare {
        change_id: i64,
        #[serde(default)]
        use_ocr: bool,
    },
    CompareCandidate {
        change_id: i64,
    },
    CompareVersions {
        source_id: i64,
        left_sha256: String,
        right_sha256: String,
    },
    Adopt {
        change_id: i64,
        review: AdoptionReview,
    },
    Dismiss {
        change_id: i64,
        expected_sha256: String,
        reviewer: LibraryReviewer,
    },
    Restore {
        source_id: i64,
        sha256: String,
        review: AdoptionReview,
    },
    Revalidate {
        rule_id: String,
        review: RuleRevalidation,
    },
    ApprovePublicationPage {
        url: String,
        domain: String,
    },
    DisablePublicationPage {
        page_id: i64,
    },
    ReviewCitation {
        link_id: i64,
        dismiss: bool,
        reviewer: LibraryReviewer,
    },
    PreviewPublication {
        url: String,
        file: std::path::PathBuf,
    },
}
fn state(catalog: &LibraryCatalog, id: Option<i64>) -> Result<Value, String> {
    let source = if let Some(id) = id {
        json!({
            "id":id,"metadata":catalog.display_metadata(id).map_err(|e|e.to_string())?,
            "versions":catalog.preserved_versions(id).map_err(|e|e.to_string())?,
            "history":catalog.source_history(id).map_err(|e|e.to_string())?,
            "downloads":catalog.downloaded_changes(id).map_err(|e|e.to_string())?,
        })
    } else {
        Value::Null
    };
    Ok(json!({
        "public_schema":5,"source":source,"sources":catalog.sources().map_err(|e|e.to_string())?,"summary":catalog.summary().map_err(|e|e.to_string())?,
        "rules":catalog.rule_library_states().map_err(|e|e.to_string())?,
        "publication_pages":catalog.publication_pages().map_err(|e|e.to_string())?,
        "discovered_issuances":catalog.discovered_issuances().map_err(|e|e.to_string())?,
    }))
}
fn execute(catalog: &LibraryCatalog, request: Request) -> Result<Value, String> {
    let result = match request {
        Request::State { source_id } => return state(catalog, source_id),
        Request::Metadata {
            source_id,
            expected_sha256,
            details,
            reviewer,
        } => catalog
            .edit_source_metadata(source_id, &expected_sha256, &details, &reviewer)
            .map(|id| json!({"event_id":id})),
        Request::Identity {
            source_id,
            expected_sha256,
            decision,
            reviewer,
        } => catalog
            .review_source_identity(source_id, &expected_sha256, decision, &reviewer)
            .map(|id| json!({"event_id":id})),
        Request::Prepare { change_id, use_ocr } => {
            let (change, raw) = catalog
                .downloaded_candidate_bytes(change_id)
                .map_err(|e| e.to_string())?;
            let file = crate::library_maintenance::prepare_public_reference(
                raw,
                &change.mime,
                use_ocr,
                &AtomicBool::new(false),
            )?;
            catalog.prepare_candidate_reading(change_id,file).map(|candidate|json!({"change_id":change_id,"text_sha256":candidate.text_sha256,"text_bytes":candidate.text.len(),"extraction":candidate.extraction}))
        }
        Request::CompareCandidate { change_id } => catalog
            .compare_candidate(change_id)
            .and_then(|result| serde_json::to_value(result).map_err(Into::into)),
        Request::CompareVersions {
            source_id,
            left_sha256,
            right_sha256,
        } => catalog
            .compare_versions(source_id, &left_sha256, &right_sha256)
            .and_then(|result| serde_json::to_value(result).map_err(Into::into)),
        Request::Adopt { change_id, review } => catalog
            .adopt_candidate(change_id, &review)
            .and_then(|metadata| serde_json::to_value(metadata).map_err(Into::into)),
        Request::Dismiss {
            change_id,
            expected_sha256,
            reviewer,
        } => catalog
            .dismiss_candidate(change_id, &expected_sha256, &reviewer)
            .map(|()| json!({"dismissed":true})),
        Request::Restore {
            source_id,
            sha256,
            review,
        } => catalog
            .restore_preserved_version(source_id, &sha256, &review)
            .map(|()| json!({"restored":true})),
        Request::Revalidate { rule_id, review } => catalog
            .revalidate_rule(&rule_id, &review)
            .map(|id| json!({"assessment_id":id})),
        Request::ApprovePublicationPage { url, domain } => catalog
            .approve_publication_page(&url, &domain)
            .map(|()| json!({"approved":true})),
        Request::DisablePublicationPage { page_id } => catalog
            .disable_publication_page(page_id)
            .map(|()| json!({"disabled":true})),
        Request::ReviewCitation {
            link_id,
            dismiss,
            reviewer,
        } => catalog
            .review_discovered_issuance(link_id, dismiss, &reviewer)
            .map(|()| json!({"recorded":true})),
        Request::PreviewPublication { url, file } => {
            let mut bytes = Vec::new();
            std::fs::File::open(file)
                .map_err(|e| e.to_string())?
                .take(MAX_PUBLIC_HTML_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            publication_links(&url, &bytes)
                .and_then(|result| serde_json::to_value(result).map_err(Into::into))
        }
    };
    result.map_err(|e| e.to_string())
}
pub fn run(args: Vec<OsString>) -> Result<(), String> {
    if args.len() != 6
        || args[0] != "--metadata"
        || args[2] != "--output"
        || args[4] != "--library-dir"
    {
        return Err("Use --library-action --metadata REQUEST-JSON --output NEW-JSON --library-dir PUBLIC-DIRECTORY.".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&args[1])
        .map_err(|e| e.to_string())?
        .take(32 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 {
        return Err("The maintenance request exceeds its 32 KiB limit.".into());
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[3])
        .map_err(|_| "Choose a new output filename; existing files are never overwritten.")?;
    let catalog = LibraryCatalog::open(Path::new(&args[5])).map_err(|e| e.to_string())?;
    crate::library_rules::register(&catalog)?;
    let result = execute(&catalog, request)?;
    serde_json::to_writer_pretty(output, &result).map_err(|e| e.to_string())
}
