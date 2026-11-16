//! Private reader orchestration. Only document bytes cross the child-process pipe.
//! The public catalog/updater never receives a document path, hash, or excerpt.

mod limits;

use complyph_core::{Document, SavedExtraction, Workspace, WorkspacePaths, document_snapshot};
use complyph_extract::{ENGINE, Format, MAX_INPUT_BYTES, MAX_REPORT_BYTES, Report, Status};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub type Cancel = Arc<AtomicBool>;

pub fn read_document(
    paths: &WorkspacePaths,
    document: &Document,
    force: bool,
    cancel: &AtomicBool,
) -> Result<SavedExtraction, String> {
    check_cancel(cancel)?;
    let snapshot = document_snapshot(&document.path).map_err(|error| error.to_string())?;
    let workspace = Workspace::open(paths).map_err(|error| error.to_string())?;
    if !force
        && let Some(cached) = workspace
            .extraction(document.id)
            .map_err(|error| error.to_string())?
        && cached.sha256 == snapshot.sha256
        && cached.report.engine == ENGINE
    {
        check_cancel(cancel)?;
        return Ok(cached);
    }
    check_cancel(cancel)?;
    let report = run_worker(document.kind.format(), snapshot.bytes.clone(), cancel)?;
    check_cancel(cancel)?;
    let current = document_snapshot(&document.path).map_err(|error| error.to_string())?;
    if current.sha256 != snapshot.sha256 {
        return Err("The document changed during reading. Save it and choose Read again.".into());
    }
    check_cancel(cancel)?;
    workspace
        .save_extraction(document, &snapshot, report)
        .map_err(|error| error.to_string())
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Reading cancelled. The original is unchanged.".into())
    } else {
        Ok(())
    }
}

fn run_worker(format: Format, bytes: Vec<u8>, cancel: &AtomicBool) -> Result<Report, String> {
    let executable =
        std::env::current_exe().map_err(|_| "The local reader could not be located.")?;
    let mut command = Command::new(executable);
    command.arg("--extract-worker").arg(match format {
        Format::Pdf => "pdf",
        Format::Docx => "docx",
    });
    // No inherited service credentials, proxy settings, or document-derived arguments.
    command.env_clear();
    for name in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = supervise(command, bytes, cancel, Duration::from_secs(20))?;
    let report: Report = serde_json::from_slice(&output)
        .map_err(|_| "The local reader returned an invalid result. The original is unchanged.")?;
    if report.engine != ENGINE
        || report.format != format
        || report.units.len() > complyph_extract::MAX_UNITS
        || report.text_bytes() > complyph_extract::MAX_TEXT_BYTES
    {
        return Err("The local reader returned an unsupported result.".into());
    }
    Ok(report)
}

fn supervise(
    mut command: Command,
    bytes: Vec<u8>,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    check_cancel(cancel)?;
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "The local reading process could not start.")?;
    let mut input = child.stdin.take().expect("piped stdin");
    let output = child.stdout.take().expect("piped stdout");
    let writer = std::thread::spawn(move || input.write_all(&bytes));
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .take(MAX_REPORT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let started = Instant::now();
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            break Err("Reading cancelled. The original is unchanged.");
        }
        if started.elapsed() >= timeout {
            break Err(
                "Reading stopped after 20 seconds. This file may be too complex or damaged. The original is unchanged.",
            );
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(_)) => {
                break Err(
                    "The local reader stopped safely. The file may exceed its memory/CPU limits, be damaged, or use unsupported content. The original is unchanged.",
                );
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break Err("The local reader could not be monitored."),
        }
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let _ = writer.join();
    let bytes = reader
        .join()
        .map_err(|_| "The local reader output could not be collected.")?
        .map_err(|_| "The local reader output could not be read.")?;
    result.map_err(str::to_owned)?;
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err("The local reader result exceeds its size limit.".into());
    }
    Ok(bytes)
}

/// Internal helper: bounded memory/CPU, no window, no file-path argument, no disk writes.
/// Always terminate before desktop or catalog initialization.
pub fn worker_main(format: Format) -> ! {
    let guard = limits::apply();
    let report = if guard.is_err() {
        Report::failure(
            format,
            Status::Unreadable,
            "The operating system could not apply local reader limits.",
        )
    } else {
        let mut bytes = Vec::new();
        match std::io::stdin().lock().take(MAX_INPUT_BYTES as u64 + 1).read_to_end(&mut bytes) {
            Ok(_) => std::panic::catch_unwind(|| complyph_extract::extract(format, &bytes)).unwrap_or_else(|_| {
                Report::failure(format, Status::Unreadable, "This document uses content the reader could not process. The original is unchanged.")
            }),
            Err(_) => Report::failure(format, Status::Unreadable, "The local reader could not receive the document."),
        }
    };
    let result = serde_json::to_writer(std::io::stdout().lock(), &report);
    drop(guard);
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_stops_before_starting_a_process() {
        let cancel = AtomicBool::new(true);
        let result = supervise(
            Command::new("this-program-does-not-exist"),
            vec![],
            &cancel,
            Duration::from_secs(1),
        );
        assert!(result.unwrap_err().contains("cancelled"));
    }

    #[test]
    fn supervisor_terminates_a_stalled_child() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "reader::tests::deliberately_stalled_fixture",
            "--ignored",
        ]);
        let start = Instant::now();
        let result = supervise(
            command,
            vec![],
            &AtomicBool::new(false),
            Duration::from_millis(100),
        );
        assert!(result.unwrap_err().contains("stopped after"));
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    #[ignore = "child-process fixture; the supervisor test invokes and terminates this"]
    fn deliberately_stalled_fixture() {
        std::thread::sleep(Duration::from_secs(120));
    }
}
