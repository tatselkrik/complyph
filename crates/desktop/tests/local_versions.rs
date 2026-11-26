//! The actual application and reader, including the installed-app acceptance matrix.
use std::{path::Path, process::Command};
#[test]
fn real_document_revision_workflows_preserve_versions_histories_and_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("acceptance.json");
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(root.join("scripts/test-document-revisions.py"))
        .arg("--app")
        .arg(env!("CARGO_BIN_EXE_complyph"))
        .arg("--samples")
        .arg(root.join("examples"))
        .arg("--work")
        .arg(temp.path().join("fixture"))
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let evidence: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(evidence["workflow_cases"].as_array().unwrap().len(), 6);
    assert!(
        evidence["checks"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v == true)
    );
    assert_eq!(evidence["pair_results"].as_array().unwrap().len(), 2);
}
