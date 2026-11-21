//! Independently expected public-library lifecycle and fail-closed regression cases.
use complyph_library::*;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn reviewer() -> LibraryReviewer {
    LibraryReviewer{actor:"Synthetic legal-library tester".into(),review_on:"2026-11-21".into(),basis:"Fictional lifecycle test only; no conclusion about real law. Compared complete preserved support and confirmed the tested unchanged profile.".into()}
}
fn setup() -> (tempfile::TempDir, LibraryCatalog, i64, SourceSnapshot) {
    let temp = tempfile::tempdir().unwrap();
    let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
    let source = catalog
        .sources()
        .unwrap()
        .into_iter()
        .find(|s| s.metadata.key == "labor-code-reference")
        .unwrap();
    let snapshot = catalog.snapshot(source.id).unwrap();
    let marker = "\nArticle 83.\n";
    let end = "\nArticle 84.\n";
    let start = snapshot.text.find(marker).unwrap();
    let finish = start + snapshot.text[start..].find(end).unwrap();
    catalog
        .register_rule_profiles(&[RuleDependency {
            rule_id: "SYNTHETIC-HOURS-TEST".into(),
            title: "Fictional dependency lifecycle fixture".into(),
            source_key: snapshot.metadata.key.clone(),
            raw_sha256: snapshot.metadata.raw_sha256.clone(),
            text_sha256: snapshot.metadata.text_sha256.clone(),
            engine: "synthetic-test-1".into(),
            pack_sha256: hash(b"synthetic profile definition"),
            provisions: vec![ProvisionBoundary {
                title: "Article 83 fixture".into(),
                start: marker.into(),
                end: end.into(),
                text_sha256: hash(&snapshot.text.as_bytes()[start..finish]),
            }],
        }])
        .unwrap();
    (temp, catalog, source.id, snapshot)
}
fn deliver(catalog: &mut LibraryCatalog, id: i64, body: &str) -> i64 {
    let url = catalog.snapshot(id).unwrap().metadata.source_url;
    catalog.set_update_watch(id, &url, true).unwrap();
    let target = catalog.claim_source_check(id, false).unwrap().unwrap();
    assert_eq!(
        catalog
            .finish_source_check(
                &target,
                UpdateReply::Download {
                    resolved_url: url,
                    mime: "text/plain".into(),
                    etag: None,
                    last_modified: None,
                    bytes: body.as_bytes().to_vec()
                }
            )
            .unwrap()
            .outcome,
        "changed"
    );
    let change = catalog
        .downloaded_changes(id)
        .unwrap()
        .into_iter()
        .find(|c| c.content_sha256 == hash(body.as_bytes()))
        .unwrap();
    catalog
        .prepare_candidate_reading(
            change.id,
            ManualSourceFile {
                raw: body.as_bytes().to_vec(),
                reading_text: body.into(),
                mime: "text/plain".into(),
                pages: None,
                extraction_note: "Fictional UTF-8 fixture; not a new legal instrument.".into(),
            },
        )
        .unwrap();
    change.id
}
fn adoption(hash: &str) -> AdoptionReview {
    AdoptionReview {
        reviewer: reviewer(),
        expected_active_sha256: hash.into(),
        identity_confirmed: true,
        original_and_reading_compared: true,
        effectivity_and_scope_reviewed: true,
    }
}
fn assess(state: &RuleLibraryState) -> RuleRevalidation {
    RuleRevalidation{expected_token:state.token.clone(),unchanged_profile:true,applicability_and_amendments_reviewed:true,test_reference:"Independent synthetic boundary/document matrix: unchanged numerical engine; fixture test passed.".into(),reviewer:reviewer(),supersedes_update_assessment:None}
}
fn details(metadata: &SourceMetadata) -> ManualSourceInput {
    ManualSourceInput {
        title: metadata.title.clone(),
        instrument: metadata.instrument.clone(),
        issuer: metadata.issuer.clone(),
        official_url: metadata.source_url.clone(),
        domains: metadata.domains.clone(),
        ..Default::default()
    }
}

