//! Invokes the same real-executable acceptance path used by the Windows installer.
use std::{path::Path, process::Command};
#[test]
fn real_difficult_document_cases_preserve_originals_metadata_histories_and_manual_review_gates() {
    let temp = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(root.join("scripts/test-reading-quality.py"))
        .args(["--app", env!("CARGO_BIN_EXE_complyph")])
        .arg("--samples")
        .arg(root.join("examples"))
        .arg("--work")
        .arg(temp.path().join("cases"))
        .arg("--output")
        .arg(temp.path().join("evidence.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: serde_json::Value =
        serde_json::from_slice(&std::fs::read(temp.path().join("evidence.json")).unwrap()).unwrap();
    assert_eq!(evidence["workflow_cases"].as_array().unwrap().len(), 6);
    assert_eq!(evidence["reader_results"].as_array().unwrap().len(), 8);
    assert_eq!(evidence["ocr_results"].as_array().unwrap().len(), 6);
}
