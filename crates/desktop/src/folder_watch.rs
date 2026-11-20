use crate::{theme::*, widgets};
use complyph_core::{
    Workspace, WorkspacePaths,
    folder_watch::{Configuration, Scan, Scanner},
};
use eframe::egui::{self, RichText, Ui};
use egui_file_dialog::FileDialog;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
type Reply = (Scanner, Result<Scan, String>, Option<Configuration>);
pub struct FolderWatch {
    picker: FileDialog,
    config: Option<Configuration>,
    scanner: Option<Scanner>,
    receiver: Option<mpsc::Receiver<Reply>>,
    next: Instant,
    last: Option<Scan>,
    error: Option<String>,
}
impl Default for FolderWatch {
    fn default() -> Self {
        Self {
            picker: FileDialog::new()
                .title("Choose a local document intake folder")
                .default_size([760.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show_devices(false)
                .show_removable_devices(false)
                .show_hidden_option(false)
                .as_modal(true),
            config: None,
            scanner: Some(Scanner::default()),
            receiver: None,
            next: Instant::now(),
            last: None,
            error: None,
        }
    }
}
impl FolderWatch {
    pub fn tick(
        &mut self,
        ctx: &egui::Context,
        workspace: Option<&Workspace>,
        paths: Option<&WorkspacePaths>,
    ) -> bool {
        let (Some(workspace), Some(paths)) = (workspace, paths) else {
            return false;
        };
        let mut changed = false;
        self.picker.update(ctx);
        if let Some(root) = self.picker.take_picked() {
            match workspace.configure_watched_folder(&root, true) {
                Ok(config) => {
                    self.config = Some(config);
                    self.next = Instant::now();
                    self.error = None;
                    self.last = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok((scanner, result, config)) => {
                    self.scanner = Some(scanner);
                    self.receiver = None;
                    if config == workspace.watched_folder().ok().flatten() {
                        match result {
                            Ok(scan) => {
                                changed = !scan.added.is_empty() || scan.updated > 0;
                                self.last = Some(scan);
                                self.error = None;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                    self.next = Instant::now() + Duration::from_secs(5);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.scanner = Some(Scanner::default());
                    self.error = Some("Folder intake stopped. It will retry locally.".into());
                    self.next = Instant::now() + Duration::from_secs(5);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
            }
        }
        if self.receiver.is_none() && Instant::now() >= self.next {
            match workspace.watched_folder() {
                Ok(config) => self.config = config,
                Err(error) => self.error = Some(error.to_string()),
            }
            if self.config.as_ref().is_some_and(|c| c.enabled) {
                let config = self.config.clone();
                let paths = paths.clone();
                let mut scanner = self.scanner.take().unwrap_or_default();
                let (sender, receiver) = mpsc::channel();
                self.receiver = Some(receiver);
                let context = ctx.clone();
                std::thread::spawn(move || {
                    let result = Workspace::open(&paths)
                        .map_err(|e| e.to_string())
                        .and_then(|w| scanner.poll(&w, Instant::now()).map_err(|e| e.to_string()));
                    let _ = sender.send((scanner, result, config));
                    context.request_repaint();
                });
            } else {
                self.next = Instant::now() + Duration::from_secs(5);
            }
        }
        ctx.request_repaint_after(Duration::from_secs(5));
        changed
    }
    pub fn show(&mut self, ui: &mut Ui, workspace: &Workspace) {
        widgets::card().show(ui,|ui| {
            ui.set_min_width(ui.available_width());ui.label(RichText::new("Watched document folder").font(heading(19.0)));
            widgets::muted(ui,"Pick up completed PDF/DOCX files while ComplyPH is open. Files stay in place; reviews and OCR are never started automatically.");
            if ui.button("Choose intake folder…").clicked(){self.picker.pick_directory();}
            if let Some(config)=&mut self.config {
                ui.add(egui::Label::new(config.root.display().to_string()).wrap().selectable(true));
                let mut enabled=config.enabled;
                if ui.checkbox(&mut enabled,"Enable folder intake while the app is open").changed() {
                    match workspace.set_folder_watch_enabled(enabled) {
                        Ok(())=>{self.config=workspace.watched_folder().ok().flatten();self.next=Instant::now();self.error=None;},
                        Err(error)=>self.error=Some(error.to_string()),
                    }
                }
            } else {widgets::muted(ui,"No folder selected. Folder intake is off.");}
            if self.config.is_some() && ui.button("Forget folder and intake history").on_hover_text("Imported document entries and originals are preserved. Selecting this folder again starts a new intake history.").clicked() {
                match workspace.forget_watched_folder() {
                    Ok(())=>{self.config=None;self.last=None;self.next=Instant::now();self.error=None;},
                    Err(error)=>self.error=Some(error.to_string()),
                }
            }
            widgets::muted(ui,"Scans this folder only. Waits for at least 10 seconds without a detected file change, skips temporary files and links, and avoids duplicate content. Removing an entry keeps that unchanged path excluded; a changed file can be added again.");
            widgets::muted(ui,"Open added documents from Documents, compare extracted text and confirm scope before running a review. Closed-app changes are checked at the next launch.");
            if let Some(scan)=&self.last {ui.label(format!("Last scan: {} added · {} updated · {} duplicate copies skipped · {} waiting · {} too large",scan.added.len(),scan.updated,scan.duplicates,scan.waiting,scan.too_large));}
            if self.receiver.is_some(){ui.horizontal(|ui|{ui.spinner();ui.label("Checking local folder…");});}
            if let Some(error)=&self.error {ui.colored_label(AMBER,error);}
        });
    }
}
