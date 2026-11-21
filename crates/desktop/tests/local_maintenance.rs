//! Exercise the installed executable's public maintenance and private review boundary.
use complyph_checks::Outcome;
use complyph_core::{SavedReview, Workspace, WorkspacePaths};
use complyph_library::{LibraryCatalog, UpdateReply};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Harness {
    temp: tempfile::TempDir,
    paths: WorkspacePaths,
    next: Cell<usize>,
    examples: PathBuf,
}
impl Harness {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let paths = WorkspacePaths::under(temp.path().join("data"));
        Self {
            temp,
            paths,
            next: Cell::new(0),
            examples: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples"),
        }
    }
    fn file(&self, label: &str) -> PathBuf {
        let id = self.next.get();
        self.next.set(id + 1);
        self.temp.path().join(format!("{id}-{label}"))
    }
    fn action(&self, request: Value, success: bool) -> Option<Value> {
        let input = self.file("request.json");
        let output = self.file("result.json");
        fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_complyph"))
            .arg("--library-action")
            .arg("--metadata")
            .arg(input)
            .arg("--output")
            .arg(&output)
            .arg("--library-dir")
            .arg(&self.paths.library_dir)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            success,
            "request={request} stderr={}",
            String::from_utf8_lossy(&result.stderr)
        );
        if success {
            Some(serde_json::from_slice(&fs::read(output).unwrap()).unwrap())
        } else {
            None
        }
    }
    fn state(&self) -> Value {
        self.action(json!({"action":"state","source_id":null}), true)
            .unwrap()
    }
    fn check(&self) -> SavedReview {
        let cases: Value =
            serde_json::from_slice(&fs::read(self.examples.join("expected-initial.json")).unwrap())
                .unwrap();
        let context = self.file("scope.json");
        let output = self.file("review.json");
        fs::write(
            &context,
            serde_json::to_vec(&cases["cases"]["Synthetic seven-domain baseline.docx"]["context"])
                .unwrap(),
        )
        .unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_complyph"))
            .arg("--check-file")
            .arg(self.examples.join("Synthetic seven-domain baseline.docx"))
            .arg("--context")
            .arg(context)
            .arg("--output")
            .arg(&output)
            .arg("--data-dir")
            .arg(&self.paths.root)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
    }
}
fn reviewer() -> Value {
    json!({"actor":"Synthetic library evaluator","review_on":"2026-11-21","basis":"Fictional test only. Compared complete provisions and amendments; the existing numeric profile is unchanged. This is not real legal certification."})
}
fn approval(hash: &str) -> Value {
    json!({"reviewer":reviewer(),"expected_active_sha256":hash,"identity_confirmed":true,"original_and_reading_compared":true,"effectivity_and_scope_reviewed":true})
}
fn deliver(catalog: &mut LibraryCatalog, id: i64, text: &str) -> i64 {
    let metadata = catalog.snapshot(id).unwrap().metadata;
    catalog
        .set_update_watch(id, &metadata.source_url, true)
        .unwrap();
    let target = catalog.claim_source_check(id, false).unwrap().unwrap();
    catalog
        .finish_source_check(
            &target,
            UpdateReply::Download {
                resolved_url: metadata.source_url,
                mime: "text/plain".into(),
                etag: None,
                last_modified: None,
                bytes: text.as_bytes().to_vec(),
            },
        )
        .unwrap();
    catalog.downloaded_changes(id).unwrap()[0].id
}

