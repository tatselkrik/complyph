use complyph_checks::{Applicability, Context, Outcome};
use complyph_core::{document_comparison::*, document_history::*, *};
use complyph_extract::{Format, Location, Report, Status, TextUnit};
use std::{fs, path::Path};

const ISSUE: &str = "Normal daily working hours are 9 hours.\nOrdinary-day overtime premium is 20% of the regular hourly wage.\nNight shift differential is 5% of the regular hourly wage for hours worked between 22:00 and 06:00.\nRegular meal period is 50 minutes of time-off.\nWeekly rest period is 20 consecutive hours after every six consecutive normal work days.";
const FIXED: &str = "Normal daily working hours are 8 hours.\nOrdinary-day overtime premium is 25% of the regular hourly wage.\nNight shift differential is 10% of the regular hourly wage for hours worked between 22:00 and 06:00.\nRegular meal period is 60 minutes of time-off.\nWeekly rest period is 24 consecutive hours after every six consecutive normal work days.";
fn setup() -> (tempfile::TempDir, WorkspacePaths, Workspace) {
    let temp = tempfile::tempdir().unwrap();
    let paths = WorkspacePaths::under(temp.path().join("data"));
    let workspace = Workspace::open(&paths).unwrap();
    (temp, paths, workspace)
}
fn context() -> Context {
    Context {
        operative_terms_confirmed: true,
        normal_hours: Applicability::Covered,
        ordinary_overtime: Applicability::Covered,
        night_differential: Applicability::Covered,
        meal_period: Applicability::Covered,
        weekly_rest: Applicability::Covered,
        ..Default::default()
    }
}
fn report(text: &str) -> Report {
    Report {
        reading: None,
        engine: complyph_extract::ENGINE.into(),
        format: Format::Docx,
        status: Status::Ready,
        summary: "Fictional retained-reading unit fixture; no real people.".into(),
        units: text
            .lines()
            .enumerate()
            .map(|(i, s)| TextUnit {
                location: Location::WordParagraph {
                    part: "word/document.xml".into(),
                    paragraph: i + 1,
                    table_cell: None,
                },
                text: s.into(),
            })
            .collect(),
        warnings: vec![],
        ocr: None,
    }
}
fn capture(workspace: &Workspace, path: &Path, text: &str) -> (Document, SavedExtraction, i64) {
    fs::write(path, format!("SYNTHETIC RAW FIXTURE\n{text}")).unwrap();
    let doc = workspace.register_document(path).unwrap();
    let saved = workspace
        .save_extraction(&doc, &document_snapshot(path).unwrap(), report(text))
        .unwrap();
    let id = workspace
        .document_history(doc.id)
        .unwrap()
        .versions
        .last()
        .unwrap()
        .readings
        .last()
        .unwrap()
        .id;
    (doc, saved, id)
}
fn review(
    workspace: &Workspace,
    doc: &Document,
    reading: &SavedExtraction,
    context: Context,
) -> SavedReview {
    workspace
        .save_review_for_reading(
            doc.id,
            reading,
            complyph_checks::review(&reading.report, &reading.sha256, context),
        )
        .unwrap()
}
fn link(workspace: &Workspace, a: i64, b: i64) {
    workspace
        .link_document_revision(&RevisionLinkRequest {
            document_id: a,
            revised_document_id: b,
            expected_group_id: workspace.document_history(a).unwrap().group_id,
            expected_revision_group_id: workspace.document_history(b).unwrap().group_id,
        })
        .unwrap();
}
fn pair(
    workspace: &Workspace,
    root: &Path,
) -> (
    Document,
    SavedExtraction,
    i64,
    Document,
    SavedExtraction,
    i64,
) {
    let (a, ar, ai) = capture(workspace, &root.join("Earlier.docx"), ISSUE);
    let (b, br, bi) = capture(workspace, &root.join("Revised.docx"), FIXED);
    link(workspace, a.id, b.id);
    (a, ar, ai, b, br, bi)
}
#[test]
fn readings_survive_overwritten_moved_originals_and_cache_replacement() {
    let (temp, _paths, w) = setup();
    let path = temp.path().join("Policy.docx");
    let (doc, first, first_id) = capture(&w, &path, ISSUE);
    let (_, second, second_id) = capture(&w, &path, FIXED);
    assert_ne!(first.sha256, second.sha256);
    assert_ne!(first_id, second_id);
    assert_eq!(w.document_history(doc.id).unwrap().versions.len(), 2);
    let again = w
        .save_extraction(
            &doc,
            &document_snapshot(&path).unwrap(),
            second.report.clone(),
        )
        .unwrap();
    assert_eq!(
        w.document_history(doc.id).unwrap().versions[1]
            .readings
            .len(),
        1
    );
    assert_eq!(again.sha256, second.sha256);
    fs::rename(&path, temp.path().join("Moved.docx")).unwrap();
    assert_eq!(
        w.retained_reading(first_id).unwrap().saved.report,
        first.report
    );
    assert_eq!(
        w.retained_reading(second_id).unwrap().saved.report,
        second.report
    );
    let result = w
        .compare_document_versions(&CompareRequest {
            left_reading_id: first_id,
            right_reading_id: second_id,
            left_review_id: None,
            right_review_id: None,
        })
        .unwrap();
    assert_eq!(result.text.added_lines, Some(5));
    assert_eq!(result.text.removed_lines, Some(5));
    assert!(!result.same_file_bytes);
}
#[test]
fn links_are_explicit_reversible_and_reject_stale_or_multi_history_merges() {
    let (temp, _paths, w) = setup();
    let (a, _, ai) = capture(&w, &temp.path().join("A.docx"), ISSUE);
    let (b, _, bi) = capture(&w, &temp.path().join("B.docx"), FIXED);
    let request = CompareRequest {
        left_reading_id: ai,
        right_reading_id: bi,
        left_review_id: None,
        right_review_id: None,
    };
    assert!(w.compare_document_versions(&request).is_err());
    let old = w.document_history(b.id).unwrap().group_id;
    link(&w, a.id, b.id);
    assert!(w.compare_document_versions(&request).is_ok());
    assert!(w.unlink_document_revision(b.id, old).is_err());
    let (c, _, _) = capture(&w, &temp.path().join("C.docx"), FIXED);
    let bad = RevisionLinkRequest {
        document_id: c.id,
        revised_document_id: a.id,
        expected_group_id: w.document_history(c.id).unwrap().group_id,
        expected_revision_group_id: w.document_history(a.id).unwrap().group_id,
    };
    assert!(w.link_document_revision(&bad).is_err());
    w.unlink_document_revision(b.id, w.document_history(b.id).unwrap().group_id)
        .unwrap();
    assert!(w.compare_document_versions(&request).is_err());
    assert_eq!(w.retained_reading(ai).unwrap().document_id, a.id);
    assert_eq!(w.retained_reading(bi).unwrap().document_id, b.id);
    assert!(fs::read(&b.path).unwrap().starts_with(b"SYNTHETIC"));
}
#[test]
fn review_binds_its_actual_reading_instead_of_another_windows_newer_cache() {
    let (temp, _paths, w) = setup();
    let (doc, first, id) = capture(&w, &temp.path().join("Policy.docx"), ISSUE);
    let mut changed = first.report.clone();
    changed.summary = "Same bytes; different reader metadata.".into();
    w.save_extraction(&doc, &document_snapshot(&doc.path).unwrap(), changed)
        .unwrap();
    let saved = review(&w, &doc, &first, context());
    assert_eq!(w.reading_for_review(saved.id).unwrap().unwrap().id, id);
    let mut invalid = saved.review.clone();
    invalid.document_sha256 = "wrong".into();
    assert!(w.save_review_for_reading(doc.id, &first, invalid).is_err());
    assert_eq!(w.reviews().unwrap().len(), 1);
    let history = w.document_history(doc.id).unwrap();
    assert_eq!(history.versions.len(), 1);
    assert_eq!(history.versions[0].readings.len(), 2);
}
#[test]
fn numeric_corrections_never_transfer_old_resolutions_or_rewrite_report_json() {
    let (temp, paths, w) = setup();
    let (a, ar, ai, b, br, bi) = pair(&w, temp.path());
    let before = review(&w, &a, &ar, context());
    let old = w.review_workflow(before.id).unwrap();
    w.append_review_action(
        before.id,
        &ReviewActionRequest {
            review_fingerprint: old.review_fingerprint.clone(),
            rule_id: Some("PH-LAB-083-01".into()),
            expected_revision: 0,
            reviewer: "Synthetic reviewer".into(),
            note: "Fictional follow-up completed on the earlier version.".into(),
            status: Some(ReviewStatus::Resolved),
        },
    )
    .unwrap();
    let original_json: String = rusqlite::Connection::open(&paths.database)
        .unwrap()
        .query_row(
            "SELECT value FROM review_runs WHERE id=?1",
            [before.id],
            |r| r.get(0),
        )
        .unwrap();
    let after = review(&w, &b, &br, context());
    let result = w
        .compare_document_versions(&CompareRequest {
            left_reading_id: ai,
            right_reading_id: bi,
            left_review_id: Some(before.id),
            right_review_id: Some(after.id),
        })
        .unwrap();
    let changed = result
        .findings
        .iter()
        .filter(|r| r.change == FindingChange::NoLongerFlagged)
        .count();
    assert_eq!(changed, 5);
    let row = result
        .findings
        .iter()
        .find(|f| f.rule_id == "PH-LAB-083-01")
        .unwrap();
    assert_eq!(row.left_status, Some(ReviewStatus::Resolved));
    assert_eq!(row.right_status, Some(ReviewStatus::Unresolved));
    assert_eq!(
        result.left.review.as_ref().unwrap().workflow.events.len(),
        1
    );
    assert!(
        result
            .right
            .review
            .as_ref()
            .unwrap()
            .workflow
            .events
            .is_empty()
    );
    let still_original: String = rusqlite::Connection::open(&paths.database)
        .unwrap()
        .query_row(
            "SELECT value FROM review_runs WHERE id=?1",
            [before.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(original_json, still_original);
}
#[test]
fn missing_clauses_and_removed_rules_are_not_reported_as_corrected_issues() {
    let (temp, _paths, w) = setup();
    let (a, ar, ai, b, _, _) = pair(&w, temp.path());
    let before = review(&w, &a, &ar, context());
    let missing = FIXED
        .lines()
        .filter(|s| !s.starts_with("Ordinary-day"))
        .collect::<Vec<_>>()
        .join("\n");
    let (_, br, bi) = capture(&w, &b.path, &missing);
    let after = review(&w, &b, &br, context());
    let request = CompareRequest {
        left_reading_id: ai,
        right_reading_id: bi,
        left_review_id: Some(before.id),
        right_review_id: Some(after.id),
    };
    let result = w.compare_document_versions(&request).unwrap();
    let row = result
        .findings
        .iter()
        .find(|f| f.rule_id == "PH-LAB-087-01")
        .unwrap();
    assert_eq!(row.right_outcome, Some(Outcome::NeedsEvidence));
    assert_eq!(row.change, FindingChange::Changed);
    assert!(!row.reasons.is_empty());
    let mut reduced = after.review;
    reduced.findings.truncate(3);
    let reduced = w.save_review_for_reading(b.id, &br, reduced).unwrap();
    let result = w
        .compare_document_versions(&CompareRequest {
            right_review_id: Some(reduced.id),
            ..request
        })
        .unwrap();
    assert!(
        result
            .findings
            .iter()
            .any(|r| r.change == FindingChange::RuleRemoved)
    );
}
#[test]
fn scope_source_engine_and_legacy_reading_changes_are_labelled_not_comparable() {
    let (temp, _paths, w) = setup();
    let (a, ar, ai, b, br, bi) = pair(&w, temp.path());
    let before = review(&w, &a, &ar, context());
    for mode in 0..4 {
        let mut result = complyph_checks::review(&br.report, &br.sha256, context());
        match mode {
            0 => result.context.normal_hours = Applicability::OutsideScope,
            1 => result.engine = "different tested engine".into(),
            2 => result.source_sha256 = "different source".into(),
            _ => (),
        }
        let after = if mode == 3 {
            w.save_review(b.id, result).unwrap()
        } else {
            w.save_review_for_reading(b.id, &br, result).unwrap()
        };
        let comparison = w
            .compare_document_versions(&CompareRequest {
                left_reading_id: ai,
                right_reading_id: bi,
                left_review_id: Some(before.id),
                right_review_id: Some(after.id),
            })
            .unwrap();
        assert_eq!(
            comparison
                .findings
                .iter()
                .find(|f| f.rule_id == "PH-LAB-083-01")
                .unwrap()
                .change,
            FindingChange::NotComparable
        );
    }
}
#[test]
fn incomplete_readings_and_changed_readings_of_the_same_file_never_imply_a_document_fix() {
    let (temp, _paths, w) = setup();
    let (doc, old, old_id) = capture(&w, &temp.path().join("Policy.docx"), ISSUE);
    let before = review(&w, &doc, &old, context());
    let alternate = w
        .save_extraction(&doc, &document_snapshot(&doc.path).unwrap(), report(FIXED))
        .unwrap();
    let new_id = w.document_history(doc.id).unwrap().versions[0]
        .readings
        .last()
        .unwrap()
        .id;
    let after = review(&w, &doc, &alternate, context());
    let result = w
        .compare_document_versions(&CompareRequest {
            left_reading_id: old_id,
            right_reading_id: new_id,
            left_review_id: Some(before.id),
            right_review_id: Some(after.id),
        })
        .unwrap();
    assert!(result.reading_only_change);
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.change == FindingChange::NotComparable)
    );
    let failed = w
        .save_extraction(
            &doc,
            &document_snapshot(&doc.path).unwrap(),
            Report::failure(
                Format::Docx,
                Status::Unreadable,
                "Fictional failed reading.",
            ),
        )
        .unwrap();
    assert_eq!(failed.report.status, Status::Unreadable);
    let failed_id = w.document_history(doc.id).unwrap().versions[0]
        .readings
        .last()
        .unwrap()
        .id;
    let result = w
        .compare_document_versions(&CompareRequest {
            left_reading_id: old_id,
            right_reading_id: failed_id,
            left_review_id: None,
            right_review_id: None,
        })
        .unwrap();
    assert!(!result.text.available);
    assert_eq!(result.text.removed_lines, None);
}
#[test]
fn diff_is_unicode_safe_location_aware_and_explicit_when_limits_truncate_it() {
    let (temp, _paths, w) = setup();
    let (a, _, ai) = capture(
        &w,
        &temp.path().join("A.docx"),
        "SYNTHETIC José\nunchanged\nold",
    );
    let (b, _, bi) = capture(
        &w,
        &temp.path().join("B.docx"),
        "SYNTHETIC José\nunchanged\nnew 🦀",
    );
    link(&w, a.id, b.id);
    let request = CompareRequest {
        left_reading_id: ai,
        right_reading_id: bi,
        left_review_id: None,
        right_review_id: None,
    };
    let result = w.compare_document_versions(&request).unwrap();
    assert_eq!(result.text.added_lines, Some(1));
    assert_eq!(result.text.removed_lines, Some(1));
    let added = result
        .text
        .lines
        .iter()
        .find(|l| l.kind == "added")
        .unwrap();
    assert!(added.text.ends_with('🦀'));
    assert_eq!(
        added.new_location,
        Some(Location::WordParagraph {
            part: "word/document.xml".into(),
            paragraph: 3,
            table_cell: None
        })
    );
    let large = format!("{} extra", "界".repeat(MAX_ROW_CHARS + 1));
    let (_, _, id) = capture(&w, &b.path, &large);
    let truncated = w
        .compare_document_versions(&CompareRequest {
            right_reading_id: id,
            ..request.clone()
        })
        .unwrap();
    assert!(truncated.text.display_truncated);
    assert!(
        truncated
            .text
            .lines
            .iter()
            .all(|l| l.text.chars().count() <= MAX_ROW_CHARS)
    );
    let snapshot = document_snapshot(&b.path).unwrap();
    let mut report = report("long fixture");
    report.units[0].text = "line\n".repeat(MAX_DIFF_LINES + 1);
    w.save_extraction(&b, &snapshot, report).unwrap();
    let id = w
        .document_history(b.id)
        .unwrap()
        .versions
        .last()
        .unwrap()
        .readings
        .last()
        .unwrap()
        .id;
    let over = w
        .compare_document_versions(&CompareRequest {
            right_reading_id: id,
            ..request
        })
        .unwrap();
    assert!(!over.text.available && over.text.display_truncated);
    assert_eq!(over.text.added_lines, None);
}
#[test]
fn deletion_removes_only_that_files_history_and_tampered_snapshots_are_refused() {
    let (temp, paths, w) = setup();
    let (a, _ar, ai, b, br, bi) = pair(&w, temp.path());
    let after = review(&w, &b, &br, context());
    w.remove_document(a.id).unwrap();
    assert!(w.retained_reading(ai).is_err());
    assert_eq!(w.document_history(b.id).unwrap().documents.len(), 1);
    assert_eq!(w.reading_for_review(after.id).unwrap().unwrap().id, bi);
    // Warm the metadata cache, then prove that a later external change is revalidated.
    assert!(
        w.review_workflow(after.id)
            .unwrap()
            .reading_context
            .is_some()
    );
    assert!(a.path.exists() && b.path.exists());
    let db = rusqlite::Connection::open(&paths.database).unwrap();
    assert!(
        db.execute("UPDATE document_readings SET value='{}' WHERE id=?1", [bi])
            .is_err()
    );
    db.execute_batch("DROP TRIGGER reading_immutable;").unwrap();
    let mut changed = br;
    changed.report.summary = "Tampered fictional reading".into();
    db.execute(
        "UPDATE document_readings SET value=?1 WHERE id=?2",
        rusqlite::params![serde_json::to_string(&changed).unwrap(), bi],
    )
    .unwrap();
    assert!(w.retained_reading(bi).is_err());
    assert!(w.review_workflow(after.id).is_err());
}
#[test]
fn schema_five_upgrade_keeps_original_review_and_cache_bytes_without_inventing_old_text() {
    let (temp, paths, w) = setup();
    let (doc, reading, _) = capture(&w, &temp.path().join("Policy.docx"), ISSUE);
    let review = review(&w, &doc, &reading, context());
    let workflow = w.review_workflow(review.id).unwrap();
    w.append_review_action(
        review.id,
        &ReviewActionRequest {
            review_fingerprint: workflow.review_fingerprint,
            rule_id: None,
            expected_revision: 0,
            reviewer: "Synthetic reviewer".into(),
            note: "Preserve this old history.".into(),
            status: None,
        },
    )
    .unwrap();
    let (_, latest, _) = capture(&w, &doc.path, FIXED);
    let db = rusqlite::Connection::open(&paths.database).unwrap();
    let original: String = db
        .query_row(
            "SELECT value FROM review_runs WHERE id=?1",
            [review.id],
            |r| r.get(0),
        )
        .unwrap();
    let cache: String = db
        .query_row(
            "SELECT value FROM document_extractions WHERE document_id=?1",
            [doc.id],
            |r| r.get(0),
        )
        .unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF; DROP TABLE review_version_links; DROP TABLE document_readings; DROP TABLE document_versions; DROP TABLE document_link_events; DROP TABLE document_group_members; DROP TABLE document_groups; PRAGMA user_version=5;").unwrap();
    drop(db);
    drop(w);
    let w = Workspace::open(&paths).unwrap();
    let history = w.document_history(doc.id).unwrap();
    assert_eq!(history.versions.len(), 2);
    assert!(
        history
            .versions
            .iter()
            .find(|v| v.document_sha256 == reading.sha256)
            .unwrap()
            .readings
            .is_empty()
    );
    assert_eq!(
        history
            .versions
            .iter()
            .find(|v| v.document_sha256 == latest.sha256)
            .unwrap()
            .readings
            .len(),
        1
    );
    assert!(w.reading_for_review(review.id).unwrap().is_none());
    assert_eq!(w.review_workflow(review.id).unwrap().events.len(), 1);
    let db = rusqlite::Connection::open(&paths.database).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        db.query_row(
            "SELECT value FROM review_runs WHERE id=?1",
            [review.id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        original
    );
    assert_eq!(
        db.query_row(
            "SELECT value FROM document_extractions WHERE document_id=?1",
            [doc.id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        cache
    );
}
