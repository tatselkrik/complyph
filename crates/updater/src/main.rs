//! A public-catalog status entry point. Network synchronization is not enabled.
//! Deliberately does not depend on complyph-core or any HTTP client.

use complyph_library::{LibraryCatalog, default_library_directory};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut directory = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--library-dir" => {
                directory = Some(PathBuf::from(
                    arguments.next().ok_or("--library-dir needs a directory")?,
                ));
            }
            "--status" => {}
            "--help" | "-h" => {
                println!(
                    "ComplyPH updater foundation\n\n--status  Read local public-catalog status\n--library-dir PATH  Use a specific public-library directory\n\nAutomatic downloads are not implemented. This binary makes no network requests."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {argument}").into()),
        }
    }
    let directory = match directory {
        Some(directory) => directory,
        None => default_library_directory()?,
    };
    let summary = LibraryCatalog::open(&directory)?.summary()?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
