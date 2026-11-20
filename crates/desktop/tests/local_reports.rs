use complyph_core::{
    ReviewActionRequest, ReviewStatus, ReviewWorkflow, SavedReview, Workspace, WorkspacePaths,
};
use std::{fs, path::Path, process::Command};

fn run(arguments: &[&str], data: &Path) -> bool {
    Command::new(env!("CARGO_BIN_EXE_complyph"))
        .args(arguments)
        .arg("--data-dir")
        .arg(data)
        .status()
        .unwrap()
        .success()
}
fn read(path: &Path) -> ReviewWorkflow {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn executable_tracks_human_actions_and_exports_historical_pdf_without_changing_findings_or_originals()
 {
    let temp = tempfile::tempdir().unwrap();
    let paths = WorkspacePaths::under(temp.path().join("data"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let original = temp.path().join("Fictional José policy.docx");
    fs::copy(root.join("Synthetic labour issues.docx"), &original).unwrap();
    let before = fs::read(&original).unwrap();
    let context = temp.path().join("context.json");
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("expected-checks.json")).unwrap()).unwrap();
    fs::write(&context, serde_json::to_vec(&expected["context"]).unwrap()).unwrap();
    let report = temp.path().join("original-review.json");
    assert!(run(
        &[
            "--check-file",
            original.to_str().unwrap(),
            "--context",
            context.to_str().unwrap(),
            "--output",
            report.to_str().unwrap()
        ],
        &paths.root
    ));
    let saved: SavedReview = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    let id = saved.id.to_string();
    let state_file = temp.path().join("state.json");
    assert!(run(
        &[
            "--review-state",
            &id,
            "--output",
            state_file.to_str().unwrap()
        ],
        &paths.root
    ));
    let mut state = read(&state_file);
    assert!(state.events.is_empty());
    let metadata = temp.path().join("action.json");
    for (index, (rule, status, note)) in [
        (None, None, "Fictional ₱500 review note."),
        (
            Some("PH-LAB-083-01"),
            Some(ReviewStatus::Acknowledged),
            "Assigned for follow-up.",
        ),
        (
            Some("PH-LAB-083-01"),
            Some(ReviewStatus::Resolved),
            "Follow-up complete; original issue preserved.",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let request = ReviewActionRequest {
            review_fingerprint: state.review_fingerprint.clone(),
            rule_id: rule.map(str::to_owned),
            expected_revision: state.revision(rule),
            reviewer: "José Example".into(),
            note: note.into(),
            status,
        };
        fs::write(&metadata, serde_json::to_vec_pretty(&request).unwrap()).unwrap();
        let output = temp.path().join(format!("event-{index}.json"));
        assert!(run(
            &[
                "--review-action",
                &id,
                "--metadata",
                metadata.to_str().unwrap(),
                "--output",
                output.to_str().unwrap()
            ],
            &paths.root
        ));
        state = read(&output);
        assert_eq!(state.events.len(), index + 1);
        // An existing output must be refused before any new history entry is recorded.
        assert!(!run(
            &[
                "--review-action",
                &id,
                "--metadata",
                metadata.to_str().unwrap(),
                "--output",
                output.to_str().unwrap()
            ],
            &paths.root
        ));
    }
    assert_eq!(state.status("PH-LAB-083-01"), ReviewStatus::Resolved);
    let stale = temp.path().join("stale.json");
    assert!(!run(
        &[
            "--review-action",
            &id,
            "--metadata",
            metadata.to_str().unwrap(),
            "--output",
            stale.to_str().unwrap()
        ],
        &paths.root
    ));
    let workspace = Workspace::open(&paths).unwrap();
    assert_eq!(workspace.review_workflow(saved.id).unwrap().events.len(), 3);
    assert_eq!(
        serde_json::to_value(workspace.review_workflow(saved.id).unwrap().saved).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    assert_eq!(fs::read(&original).unwrap(), before);
    let pdf = temp.path().join("review.pdf");
    assert!(!run(
        &[
            "--export-review-pdf",
            &id,
            "--output",
            original.to_str().unwrap()
        ],
        &paths.root
    ));
    assert_eq!(fs::read(&original).unwrap(), before);
    fs::remove_file(&original).unwrap();
    assert!(run(
        &[
            "--export-review-pdf",
            &id,
            "--output",
            pdf.to_str().unwrap()
        ],
        &paths.root
    ));
    let bytes = fs::read(&pdf).unwrap();
    assert!(!run(
        &[
            "--export-review-pdf",
            &id,
            "--output",
            pdf.to_str().unwrap()
        ],
        &paths.root
    ));
    assert_eq!(fs::read(&pdf).unwrap(), bytes);
    let read_pdf = temp.path().join("pdf-reading.json");
    assert!(run(
        &[
            "--read-file",
            pdf.to_str().unwrap(),
            "--output",
            read_pdf.to_str().unwrap()
        ],
        &paths.root
    ));
    let extracted: complyph_core::SavedExtraction =
        serde_json::from_slice(&fs::read(read_pdf).unwrap()).unwrap();
    assert_eq!(extracted.report.status, complyph_extract::Status::Ready);
    let text = extracted
        .report
        .units
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "José",
        "₱500",
        "Acknowledged to Resolved",
        "Automated result: Potential issue",
        "Human follow-up status: Resolved",
        "PH-LAB-083-01",
        "PH-LAB-087-01",
        "PH-LAB-086-01",
        "PH-LAB-085-01",
        "PH-LAB-091-01",
        "complyph-pdf-1",
    ] {
        assert!(text.contains(needle), "Missing {needle} from exported PDF");
    }
}
