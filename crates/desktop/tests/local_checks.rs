use complyph_core::{Workspace, WorkspacePaths};
use std::{fs, path::Path, process::Command};
#[test]
fn installed_path_checks_save_reports_preserve_files_and_reuse_private_cache() {
    let temp = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("expected-checks.json")).unwrap()).unwrap();
    let context = temp.path().join("context.json");
    fs::write(&context, serde_json::to_vec(&expected["context"]).unwrap()).unwrap();
    let paths = WorkspacePaths::under(temp.path().join("data"));
    for (index, (name, outcomes)) in expected["expected"].as_object().unwrap().iter().enumerate() {
        let source = root.join(name);
        let before = fs::read(&source).unwrap();
        let output = temp.path().join(format!("{index}.json"));
        let status = Command::new(env!("CARGO_BIN_EXE_complyph"))
            .arg("--check-file")
            .arg(&source)
            .arg("--context")
            .arg(&context)
            .arg("--output")
            .arg(&output)
            .arg("--data-dir")
            .arg(&paths.root)
            .status()
            .unwrap();
        assert!(status.success(), "{name}");
        let saved: complyph_core::SavedReview =
            serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        let actual = serde_json::to_value(
            saved
                .review
                .findings
                .iter()
                .map(|f| f.outcome)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(actual, *outcomes, "{name}");
        assert_eq!(fs::read(&source).unwrap(), before);
        let workspace = Workspace::open(&paths).unwrap();
        assert_eq!(workspace.reviews().unwrap().len(), index + 1);
        assert_eq!(
            workspace
                .extraction(saved.document_id)
                .unwrap()
                .unwrap()
                .sha256,
            saved.review.document_sha256
        );
    }
    let workspace = Workspace::open(&paths).unwrap();
    let first = workspace.documents().unwrap()[0].clone();
    let cached = workspace.extraction(first.id).unwrap().unwrap();
    drop(workspace);
    let status = Command::new(env!("CARGO_BIN_EXE_complyph"))
        .arg("--check-file")
        .arg(&first.path)
        .arg("--context")
        .arg(&context)
        .arg("--output")
        .arg(temp.path().join("repeat.json"))
        .arg("--data-dir")
        .arg(&paths.root)
        .status()
        .unwrap();
    assert!(status.success());
    let workspace = Workspace::open(&paths).unwrap();
    assert_eq!(
        workspace.extraction(first.id).unwrap().unwrap().read_at,
        cached.read_at
    );
    assert_eq!(workspace.reviews().unwrap().len(), 4);
}
