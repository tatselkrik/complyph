#![cfg(target_os = "linux")]
use std::{fs, process::Command};

#[test]
fn linux_worker_denies_public_catalog_private_store_and_original_or_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let files = [
        temp.path().join("workspace.sqlite3"),
        temp.path().join("catalog.sqlite3"),
        temp.path().join("Synthetic original.docx"),
    ];
    let canary = b"Fictional file-denial sentinel; never a network input.";
    let mut command = Command::new(env!("CARGO_BIN_EXE_complyph-updater"));
    command.arg("--self-test-isolation").env_clear();
    for path in &files {
        fs::write(path, canary).unwrap();
        command.arg("--probe-file").arg(path);
    }
    let result = command.output().unwrap();
    if result.status.success() {
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["app_container"], false);
        assert_eq!(value["isolation"], "linux_landlock_seccomp");
        assert_eq!(value["all_files_denied"], true);
        assert_eq!(value["files_tested"], 3);
    } else {
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(
            error.contains("Landlock") || error.contains("isolation") || error.contains("isolated"),
            "{error}"
        );
        assert!(
            result.stdout.is_empty(),
            "Failed confinement must not yield a usable response."
        );
        assert_ne!(
            std::env::var("COMPLYPH_REQUIRE_LINUX_ISOLATION").as_deref(),
            Ok("1"),
            "The release CI kernel must pass real file-denial confinement: {error}"
        );
    }
    for path in &files {
        assert_eq!(fs::read(path).unwrap(), canary);
    }
}

#[test]
fn linux_confined_worker_rejects_unknown_request_fields_before_http() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_complyph-updater"))
        .arg("--linux-network-worker")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let _=child.stdin.take().unwrap().write_all(br#"{"type":"fetch","url":"https://privacy.gov.ph/","etag":null,"last_modified":null,"document_text":"FICTIONAL DO NOT SEND"}"#);
    let result = child.wait_with_output().unwrap();
    assert!(result.stdout.len() >= 4);
    let length = u32::from_le_bytes(result.stdout[..4].try_into().unwrap()) as usize;
    let reply: serde_json::Value = serde_json::from_slice(&result.stdout[4..length + 4]).unwrap();
    assert_eq!(reply["type"], "failed");
    assert_eq!(result.stdout.len(), length + 4);
}
