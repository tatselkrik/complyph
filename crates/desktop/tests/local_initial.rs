use complyph_core::{ReviewActionRequest, ReviewStatus, SavedReview, Workspace, WorkspacePaths};
use std::{fs, path::Path, process::Command};
#[test]
fn executable_cross_domain_documents_keep_sources_scope_history_and_pdf_exports_local() {
    let temp = tempfile::tempdir().unwrap();
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let cases: serde_json::Value =
        serde_json::from_slice(&fs::read(examples.join("expected-initial.json")).unwrap()).unwrap();
    let paths = WorkspacePaths::under(temp.path().join("data"));
    let mut issue = None;
    for (i, (name, case)) in cases["cases"].as_object().unwrap().iter().enumerate() {
        let source = examples.join(name);
        let bytes = fs::read(&source).unwrap();
        let context = temp.path().join(format!("context-{i}.json"));
        fs::write(&context, serde_json::to_vec(&case["context"]).unwrap()).unwrap();
        let output = temp.path().join(format!("review-{i}.json"));
        assert!(
            Command::new(env!("CARGO_BIN_EXE_complyph"))
                .arg("--check-file")
                .arg(&source)
                .arg("--context")
                .arg(&context)
                .arg("--output")
                .arg(&output)
                .arg("--data-dir")
                .arg(&paths.root)
                .status()
                .unwrap()
                .success()
        );
        let saved: SavedReview = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(saved.review.engine, complyph_checks::ENGINE);
        assert_eq!(saved.review.findings.len(), 20);
        assert_eq!(
            serde_json::to_value(
                saved
                    .review
                    .findings
                    .iter()
                    .map(|f| f.outcome)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            case["outcomes"],
            "{name}"
        );
        for (finding, rule) in saved.review.findings[6..]
            .iter()
            .zip(complyph_checks::initial_rules())
        {
            assert_eq!(finding.source.as_ref().unwrap(), &rule.source);
            assert_eq!(finding.provision.as_ref().unwrap(), &rule.provision);
            assert_eq!(
                saved
                    .review
                    .context
                    .additional_checks
                    .get(&rule.id)
                    .map(|s| s.review_on.as_str())
                    .unwrap_or(""),
                case["context"]["additional_checks"][&rule.id]["review_on"]
                    .as_str()
                    .unwrap_or("")
            );
        }
        assert_eq!(fs::read(source).unwrap(), bytes);
        if name == "Synthetic seven-domain issues.docx" {
            issue = Some(saved);
        }
    }
    let saved = issue.unwrap();
    let workspace = Workspace::open(&paths).unwrap();
    let state = workspace.review_workflow(saved.id).unwrap();
    let before = serde_json::to_value(&state.saved.review).unwrap();
    workspace
        .append_review_action(
            saved.id,
            &ReviewActionRequest {
                review_fingerprint: state.review_fingerprint.clone(),
                rule_id: Some("PH-BIR-024-01".into()),
                expected_revision: 0,
                reviewer: "Fictional Reviewer".into(),
                note: "Synthetic review: verify taxable compensation definition and period.".into(),
                status: Some(ReviewStatus::Acknowledged),
            },
        )
        .unwrap();
    let state = workspace.review_workflow(saved.id).unwrap();
    assert_eq!(serde_json::to_value(&state.saved.review).unwrap(), before);
    drop(workspace);
    let pdf = temp.path().join("seven-domain-review.pdf");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_complyph"))
            .arg("--export-review-pdf")
            .arg(saved.id.to_string())
            .arg("--output")
            .arg(&pdf)
            .arg("--data-dir")
            .arg(&paths.root)
            .status()
            .unwrap()
            .success()
    );
    let reading = complyph_extract::extract(complyph_extract::Format::Pdf, &fs::read(pdf).unwrap());
    assert_eq!(reading.status, complyph_extract::Status::Ready);
    let text = reading
        .units
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for rule in complyph_checks::initial_rules() {
        for needle in [
            &rule.id,
            &rule.source.key,
            &rule.source.sha256,
            &rule.source.text_sha256,
        ] {
            assert!(text.contains(needle), "missing {needle}");
        }
    }
    for needle in [
        "2026-11-01",
        "Fictional Reviewer",
        "Acknowledged",
        "taxable compensation definition",
        "Synthetic assertions only",
    ] {
        assert!(text.contains(needle), "missing {needle}");
    }
}
