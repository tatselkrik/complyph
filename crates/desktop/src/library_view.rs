use crate::{theme::*, widgets};
use complyph_library::{
    LibraryCatalog, LibraryError, LibrarySummary, SourceMetadata, SourceRecord, starter_manifest,
};
use eframe::egui::{self, RichText, ScrollArea, TextEdit, Ui};
use egui_file_dialog::FileDialog;

struct Reader {
    id: i64,
    metadata: SourceMetadata,
    pages: Vec<String>,
    page: usize,
    find: String,
    find_message: String,
}

pub struct LibraryView {
    sources: Vec<SourceRecord>,
    query: String,
    domain: String,
    reader: Option<Reader>,
    export: FileDialog,
    export_id: Option<i64>,
    message: Option<String>,
    import: crate::source_import::SourceImportView,
}

impl Default for LibraryView {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            query: String::new(),
            domain: String::new(),
            reader: None,
            export: FileDialog::new()
                .title("Save preserved source copy")
                .default_size([760.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show_devices(false)
                .show_removable_devices(false)
                .show_hidden_option(false)
                .as_modal(true),
            export_id: None,
            message: None,
            import: crate::source_import::SourceImportView::default(),
        }
    }
}

impl LibraryView {
    pub fn load(&mut self, catalog: &LibraryCatalog) -> Result<(), LibraryError> {
        self.sources = catalog.sources()?;
        Ok(())
    }

