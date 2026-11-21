//! Exercise reference intake through the shipped executable and bounded parser.
use complyph_library::{
    ImportOutcome, LibraryCatalog, ManualSourceInput, ManualVerification, Provenance,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn import(original: &Path, metadata: &Path, output: &Path, library: &Path) -> bool {
    Command::new(env!("CARGO_BIN_EXE_complyph"))
        .args(["--import-legal-source"])
        .arg(original)
        .arg("--metadata")
        .arg(metadata)
        .arg("--output")
        .arg(output)
        .arg("--library-dir")
        .arg(library)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .status()
        .unwrap()
        .success()
}

#[test]
fn executable_imports_pdf_and_docx_locally_with_history_and_separate_verification() {
    let temp = tempfile::tempdir().unwrap();
    let public = temp.path().join("library");
    fs::create_dir(temp.path().join("private")).unwrap();
    let private = temp.path().join("private/workspace.sqlite3");
    fs::write(&private, b"private sentinel: importer must not open this").unwrap();
    let pdf = temp.path().join("Synthetic.pdf");
    let mut bytes = fs::read(examples().join("Synthetic employment.pdf")).unwrap();
    fs::write(&pdf, &bytes).unwrap();
    let metadata = temp.path().join("source.json");
    fs::copy(examples().join("synthetic-source.json"), &metadata).unwrap();
    let output = temp.path().join("first.json");
    assert!(import(&pdf, &metadata, &output, &public));
    let first: ImportOutcome = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert!(first.added && !first.new_version);
    assert_eq!(first.metadata.provenance, Provenance::ManualPending);
    assert_eq!(first.metadata.verified_at_unix, 0);
    assert!(
        !serde_json::to_string(&first)
            .unwrap()
            .contains(temp.path().to_str().unwrap())
    );
    assert_eq!(fs::read(&pdf).unwrap(), bytes);
    let duplicate = temp.path().join("duplicate.json");
    assert!(import(&pdf, &metadata, &duplicate, &public));
    let duplicate: ImportOutcome = serde_json::from_slice(&fs::read(duplicate).unwrap()).unwrap();
    assert!(!duplicate.added);
    let position = bytes
        .windows(14)
        .position(|p| p == b"PAGE-TWO-TERMS")
        .unwrap();
    bytes[position..position + 14].copy_from_slice(b"PAGE-TWO-EDIT!");
    fs::write(&pdf, &bytes).unwrap();
    let output = temp.path().join("new-version.json");
    assert!(import(&pdf, &metadata, &output, &public));
    let new: ImportOutcome = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert!(new.added && new.new_version);
    assert_eq!(first.source_id, new.source_id);
    assert_ne!(first.metadata.raw_sha256, new.metadata.raw_sha256);
    let mut input: ManualSourceInput =
        serde_json::from_slice(&fs::read(&metadata).unwrap()).unwrap();
    input.official_url = "https://example.invalid/complyph/synthetic-reference.docx".into();
    input.verification = ManualVerification::IdentityChecked;
    input.verification_note =
        "Synthetic user-comparison record for a fictional publication.".into();
    fs::write(&metadata, serde_json::to_vec(&input).unwrap()).unwrap();
    let docx = examples().join("Synthetic workplace policy.docx");
    let docx_before = fs::read(&docx).unwrap();
    assert!(import(
        &docx,
        &metadata,
        &temp.path().join("word.json"),
        &public
    ));
    assert_eq!(fs::read(&docx).unwrap(), docx_before);
    let catalog = LibraryCatalog::open(&public).unwrap();
    let summary = catalog.summary().unwrap();
    assert_eq!(summary.source_count, 21);
    assert_eq!(summary.verified_source_count, 14);
    assert_eq!(summary.awaiting_verification_count, 1);
    assert_eq!(summary.user_checked_source_count, 1);
    assert_eq!(summary.manually_imported_source_count, 2);
    assert_eq!(
        summary.automatic_updates_available,
        cfg!(any(windows, target_os = "linux"))
    );
    assert_eq!(summary.last_successful_check, None);
    assert!(
        catalog
            .snapshot_for_review(
                &first.metadata.key,
                &first.metadata.raw_sha256,
                &first.metadata.text_sha256
            )
            .unwrap()
            .text
            .contains("PAGE-TWO-TERMS")
    );
    assert!(
        catalog
            .snapshot(first.source_id)
            .unwrap()
            .text
            .contains("PAGE-TWO-EDIT!")
    );
    assert_eq!(
        catalog
            .snapshot_by_key("labor-code-reference")
            .unwrap()
            .metadata
            .raw_sha256,
        complyph_checks::SOURCE_SHA256
    );
    assert_eq!(
        fs::read(private).unwrap(),
        b"private sentinel: importer must not open this"
    );
}

#[test]
fn failed_intake_and_existing_outputs_do_not_create_or_modify_a_catalog() {
    let temp = tempfile::tempdir().unwrap();
    let metadata = examples().join("synthetic-source.json");
    let public = temp.path().join("library");
    for (index, name) in [
        "Synthetic scan.pdf",
        "Synthetic protected.pdf",
        "Synthetic damaged.docx",
    ]
    .iter()
    .enumerate()
    {
        let original = examples().join(name);
        let before = fs::read(&original).unwrap();
        assert!(!import(
            &original,
            &metadata,
            &temp.path().join(format!("failed-{index}.json")),
            &public
        ));
        assert_eq!(fs::read(original).unwrap(), before);
        assert!(!public.exists());
    }
    let original = examples().join("Synthetic employment.pdf");
    let before = fs::read(&original).unwrap();
    assert!(!import(&original, &metadata, &original, &public));
    assert_eq!(fs::read(original).unwrap(), before);
    assert!(!public.exists());
    assert!(!temp.path().join("private").exists());
}
