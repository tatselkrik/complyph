use complyph_core::{document_comparison::*, document_history::*, *};
use std::fs;

fn setup() -> (tempfile::TempDir, DocumentComparison) {
    let temp = tempfile::tempdir().unwrap();
    let w = Workspace::open(&WorkspacePaths::under(temp.path().join("data"))).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/expected-revisions.json")).unwrap();
    let context: complyph_checks::Context =
        serde_json::from_value(expected["context"].clone()).unwrap();
    let mut readings = vec![];
    let mut docs = vec![];
    let mut reviews = vec![];
    for (name, bytes) in [
        (
            "Earlier.docx",
            include_bytes!("../../../examples/Synthetic revision original.docx").as_slice(),
        ),
        (
            "Revised.docx",
            include_bytes!("../../../examples/Synthetic revision corrected.docx").as_slice(),
        ),
    ] {
        let path = temp.path().join(name);
        fs::write(&path, bytes).unwrap();
        let doc = w.register_document(&path).unwrap();
        let snapshot = document_snapshot(&path).unwrap();
        let extraction = w
            .save_extraction(
                &doc,
                &snapshot,
                complyph_extract::extract(complyph_extract::Format::Docx, bytes),
            )
            .unwrap();
        let review = w
            .save_review_for_reading(
                doc.id,
                &extraction,
                complyph_checks::review(&extraction.report, &extraction.sha256, context.clone()),
            )
            .unwrap();
        let reading = w.reading_for_review(review.id).unwrap().unwrap();
        readings.push(reading.id);
        docs.push(doc.id);
        reviews.push(review.id);
    }
    w.link_document_revision(&RevisionLinkRequest {
        document_id: docs[0],
        revised_document_id: docs[1],
        expected_group_id: w.document_history(docs[0]).unwrap().group_id,
        expected_revision_group_id: w.document_history(docs[1]).unwrap().group_id,
    })
    .unwrap();
    for (rule, status, note) in [
        (
            None,
            None,
            "Earlier note: fictional José and PHP 12345.67; preserve this full text.",
        ),
        (
            Some("PH-LAB-083-01"),
            Some(ReviewStatus::Resolved),
            "Earlier follow-up resolved; never transfer this status.",
        ),
    ] {
        let workflow = w.review_workflow(reviews[0]).unwrap();
        w.append_review_action(
            reviews[0],
            &ReviewActionRequest {
                review_fingerprint: workflow.review_fingerprint.clone(),
                expected_revision: workflow.revision(rule),
                rule_id: rule.map(str::to_owned),
                reviewer: "José Example".into(),
                note: note.into(),
                status,
            },
        )
        .unwrap();
    }
    let comparison = w
        .compare_document_versions(&CompareRequest {
            left_reading_id: readings[0],
            right_reading_id: readings[1],
            left_review_id: Some(reviews[0]),
            right_review_id: Some(reviews[1]),
        })
        .unwrap();
    (temp, comparison)
}
#[test]
fn comparison_pdf_and_json_preserve_both_readings_outcomes_and_complete_separate_histories() {
    let (temp, comparison) = setup();
    let pdf = temp.path().join("comparison.pdf");
    let json = temp.path().join("comparison.json");
    complyph_reports::export_comparison_pdf(&comparison, &pdf).unwrap();
    complyph_reports::export_comparison_json(&comparison, &json).unwrap();
    let text = pdf_extract::extract_text_from_mem(&fs::read(&pdf).unwrap()).unwrap();
    assert_eq!(
        comparison
            .findings
            .iter()
            .filter(|f| f.change == FindingChange::NoLongerFlagged)
            .count(),
        5,
        "{:?}",
        comparison.findings.iter().take(6).collect::<Vec<_>>()
    );
    for phrase in [
        "Document version comparison",
        "Earlier saved review",
        "Revised saved review",
        "Numeric issue no longer flagged",
        "Resolved",
        "Unresolved",
        "Earlier follow-up resolved; never transfer this status.",
    ] {
        assert!(text.contains(phrase), "missing {phrase}");
    }
    for side in [&comparison.left, &comparison.right] {
        assert!(text.contains(&side.reading.saved.sha256));
        assert!(text.contains(&side.reading.reading_sha256));
        for finding in &side.review.as_ref().unwrap().workflow.saved.review.findings {
            assert!(text.contains(&finding.rule_id));
            assert!(text.contains(finding.outcome.label()));
        }
    }
    let parsed: DocumentComparison = serde_json::from_slice(&fs::read(&json).unwrap()).unwrap();
    assert_eq!(
        parsed.left.reading.saved.report,
        comparison.left.reading.saved.report
    );
    assert_eq!(
        parsed.right.reading.saved.report,
        comparison.right.reading.saved.report
    );
    assert_eq!(
        parsed.left.review.as_ref().unwrap().workflow.events.len(),
        2
    );
    assert!(
        parsed
            .right
            .review
            .as_ref()
            .unwrap()
            .workflow
            .events
            .is_empty()
    );
    assert!(
        parsed
            .findings
            .iter()
            .filter(|f| f.change == FindingChange::NoLongerFlagged)
            .count()
            == 5
    );
    assert!(
        !String::from_utf8(fs::read(json).unwrap())
            .unwrap()
            .contains(temp.path().to_str().unwrap())
    );
    let doc = lopdf::Document::load(&pdf).unwrap();
    assert!(doc.get_pages().len() > 1);
    assert!(!doc.objects.values().any(|o| {
        o.as_dict()
            .is_ok_and(|d| d.has(b"OpenAction") || d.has(b"JavaScript") || d.has(b"AA"))
    }));
}
#[test]
fn comparison_exports_refuse_overwrite_and_unsupported_glyphs_without_partial_destination() {
    let (temp, mut comparison) = setup();
    let pdf = temp.path().join("existing.pdf");
    let json = temp.path().join("existing.json");
    fs::write(&pdf, b"existing PDF sentinel").unwrap();
    fs::write(&json, b"existing JSON sentinel").unwrap();
    assert!(complyph_reports::export_comparison_pdf(&comparison, &pdf).is_err());
    assert!(complyph_reports::export_comparison_json(&comparison, &json).is_err());
    assert_eq!(fs::read(&pdf).unwrap(), b"existing PDF sentinel");
    assert_eq!(fs::read(&json).unwrap(), b"existing JSON sentinel");
    comparison
        .limitation
        .push_str(" Unsupported font fixture: \u{1f980}");
    let unsupported = temp.path().join("unsupported.pdf");
    assert!(complyph_reports::export_comparison_pdf(&comparison, &unsupported).is_err());
    assert!(!unsupported.exists());
    let preserved = temp.path().join("complete.json");
    complyph_reports::export_comparison_json(&comparison, &preserved).unwrap();
    assert!(fs::read_to_string(preserved).unwrap().contains('\u{1f980}'));
}
