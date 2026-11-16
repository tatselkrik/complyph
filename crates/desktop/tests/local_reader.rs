//! Exercise the shipped executable and its child worker, not just parser functions.
use complyph_core::{SavedExtraction, Workspace, WorkspacePaths, document_snapshot};
use std::{fs, path::Path, process::Command};

fn examples() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn read(original: &Path, output: &Path, data: &Path) -> SavedExtraction {
    let status = Command::new(env!("CARGO_BIN_EXE_complyph"))
        .arg("--read-file")
        .arg(original)
        .arg("--output")
        .arg(output)
        .arg("--data-dir")
        .arg(data)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .status()
        .unwrap();
    assert!(
        status.success(),
        "headless executable reader failed: {original:?}"
    );
    serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
}

#[test]
fn shipped_executable_reads_all_synthetic_formats_without_changing_originals() {
    let temp = tempfile::TempDir::new().unwrap();
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(examples().join("expected-results.json")).unwrap())
            .unwrap();
    for (index, (name, expected)) in expected.as_object().unwrap().iter().enumerate() {
        let original = examples().join(name);
        let before = fs::read(&original).unwrap();
        let result = read(
            &original,
            &temp.path().join(format!("result-{index}.json")),
            &temp.path().join("workspace"),
        );
        assert_eq!(
            serde_json::to_value(result.report.status).unwrap(),
            expected["status"],
            "{name}"
        );
        assert_eq!(fs::read(original).unwrap(), before);
    }
    assert!(
        !temp.path().join("workspace/library").exists(),
        "private reader initialized the public catalog"
    );
}

#[test]
fn content_hash_replaces_stale_cache_and_removal_preserves_original() {
    let temp = tempfile::TempDir::new().unwrap();
    let data = temp.path().join("workspace");
    let original = temp.path().join("Synthetic.pdf");
    let mut bytes = fs::read(examples().join("Synthetic employment.pdf")).unwrap();
    fs::write(&original, &bytes).unwrap();
    let first = read(&original, &temp.path().join("first.json"), &data);
    let cached = read(&original, &temp.path().join("cached.json"), &data);
    assert_eq!(first.sha256, cached.sha256);
    assert_eq!(first.read_at, cached.read_at);
    // Same-size change: timestamps/size alone must never validate a text cache.
    let position = bytes
        .windows(14)
        .position(|window| window == b"PAGE-TWO-TERMS")
        .unwrap();
    bytes[position..position + 14].copy_from_slice(b"PAGE-TWO-EDIT!");
    fs::write(&original, &bytes).unwrap();
    let changed = read(&original, &temp.path().join("changed.json"), &data);
    assert_ne!(first.sha256, changed.sha256);
    assert!(changed.report.units[1].text.contains("PAGE-TWO-EDIT!"));
    assert_eq!(changed.sha256, document_snapshot(&original).unwrap().sha256);
    let workspace = Workspace::open(&WorkspacePaths::under(&data)).unwrap();
    let document = workspace.documents().unwrap().remove(0);
    assert_eq!(workspace.documents().unwrap().len(), 1);
    workspace.remove_document(document.id).unwrap();
    assert!(workspace.extraction(document.id).unwrap().is_none());
    assert_eq!(fs::read(&original).unwrap(), bytes);
}

#[test]
fn diagnostic_mode_refuses_to_overwrite_a_document_or_existing_export() {
    let temp = tempfile::TempDir::new().unwrap();
    let original = temp.path().join("Synthetic.pdf");
    let bytes = fs::read(examples().join("Synthetic employment.pdf")).unwrap();
    fs::write(&original, &bytes).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_complyph"))
        .arg("--read-file")
        .arg(&original)
        .arg("--output")
        .arg(&original)
        .arg("--data-dir")
        .arg(temp.path().join("data"))
        .status()
        .unwrap();
    assert!(!status.success());
    assert_eq!(fs::read(&original).unwrap(), bytes);
    assert!(!temp.path().join("data").exists());
}
