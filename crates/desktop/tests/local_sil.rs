use complyph_checks::{Context, Outcome};
use complyph_core::{SavedReview, Workspace, WorkspacePaths};
use std::{fs, path::Path, process::Command};

#[test]
fn real_pdf_word_sil_cases_preserve_scope_locations_originals_and_exported_basis() {
    let temp = tempfile::tempdir().unwrap();
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let cases: serde_json::Value =
        serde_json::from_slice(&fs::read(examples.join("expected-sil.json")).unwrap()).unwrap();
    let paths = WorkspacePaths::under(temp.path().join("data"));
    let mut issue = None;
    for (index, (name, case)) in cases.as_object().unwrap().iter().enumerate() {
        let source = examples.join(name);
        let before = fs::read(&source).unwrap();
        let context_file = temp.path().join(format!("context-{index}.json"));
        fs::write(&context_file, serde_json::to_vec(&case["context"]).unwrap()).unwrap();
        let output = temp.path().join(format!("review-{index}.json"));
        let status = Command::new(env!("CARGO_BIN_EXE_complyph"))
            .arg("--check-file")
            .arg(&source)
            .arg("--context")
            .arg(&context_file)
            .arg("--output")
            .arg(&output)
            .arg("--data-dir")
            .arg(&paths.root)
            .status()
            .unwrap();
        assert!(status.success(), "{name}");
        let saved: SavedReview = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        let expected: Outcome = serde_json::from_value(case["outcome"].clone()).unwrap();
        assert_eq!(saved.review.engine, complyph_checks::ENGINE);
        assert_eq!(saved.review.findings.len(), complyph_checks::RULE_COUNT);
        assert!(
            saved.review.findings[..5]
                .iter()
                .all(|f| f.outcome == Outcome::NeedsContext)
        );
        let finding = &saved.review.findings[5];
        assert_eq!(finding.rule_id, "PH-LAB-095-01");
        assert_eq!(finding.outcome, expected, "{name}");
        let context: Context = serde_json::from_value(case["context"].clone()).unwrap();
        assert_eq!(saved.review.context, context);
        assert_eq!(fs::read(source).unwrap(), before);
        assert_eq!(saved.review.source_sha256, complyph_checks::SOURCE_SHA256);
        if matches!(
            expected,
            Outcome::PotentialIssue | Outcome::StatedTermMeetsMinimum
        ) {
            assert_eq!(finding.evidence.len(), 1);
            assert!(matches!(
                finding.evidence[0].location,
                complyph_extract::Location::PdfPage { page: 1 }
                    | complyph_extract::Location::WordParagraph { paragraph: 3, .. }
            ));
            assert!(finding.evidence[0].text.contains("service incentive leave"));
        }
        if expected == Outcome::PotentialIssue {
            issue = Some(saved);
        }
    }
    // Reopen persisted state and exercise the actual PDF export entry point.
    let saved = issue.unwrap();
    let workspace = Workspace::open(&paths).unwrap();
    let state = workspace.review_workflow(saved.id).unwrap();
    assert_eq!(state.saved.review.context, saved.review.context);
    assert_eq!(
        state.saved.review.findings[5].outcome,
        Outcome::PotentialIssue
    );
    drop(workspace);
    let pdf = temp.path().join("SIL review.pdf");
    let result = Command::new(env!("CARGO_BIN_EXE_complyph"))
        .arg("--export-review-pdf")
        .arg(saved.id.to_string())
        .arg("--output")
        .arg(&pdf)
        .arg("--data-dir")
        .arg(&paths.root)
        .status()
        .unwrap();
    assert!(result.success());
    let report = complyph_extract::extract(complyph_extract::Format::Pdf, &fs::read(pdf).unwrap());
    assert_eq!(report.status, complyph_extract::Status::Ready);
    let text = report
        .units
        .iter()
        .map(|unit| unit.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "PH-LAB-095-01",
        "Article 95",
        "Potential issue",
        "five days with pay",
        "At least one qualifying year confirmed",
        "regularly employs 10 workers",
    ] {
        assert!(text.contains(needle), "Missing {needle} from {text}");
    }
}
