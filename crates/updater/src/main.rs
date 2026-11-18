//! Public catalog coordinator. Only its AppContainer child implements HTTP.
#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
mod http;
mod protocol;
mod sandbox;

use complyph_library::{CheckReceipt, LibraryCatalog, UpdateReply, default_library_directory};
use protocol::{Reply, Request};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct BatchReport {
    supported: bool,
    checks: Vec<CheckReceipt>,
    remaining_due: usize,
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--network-worker") {
        worker();
        return;
    }
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn worker() {
    let result = (|| -> Result<(Reply, Vec<u8>), String> {
        sandbox::require_isolation()?;
        let request = protocol::read_request()?;
        match &request {
            Request::Probe { files } => {
                if files.is_empty() || files.len() > 8 {
                    return Err("Invalid isolation probe.".into());
                }
                Ok((
                    Reply::Probe {
                        app_container: true,
                        all_files_denied: files.iter().all(|p| std::fs::File::open(p).is_err()),
                        files_tested: files.len(),
                    },
                    Vec::new(),
                ))
            }
            Request::Fetch { .. } => {
                #[cfg(windows)]
                {
                    Ok(http::fetch(&request))
                }
                #[cfg(not(windows))]
                {
                    Err("This platform has no enabled network worker.".into())
                }
            }
        }
    })();
    let (reply, body) = result.unwrap_or_else(|message| (Reply::Failed { message }, Vec::new()));
    if protocol::encode(&reply, &body, std::io::stdout().lock()).is_err() {
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mut directory = None;
    let mut check = false;
    let mut due = false;
    let mut probe = false;
    let mut files = Vec::new();
    let mut desktop_session = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--status") => {}
            Some("--check") => check = true,
            Some("--due") => due = true,
            Some("--library-dir") => {
                directory = Some(PathBuf::from(
                    args.next().ok_or("--library-dir requires a directory")?,
                ))
            }
            Some("--self-test-isolation") => probe = true,
            Some("--desktop-session") => desktop_session = true,
            Some("--probe-file") => files.push(PathBuf::from(
                args.next()
                    .ok_or("--probe-file requires a synthetic fixture path")?,
            )),
            Some("--help" | "-h") => {
                println!(
                    "ComplyPH source updater\n\n--status  Read public catalog status without networking\n--check [--due]  Check approved source URLs; --due skips attempts within 24 hours\n--library-dir PATH  Use an explicit public catalog\n--self-test-isolation --probe-file PATH  Test denial of a synthetic file\n\nNetworking requires the Windows AppContainer. No private workspace is opened."
                );
                return Ok(());
            }
            _ => return Err("Unknown updater argument. Use --help.".into()),
        }
    }
    if probe {
        if files.is_empty() || files.iter().any(|p| !p.is_file()) {
            return Err("Isolation probes require existing synthetic fixture files.".into());
        }
        let launcher = sandbox::Launcher::new()?;
        let (reply, _) = launcher.run(&Request::Probe { files })?;
        match reply {
            Reply::Probe {
                app_container: true,
                all_files_denied: true,
                ..
            } => println!(
                "{}",
                serde_json::to_string(&reply).map_err(|e| e.to_string())?
            ),
            _ => {
                return Err(
                    "The downloader isolation probe failed. Networking must remain disabled."
                        .into(),
                );
            }
        }
        return Ok(());
    }
    if !files.is_empty() {
        return Err("--probe-file requires --self-test-isolation.".into());
    }
    if desktop_session {
        use std::io::Read;
        let mut signal = [0u8; 1];
        std::io::stdin()
            .read_exact(&mut signal)
            .map_err(|_| "The desktop closed before starting its update session.")?;
        if signal != [1] {
            return Err("Invalid desktop update session.".into());
        }
    }
    let directory = directory
        .map(Ok)
        .unwrap_or_else(default_library_directory)
        .map_err(|e| e.to_string())?;
    let mut catalog = LibraryCatalog::open(&directory).map_err(|e| e.to_string())?;
    if !check {
        println!(
            "{}",
            serde_json::to_string_pretty(&catalog.summary().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let ids = catalog.due_source_ids(due).map_err(|e| e.to_string())?;
    let mut checks = Vec::new();
    if !ids.is_empty() {
        let launcher = sandbox::Launcher::new();
        for id in ids {
            let Some(target) = catalog
                .claim_source_check(id, due)
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let result = match &launcher {
                Ok(launcher) => launcher.run(&Request::Fetch {
                    url: target.url.clone(),
                    etag: target.etag.clone(),
                    last_modified: target.last_modified.clone(),
                }),
                Err(error) => Err(error.clone()),
            };
            let reply = match result {
                Ok((
                    Reply::Download {
                        resolved_url,
                        mime,
                        etag,
                        last_modified,
                    },
                    bytes,
                )) => UpdateReply::Download {
                    resolved_url,
                    mime,
                    etag,
                    last_modified,
                    bytes,
                },
                Ok((Reply::NotModified { resolved_url }, _)) => {
                    UpdateReply::NotModified { resolved_url }
                }
                Ok((Reply::Failed { message }, _)) | Err(message) => {
                    UpdateReply::Failed { message }
                }
                _ => UpdateReply::Failed {
                    message: "The isolated downloader returned an unexpected result.".into(),
                },
            };
            checks.push(
                catalog
                    .finish_source_check(&target, reply)
                    .map_err(|e| e.to_string())?,
            );
        }
    }
    let report = BatchReport {
        supported: cfg!(windows),
        checks,
        remaining_due: catalog
            .due_source_ids(true)
            .map_err(|e| e.to_string())?
            .len(),
    };
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_rejects_hidden_fields_truncation_and_unexpected_body() {
        assert!(serde_json::from_str::<Request>(r#"{"type":"fetch","url":"https://privacy.gov.ph/","etag":null,"last_modified":null,"document":"private"}"#).is_err());
        assert!(protocol::decode(&[1, 0, 0, 0]).is_err());
        let mut bytes = Vec::new();
        protocol::encode(
            &Reply::Failed {
                message: "failure".into(),
            },
            b"private",
            &mut bytes,
        )
        .unwrap();
        assert!(protocol::decode(&bytes).is_err());
    }
}