#[test]
fn metadata_annotations_are_validated_version_bound_and_never_rewrite_originals() {
    let (_temp, catalog, id, old) = setup();
    let mut input = details(&old.metadata);
    input.title = "Reviewed display title — synthetic annotation".into();
    input.issued_on = Some("2025-02-29".into());
    assert!(
        catalog
            .edit_source_metadata(id, &old.metadata.raw_sha256, &input, &reviewer())
            .is_err()
    );
    input.issued_on = Some("2024-02-29".into());
    assert!(
        catalog
            .edit_source_metadata(id, &"0".repeat(64), &input, &reviewer())
            .is_err()
    );
    for bad in [
        LibraryReviewer::default(),
        LibraryReviewer {
            actor: "x".into(),
            basis: "x".into(),
            review_on: "2026-02-30".into(),
        },
        LibraryReviewer {
            actor: "x".into(),
            basis: "x".into(),
            review_on: "2099-01-01".into(),
        },
    ] {
        assert!(
            catalog
                .edit_source_metadata(id, &old.metadata.raw_sha256, &input, &bad)
                .is_err()
        );
    }
    assert!(catalog.source_history(id).unwrap().is_empty());
    catalog
        .edit_source_metadata(id, &old.metadata.raw_sha256, &input, &reviewer())
        .unwrap();
    assert_eq!(catalog.display_metadata(id).unwrap().title, input.title);
    assert_eq!(
        catalog.display_metadata(id).unwrap().issued_on.as_deref(),
        Some("2024-02-29")
    );
    assert_eq!(
        catalog.snapshot(id).unwrap().metadata.title,
        old.metadata.title
    );
    assert_eq!(catalog.snapshot(id).unwrap().raw, old.raw);
    let exact = catalog
        .snapshot_for_review(
            &old.metadata.key,
            &old.metadata.raw_sha256,
            &old.metadata.text_sha256,
        )
        .unwrap();
    assert!(exact.text == old.text);
    assert_eq!(catalog.source_history(id).unwrap().len(), 1);
    assert_eq!(
        catalog.rule_library_states().unwrap()[0].status,
        RuleReadiness::RevalidationRequired
    );
}

#[test]
fn adoption_preserves_history_and_requires_revalidation_of_unchanged_provisions() {
    let (temp, mut catalog, id, old) = setup();
    let candidate = format!(
        "{}SYNTHETIC editorial footer only; not a law change.\n",
        old.text
    );
    let change = deliver(&mut catalog, id, &candidate);
    let state = catalog.rule_library_states().unwrap().remove(0);
    assert!(!state.status.usable());
    assert_eq!(state.pending_downloads, 1);
    assert!(
        catalog
            .revalidate_rule(&state.dependency.rule_id, &assess(&state))
            .is_err()
    );
    assert_eq!(
        catalog.snapshot(id).unwrap().metadata.raw_sha256,
        old.metadata.raw_sha256
    );
    let diff = catalog.compare_candidate(change).unwrap();
    assert_eq!((diff.added_lines, diff.removed_lines), (1, 0));
    assert!(
        diff.lines
            .iter()
            .any(|l| l.kind == "added" && l.text.contains("SYNTHETIC editorial footer"))
    );
    let mut review = adoption(&old.metadata.raw_sha256);
    for gate in 0..4 {
        let mut bad = review.clone();
        match gate {
            0 => bad.identity_confirmed = false,
            1 => bad.original_and_reading_compared = false,
            2 => bad.effectivity_and_scope_reviewed = false,
            _ => bad.expected_active_sha256 = "f".repeat(64),
        }
        assert!(catalog.adopt_candidate(change, &bad).is_err());
    }
    let active = catalog.adopt_candidate(change, &review).unwrap();
    assert_eq!(active.provenance, Provenance::ReviewedDownload);
    assert_eq!(active.issued_on, None);
    assert_eq!(active.effective_from, None);
    assert_eq!(catalog.pending_update_count().unwrap(), 0);
    assert_eq!(catalog.preserved_versions(id).unwrap().len(), 2);
    assert_eq!(
        catalog
            .snapshot_for_review(
                &old.metadata.key,
                &old.metadata.raw_sha256,
                &old.metadata.text_sha256
            )
            .unwrap()
            .raw,
        old.raw
    );
    let held = catalog.rule_library_states().unwrap().remove(0);
    assert_eq!(held.status, RuleReadiness::RevalidationRequired);
    assert!(held.matching_provisions);
    catalog
        .revalidate_rule(&held.dependency.rule_id, &assess(&held))
        .unwrap();
    let ready = catalog.rule_library_states().unwrap().remove(0);
    assert_eq!(ready.status, RuleReadiness::Revalidated);
    assert_eq!(ready.active_sha256, hash(candidate.as_bytes()));
    review.expected_active_sha256 = active.raw_sha256;
    catalog
        .restore_preserved_version(id, &old.metadata.raw_sha256, &review)
        .unwrap();
    assert!(!catalog.rule_library_states().unwrap()[0].status.usable());
    assert_eq!(catalog.preserved_versions(id).unwrap().len(), 2);
    drop(catalog);
    let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
    assert_eq!(
        catalog.snapshot(id).unwrap().metadata.raw_sha256,
        old.metadata.raw_sha256
    );
    assert!(
        catalog
            .source_history(id)
            .unwrap()
            .iter()
            .any(|e| e.kind == "version_restored")
    );
}

