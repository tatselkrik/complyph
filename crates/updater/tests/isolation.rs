use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn direct_network_worker_refuses_to_run_without_os_isolation() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_complyph-updater"))
        .arg("--network-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let _=child.stdin.take().unwrap().write_all(br#"{"type":"fetch","url":"https://privacy.gov.ph/data-privacy-act/","etag":null,"last_modified":null}"#);
    let output = child.wait_with_output().unwrap();
    assert!(output.stdout.len() > 4);
    let n = u32::from_le_bytes(output.stdout[..4].try_into().unwrap()) as usize;
    let header: serde_json::Value = serde_json::from_slice(&output.stdout[4..4 + n]).unwrap();
    assert_eq!(header["type"], "failed");
    assert_eq!(output.stdout.len(), 4 + n);
}

#[test]
fn unapproved_catalog_status_and_empty_batch_never_open_private_documents() {
    let temp = tempfile::tempdir().unwrap();
    let private = temp.path().join("private");
    std::fs::create_dir(&private).unwrap();
    let sentinel = private.join("workspace.sqlite3");
    std::fs::write(&sentinel, b"private canary").unwrap();
    let library = temp.path().join("library");
    for mode in ["--status", "--check"] {
        let result = Command::new(env!("CARGO_BIN_EXE_complyph-updater"))
            .arg(mode)
            .arg("--library-dir")
            .arg(&library)
            .output()
            .unwrap();
        assert!(result.status.success());
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        if mode == "--check" {
            assert_eq!(value["checks"].as_array().unwrap().len(), 0);
        }
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"private canary");
    }
}

#[cfg(windows)]
#[test]
fn appcontainer_cannot_open_the_catalog_private_store_or_original_document() {
    let temp = tempfile::tempdir().unwrap();
    let protected = temp.path().join("protected");
    complyph_paths::secure_directory(&protected).unwrap();
    let files = [
        protected.join("workspace.sqlite3"),
        protected.join("catalog.sqlite3"),
        temp.path().join("Synthetic original.txt"),
    ];
    for file in &files {
        std::fs::write(file, b"synthetic canary: no network worker may read this").unwrap();
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_complyph-updater"));
    command.arg("--self-test-isolation").env_clear();
    for name in [
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    for file in &files {
        command.arg("--probe-file").arg(file);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["app_container"], true);
    assert_eq!(value["all_files_denied"], true);
    assert_eq!(value["files_tested"], 3);
    for file in &files {
        assert_eq!(
            std::fs::read(file).unwrap(),
            b"synthetic canary: no network worker may read this"
        );
    }
}