#[test]
fn executable_maintenance_holds_affected_rules_preserves_old_reports_and_cites_revalidated_versions()
 {
    let h = Harness::new();
    let original = h.examples.join("Synthetic seven-domain baseline.docx");
    let original_bytes = fs::read(&original).unwrap();
    let first = h.check();
    assert_eq!(first.review.findings.len(), 20);
    let first_value = serde_json::to_value(&first.review).unwrap();
    let mut catalog = LibraryCatalog::open(&h.paths.library_dir).unwrap();
    let sss = catalog
        .sources()
        .unwrap()
        .into_iter()
        .find(|s| s.metadata.key == "ra-11199")
        .unwrap();
    let source = catalog.snapshot(sss.id).unwrap();
    let appended = format!(
        "{}\nSYNTHETIC editorial footer. NOT A LAW OR NEW ISSUANCE.",
        source.text
    );
    let change = deliver(&mut catalog, sss.id, &appended);
    drop(catalog);
    let held = h.check();
    for (before, after) in first.review.findings.iter().zip(&held.review.findings) {
        if before.rule_id.starts_with("PH-SSS-") {
            assert_eq!(after.outcome, Outcome::NeedsReview);
            assert!(after.library_review.is_some());
        } else {
            assert_eq!(before.outcome, after.outcome, "{}", before.rule_id);
        }
    }
    let private_before = fs::read(&h.paths.database).unwrap();
    h.action(
        json!({"action":"prepare","change_id":change,"use_ocr":false}),
        true,
    );
    let diff = h
        .action(
            json!({"action":"compare_candidate","change_id":change}),
            true,
        )
        .unwrap();
    assert!(diff["added_lines"].as_u64().unwrap() > 0);
    assert_eq!(diff["removed_lines"], 0);
    let mut denied = approval(&source.metadata.raw_sha256);
    denied["identity_confirmed"] = json!(false);
    h.action(
        json!({"action":"adopt","change_id":change,"review":denied}),
        false,
    );
    let adopted=h.action(json!({"action":"adopt","change_id":change,"review":approval(&source.metadata.raw_sha256)}),true).unwrap();
    let new_hash = adopted["raw_sha256"].as_str().unwrap().to_string();
    let states = h.state();
    let affected = states["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["dependency"]["source_key"] == "ra-11199")
        .collect::<Vec<_>>();
    assert_eq!(affected.len(), 3);
    for state in &affected {
        assert_eq!(state["status"], "revalidation_required");
        let request = json!({"action":"revalidate","rule_id":state["dependency"]["rule_id"],"review":{
            "expected_token":state["token"],"unchanged_profile":true,"applicability_and_amendments_reviewed":true,
            "test_reference":"ComplyPH independently expected SSS 15%/10%/5% boundary fixtures and seven-domain baseline: passed.",
            "reviewer":reviewer()
        }});
        h.action(request, true);
    }
    assert!(
        fs::read(&h.paths.database).unwrap() == private_before,
        "Public maintenance changed the private document database."
    );
    let revalidated = h.check();
    for finding in &revalidated.review.findings {
        let before = first
            .review
            .findings
            .iter()
            .find(|f| f.rule_id == finding.rule_id)
            .unwrap();
        assert_eq!(finding.outcome, before.outcome, "{}", finding.rule_id);
        if finding.rule_id.starts_with("PH-SSS-") {
            assert_eq!(finding.source.as_ref().unwrap().sha256, new_hash);
            assert!(
                finding
                    .library_review
                    .as_ref()
                    .unwrap()
                    .assessment_id
                    .is_some()
            );
        }
    }
    let workspace = Workspace::open(&h.paths).unwrap();
    assert_eq!(
        serde_json::to_value(workspace.review_workflow(first.id).unwrap().saved.review).unwrap(),
        first_value
    );
    drop(workspace);
    let pdf = h.file("revalidated-report.pdf");
    let result = Command::new(env!("CARGO_BIN_EXE_complyph"))
        .arg("--export-review-pdf")
        .arg(revalidated.id.to_string())
        .arg("--output")
        .arg(&pdf)
        .arg("--data-dir")
        .arg(&h.paths.root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reading = complyph_extract::extract(complyph_extract::Format::Pdf, &fs::read(pdf).unwrap());
    assert_eq!(reading.status, complyph_extract::Status::Ready);
    let text = reading
        .units
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for needle in [
        &new_hash,
        "Synthetic library evaluator",
        "Library decision token",
        "SSS 15%/10%/5%",
    ] {
        assert!(text.contains(needle), "missing {needle}");
    }

    let metadata = LibraryCatalog::open(&h.paths.library_dir)
        .unwrap()
        .display_metadata(sss.id)
        .unwrap();
    h.action(json!({"action":"metadata","source_id":sss.id,"expected_sha256":new_hash,"reviewer":reviewer(),"details":{
        "title":"SYNTHETIC reviewed SSS display annotation","instrument":metadata.instrument,"issuer":metadata.issuer,
        "official_url":metadata.source_url,"domains":metadata.domains,"applicability_note":"Synthetic context revision: revalidation required.","effective_from":"2026-11-02"
    }}),true);
    let stale = &affected[0];
    h.action(json!({"action":"revalidate","rule_id":stale["dependency"]["rule_id"],"review":{
        "expected_token":stale["token"],"unchanged_profile":true,"applicability_and_amendments_reviewed":true,
        "test_reference":"Synthetic stale request must be rejected.","reviewer":reviewer()
    }}),false);
    let period_states = h.state();
    for state in period_states["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["dependency"]["source_key"] == "ra-11199")
    {
        h.action(json!({"action":"revalidate","rule_id":state["dependency"]["rule_id"],"review":{
            "expected_token":state["token"],"unchanged_profile":true,"applicability_and_amendments_reviewed":true,
            "test_reference":"Synthetic unchanged source profile with a separately recorded period boundary.","reviewer":reviewer()
        }}),true);
    }
    let earlier_period = h.check();
    for finding in earlier_period
        .review
        .findings
        .iter()
        .filter(|f| f.rule_id.starts_with("PH-SSS-"))
    {
        assert_eq!(finding.outcome, Outcome::NeedsReview);
        assert_eq!(
            finding.library_review.as_ref().unwrap().status,
            "Source effective after document period"
        );
    }
    let mut catalog = LibraryCatalog::open(&h.paths.library_dir).unwrap();
    let active = catalog.snapshot(sss.id).unwrap();
    let rule = complyph_checks::initial_rules()
        .iter()
        .find(|r| r.id == "PH-SSS-TOTAL-01")
        .unwrap();
    let altered = active.text.replacen(
        &rule.provision.start,
        "SYNTHETIC ALTERED SUPPORTING PROVISION — NOT A REAL LAW",
        1,
    );
    assert!(altered != active.text);
    let changed = deliver(&mut catalog, sss.id, &altered);
    drop(catalog);
    h.action(json!({"action":"prepare","change_id":changed}), true);
    h.action(
        json!({"action":"adopt","change_id":changed,"review":approval(&new_hash)}),
        true,
    );
    let current = h.state();
    for state in current["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["dependency"]["source_key"] == "ra-11199")
    {
        assert_eq!(state["status"], "update_required");
        h.action(json!({"action":"revalidate","rule_id":state["dependency"]["rule_id"],"review":{
            "expected_token":state["token"],"unchanged_profile":true,"applicability_and_amendments_reviewed":true,
            "test_reference":"A checkbox cannot authorize altered supporting provisions.","reviewer":reviewer()
        }}),false);
    }
    let workspace = Workspace::open(&h.paths).unwrap();
    assert_eq!(
        serde_json::to_value(workspace.review_workflow(first.id).unwrap().saved.review).unwrap(),
        first_value
    );
    assert_eq!(fs::read(original).unwrap(), original_bytes);
    let catalog = LibraryCatalog::open(&h.paths.library_dir).unwrap();
    assert_eq!(
        catalog
            .snapshot_for_review(
                &source.metadata.key,
                &source.metadata.raw_sha256,
                &source.metadata.text_sha256
            )
            .unwrap()
            .raw,
        source.raw
    );
    assert_eq!(catalog.preserved_versions(sss.id).unwrap().len(), 3);
}

#[test]
fn executable_prepares_real_pdf_candidates_and_discovery_actions_remain_public_only() {
    let h = Harness::new();
    let mut catalog = LibraryCatalog::open(&h.paths.library_dir).unwrap();
    let source = catalog
        .sources()
        .unwrap()
        .into_iter()
        .find(|s| s.metadata.key == "dole-labor-code-2022")
        .or_else(|| {
            catalog
                .sources()
                .unwrap()
                .into_iter()
                .find(|s| s.metadata.key == "labor-code-reference")
        })
        .unwrap();
    let raw = fs::read(h.examples.join("Synthetic employment.pdf")).unwrap();
    catalog
        .set_update_watch(source.id, &source.metadata.source_url, true)
        .unwrap();
    let target = catalog
        .claim_source_check(source.id, false)
        .unwrap()
        .unwrap();
    catalog
        .finish_source_check(
            &target,
            UpdateReply::Download {
                resolved_url: source.metadata.source_url,
                mime: "application/pdf".into(),
                etag: None,
                last_modified: None,
                bytes: raw.clone(),
            },
        )
        .unwrap();
    let change = catalog.downloaded_changes(source.id).unwrap()[0].id;
    h.action(json!({"action":"prepare","change_id":change}), true);
    let prepared = catalog.prepared_candidate(change).unwrap();
    assert!(prepared.text.contains("PAGE-TWO-TERMS"));
    assert_eq!(prepared.pages, Some(3));
    assert_eq!(catalog.downloaded_candidate_bytes(change).unwrap().1, raw);
    let page = "https://privacy.gov.ph/complyph-fixtures/listing/";
    h.action(
        json!({"action":"approve_publication_page","url":page,"domain":"ph.privacy"}),
        true,
    );
    let page_id = catalog.publication_pages().unwrap()[0].id;
    let target = catalog
        .claim_publication_page(page_id, false)
        .unwrap()
        .unwrap();
    catalog
        .finish_publication_page(
            &target,
            UpdateReply::Download {
                resolved_url: page.into(),
                mime: "text/html".into(),
                etag: None,
                last_modified: None,
                bytes: fs::read(h.examples.join("Synthetic publication listing.html")).unwrap(),
            },
        )
        .unwrap();
    let state = h.state();
    assert_eq!(state["discovered_issuances"].as_array().unwrap().len(), 3);
    assert_eq!(state["summary"]["source_count"], 19);
    assert!(
        !h.paths.database.exists(),
        "Public library actions opened a private workspace."
    );
    let link = &state["discovered_issuances"][0];
    h.action(json!({"action":"review_citation","link_id":link["id"],"dismiss":true,"reviewer":reviewer()}),true);
    assert!(
        h.state()["discovered_issuances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["id"] == link["id"])
            .unwrap()["dismissed"]
            .as_bool()
            .unwrap()
    );
    h.action(
        json!({"action":"disable_publication_page","page_id":page_id}),
        true,
    );
    assert!(!catalog.publication_pages().unwrap()[0].enabled);
    assert_eq!(
        catalog.publication_page_original(page_id).unwrap(),
        fs::read(h.examples.join("Synthetic publication listing.html")).unwrap()
    );
}