#[test]
fn changed_legal_support_cannot_be_reactivated_by_a_reviewer_checkbox() {
    let (_temp, mut catalog, id, old) = setup();
    let changed = old.text.replacen(
        "The normal hours of work of any employee shall not exceed eight in a day.",
        "SYNTHETIC altered law text: not a real enactment.",
        1,
    );
    assert!(changed != old.text);
    let change = deliver(&mut catalog, id, &changed);
    catalog
        .adopt_candidate(change, &adoption(&old.metadata.raw_sha256))
        .unwrap();
    let held = catalog.rule_library_states().unwrap().remove(0);
    assert_eq!(held.status, RuleReadiness::UpdateRequired);
    assert!(!held.matching_provisions);
    assert!(
        catalog
            .revalidate_rule(&held.dependency.rule_id, &assess(&held))
            .is_err()
    );
    let mut needs_update = assess(&held);
    needs_update.unchanged_profile = false;
    catalog
        .revalidate_rule(&held.dependency.rule_id, &needs_update)
        .unwrap();
    assert!(!catalog.rule_library_states().unwrap()[0].status.usable());
}

#[test]
fn stale_rule_decisions_do_not_survive_metadata_identity_or_candidate_changes() {
    let (_temp, mut catalog, id, old) = setup();
    let change = deliver(
        &mut catalog,
        id,
        &format!("{}\nSYNTHETIC harmless footer", old.text),
    );
    catalog
        .adopt_candidate(change, &adoption(&old.metadata.raw_sha256))
        .unwrap();
    let first = catalog.rule_library_states().unwrap().remove(0);
    let active = catalog.snapshot(id).unwrap().metadata;
    let mut input = details(&active);
    input.applicability_note = "Synthetic reviewed scope note.".into();
    catalog
        .edit_source_metadata(id, &active.raw_sha256, &input, &reviewer())
        .unwrap();
    assert!(
        catalog
            .revalidate_rule(&first.dependency.rule_id, &assess(&first))
            .is_err()
    );
    let next = catalog.rule_library_states().unwrap().remove(0);
    catalog
        .review_source_identity(
            id,
            &active.raw_sha256,
            SourceIdentityDecision::Rejected,
            &reviewer(),
        )
        .unwrap();
    assert!(
        catalog
            .revalidate_rule(&next.dependency.rule_id, &assess(&next))
            .is_err()
    );
    let rejected = catalog.rule_library_states().unwrap().remove(0);
    assert!(!rejected.identity_ready);
    assert!(
        catalog
            .revalidate_rule(&rejected.dependency.rule_id, &assess(&rejected))
            .is_err()
    );
    catalog
        .review_source_identity(
            id,
            &active.raw_sha256,
            SourceIdentityDecision::IdentityChecked,
            &reviewer(),
        )
        .unwrap();
    let identity = catalog.rule_library_states().unwrap().remove(0);
    catalog
        .revalidate_rule(&identity.dependency.rule_id, &assess(&identity))
        .unwrap();
    assert!(catalog.rule_library_states().unwrap()[0].status.usable());
    let second = deliver(
        &mut catalog,
        id,
        &format!("{}\nSYNTHETIC next footer", old.text),
    );
    assert!(!catalog.rule_library_states().unwrap()[0].status.usable());
    assert!(
        catalog
            .revalidate_rule(&identity.dependency.rule_id, &assess(&identity))
            .is_err()
    );
    catalog
        .dismiss_candidate(second, &active.raw_sha256, &reviewer())
        .unwrap();
    assert!(!catalog.rule_library_states().unwrap()[0].status.usable());
    let final_state = catalog.rule_library_states().unwrap().remove(0);
    catalog
        .revalidate_rule(&final_state.dependency.rule_id, &assess(&final_state))
        .unwrap();
    assert!(catalog.rule_library_states().unwrap()[0].status.usable());
}

