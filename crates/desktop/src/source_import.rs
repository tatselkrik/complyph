//! Explicit local public-reference import. Does not register private documents.
use crate::{reader, theme::*, widgets};
use complyph_extract::{Format, Status};
use complyph_library::{
    ImportOutcome, LibraryCatalog, ManualSourceFile, ManualSourceInput, ManualVerification,
    starter_manifest,
};
use eframe::egui::{self, RichText, ScrollArea};
use egui_file_dialog::{DialogState, FileDialog, Filter};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

pub(crate) fn prepare_reference(
    path: &Path,
    cancel: &AtomicBool,
) -> Result<ManualSourceFile, String> {
    let kind = complyph_core::DocumentKind::from_path(path)
        .ok_or("Choose a PDF or DOCX legal reference.")?;
    let snapshot = complyph_core::document_snapshot(path).map_err(|e| e.to_string())?;
    let report = reader::run_worker(kind.format(), snapshot.bytes.clone(), cancel)?;
    if report.status != Status::Ready || report.units.is_empty() {
        return Err(format!(
            "{}: {} Nothing was imported. Scanned references need external OCR before import.",
            report.status.label(),
            report.summary
        ));
    }
    let current = complyph_core::document_snapshot(path).map_err(|e| e.to_string())?;
    if current.sha256 != snapshot.sha256 || cancel.load(Ordering::Relaxed) {
        return Err(
            "The selected reference changed or reading was cancelled. Nothing was imported.".into(),
        );
    }
    let reading_text = report
        .units
        .iter()
        .map(|unit| format!("— {} —\n{}", unit.location.label(), unit.text))
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(ManualSourceFile {
        raw: snapshot.bytes,
        reading_text,
        mime: match kind.format() {
            Format::Pdf => "application/pdf",
            Format::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
        }
        .into(),
        pages: if kind.format() == Format::Pdf {
            Some(report.units.len() as u64)
        } else {
            None
        },
        extraction_note: format!(
            "{}; local bounded worker. {} {}",
            report.engine,
            report.summary,
            report.warnings.join(" ")
        ),
    })
}

enum Message {
    Prepared(Result<ManualSourceFile, String>),
    Saved(Box<Result<ImportOutcome, String>>),
}

pub struct SourceImportView {
    open: bool,
    input: ManualSourceInput,
    issued: String,
    effective: String,
    identity_confirmed: bool,
    file_name: Option<String>,
    prepared: Option<ManualSourceFile>,
    picker: FileDialog,
    receiver: Option<Receiver<Message>>,
    cancel: Arc<AtomicBool>,
    saving: bool,
    error: Option<String>,
}

