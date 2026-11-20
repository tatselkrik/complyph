use complyph_core::{SavedExtraction, SavedReview, Workspace, WorkspacePaths};
use std::{fs, path::Path, process::Command};
fn execute(arguments: &[&str], data: &Path) -> bool {
    Command::new(env!("CARGO_BIN_EXE_complyph"))
        .args(arguments)
        .arg("--data-dir")
        .arg(data)
        .status()
        .unwrap()
        .success()
}
#[test]
fn real_ocr_preserves_pages_originals_cache_and_requires_separate_accuracy_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let paths = WorkspacePaths::under(temp.path().join("data"));
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let cases: serde_json::Value =
        serde_json::from_slice(&fs::read(examples.join("expected-ocr.json")).unwrap()).unwrap();
    for (index, (name, expected)) in cases.as_object().unwrap().iter().enumerate() {
        let original = examples.join(name);
        let before = fs::read(&original).unwrap();
        let output = temp.path().join(format!("ocr-{index}.json"));
        assert!(execute(
            &[
                "--ocr-file",
                original.to_str().unwrap(),
                "--output",
                output.to_str().unwrap()
            ],
            &paths.root
        ));
        let saved: SavedExtraction = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(
            saved.report.status,
            complyph_extract::Status::Ready,
            "{name}: {}",
            saved.report.summary
        );
        assert!(complyph_extract::valid_ocr(&saved.report));
        assert_eq!(
            saved.report.units.len(),
            expected["pages"].as_u64().unwrap() as usize
        );
        let text = saved
            .report
            .units
            .iter()
            .map(|u| u.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for needle in expected["contains"].as_array().unwrap() {
            assert!(
                normalized.contains(needle.as_str().unwrap()),
                "{name}: missing {needle} from {normalized}"
            );
        }
        assert_eq!(fs::read(&original).unwrap(), before);
    }
    let original = examples.join("Synthetic OCR policy.pdf");
    let mut context: serde_json::Value =
        serde_json::from_slice(&fs::read(examples.join("expected-checks.json")).unwrap()).unwrap();
    let mut context = context["context"].take();
    let file = temp.path().join("context.json");
    for confirmed in [false, true] {
        context["ocr_text_confirmed"] = confirmed.into();
        fs::write(&file, serde_json::to_vec(&context).unwrap()).unwrap();
        let output = temp.path().join(format!("confirmed-{confirmed}.json"));
        assert!(execute(
            &[
                "--check-file",
                original.to_str().unwrap(),
                "--context",
                file.to_str().unwrap(),
                "--output",
                output.to_str().unwrap()
            ],
            &paths.root
        ));
        let saved: SavedReview = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert!(saved.review.ocr.is_some());
        assert!(saved.review.findings[..5].iter().all(|f| f.outcome
            == if confirmed {
                complyph_checks::Outcome::PotentialIssue
            } else {
                complyph_checks::Outcome::NeedsReview
            }));
    }
    // Old five-rule scope never establishes eligibility for the added SIL comparison.
    let last: SavedReview =
        serde_json::from_slice(&fs::read(temp.path().join("confirmed-true.json")).unwrap())
            .unwrap();
    assert_eq!(
        last.review.findings[5].outcome,
        complyph_checks::Outcome::NeedsContext
    );
    // Non-OCR reopening reuses the recognized cache without returning to blank text.
    let output = temp.path().join("cached.json");
    assert!(execute(
        &[
            "--read-file",
            original.to_str().unwrap(),
            "--output",
            output.to_str().unwrap()
        ],
        &paths.root
    ));
    let cached: SavedExtraction = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(cached.report.engine, complyph_extract::OCR_ENGINE);
    let protected = examples.join("Synthetic protected.pdf");
    assert!(!execute(
        &[
            "--ocr-file",
            protected.to_str().unwrap(),
            "--output",
            temp.path().join("protected.json").to_str().unwrap()
        ],
        &paths.root
    ));
}
#[test]
fn executable_watched_intake_waits_rejects_partial_files_and_does_not_create_reviews() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("intake");
    fs::create_dir(&root).unwrap();
    let paths = WorkspacePaths::under(temp.path().join("data"));
    let bytes = include_bytes!("../../../examples/Synthetic employment.pdf");
    fs::write(root.join("Complete.pdf"), bytes).unwrap();
    fs::write(root.join("Duplicate.pdf"), bytes).unwrap();
    fs::write(root.join("Partial.pdf"), &bytes[..bytes.len() / 2]).unwrap();
    for i in 0..2 {
        let output = temp.path().join(format!("watch-{i}.json"));
        assert!(execute(
            &[
                "--watch-folder",
                root.to_str().unwrap(),
                "--output",
                output.to_str().unwrap()
            ],
            &paths.root
        ));
        let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(
            result["scan"]["added"].as_array().unwrap().len(),
            usize::from(i == 0)
        );
        assert_eq!(
            result["scan"]["duplicates"].as_u64().unwrap(),
            u64::from(i == 0)
        );
        let workspace = Workspace::open(&paths).unwrap();
        assert_eq!(workspace.documents().unwrap().len(), 1);
        assert!(workspace.reviews().unwrap().is_empty());
        assert!(
            workspace
                .extraction(workspace.documents().unwrap()[0].id)
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(fs::read(root.join("Complete.pdf")).unwrap(), bytes);
}