#[test]
fn legacy_download_notes_do_not_clear_a_rule_hold_and_dismissals_retain_bytes() {
    let (_temp, mut catalog, id, old) = setup();
    let change = deliver(
        &mut catalog,
        id,
        "SYNTHETIC wrong instrument, no real law content.",
    );
    catalog
        .mark_download_reviewed(change, "Old-style source note only.")
        .unwrap();
    assert_eq!(catalog.pending_update_count().unwrap(), 1);
    assert!(
        catalog
            .reference_has_pending_downloads(&old.metadata.key)
            .unwrap()
    );
    assert!(!catalog.rule_library_states().unwrap()[0].status.usable());
    assert!(
        catalog
            .dismiss_candidate(
                change,
                &old.metadata.raw_sha256,
                &LibraryReviewer::default()
            )
            .is_err()
    );
    catalog
        .dismiss_candidate(change, &old.metadata.raw_sha256, &reviewer())
        .unwrap();
    assert_eq!(catalog.pending_update_count().unwrap(), 0);
    assert!(
        !catalog
            .reference_has_pending_downloads(&old.metadata.key)
            .unwrap()
    );
    assert!(
        catalog
            .reference_has_downloaded_changes(&old.metadata.key)
            .unwrap()
    );
    assert_eq!(
        catalog.rule_library_states().unwrap()[0].status,
        RuleReadiness::Current
    );
    assert_eq!(
        catalog.downloaded_candidate_bytes(change).unwrap().1,
        b"SYNTHETIC wrong instrument, no real law content."
    );
}

#[test]
fn reviewed_legal_change_holds_survive_unrelated_edits_and_new_compiled_profiles() {
    let (_temp, catalog, id, old) = setup();
    let current = catalog.rule_library_states().unwrap().remove(0);
    let mut needs_update = assess(&current);
    needs_update.unchanged_profile = false;
    let hold_id = catalog
        .revalidate_rule(&current.dependency.rule_id, &needs_update)
        .unwrap();
    let mut input = details(&old.metadata);
    input.title = "Synthetic title correction".into();
    catalog
        .edit_source_metadata(id, &old.metadata.raw_sha256, &input, &reviewer())
        .unwrap();
    let mut updated = current.dependency.clone();
    updated.engine = "synthetic-new-app-release".into();
    updated.pack_sha256 = hash(b"synthetic new compiled profile");
    catalog.register_rule_profiles(&[updated]).unwrap();
    let held = catalog.rule_library_states().unwrap().remove(0);
    assert_eq!(held.update_hold_id, Some(hold_id));
    assert_eq!(held.status, RuleReadiness::UpdateRequired);
    assert!(
        catalog
            .revalidate_rule(&held.dependency.rule_id, &assess(&held))
            .is_err()
    );
    let mut explicit = assess(&held);
    explicit.supersedes_update_assessment = Some(hold_id + 999);
    assert!(
        catalog
            .revalidate_rule(&held.dependency.rule_id, &explicit)
            .is_err()
    );
    explicit.supersedes_update_assessment = Some(hold_id);
    catalog
        .revalidate_rule(&held.dependency.rule_id, &explicit)
        .unwrap();
    let resolved = catalog.rule_library_states().unwrap().remove(0);
    assert_eq!(resolved.status, RuleReadiness::Revalidated);
    assert_eq!(resolved.update_hold_id, None);
    assert!(catalog.source_history(id).unwrap().iter().any(
        |e| e.kind == "rule_revalidation" && e.data["supersedes_update_assessment"] == hold_id
    ));
}

