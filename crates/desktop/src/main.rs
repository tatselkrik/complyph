#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod theme;
mod widgets;

use laborlens_core::WorkspacePaths;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let data_override = parse_arguments();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("LaborLens PH")
            .with_app_id("ph.laborlens.desktop")
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
        "LaborLens PH",
        options,
        Box::new(move |creation| {
            theme::install(&creation.egui_ctx);
            let paths = data_override
                .map(WorkspacePaths::under)
                .map(Ok)
                .unwrap_or_else(WorkspacePaths::default_location);
            Ok(Box::new(app::LaborLensApp::new(paths)))
        }),
    )
}

fn parse_arguments() -> Option<PathBuf> {
    let mut arguments = std::env::args_os().skip(1);
    match arguments.next() {
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
                "LaborLens PH desktop\n\nOpen the application normally, or use --data-dir PATH for an isolated test workspace."
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