    pub fn show(&mut self, ui: &mut Ui, catalog: &LibraryCatalog, summary: &LibrarySummary) {
        if let Some(message) = &self.message {
            ui.label(RichText::new(message).color(AMBER));
            ui.add_space(8.0);
        }
        if self.reader.is_some() {
            self.show_reader(ui);
            return;
        }
        let Ok(manifest) = starter_manifest() else {
            ui.colored_label(RED, "Source metadata is unavailable.");
            return;
        };
        ui.horizontal_wrapped(|ui| {
            if widgets::primary(ui, "Add legal source", true).clicked() {
                self.import.start();
            }
            widgets::pill(ui, "Local references", GREEN, GREEN_TINT, true);
            ui.label(
                RichText::new(format!(
                    "{} saved  ·  {} verified agency downloads",
                    summary.source_count, summary.verified_source_count
                ))
                .color(MUTED),
            );
            ui.label(
                RichText::new(format!("Checked {} UTC", &manifest.verified_at_utc[..10]))
                    .size(12.0)
                    .color(MUTED),
            );
        });
        ui.label(
            RichText::new(format!(
                "{} identity-checked imported copies · {} provisional third-party copies",
                summary.identity_checked_upload_count, summary.provisional_source_count
            ))
            .color(AMBER),
        );
        if summary.manually_imported_source_count > 0 {
            ui.label(
                RichText::new(format!(
                    "{} manual imports · {} awaiting verification · {} identity checked by user",
                    summary.manually_imported_source_count,
                    summary.awaiting_verification_count,
                    summary.user_checked_source_count
                ))
                .color(AMBER),
            );
        }
        ui.add_space(10.0);
        widgets::muted(
            ui,
            "A starter collection, with coverage gaps. Source verification is not a complete or current compliance assessment.",
        );
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(self.domain.is_empty(), "All domains")
                .clicked()
            {
                self.domain.clear();
            }
            for domain in &summary.domains {
                if ui
                    .selectable_label(
                        self.domain == domain.id,
                        format!("{} ({})", domain.label, domain.source_count),
                    )
                    .clicked()
                {
                    self.domain = domain.id.clone();
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut self.query)
                    .hint_text("Search titles, instruments, or issuers…")
                    .desired_width(400.0),
            );
            if !self.query.is_empty() && widgets::secondary(ui, "Clear").clicked() {
                self.query.clear();
            }
        });
        ui.add_space(12.0);
        let query = self.query.to_lowercase();
        let mut selected = None;
        let mut count = 0;
        for source in &self.sources {
            let metadata = &source.metadata;
            if !self.domain.is_empty() && !metadata.domains.contains(&self.domain) {
                continue;
            }
            if !format!(
                "{} {} {}",
                metadata.title, metadata.instrument, metadata.issuer
            )
            .to_lowercase()
            .contains(&query)
            {
                continue;
            }
            count += 1;
            widgets::card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(RichText::new(&metadata.title).font(heading(17.0)));
                widgets::muted(
                    ui,
                    &format!("{}  ·  {}", metadata.instrument, metadata.issuer),
                );
                let labels = metadata
                    .domains
                    .iter()
                    .filter_map(|id| {
                        manifest
                            .domains
                            .iter()
                            .find(|d| &d.id == id)
                            .map(|d| d.label.as_str())
                    })
                    .collect::<Vec<_>>()
                    .join("  /  ");
                ui.label(RichText::new(labels).size(12.0).color(MUTED));
                let verified = metadata.provenance == complyph_library::Provenance::AgencyDownload;
                widgets::pill(
                    ui,
                    metadata.provenance.label(),
                    if verified { GREEN } else { AMBER },
                    if verified { GREEN_TINT } else { AMBER_TINT },
                    false,
                );
                ui.add_space(5.0);
                if widgets::secondary(ui, "Read local snapshot").clicked() {
                    selected = Some(source.id);
                }
            });
            ui.add_space(8.0);
        }
        if count == 0 {
            widgets::muted(ui, "No sources match this filter.");
        }
        if let Some(id) = selected {
            match catalog.snapshot(id) {
                Ok(snapshot) => {
                    let mut reading = snapshot
                        .text
                        .replace('\u{000c}', "\n\n— Original page break —\n\n");
                    while reading.contains("\n\n\n") {
                        reading = reading.replace("\n\n\n", "\n\n");
                    }
                    let chars = reading.chars().collect::<Vec<_>>();
                    let pages = chars
                        .chunks(7000)
                        .map(|part| part.iter().collect())
                        .collect();
                    self.reader = Some(Reader {
                        id,
                        metadata: snapshot.metadata,
                        pages,
                        page: 0,
                        find: String::new(),
                        find_message: String::new(),
                    });
                    self.message = None;
                    ui.ctx().request_repaint();
                }
                Err(error) => self.message = Some(error.to_string()),
            }
        }
        ui.add_space(12.0);
        widgets::card().show(ui,|ui| {
            ui.set_min_width(ui.available_width());
            ui.collapsing("Coverage gaps and update status",|ui| {
                for gap in &manifest.coverage_gaps {
                    ui.label(RichText::new(&gap.instrument).font(heading(14.0)));
                    widgets::muted(ui,&gap.reason);
                    ui.add_space(6.0);
                }
                for reference in &manifest.external_references {
                    ui.label(RichText::new(&reference.title).font(heading(13.0)));
                    widgets::muted(ui, &reference.note);
                    ui.add(egui::Label::new(&reference.url).wrap().selectable(true));
                }
                ui.label(RichText::new("Daily source checks are not available yet. No runtime update check has run. Manually imported references do not automatically resolve coverage gaps or add checking rules.").color(AMBER));
            });
        });
    }

    fn show_reader(&mut self, ui: &mut Ui) {
        let mut back = false;
        let mut export = None;
        let reader = self.reader.as_mut().expect("reader was selected");
        ui.horizontal(|ui| {
            back = widgets::secondary(ui, "Back to library").clicked();
            if widgets::secondary(ui, "Save source copy…").clicked() {
                let name = reader
                    .metadata
                    .raw_path
                    .rsplit('/')
                    .next()
                    .unwrap_or("source.dat")
                    .to_owned();
                export = Some((reader.id, name));
            }
        });
        ui.add_space(12.0);
        ui.label(RichText::new(&reader.metadata.title).font(heading(22.0)));
        widgets::muted(
            ui,
            &format!(
                "{}  ·  {}",
                reader.metadata.instrument, reader.metadata.issuer
            ),
        );
        ui.add_space(8.0);
        widgets::pill(
            ui,
            reader.metadata.provenance.label(),
            AMBER,
            AMBER_TINT,
            false,
        );
        ui.label(RichText::new(&reader.metadata.applicability_note).color(AMBER));
        ui.add_space(10.0);
        ui.collapsing("Source and verification details", |ui| {
            ui.label(format!(
                "Checked: {}",
                if reader.metadata.verified_at_utc.is_empty() {
                    "Awaiting verification"
                } else {
                    &reader.metadata.verified_at_utc
                }
            ));
            ui.label(format!("Acquired: {}", reader.metadata.acquired_at_utc));
            ui.label(format!(
                "Issued/signed: {}",
                reader
                    .metadata
                    .issued_on
                    .as_deref()
                    .unwrap_or("See original and notes")
            ));
            if !reader.metadata.effectivity_note.is_empty() {
                widgets::muted(ui, &reader.metadata.effectivity_note);
            }
            ui.label(format!(
                "Effective from: {}",
                reader
                    .metadata
                    .effective_from
                    .as_deref()
                    .unwrap_or("Not established; see notes")
            ));
            ui.label("Recorded source URL (saved citation; no online request is made):");
            ui.add(
                egui::Label::new(&reader.metadata.source_url)
                    .wrap()
                    .selectable(true),
            );
            ui.label(format!(
                "Preserved copy SHA-256: {}",
                reader.metadata.raw_sha256
            ));
            if !reader.metadata.provenance.is_manual()
                && let Some(url) = &reader.metadata.official_reference_url
            {
                ui.label("Official corroboration URL:");
                ui.add(egui::Label::new(url).wrap().selectable(true));
            }
            widgets::muted(ui, &reader.metadata.extraction);
            widgets::muted(ui, &reader.metadata.verification_scope);
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut reader.find)
                    .hint_text("Find in this source…")
                    .desired_width(330.0),
            );
            if widgets::secondary(ui, "Find next").clicked() && !reader.find.trim().is_empty() {
                let query = reader.find.trim().to_lowercase();
                let next = (1..=reader.pages.len())
                    .map(|offset| (reader.page + offset) % reader.pages.len())
                    .find(|&page| reader.pages[page].to_lowercase().contains(&query));
                if let Some(page) = next {
                    reader.page = page;
                    reader.find_message = format!("Match in reading section {}.", page + 1);
                } else {
                    reader.find_message =
                        "No matching text. OCR may differ from the original.".into();
                }
            }
        });
        if !reader.find_message.is_empty() {
            widgets::muted(ui, &reader.find_message);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(reader.page > 0, egui::Button::new("Previous"))
                .clicked()
            {
                reader.page -= 1;
            }
            ui.label(format!(
                "Reading section {} of {}",
                reader.page + 1,
                reader.pages.len()
            ));
            if ui
                .add_enabled(
                    reader.page + 1 < reader.pages.len(),
                    egui::Button::new("Next"),
                )
                .clicked()
            {
                reader.page += 1;
            }
        });
        widgets::muted(
            ui,
            "Reading sections are display chunks, not original page numbers. OCR and table layouts may be imperfect; compare exact wording and figures with the saved page images and provenance notes.",
        );
        if let Some(page) = reader.pages.get(reader.page) {
            widgets::card().show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt(("source_text", reader.id, reader.page))
                    .max_height(460.0)
                    .show(ui, |ui| {
                        let mut text = page.as_str();
                        ui.add(
                            TextEdit::multiline(&mut text)
                                .desired_width(f32::INFINITY)
                                .desired_rows(22)
                                .font(egui::TextStyle::Body),
                        );
                    });
            });
        }
        if let Some((id, name)) = export {
            self.export = FileDialog::new()
                .title("Save preserved source copy")
                .default_size([760.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show_devices(false)
                .show_removable_devices(false)
                .default_file_name(&name)
                .show_hidden_option(false)
                .as_modal(true);
            self.export.save_file();
            self.export_id = Some(id);
        }
        if back {
            self.reader = None;
            ui.ctx().request_repaint();
        }
    }

    pub fn update_dialog(
        &mut self,
        ctx: &egui::Context,
        catalog: &LibraryCatalog,
        directory: &std::path::Path,
    ) -> bool {
        self.export.update(ctx);
        if let Some(output) = self.export.take_picked()
            && let Some(id) = self.export_id.take()
        {
            self.message = Some(match catalog.export_source(id, &output) {
                Ok(()) => format!("Preserved source copy saved to {}", output.display()),
                Err(error) => format!(
                    "Could not save the copy: {error}. Choose a new filename if one already exists."
                ),
            });
            ctx.request_repaint();
        }
        if let Some(outcome) = self.import.show(ctx, directory) {
            let action = if !outcome.added {
                "Already saved; original metadata kept"
            } else if outcome.new_version {
                "Added new version; earlier snapshot retained"
            } else {
                "Added legal source"
            };
            self.message = Some(format!(
                "{action}: {}. {}.",
                outcome.metadata.title,
                outcome.metadata.provenance.label()
            ));
            if let Err(error) = self.load(catalog) {
                self.message = Some(error.to_string());
            }
            self.reader = None;
            ctx.request_repaint();
            return true;
        }
        false
    }
}