#[test]
fn comparison_is_exact_about_replacements_unicode_repeated_lines_and_display_limits() {
    let diff = compare_readings(
        &hash(b"old"),
        "Repeated\nRate: 15%\nRepeated\nPPE: ₱0\n",
        &hash(b"new"),
        "Repeated\nRate: 16%\nRepeated\nPPE: ₱0\n",
    )
    .unwrap();
    assert_eq!(
        (diff.added_lines, diff.removed_lines, diff.unchanged_lines),
        (1, 1, 3)
    );
    assert!(
        diff.lines
            .iter()
            .any(|l| l.kind == "removed" && l.text == "Rate: 15%\n")
    );
    assert!(
        diff.lines
            .iter()
            .any(|l| l.kind == "added" && l.text == "Rate: 16%\n")
    );
    assert!(!diff.display_truncated);
    let long = format!("{}\n", "é".repeat(3000));
    assert!(
        compare_readings("old", "old\n", "new", &long)
            .unwrap()
            .display_truncated
    );
    assert!(
        compare_readings("old", &"a\n".repeat(20_001), "new", "")
            .unwrap()
            .display_truncated
    );
}

const LISTING: &[u8] = include_bytes!("../../../examples/Synthetic publication listing.html");
const UPDATED: &[u8] =
    include_bytes!("../../../examples/Synthetic updated publication listing.html");
const PAGE: &str = "https://privacy.gov.ph/complyph-fixtures/listing/";
fn page_reply(body: &[u8]) -> UpdateReply {
    UpdateReply::Download {
        resolved_url: PAGE.into(),
        mime: "text/html".into(),
        etag: None,
        last_modified: None,
        bytes: body.to_vec(),
    }
}

#[test]
fn discovery_finds_publication_citations_without_following_hidden_unsafe_or_duplicate_links() {
    let found = publication_links(PAGE, LISTING).unwrap();
    let urls = found.links.iter().map(|p| p.0.as_str()).collect::<Vec<_>>();
    assert_eq!(
        urls,
        vec![
            "https://bir.gov.ph/complyph-fixtures/test.docx",
            "https://privacy.gov.ph/complyph-fixtures/circular-001/",
            "https://privacy.gov.ph/complyph-fixtures/synthetic-order-001.pdf",
        ]
    );
    assert!(!found.truncated);
    let reading = html_reading_text(LISTING).unwrap();
    assert!(!reading.contains("const fake"));
    assert!(!reading.contains("Hidden fictional PDF"));
    assert!(reading.contains("Fictional references"));
    assert!(publication_links(PAGE, &vec![b' '; MAX_PUBLIC_HTML_BYTES + 1]).is_err());
    assert!(publication_links(PAGE, &[255, 254]).is_err());
}

#[test]
fn discovery_distinguishes_initial_inventory_from_new_links_and_retains_page_versions() {
    let temp = tempfile::tempdir().unwrap();
    let catalog = LibraryCatalog::open(temp.path()).unwrap();
    catalog
        .approve_publication_page(PAGE, "ph.privacy")
        .unwrap();
    let id = catalog.publication_pages().unwrap()[0].id;
    let target = catalog.claim_publication_page(id, true).unwrap().unwrap();
    assert!(catalog.claim_publication_page(id, false).unwrap().is_none());
    let first = catalog
        .finish_publication_page(&target, page_reply(LISTING))
        .unwrap();
    assert_eq!((first.links_seen, first.new_links), (3, 3));
    assert!(
        catalog
            .discovered_issuances()
            .unwrap()
            .iter()
            .all(|l| l.initial_scan)
    );
    assert!(catalog.due_publication_page_ids(true).unwrap().is_empty());
    let target = catalog.claim_publication_page(id, false).unwrap().unwrap();
    let later = catalog
        .finish_publication_page(&target, page_reply(UPDATED))
        .unwrap();
    assert_eq!((later.links_seen, later.new_links), (4, 1));
    let links = catalog.discovered_issuances().unwrap();
    let new = links.iter().find(|l| l.url.ends_with("002.pdf")).unwrap();
    assert!(!new.initial_scan);
    catalog
        .review_discovered_issuance(new.id, true, &reviewer())
        .unwrap();
    assert!(
        catalog
            .discovered_issuances()
            .unwrap()
            .iter()
            .find(|l| l.id == new.id)
            .unwrap()
            .dismissed
    );
    catalog
        .review_discovered_issuance(new.id, false, &reviewer())
        .unwrap();
    assert!(
        !catalog
            .discovered_issuances()
            .unwrap()
            .iter()
            .find(|l| l.id == new.id)
            .unwrap()
            .dismissed
    );
    assert_eq!(catalog.publication_page_original(id).unwrap(), UPDATED);
    assert_eq!(catalog.summary().unwrap().source_count, 19);
    assert!(catalog.update_watches().unwrap().is_empty());
    assert_eq!(catalog.rule_library_states().unwrap().len(), 0);
}

