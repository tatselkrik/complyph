#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod check_view;
mod library_view;
mod reader;
mod reader_view;
mod theme;
mod widgets;

use complyph_core::WorkspacePaths;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let data_override = parse_arguments();
    // Resolve/migrate before creating a desktop window or opening a database.
    let paths = data_override
        .map(WorkspacePaths::under)
        .map(Ok)
        .unwrap_or_else(WorkspacePaths::default_location);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("ComplyPH")
            .with_app_id("ph.complyph.desktop")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon.png"))
                    .expect("the bundled application icon is valid"),
            )
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([940.0, 680.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "ComplyPH",
        options,
        Box::new(move |creation| {
            theme::install(&creation.egui_ctx);
            Ok(Box::new(app::ComplyApp::new(paths)))
        }),
    )
}

fn parse_arguments() -> Option<PathBuf> {
    let mut arguments = std::env::args_os().skip(1);
    match arguments.next() {
        Some(argument) if argument == "--extract-worker" => {
            let format = match arguments.next().as_deref().and_then(|value| value.to_str()) {
                Some("pdf") => complyph_extract::Format::Pdf,
                Some("docx") => complyph_extract::Format::Docx,
                _ => std::process::exit(2),
            };
            if arguments.next().is_some() {
                std::process::exit(2);
            }
            reader::worker_main(format);
        }
        Some(argument) if argument == "--check-file" => {
            if let Err(error) = diagnostic_check(arguments.collect()) {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument) if argument == "--read-file" => {
            // Diagnostic entry point for isolated installer/reader verification.
            let result = diagnostic_read(arguments.collect());
            if let Err(error) = result {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument) if argument == "--data-dir" => {
            let Some(path) = arguments.next() else {
                eprintln!("--data-dir requires a path");
                std::process::exit(2);
            };
            if arguments.next().is_some() {
                eprintln!("Unexpected argument after --data-dir");
                std::process::exit(2);
            }
            Some(PathBuf::from(path))
        }
        Some(argument) if argument == "--help" || argument == "-h" => {
            println!(
                "ComplyPH desktop\n\nOpen normally to read documents locally. Developer options:\n  --data-dir PATH\n  --read-file PATH --output NEW-JSON-FILE [--data-dir PATH]\n  --check-file PATH --context CONTEXT-JSON --output NEW-JSON-FILE [--data-dir PATH]"
            );
            std::process::exit(0);
        }
        Some(_) => {
            eprintln!("Unknown argument. Use --help for usage.");
            std::process::exit(2);
        }
        None => None,
    }
}

fn diagnostic_read(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    if !matches!(arguments.len(), 3 | 5)
        || arguments[1] != "--output"
        || (arguments.len() == 5 && arguments[3] != "--data-dir")
    {
        return Err("Use --read-file PATH --output NEW-JSON-FILE [--data-dir PATH]".into());
    }
    // Refuse to overwrite any existing file, including the original document.
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments[2])
        .map_err(|_| "Choose a new output file; existing files are never overwritten.")?;
    let paths = if arguments.len() == 5 {
        WorkspacePaths::under(PathBuf::from(&arguments[4]))
    } else {
        WorkspacePaths::default_location().map_err(|error| error.to_string())?
    };
    let workspace = complyph_core::Workspace::open(&paths).map_err(|error| error.to_string())?;
    let document = workspace
        .register_document(std::path::Path::new(&arguments[0]))
        .map_err(|error| error.to_string())?;
    let saved = reader::read_document(
        &paths,
        &document,
        false,
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    serde_json::to_writer_pretty(output, &saved)
        .map_err(|_| "The diagnostic result could not be saved.".into())
}

fn diagnostic_check(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    if !matches!(arguments.len(), 5 | 7)
        || arguments[1] != "--context"
        || arguments[3] != "--output"
        || (arguments.len() == 7 && arguments[5] != "--data-dir")
    {
        return Err(
            "Use --check-file PATH --context CONTEXT-JSON --output NEW-JSON-FILE [--data-dir PATH]"
                .into(),
        );
    }
    let context: complyph_checks::Context =
        serde_json::from_slice(&std::fs::read(&arguments[2]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments[4])
        .map_err(|_| "Choose a new output file; existing files are never overwritten.")?;
    let paths = if arguments.len() == 7 {
        WorkspacePaths::under(PathBuf::from(&arguments[6]))
    } else {
        WorkspacePaths::default_location().map_err(|e| e.to_string())?
    };
    let workspace = complyph_core::Workspace::open(&paths).map_err(|e| e.to_string())?;
    let catalog =
        complyph_library::LibraryCatalog::open(&paths.library_dir).map_err(|e| e.to_string())?;
    check_view::approved_source(&catalog)?;
    let document = workspace
        .register_document(std::path::Path::new(&arguments[0]))
        .map_err(|e| e.to_string())?;
    let saved = reader::read_document(
        &paths,
        &document,
        false,
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    let current = complyph_core::document_snapshot(&document.path).map_err(|e| e.to_string())?;
    if current.sha256 != saved.sha256 {
        return Err("The original changed; read again before checking.".into());
    }
    let review = workspace
        .save_review(
            document.id,
            complyph_checks::review(&saved.report, &saved.sha256, context),
        )
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(output, &review).map_err(|e| e.to_string())
}