impl Default for SourceImportView {
    fn default() -> Self {
        Self {
            open: false,
            input: ManualSourceInput::default(),
            issued: String::new(),
            effective: String::new(),
            identity_confirmed: false,
            file_name: None,
            prepared: None,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            saving: false,
            error: None,
            picker: FileDialog::new()
                .title("Choose a legal reference")
                .add_file_filter(
                    "PDF and DOCX references",
                    Filter::new(|p: &Path| complyph_core::DocumentKind::from_path(p).is_some()),
                )
                .default_file_filter("PDF and DOCX references")
                .show_all_files_filter(false)
                .default_size([760.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show_devices(false)
                .show_removable_devices(false)
                .show_hidden_option(false)
                .as_modal(true),
        }
    }
}

impl Drop for SourceImportView {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl SourceImportView {
    pub fn start(&mut self) {
        *self = Self::default();
        self.open = true;
    }
    pub fn start_prefilled(&mut self, input: ManualSourceInput) {
        self.start();
        self.input = input;
    }

    pub fn show(&mut self, ctx: &egui::Context, directory: &Path) -> Option<ImportOutcome> {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(Message::Prepared(result)) => {
                    self.receiver = None;
                    match result {
                        Ok(file) => self.prepared = Some(file),
                        Err(error) => self.error = Some(error),
                    }
                }
                Ok(Message::Saved(result)) => {
                    self.receiver = None;
                    self.saving = false;
                    match *result {
                        Ok(outcome) => {
                            self.open = false;
                            return Some(outcome);
                        }
                        Err(error) => {
                            self.error = Some(format!(
                                "Import failed: {error} Choose the file again to retry."
                            ))
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.saving = false;
                    self.error = Some(
                        "The local import worker stopped. Choose the file again to retry.".into(),
                    );
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100))
                }
            }
        }
        if !self.open {
            return None;
        }
        // The themed picker uses its own window/overlay. Suspend the form's
        // egui Modal while it is open, so it cannot obscure or block the picker.
        if self.picker.state() == &DialogState::Open {
            self.picker.update(ctx);
            if let Some(path) = self.picker.take_picked() {
                self.read(path, ctx);
            }
            ctx.request_repaint();
            return None;
        }
        let busy = self.receiver.is_some();
        let mut save = false;
        let mut close = false;
        egui::Modal::new(egui::Id::unique("add_legal_source")).show(ctx, |ui| {
            ui.set_width(670.0_f32.min(ctx.content_rect().width() - 64.0));
            ui.label(RichText::new("Add legal source").font(heading(24.0)));
            widgets::muted(ui, "Save a public legal reference to your local library. The original stays unchanged.");
            ScrollArea::vertical().id_salt("source_import_form").max_height((ctx.content_rect().height() - 300.0).max(100.0)).show(ui, |ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if widgets::secondary(ui, "Choose PDF / DOCX…").clicked() { self.picker.pick_file(); }
                        if let Some(name) = &self.file_name { ui.label(name); }
                    });
                    widgets::muted(ui, "PDF/DOCX · 32 MiB · up to 300 PDF pages. Scans need external OCR.");
                    if let Some(file) = &self.prepared {
                        widgets::pill(ui, "Text ready · local copy", GREEN, GREEN_TINT, false);
                        ui.collapsing("Preview extracted text", |ui| {
                            let preview = file.reading_text.chars().take(1800).collect::<String>();
                            ui.add(egui::Label::new(preview).wrap().selectable(true));
                            widgets::muted(ui, &file.extraction_note);
                        });
                    }
                        for (label, text, hint) in [
                            ("Title *", &mut self.input.title, "Publication title"),
                            ("Instrument / number *", &mut self.input.instrument, "e.g. Department Order 252-25"),
                            ("Issuing agency *", &mut self.input.issuer, "e.g. Department of Labor and Employment"),
                            ("Official URL *", &mut self.input.official_url, "https://…"),
                        ] {
                            crate::layout::field(ui,label,text,hint);
                        }
                        crate::layout::field(ui,"Issue date",&mut self.issued,"YYYY-MM-DD · blank if unknown");
                        crate::layout::field(ui,"Effective from",&mut self.effective,"YYYY-MM-DD · blank if unknown");
                    widgets::muted(ui, "The official URL is recorded as a citation. ComplyPH does not open or verify it online.");
                    ui.label(RichText::new("Compliance domains *").font(heading(15.0)));
                    if let Ok(manifest) = starter_manifest() {
                        ui.horizontal_wrapped(|ui| {
                            for domain in manifest.domains {
                                let mut checked = self.input.domains.contains(&domain.id);
                                if ui.checkbox(&mut checked, domain.label).changed() {
                                    if checked { self.input.domains.push(domain.id); }
                                    else { self.input.domains.retain(|id| id != &domain.id); }
                                }
                            }
                        });
                    }
                    ui.collapsing("Effectivity and applicability notes", |ui| {
                        crate::layout::multiline(ui,"Effectivity conditions or publication details",&mut self.input.effectivity_note,2,usize::MAX);
                        crate::layout::multiline(ui,"Scope, exclusions or amendments to reconcile",&mut self.input.applicability_note,2,usize::MAX);
                    });
                    ui.label(RichText::new("Verification status").font(heading(15.0)));
                    ui.radio_value(&mut self.input.verification, ManualVerification::AwaitingVerification, "Awaiting verification");
                    ui.radio_value(&mut self.input.verification, ManualVerification::IdentityChecked, "Identity checked by me");
                    if self.input.verification == ManualVerification::IdentityChecked {
                        ui.checkbox(&mut self.identity_confirmed, "I compared this copy with the official publication.");
                        widgets::muted(ui, "This records your review; it does not become a verified agency download.");
                    }
                    crate::layout::multiline(ui,"Verification notes · required for identity checked",&mut self.input.verification_note,2,usize::MAX);
                });
            });
            if let Some(error) = &self.error { ui.colored_label(RED, error); }
            if busy {
                ui.horizontal(|ui| { ui.spinner(); ui.label(if self.saving { "Saving local reference…" } else { "Reading reference locally…" }); });
            }
            widgets::muted(ui, "Adding a source does not add checking rules. The same URL can retain successive file versions.");
            ui.horizontal(|ui| {
                save = ui.add_enabled(!busy && self.prepared.is_some(), egui::Button::new(RichText::new("Save legal source").color(CREAM)).fill(NAVY)).clicked();
                close = ui.add_enabled(!self.saving, egui::Button::new(if busy && !self.saving { "Cancel reading" } else { "Cancel" })).clicked();
            });
        });
        if self.picker.state() == &DialogState::Open {
            ctx.request_repaint();
        }
        if close {
            self.cancel.store(true, Ordering::Relaxed);
            self.receiver = None;
            self.prepared = None;
            self.open = false;
        }
        if save {
            self.save(directory, ctx);
        }
        None
    }

    fn read(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.prepared = None;
        self.error = None;
        self.file_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let cancel = self.cancel.clone();
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let _ = sender.send(Message::Prepared(prepare_reference(&path, &cancel)));
            ctx.request_repaint();
        });
    }

    fn save(&mut self, directory: &Path, ctx: &egui::Context) {
        self.input.issued_on = Some(self.issued.clone());
        self.input.effective_from = Some(self.effective.clone());
        if self.input.verification == ManualVerification::IdentityChecked
            && !self.identity_confirmed
        {
            self.error = Some("Confirm your comparison with the official publication, or choose Awaiting verification.".into());
            return;
        }
        let input = match self.input.validated() {
            Ok(input) => input,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let Some(file) = self.prepared.take() else {
            return;
        };
        self.saving = true;
        self.error = None;
        let directory = directory.to_owned();
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let result = LibraryCatalog::open(&directory)
                .and_then(|mut catalog| catalog.import_manual(&input, file))
                .map_err(|e| e.to_string());
            let _ = sender.send(Message::Saved(Box::new(result)));
            ctx.request_repaint();
        });
    }
}