#[test]
fn publication_approvals_revocation_and_non_html_responses_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let catalog = LibraryCatalog::open(temp.path()).unwrap();
    for url in [
        "https://example.invalid/new-laws/",
        "http://privacy.gov.ph/",
        "https://user:pass@privacy.gov.ph/",
        "https://privacy.gov.ph:444/",
    ] {
        assert!(catalog.approve_publication_page(url, "ph.privacy").is_err());
    }
    assert!(
        catalog
            .approve_publication_page(PAGE, "unknown-domain")
            .is_err()
    );
    catalog
        .approve_publication_page(PAGE, "ph.privacy")
        .unwrap();
    let id = catalog.publication_pages().unwrap()[0].id;
    let target = catalog.claim_publication_page(id, false).unwrap().unwrap();
    catalog.disable_publication_page(id).unwrap();
    assert!(
        catalog
            .finish_publication_page(&target, page_reply(LISTING))
            .is_err()
    );
    assert!(catalog.discovered_issuances().unwrap().is_empty());
    catalog
        .approve_publication_page(PAGE, "ph.privacy")
        .unwrap();
    let target = catalog.claim_publication_page(id, false).unwrap().unwrap();
    let result = catalog
        .finish_publication_page(
            &target,
            UpdateReply::Download {
                resolved_url: PAGE.into(),
                mime: "application/pdf".into(),
                bytes: b"%PDF-1.7\nSYNTHETIC".to_vec(),
                etag: None,
                last_modified: None,
            },
        )
        .unwrap();
    assert_eq!(result.outcome, "failed");
    assert!(catalog.discovered_issuances().unwrap().is_empty());
}

