//! Exercises the actual shipped egui UI without a GPU or a user's workspace.
use std::{fs, process::Command};

#[test]
fn compact_layout_keyboard_and_failed_reads_preserve_the_saved_history() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("evidence.json");
    let fixtures = temp.path().join("synthetic");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_complyph"))
            .arg("--desktop-quality-check")
            .arg("--output")
            .arg(&output)
            .arg("--data-dir")
            .arg(&fixtures)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&output).unwrap();
    let evidence: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(evidence["profile"], "initial-desktop-quality-v1");
    let layouts = evidence["layout_cases"].as_array().unwrap();
    assert_eq!(layouts.len(), 12);
    for (width, height) in [(1280, 960), (1920, 1080), (2560, 1440)] {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let case = layouts
                .iter()
                .find(|c| {
                    c["physical_width"] == width
                        && c["physical_height"] == height
                        && c["scale"] == scale
                })
                .unwrap();
            assert_eq!(case["passed"], true);
            assert_eq!(case["scenarios"], 14);
        }
    }
    for (name, count) in [("keyboard_cases", 5), ("recovery_cases", 3)] {
        let cases = evidence[name].as_array().unwrap();
        assert_eq!(cases.len(), count);
        assert!(cases.iter().all(|c| c["passed"] == true));
    }
    assert_eq!(evidence["interactive_windows_display_tested"], false);
    assert_eq!(evidence["qualified_legal_review"], false);
    // Refuse both an existing result and an existing workspace before opening either.
    assert!(!run().status.success());
    assert_eq!(fs::read(&output).unwrap(), bytes);
    fs::remove_file(&output).unwrap();
    let database = fixtures.join("data/private/workspace.sqlite3");
    let before = fs::read(&database).unwrap();
    assert!(!run().status.success());
    assert!(!output.exists());
    assert_eq!(fs::read(database).unwrap(), before);
}
