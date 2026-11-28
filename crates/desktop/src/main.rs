#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod check_view;
mod document_cli;
mod document_versions;
mod folder_watch;
mod layout;
mod library_cli;
mod library_maintenance;
mod library_rules;
mod library_view;
mod provision_view;
mod reader;
mod reader_view;
mod review_tools;
mod source_import;
mod source_updates;
mod text_navigation;
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
        viewport: layout::native_viewport().with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon.png"))
                .expect("the bundled application icon is valid"),
        ),
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
        Some(argument) if argument == "--desktop-quality-check" => {
            if let Err(error) = app::quality::run_cli(arguments.collect()) {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument) if argument == "--document-action" => {
            if let Err(error) = document_cli::run(arguments.collect()) {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument) if argument == "--library-action" => {
            if let Err(error) = library_cli::run(arguments.collect()) {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument) if argument == "--watch-folder" => {
            if let Err(error) = diagnostic_watch(arguments.collect()) {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument) if argument == "--ocr-worker" => {
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("pdf")) {
                std::process::exit(2);
            }
            let settings = match arguments.next() {
                None => complyph_extract::OcrSettings::default(),
                Some(flag) if flag == "--settings" => {
                    let parsed = arguments
                        .next()
                        .and_then(|v| v.into_string().ok())
                        .filter(|v| v.len() <= 2048)
                        .and_then(|v| {
                            serde_json::from_str::<complyph_extract::OcrSettings>(&v).ok()
                        });
                    if arguments.next().is_some() {
                        std::process::exit(2);
                    }
                    match parsed {
                        Some(settings) => settings,
                        None => std::process::exit(2),
                    }
                }
                _ => std::process::exit(2),
            };
            reader::ocr_worker_main(settings);
        }
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
        Some(argument) if argument == "--ocr-file" => {
            if let Err(error) = diagnostic_ocr(arguments.collect()) {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument) if argument == "--import-legal-source" => {
            if let Err(error) = diagnostic_import(arguments.collect()) {
                eprintln!("{error}");
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        Some(argument)
            if argument == "--review-state"
                || argument == "--review-action"
                || argument == "--export-review-pdf" =>
        {
            if let Err(error) = diagnostic_review(argument.to_str().unwrap(), arguments.collect()) {
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
                "ComplyPH desktop\n\nOpen normally to read documents locally. Developer options:\n  --desktop-quality-check --output NEW-JSON --data-dir UNUSED-FIXTURE-DIRECTORY\n  --data-dir PATH\n  --read-file PATH --output NEW-JSON-FILE [--data-dir PATH]\n  --check-file PATH --context CONTEXT-JSON --output NEW-JSON-FILE [--data-dir PATH]\n  --import-legal-source PATH --metadata JSON --output NEW-JSON-FILE --library-dir PATH\n  --library-action --metadata JSON --output NEW-JSON-FILE --library-dir PATH\n  --document-action --metadata JSON --output NEW-JSON-FILE [--data-dir PATH]\n  --review-state ID --output NEW-JSON-FILE [--data-dir PATH]\n  --review-action ID --metadata JSON --output NEW-JSON-FILE [--data-dir PATH]\n  --export-review-pdf ID --output NEW-PDF-FILE [--data-dir PATH]"
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
fn diagnostic_watch(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    if !matches!(arguments.len(), 3 | 5)
        || arguments[1] != "--output"
        || (arguments.len() == 5 && arguments[3] != "--data-dir")
    {
        return Err("Use --watch-folder PATH --output NEW-JSON [--data-dir PATH]".into());
    }
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments[2])
        .map_err(|_| "Choose a new output file; existing files are never overwritten.")?;
    let paths = if arguments.len() == 5 {
        WorkspacePaths::under(PathBuf::from(&arguments[4]))
    } else {
        WorkspacePaths::default_location().map_err(|e| e.to_string())?
    };
    let workspace = complyph_core::Workspace::open(&paths).map_err(|e| e.to_string())?;
    let config = workspace
        .configure_watched_folder(std::path::Path::new(&arguments[0]), true)
        .map_err(|e| e.to_string())?;
    let mut scanner = complyph_core::folder_watch::Scanner::default();
    scanner
        .poll(&workspace, std::time::Instant::now())
        .map_err(|e| e.to_string())?;
    std::thread::sleep(complyph_core::folder_watch::QUIET_PERIOD);
    let scan = scanner
        .poll(&workspace, std::time::Instant::now())
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(
        output,
        &serde_json::json!({"configuration":config,"scan":scan}),
    )
    .map_err(|e| e.to_string())
}

fn diagnostic_ocr(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    use std::io::Read;
    let mut index = 1;
    let settings = if arguments.get(1).is_some_and(|v| v == "--options") {
        let path = arguments
            .get(2)
            .ok_or("Choose a local OCR options JSON file.")?;
        let mut bytes = vec![];
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(4097)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 4096 {
            return Err("The OCR options exceed their limit.".into());
        }
        index = 3;
        Some(serde_json::from_slice::<complyph_extract::OcrSettings>(&bytes).map_err(|_|"The OCR options are invalid; use supported rotation/layout values and no extra fields.")?)
    } else {
        None
    };
    if (arguments.len() != index + 2 && arguments.len() != index + 4)
        || arguments.get(index).is_none_or(|v| v != "--output")
        || (arguments.len() == index + 4 && arguments[index + 2] != "--data-dir")
    {
        return Err(
            "Use --ocr-file PDF [--options JSON] --output NEW-JSON [--data-dir PATH]".into(),
        );
    }
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments[index + 1])
        .map_err(|_| "Choose a new output file; existing files are never overwritten.")?;
    let paths = if arguments.len() == index + 4 {
        WorkspacePaths::under(PathBuf::from(&arguments[index + 3]))
    } else {
        WorkspacePaths::default_location().map_err(|e| e.to_string())?
    };
    let workspace = complyph_core::Workspace::open(&paths).map_err(|e| e.to_string())?;
    let document = workspace
        .register_document(std::path::Path::new(&arguments[0]))
        .map_err(|e| e.to_string())?;
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let saved = match settings {
        Some(settings) => {
            reader::read_document_ocr_with_settings(&paths, &document, settings, &cancel)?
        }
        None => reader::read_document_ocr(&paths, &document, &cancel)?,
    };
    serde_json::to_writer_pretty(output, &saved).map_err(|e| e.to_string())
}

fn diagnostic_review(mode: &str, arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    use std::io::Read;
    let action = mode == "--review-action";
    let base = if action { 5 } else { 3 };
    let output_index = if action { 4 } else { 2 };
    if !(arguments.len() == base || arguments.len() == base + 2)
        || (action && arguments[1] != "--metadata")
        || arguments[output_index - 1] != "--output"
        || (arguments.len() > base && arguments[base] != "--data-dir")
    {
        return Err("Use --review-state ID --output NEW-JSON, --review-action ID --metadata JSON --output NEW-JSON, or --export-review-pdf ID --output NEW-PDF; optionally add --data-dir PATH.".into());
    }
    let id = arguments[0]
        .to_str()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|id| *id > 0)
        .ok_or("A positive saved review ID is required.")?;
    let output = std::path::Path::new(&arguments[output_index]);
    let json_output = if mode == "--export-review-pdf" {
        if output.exists() {
            return Err("Choose a new PDF filename; existing files are never overwritten.".into());
        }
        None
    } else {
        Some(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)
                .map_err(|_| "Choose a new output file; existing files are never overwritten.")?,
        )
    };
    let request = if action {
        let mut bytes = Vec::new();
        std::fs::File::open(&arguments[2])
            .map_err(|e| e.to_string())?
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 32 * 1024 {
            return Err("The reviewer update exceeds its size limit.".into());
        }
        Some(
            serde_json::from_slice::<complyph_core::ReviewActionRequest>(&bytes)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let paths = if arguments.len() > base {
        WorkspacePaths::under(PathBuf::from(&arguments[base + 1]))
    } else {
        WorkspacePaths::default_location().map_err(|e| e.to_string())?
    };
    let workspace = complyph_core::Workspace::open(&paths).map_err(|e| e.to_string())?;
    if let Some(request) = request {
        workspace
            .append_review_action(id, &request)
            .map_err(|e| e.to_string())?;
    }
    let state = workspace.review_workflow(id).map_err(|e| e.to_string())?;
    if let Some(output) = json_output {
        serde_json::to_writer_pretty(output, &state).map_err(|e| e.to_string())
    } else {
        complyph_reports::export_pdf(&state, output).map_err(|e| e.to_string())
    }
}

fn diagnostic_import(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    use std::io::Read;
    if arguments.len() != 7
        || arguments[1] != "--metadata"
        || arguments[3] != "--output"
        || arguments[5] != "--library-dir"
    {
        return Err("Use --import-legal-source PATH --metadata JSON --output NEW-JSON-FILE --library-dir PATH".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&arguments[2])
        .map_err(|e| e.to_string())?
        .take(32 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 {
        return Err("The source metadata exceeds its size limit.".into());
    }
    let input: complyph_library::ManualSourceInput =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let input = input.validated().map_err(|e| e.to_string())?;
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments[4])
        .map_err(|_| "Choose a new output file; existing files are never overwritten.")?;
    let file = source_import::prepare_reference(
        std::path::Path::new(&arguments[0]),
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    let mut catalog = complyph_library::LibraryCatalog::open(std::path::Path::new(&arguments[6]))
        .map_err(|e| e.to_string())?;
    let outcome = catalog
        .import_manual(&input, file)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(output, &outcome).map_err(|e| e.to_string())
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
        .save_review_for_reading(
            document.id,
            &saved,
            library_rules::apply(
                &catalog,
                complyph_checks::review(&saved.report, &saved.sha256, context),
            )?,
        )
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(output, &review).map_err(|e| e.to_string())
}