#[test]
fn corrupt_prepared_text_is_rejected_and_migration_keeps_source_bytes_and_legacy_notes() {
    let (temp, mut catalog, id, old) = setup();
    let change = deliver(&mut catalog, id, "SYNTHETIC candidate text");
    let db = rusqlite::Connection::open(temp.path().join("library/catalog.sqlite3")).unwrap();
    db.execute(
        "UPDATE candidate_readings SET reading_text='tampered' WHERE change_id=?1",
        [change],
    )
    .unwrap();
    assert!(catalog.prepared_candidate(change).is_err());
    assert!(
        catalog
            .adopt_candidate(change, &adoption(&old.metadata.raw_sha256))
            .is_err()
    );
    assert_eq!(
        catalog.snapshot(id).unwrap().metadata.raw_sha256,
        old.metadata.raw_sha256
    );
    drop(catalog);
    drop(db);
    let legacy = temp.path().join("legacy-library");
    let catalog = LibraryCatalog::open(&legacy).unwrap();
    let before = catalog.snapshot_by_key("labor-code-reference").unwrap();
    drop(catalog);
    let db = rusqlite::Connection::open(legacy.join("catalog.sqlite3")).unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF;DROP TABLE rule_hold_resolutions;DROP TABLE rule_revalidations;DROP TABLE rule_profiles;DROP TABLE candidate_readings;DROP TABLE candidate_decisions;DROP TABLE source_events;DROP TABLE active_source_versions;DROP TABLE publication_link_reviews;DROP TABLE publication_links;DROP TABLE publication_versions;DROP TABLE publication_pages;PRAGMA user_version=4;").unwrap();
    drop(db);
    let reopened = LibraryCatalog::open(&legacy).unwrap();
    assert_eq!(
        reopened
            .snapshot_by_key("labor-code-reference")
            .unwrap()
            .raw,
        before.raw
    );
    assert_eq!(reopened.summary().unwrap().source_count, 19);
    let db = rusqlite::Connection::open(legacy.join("catalog.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0))
            .unwrap(),
        5
    );
}

#[test]
fn manual_identity_review_updates_display_status_without_rewriting_snapshot_provenance() {
    let temp = tempfile::tempdir().unwrap();
    let mut catalog = LibraryCatalog::open(temp.path()).unwrap();
    let input = ManualSourceInput {
        title: "SYNTHETIC manual reference".into(),
        instrument: "TEST ONLY".into(),
        issuer: "Fictional issuer".into(),
        official_url: "https://privacy.gov.ph/complyph-fixtures/manual.pdf".into(),
        domains: vec!["ph.privacy".into()],
        ..Default::default()
    };
    let outcome = catalog
        .import_manual(
            &input,
            ManualSourceFile {
                raw: b"%PDF-1.7\nSYNTHETIC".to_vec(),
                reading_text: "Fictional manually supplied reading.".into(),
                mime: "application/pdf".into(),
                pages: Some(1),
                extraction_note: "Synthetic test only.".into(),
            },
        )
        .unwrap();
    catalog
        .review_source_identity(
            outcome.source_id,
            &outcome.metadata.raw_sha256,
            SourceIdentityDecision::IdentityChecked,
            &reviewer(),
        )
        .unwrap();
    assert_eq!(
        catalog
            .display_metadata(outcome.source_id)
            .unwrap()
            .provenance,
        Provenance::ManualIdentityChecked
    );
    assert!(
        catalog
            .display_metadata(outcome.source_id)
            .unwrap()
            .verified_at_unix
            > 0
    );
    assert_eq!(
        catalog
            .snapshot(outcome.source_id)
            .unwrap()
            .metadata
            .provenance,
        Provenance::ManualPending
    );
    catalog
        .review_source_identity(
            outcome.source_id,
            &outcome.metadata.raw_sha256,
            SourceIdentityDecision::Rejected,
            &reviewer(),
        )
        .unwrap();
    assert_eq!(
        catalog
            .display_metadata(outcome.source_id)
            .unwrap()
            .provenance,
        Provenance::ManualPending
    );
    assert_eq!(
        catalog
            .display_metadata(outcome.source_id)
            .unwrap()
            .verified_at_unix,
        0
    );
    assert_eq!(catalog.source_history(outcome.source_id).unwrap().len(), 2);
}

#[test]
fn public_maintenance_never_opens_or_modifies_private_files() {
    let temp = tempfile::tempdir().unwrap();
    let private = temp.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::write(
        private.join("workspace.sqlite3"),
        b"SYNTHETIC private sentinel",
    )
    .unwrap();
    let catalog = LibraryCatalog::open(&temp.path().join("library")).unwrap();
    catalog
        .approve_publication_page(PAGE, "ph.privacy")
        .unwrap();
    let target = catalog
        .claim_publication_page(catalog.publication_pages().unwrap()[0].id, false)
        .unwrap()
        .unwrap();
    catalog
        .finish_publication_page(&target, page_reply(LISTING))
        .unwrap();
    assert_eq!(
        fs::read(private.join("workspace.sqlite3")).unwrap(),
        b"SYNTHETIC private sentinel"
    );
    assert!(!Path::new(&catalog.discovered_issuances().unwrap()[0].url).exists());
}

#[test]
fn future_source_effectivity_keeps_unchanged_profiles_held() {
    let (_temp, catalog, id, old) = setup();
    let mut input = details(&old.metadata);
    input.effective_from = Some("2099-01-01".into());
    catalog
        .edit_source_metadata(id, &old.metadata.raw_sha256, &input, &reviewer())
        .unwrap();
    let state = catalog.rule_library_states().unwrap().remove(0);
    assert!(state.matching_provisions);
    assert!(!state.period_ready);
    assert_eq!(state.source_effective_from.as_deref(), Some("2099-01-01"));
    assert!(!state.status.usable());
    assert!(
        catalog
            .revalidate_rule(&state.dependency.rule_id, &assess(&state))
            .is_err()
    );
    assert_eq!(
        catalog.snapshot(id).unwrap().metadata.effective_from,
        old.metadata.effective_from
    );
}
