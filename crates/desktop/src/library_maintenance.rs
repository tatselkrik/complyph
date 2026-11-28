//! Native, public-only library maintenance. All mutations have a reviewed action.
use crate::{reader, theme::*, widgets};
use complyph_library::*;
use eframe::egui::{self, RichText, ScrollArea, TextEdit, Ui};
use egui_file_dialog::FileDialog;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
enum Tab {
    #[default]
    Source,
    Compare,
    Rules,
    Discovery,
    History,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Right {
    Candidate(i64),
    Version(String),
}
struct Comparison {
    data: ReferenceComparison,
    left: String,
    right: String,
    page: usize,
}
enum Action {
    Source(i64),
    SaveMetadata,
    Identity(SourceIdentityDecision),
    Prepare,
    Compare,
    Adopt,
    Dismiss,
    Restore,
    Revalidate(bool),
    ApprovePage,
    DisablePage(i64),
    ReviewLink(i64, bool),
    ImportLink(i64),
    AssessLink(i64),
    AssessRule(String),
    CompareRuleSource(String),
    Export,
}
pub struct LibraryMaintenance {
    open: bool,
    tab: Tab,
    scroll_top: bool,
    sources: Vec<SourceRecord>,
    rules: Vec<RuleLibraryState>,
    pages: Vec<PublicationPage>,
    links: Vec<DiscoveredIssuance>,
    source_id: Option<i64>,
    details: ManualSourceInput,
    issued: String,
    effective: String,
    active_hash: String,
    versions: Vec<PreservedVersion>,
    changes: Vec<DownloadedChange>,
    history: Vec<LibraryEvent>,
    reviewer: LibraryReviewer,
    identity: SourceIdentityDecision,
    identity_confirmed: bool,
    right: Option<Right>,
    left_hash: String,
    comparison: Option<Comparison>,
    full_text: bool,
    old_chunk: usize,
    new_chunk: usize,
    original_compared: bool,
    effectivity_reviewed: bool,
    use_ocr: bool,
    rule_id: String,
    test_reference: String,
    amendments_reviewed: bool,
    withdraw_update_requirement: bool,
    impact_reference: Option<String>,
    page_url: String,
    page_domain: String,
    query: String,
    show_dismissed: bool,
    link_page: usize,
    receiver: Option<Receiver<Result<i64, String>>>,
    cancel: Arc<AtomicBool>,
    export: FileDialog,
    export_right: Option<Right>,
    message: Option<String>,
    changed: bool,
    pub request_check: bool,
    pub import_request: Option<ManualSourceInput>,
}
impl Default for LibraryMaintenance {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Source,
            scroll_top: true,
            sources: vec![],
            rules: vec![],
            pages: vec![],
            links: vec![],
            source_id: None,
            details: Default::default(),
            issued: String::new(),
            effective: String::new(),
            active_hash: String::new(),
            versions: vec![],
            changes: vec![],
            history: vec![],
            reviewer: Default::default(),
            identity: SourceIdentityDecision::AwaitingVerification,
            identity_confirmed: false,
            right: None,
            left_hash: String::new(),
            comparison: None,
            full_text: false,
            old_chunk: 0,
            new_chunk: 0,
            original_compared: false,
            effectivity_reviewed: false,
            use_ocr: false,
            rule_id: String::new(),
            test_reference: String::new(),
            amendments_reviewed: false,
            withdraw_update_requirement: false,
            impact_reference: None,
            page_url: String::new(),
            page_domain: "ph.labor".into(),
            query: String::new(),
            show_dismissed: false,
            link_page: 0,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            export: FileDialog::new().as_modal(true),
            export_right: None,
            message: None,
            changed: false,
            request_check: false,
            import_request: None,
        }
    }
}
impl Drop for LibraryMaintenance {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn ui_result<T>(result: std::result::Result<T, LibraryError>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}
fn field(ui: &mut Ui, label: &str, value: &mut String) {
    let label = ui.label(label);
    ui.add(TextEdit::singleline(value).desired_width(f32::INFINITY))
        .labelled_by(label.id);
    ui.end_row();
}
pub(crate) fn prepare_public_reference(
    raw: Vec<u8>,
    mime: &str,
    ocr: bool,
    cancel: &AtomicBool,
) -> Result<ManualSourceFile, String> {
    let (reading_text,pages,extraction_note)=match mime {
        "text/html"=>(html_reading_text(&raw).map_err(|e|e.to_string())?,None,"Inert UTF-8 HTML reading; scripts/styles/hidden elements omitted. Compare the preserved original.".into()),
        "text/plain"=>{
            let text=std::str::from_utf8(&raw).map_err(|_|"The plain-text publication is not UTF-8.")?.to_string();
            if text.len()>MAX_REFERENCE_TEXT_BYTES || text.trim().is_empty() {return Err("The plain-text publication is empty or exceeds the reading limit.".into());}
            (text,None,"Preserved UTF-8 public text; no executable content.".into())
        }
        _=>{
            let format=match mime {
                "application/pdf"=>complyph_extract::Format::Pdf,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"=>complyph_extract::Format::Docx,
                _=>return Err("Unsupported public reference format.".into()),
            };
            let report=reader::run_worker_mode(format,raw.clone(),cancel,ocr && format==complyph_extract::Format::Pdf)?;
            if report.status!=complyph_extract::Status::Ready || report.units.is_empty() {return Err(format!("{}: {}. The candidate was not adopted. Scanned PDFs can use local OCR.",report.status.label(),report.summary));}
            let text=report.units.iter().map(|u|format!("— {} —\n{}",u.location.label(),u.text)).collect::<Vec<_>>().join("\n\n");
            let pages=(format==complyph_extract::Format::Pdf).then_some(report.units.len() as u64);
            (text,pages,format!("{}; local bounded worker. {} {}",report.engine,report.summary,report.warnings.join(" ")))
        }
    };
    if cancel.load(Ordering::Relaxed) {
        return Err("Local reference reading was cancelled.".into());
    }
    Ok(ManualSourceFile {
        raw,
        reading_text,
        mime: mime.into(),
        pages,
        extraction_note,
    })
}
impl LibraryMaintenance {
    pub fn load(&mut self, catalog: &LibraryCatalog) -> Result<(), String> {
        crate::library_rules::register(catalog)?;
        self.sources = ui_result(catalog.sources())?;
        self.rules = ui_result(catalog.rule_library_states())?;
        self.pages = ui_result(catalog.publication_pages())?;
        self.links = ui_result(catalog.discovered_issuances())?;
        if let Some(id) = self.source_id {
            self.reload_source_history(catalog, id)?;
        }
        Ok(())
    }
    fn reload_source_history(&mut self, catalog: &LibraryCatalog, id: i64) -> Result<(), String> {
        self.versions = ui_result(catalog.preserved_versions(id))?;
        self.changes = ui_result(catalog.downloaded_changes(id))?;
        self.history = ui_result(catalog.source_history(id))?;
        Ok(())
    }
    fn select_source(&mut self, catalog: &LibraryCatalog, id: i64) -> Result<(), String> {
        let m = ui_result(catalog.display_metadata(id))?;
        self.identity = ui_result(catalog.source_identity_decision(id, &m.raw_sha256))?
            .unwrap_or(SourceIdentityDecision::AwaitingVerification);
        self.active_hash = m.raw_sha256.clone();
        self.left_hash = m.raw_sha256;
        self.details = ManualSourceInput {
            title: m.title,
            instrument: m.instrument,
            issuer: m.issuer,
            official_url: m.official_reference_url.unwrap_or(m.source_url),
            domains: m.domains,
            issued_on: m.issued_on,
            effective_from: m.effective_from,
            effectivity_note: m.effectivity_note,
            applicability_note: m.applicability_note,
            ..Default::default()
        };
        self.issued = self.details.issued_on.clone().unwrap_or_default();
        self.effective = self.details.effective_from.clone().unwrap_or_default();
        self.source_id = Some(id);
        self.right = None;
        self.comparison = None;
        self.reset_confirmations();
        self.reload_source_history(catalog, id)
    }
    fn reset_confirmations(&mut self) {
        self.identity_confirmed = false;
        self.original_compared = false;
        self.effectivity_reviewed = false;
        self.amendments_reviewed = false;
        self.withdraw_update_requirement = false;
        self.impact_reference = None;
        self.reviewer.basis.clear();
    }
    pub fn open(&mut self, catalog: &LibraryCatalog, source: Option<i64>, discovery: bool) {
        self.open = true;
        self.scroll_top = true;
        self.tab = if discovery {
            Tab::Discovery
        } else {
            Tab::Source
        };
        let result = self.load(catalog).and_then(|_| {
            if let Some(id) = source
                .or(self.source_id)
                .or_else(|| self.sources.first().map(|s| s.id))
            {
                self.select_source(catalog, id)?;
            }
            Ok(())
        });
        if let Err(error) = result {
            self.message = Some(error);
        }
    }
    pub fn open_candidate(&mut self, catalog: &LibraryCatalog, change_id: i64) {
        self.open = true;
        self.scroll_top = true;
        self.tab = Tab::Compare;
        let result = (|| {
            self.load(catalog)?;
            let (change, _) = ui_result(catalog.downloaded_candidate_bytes(change_id))?;
            self.select_source(catalog, change.source_id)?;
            self.right = Some(Right::Candidate(change_id));
            self.load_comparison(catalog)
        })();
        if let Err(error) = result {
            self.message = Some(error);
        }
    }
    fn load_comparison(&mut self, catalog: &LibraryCatalog) -> Result<(), String> {
        self.comparison = None;
        self.old_chunk = 0;
        self.new_chunk = 0;
        self.identity_confirmed = false;
        self.original_compared = false;
        self.effectivity_reviewed = false;
        let id = self.source_id.ok_or("Select a source.")?;
        let metadata = self
            .versions
            .iter()
            .find(|v| v.metadata.raw_sha256 == self.left_hash)
            .ok_or("Select a preserved left-hand source version.")?
            .metadata
            .clone();
        let old = ui_result(catalog.snapshot_for_review(
            &metadata.key,
            &metadata.raw_sha256,
            &metadata.text_sha256,
        ))?;
        let (new_hash, new) = match self
            .right
            .clone()
            .ok_or("Choose a downloaded candidate or a preserved version.")?
        {
            Right::Candidate(id) => match catalog.prepared_candidate(id) {
                Ok(candidate) => (candidate.change.content_sha256, candidate.text),
                Err(_) => {
                    self.message=Some("Read the downloaded candidate locally, then compare its reading with the original before adoption.".into());
                    return Ok(());
                }
            },
            Right::Version(hash) => {
                let m = &self
                    .versions
                    .iter()
                    .find(|v| v.metadata.raw_sha256 == hash)
                    .ok_or("This preserved version is unavailable.")?
                    .metadata;
                (
                    hash.clone(),
                    ui_result(catalog.snapshot_for_review(&m.key, &hash, &m.text_sha256))?.text,
                )
            }
        };
        let data = ui_result(compare_readings(
            &self.left_hash,
            &old.text,
            &new_hash,
            &new,
        ))?;
        self.comparison = Some(Comparison {
            data,
            left: old.text,
            right: new,
            page: 0,
        });
        self.message = Some(format!(
            "Comparison for source #{id} prepared locally. Byte and text differences do not establish legal effect."
        ));
        Ok(())
    }
    fn reviewer_form(&mut self, ui: &mut Ui) {
        ui.add_space(10.0);
        egui::CollapsingHeader::new("Reviewer and decision basis").id_salt("library-reviewer").default_open(true).show(ui,|ui|{
            egui::Grid::new("library-reviewer-grid").num_columns(2).show(ui,|ui|{
                field(ui,"Reviewer *",&mut self.reviewer.actor);
                field(ui,"Review date · YYYY-MM-DD *",&mut self.reviewer.review_on);
            });
            crate::layout::multiline(ui,"Decision basis *",&mut self.reviewer.basis,2,usize::MAX);
            widgets::muted(ui,"Record what was compared, the legal/scope conclusion and why this decision is appropriate.");
            widgets::muted(ui,"Reviewer identity and conclusions are self-reported. The local audit records are not a legal certification.");
        });
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        catalog: &LibraryCatalog,
        directory: &Path,
    ) -> bool {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.receiver = None;
                    match result {
                        Ok(id) => {
                            if self.right == Some(Right::Candidate(id))
                                && let Err(e) = self.load_comparison(catalog)
                            {
                                self.message = Some(e);
                            }
                            self.changed = true;
                        }
                        Err(e) => self.message = Some(e),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.message = Some(
                        "The local candidate reader stopped. The current reference is intact."
                            .into(),
                    );
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100))
                }
            }
        }
        self.export.update(ctx);
        if let Some(path) = self.export.take_picked() {
            let result = (|| -> Result<(), String> {
                use std::io::Write;
                let raw = match self
                    .export_right
                    .take()
                    .ok_or("Select a public copy to export.")?
                {
                    Right::Candidate(id) => ui_result(catalog.downloaded_candidate_bytes(id))?.1,
                    Right::Version(hash) => {
                        let metadata = &self
                            .versions
                            .iter()
                            .find(|v| v.metadata.raw_sha256 == hash)
                            .ok_or("This version is unavailable.")?
                            .metadata;
                        ui_result(catalog.snapshot_for_review(
                            &metadata.key,
                            &hash,
                            &metadata.text_sha256,
                        ))?
                        .raw
                    }
                };
                let mut file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(path)
                    .map_err(|e| e.to_string())?;
                file.write_all(&raw)
                    .and_then(|_| file.sync_all())
                    .map_err(|e| e.to_string())
            })();
            self.message = Some(match result {
                Ok(()) => "Public original exported; no existing file was replaced.".into(),
                Err(e) => e,
            });
        }
        if self.open {
            let mut open = true;
            let mut action = None;
            crate::layout::window(
                ctx,
                egui::Window::new("Library maintenance")
                    .id(egui::Id::unique("library-maintenance"))
                    .open(&mut open)
                    .default_size([1120.0, 760.0])
                    .resizable(true)
                    .collapsible(false),
            )
            .show(ctx, |ui| {
                let previous_tab = self.tab;
                ui.horizontal_wrapped(|ui| {
                    for (tab, label) in [
                        (Tab::Source, "Source details"),
                        (Tab::Compare, "Version comparison"),
                        (Tab::Rules, "Rule revalidation"),
                        (Tab::Discovery, "Publication discovery"),
                        (Tab::History, "Audit history"),
                    ] {
                        ui.selectable_value(&mut self.tab, tab, label);
                    }
                });
                if previous_tab != self.tab {
                    self.scroll_top = true;
                }
                ui.separator();
                if let Some(message) = &self.message {
                    ui.colored_label(AMBER, message);
                }
                if self.receiver.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading a public reference locally…");
                    });
                }
                let mut area = ScrollArea::vertical()
                    .id_salt(("maintenance-content", self.tab))
                    .max_height((ui.available_height() - 12.0).max(300.0));
                if std::mem::take(&mut self.scroll_top) {
                    area = area.vertical_scroll_offset(0.0);
                }
                area.show(ui, |ui| {
                    if matches!(self.tab, Tab::Source | Tab::Compare | Tab::History) {
                        let mut selected = self.source_id;
                        egui::ComboBox::from_id_salt("maintenance-source")
                            .selected_text(
                                self.sources
                                    .iter()
                                    .find(|s| Some(s.id) == self.source_id)
                                    .map(|s| s.metadata.title.as_str())
                                    .unwrap_or("Choose source"),
                            )
                            .width(ui.available_width().min(800.0))
                            .show_ui(ui, |ui| {
                                for source in &self.sources {
                                    ui.selectable_value(
                                        &mut selected,
                                        Some(source.id),
                                        &source.metadata.title,
                                    );
                                }
                            });
                        if selected != self.source_id
                            && let Some(id) = selected
                        {
                            action = Some(Action::Source(id));
                        }
                    }
                    match self.tab {
                        Tab::Source => self.source_ui(ui, &mut action),
                        Tab::Compare => self.comparison_ui(ui, &mut action),
                        Tab::Rules => self.rules_ui(ui, &mut action),
                        Tab::Discovery => self.discovery_ui(ui, &mut action),
                        Tab::History => self.history_ui(ui),
                    }
                });
            });
            self.open = open;
            if let Some(action) = action
                && let Err(error) = self.perform(action, catalog, directory, ctx)
            {
                self.message = Some(error);
            }
        }
        std::mem::take(&mut self.changed)
    }
    fn source_ui(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        if self.source_id.is_none() {
            return;
        }
        widgets::muted(
            ui,
            &format!("Current original SHA-256: {}", self.active_hash),
        );
        egui::Grid::new("maintenance-source-details")
            .num_columns(2)
            .show(ui, |ui| {
                field(ui, "Title *", &mut self.details.title);
                field(ui, "Instrument / number *", &mut self.details.instrument);
                field(ui, "Issuing agency *", &mut self.details.issuer);
                field(
                    ui,
                    "Official citation URL *",
                    &mut self.details.official_url,
                );
                field(ui, "Issued / signed · optional", &mut self.issued);
                field(ui, "Effective from · optional", &mut self.effective);
            });
        if let Ok(manifest) = starter_manifest() {
            ui.horizontal_wrapped(|ui| {
                for domain in manifest.domains {
                    let mut selected = self.details.domains.contains(&domain.id);
                    if ui.checkbox(&mut selected, &domain.label).changed() {
                        self.details.domains.retain(|d| d != &domain.id);
                        if selected {
                            self.details.domains.push(domain.id);
                        }
                    }
                }
            });
        }
        let label = ui.label("Effectivity assessment");
        ui.add(
            TextEdit::multiline(&mut self.details.effectivity_note)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
        let label = ui.label("Applicability and exceptions");
        ui.add(
            TextEdit::multiline(&mut self.details.applicability_note)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
        widgets::muted(
            ui,
            "Edits are append-only annotations for this exact version. Original acquisition metadata, hashes and historical citations remain preserved. Editing a URL does not approve online checks.",
        );
        self.reviewer_form(ui);
        if widgets::primary(ui, "Save reviewed metadata", true).clicked() {
            *action = Some(Action::SaveMetadata);
        }
        ui.separator();
        ui.label(RichText::new("Source identity review").font(heading(17.0)));
        ui.checkbox(
            &mut self.identity_confirmed,
            "I compared the preserved original with the cited official publication.",
        );
        egui::ComboBox::from_id_salt("source-identity-decision")
            .selected_text(self.identity.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.identity,
                    SourceIdentityDecision::AwaitingVerification,
                    "Awaiting verification",
                );
                ui.selectable_value(
                    &mut self.identity,
                    SourceIdentityDecision::IdentityChecked,
                    "Identity checked by reviewer",
                );
                ui.selectable_value(
                    &mut self.identity,
                    SourceIdentityDecision::Rejected,
                    "Rejected / unsuitable source",
                );
            });
        if widgets::secondary(ui, "Record identity decision").clicked() {
            if self.identity != SourceIdentityDecision::IdentityChecked || self.identity_confirmed {
                *action = Some(Action::Identity(self.identity));
            } else {
                self.message = Some(
                    "Compare the official publication before recording identity checked.".into(),
                );
            }
        }
    }
    fn comparison_ui(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        if self.source_id.is_none() {
            return;
        }
        if let Some(source) = self.sources.iter().find(|s| Some(s.id) == self.source_id) {
            let related = self
                .rules
                .iter()
                .filter(|r| r.dependency.source_key == source.metadata.key)
                .collect::<Vec<_>>();
            if related.is_empty() {
                widgets::muted(
                    ui,
                    "Reference only: no automated checks are linked to this source.",
                );
            } else {
                egui::CollapsingHeader::new(format!(
                    "Checks linked to this source ({})",
                    related.len()
                ))
                .id_salt(("source-linked-rules", source.id))
                .default_open(true)
                .show(ui, |ui| {
                    for rule in related {
                        ui.push_id(&rule.dependency.rule_id, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!(
                                    "{} · {}",
                                    rule.dependency.title,
                                    rule.status.label()
                                ));
                                if widgets::secondary(ui, "Assess check").clicked() {
                                    *action =
                                        Some(Action::AssessRule(rule.dependency.rule_id.clone()));
                                }
                            });
                        });
                    }
                });
            }
        }
        let mut changed = false;
        egui::ComboBox::from_id_salt("comparison-left")
            .selected_text(format!("Left version: {}", short(&self.left_hash)))
            .show_ui(ui, |ui| {
                for version in &self.versions {
                    changed |= ui
                        .selectable_value(
                            &mut self.left_hash,
                            version.metadata.raw_sha256.clone(),
                            format!(
                                "{} · {}{}",
                                short(&version.metadata.raw_sha256),
                                version.metadata.acquired_at_utc,
                                if version.active { " · current" } else { "" }
                            ),
                        )
                        .changed();
                }
            });
        egui::ComboBox::from_id_salt("comparison-right")
            .selected_text(match &self.right {
                Some(Right::Candidate(id)) => format!("Downloaded candidate #{id}"),
                Some(Right::Version(hash)) => format!("Preserved version {}", short(hash)),
                None => "Choose right-hand candidate/version".into(),
            })
            .show_ui(ui, |ui| {
                for change in &self.changes {
                    changed |= ui
                        .selectable_value(
                            &mut self.right,
                            Some(Right::Candidate(change.id)),
                            format!(
                                "Download #{} · {}",
                                change.id,
                                short(&change.content_sha256)
                            ),
                        )
                        .changed();
                }
                for version in &self.versions {
                    changed |= ui
                        .selectable_value(
                            &mut self.right,
                            Some(Right::Version(version.metadata.raw_sha256.clone())),
                            format!(
                                "Preserved {}{}",
                                short(&version.metadata.raw_sha256),
                                if version.active { " · current" } else { "" }
                            ),
                        )
                        .changed();
                }
            });
        if changed {
            *action = Some(Action::Compare);
        }
        if matches!(self.right, Some(Right::Candidate(_))) {
            ui.checkbox(
                &mut self.use_ocr,
                "Use local OCR for a scanned PDF candidate",
            );
            if widgets::primary(ui, "Read candidate locally", self.receiver.is_none()).clicked() {
                *action = Some(Action::Prepare);
            }
        }
        ui.horizontal_wrapped(|ui| {
            if widgets::secondary(ui, "Refresh comparison").clicked() {
                *action = Some(Action::Compare);
            }
            if self.right.is_some() && widgets::secondary(ui, "Save right-hand original…").clicked()
            {
                *action = Some(Action::Export);
            }
        });
        if let Some(comparison) = &mut self.comparison {
            ui.label(format!(
                "+{} added · −{} removed · {} unchanged lines",
                comparison.data.added_lines,
                comparison.data.removed_lines,
                comparison.data.unchanged_lines
            ));
            if comparison.data.display_truncated {
                ui.colored_label(AMBER,"The diff display is bounded. Use the full reading sections and preserved originals to review omitted content.");
            }
            ui.checkbox(
                &mut self.full_text,
                "Read complete preserved text in sections",
            );
            if self.full_text {
                ui.columns(2, |columns| {
                    text_sections(
                        &mut columns[0],
                        "Left reading",
                        &comparison.left,
                        &mut self.old_chunk,
                    );
                    text_sections(
                        &mut columns[1],
                        "Right reading",
                        &comparison.right,
                        &mut self.new_chunk,
                    );
                });
            } else {
                let pages = comparison.data.lines.len().div_ceil(60).max(1);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            comparison.page > 0,
                            egui::Button::new("Previous differences"),
                        )
                        .clicked()
                    {
                        comparison.page -= 1;
                    }
                    ui.label(format!(
                        "Difference section {} / {pages}",
                        comparison.page + 1
                    ));
                    if ui
                        .add_enabled(
                            comparison.page + 1 < pages,
                            egui::Button::new("Next differences"),
                        )
                        .clicked()
                    {
                        comparison.page += 1;
                    }
                });
                egui::Grid::new("source-differences")
                    .num_columns(2)
                    .striped(true)
                    .min_col_width((ui.available_width() / 2.0 - 20.0).max(200.0))
                    .show(ui, |ui| {
                        ui.strong("Left source");
                        ui.strong("Right source");
                        ui.end_row();
                        for row in comparison
                            .data
                            .lines
                            .iter()
                            .skip(comparison.page * 60)
                            .take(60)
                        {
                            let text = format!(
                                "{} {}",
                                row.old_line.map(|n| n.to_string()).unwrap_or_default(),
                                row.text.trim_end()
                            );
                            if row.kind == "added" {
                                ui.label("");
                            } else {
                                ui.add(
                                    egui::Label::new(RichText::new(text).background_color(
                                        if row.kind == "removed" {
                                            RED_TINT
                                        } else {
                                            IVORY
                                        },
                                    ))
                                    .wrap(),
                                );
                            }
                            let text = format!(
                                "{} {}",
                                row.new_line.map(|n| n.to_string()).unwrap_or_default(),
                                row.text.trim_end()
                            );
                            if row.kind == "removed" {
                                ui.label("");
                            } else {
                                ui.add(
                                    egui::Label::new(RichText::new(text).background_color(
                                        if row.kind == "added" {
                                            GREEN_TINT
                                        } else {
                                            IVORY
                                        },
                                    ))
                                    .wrap(),
                                );
                            }
                            ui.end_row();
                        }
                    });
                if comparison.data.lines.is_empty() {
                    widgets::muted(
                        ui,
                        if comparison.data.display_truncated {
                            "The line-count limit was reached. Read the complete text in sections."
                        } else {
                            "The preserved reading texts are identical. Raw file bytes may still differ."
                        },
                    );
                }
            }
        }
        ui.separator();
        self.reviewer_form(ui);
        ui.checkbox(
            &mut self.identity_confirmed,
            "I confirmed this is the intended official instrument or version.",
        );
        ui.checkbox(&mut self.original_compared,"I compared the complete reading text with the preserved original, including OCR and numbers.");
        ui.checkbox(
            &mut self.effectivity_reviewed,
            "I reviewed effectivity, amendments, scope and any transition arrangements.",
        );
        let ready = self.comparison.is_some()
            && self.left_hash == self.active_hash
            && self.receiver.is_none();
        match self.right {
            Some(Right::Candidate(_)) => {
                if widgets::primary(
                    ui,
                    "Adopt reviewed candidate",
                    ready
                        && self.identity_confirmed
                        && self.original_compared
                        && self.effectivity_reviewed,
                )
                .clicked()
                {
                    *action = Some(Action::Adopt);
                }
                if widgets::secondary(ui, "Dismiss candidate with no rule impact").clicked() {
                    *action = Some(Action::Dismiss);
                }
                widgets::muted(
                    ui,
                    "Dismiss only a duplicate, unsuitable download or change with no effect on the checking profiles; record the reason. Adoption preserves all previous versions and holds affected rules for revalidation.",
                );
            }
            Some(Right::Version(ref hash))
                if hash != &self.active_hash
                    && widgets::primary(
                        ui,
                        "Restore this reviewed version",
                        ready
                            && self.identity_confirmed
                            && self.original_compared
                            && self.effectivity_reviewed,
                    )
                    .clicked() =>
            {
                *action = Some(Action::Restore);
            }
            _ => {}
        }
    }
    fn rules_ui(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        let held = self.rules.iter().filter(|r| !r.status.usable()).count();
        ui.label(
            RichText::new(format!(
                "{held} checks held · {} compiled profiles",
                self.rules.len()
            ))
            .font(heading(18.0)),
        );
        widgets::muted(
            ui,
            "Each rule is tied to exact source/provision fingerprints. This screen records an assessment of the existing compiled profile; changed thresholds or supporting text need a new tested app/rule release.",
        );
        if self.rule_id.is_empty()
            && let Some(rule) = self
                .rules
                .iter()
                .find(|r| !r.status.usable())
                .or(self.rules.first())
        {
            self.rule_id = rule.dependency.rule_id.clone();
        }
        let old = self.rule_id.clone();
        egui::ComboBox::from_id_salt("revalidation-rule")
            .selected_text(
                self.rules
                    .iter()
                    .find(|r| r.dependency.rule_id == self.rule_id)
                    .map(|r| r.dependency.title.as_str())
                    .unwrap_or("Choose rule"),
            )
            .width(ui.available_width().min(700.0))
            .show_ui(ui, |ui| {
                for rule in &self.rules {
                    ui.selectable_value(
                        &mut self.rule_id,
                        rule.dependency.rule_id.clone(),
                        format!("{} · {}", rule.dependency.rule_id, rule.status.label()),
                    );
                }
            });
        if old != self.rule_id {
            self.amendments_reviewed = false;
            self.withdraw_update_requirement = false;
            self.test_reference.clear();
            self.reviewer.basis.clear();
        }
        if let Some(rule) = self
            .rules
            .iter()
            .find(|r| r.dependency.rule_id == self.rule_id)
            .cloned()
        {
            widgets::pill(
                ui,
                rule.status.label(),
                if rule.status.usable() { GREEN } else { AMBER },
                if rule.status.usable() {
                    GREEN_TINT
                } else {
                    AMBER_TINT
                },
                false,
            );
            ui.label(&rule.reason);
            ui.label(format!(
                "Source: {} · pending downloads: {}",
                rule.dependency.source_key, rule.pending_downloads
            ));
            if let Some(source) = self
                .sources
                .iter()
                .find(|s| s.metadata.key == rule.dependency.source_key)
            {
                ui.label(&source.metadata.title);
                if widgets::secondary(ui, "Compare this source's versions").clicked() {
                    *action = Some(Action::CompareRuleSource(
                        rule.dependency.source_key.clone(),
                    ));
                }
            }
            widgets::muted(
                ui,
                &format!("Compiled source: {}", rule.dependency.raw_sha256),
            );
            widgets::muted(ui, &format!("Current source: {}", rule.active_sha256));
            ui.label(format!(
                "Complete supporting provisions unchanged: {}",
                if rule.matching_provisions {
                    "Yes"
                } else {
                    "No"
                }
            ));
            for provision in &rule.dependency.provisions {
                widgets::muted(ui, &provision.title);
            }
            if let Some(reviewer) = &rule.reviewer {
                ui.label(format!(
                    "Last matching assessment: {} · {} · {}",
                    reviewer.actor, reviewer.review_on, reviewer.basis
                ));
            }
            if let Some(reference) = self.impact_reference.clone() {
                ui.add(
                    egui::Label::new(format!("Related discovered issuance: {reference}"))
                        .wrap()
                        .selectable(true),
                );
                if widgets::secondary(ui, "Clear related issuance").clicked() {
                    self.impact_reference = None;
                }
            }
            self.reviewer_form(ui);
            ui.checkbox(&mut self.amendments_reviewed,"I reviewed the legal change, amendments, applicability and exceptions for this exact profile.");
            if let Some(id) = rule.update_hold_id {
                ui.checkbox(&mut self.withdraw_update_requirement,format!("I explicitly resolve the reviewed update requirements through assessment #{id}; my basis explains why this compiled profile is valid."));
            }
            ui.add(TextEdit::singleline(&mut self.test_reference).desired_width(f32::INFINITY).hint_text("Test/evaluation reference and result, or required implementation/testing work")).accessible_name("Rule evaluation reference and result");
            ui.horizontal_wrapped(|ui| {
                if widgets::primary(
                    ui,
                    "Confirm unchanged profile",
                    rule.matching_provisions
                        && rule.pending_downloads == 0
                        && rule.identity_ready
                        && rule.period_ready
                        && self.amendments_reviewed
                        && (rule.update_hold_id.is_none() || self.withdraw_update_requirement),
                )
                .clicked()
                {
                    *action = Some(Action::Revalidate(true));
                }
                if widgets::secondary(ui, "Record rule update required").clicked() {
                    *action = Some(Action::Revalidate(false));
                }
            });
        }
    }
    fn discovery_ui(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        widgets::muted(
            ui,
            "Approve an official publication/listing page. Daily scans inventory PDF/DOCX and issuance links; they do not infer issue dates or legal effect, crawl other pages, or download discovered documents.",
        );
        let label = ui.label("Official agency publication-page HTTPS URL");
        ui.add(
            TextEdit::singleline(&mut self.page_url)
                .desired_width(f32::INFINITY)
                .hint_text("Official agency publication-page HTTPS URL"),
        )
        .labelled_by(label.id);
        if let Ok(manifest) = starter_manifest() {
            egui::ComboBox::from_id_salt("discovery-domain")
                .selected_text(
                    manifest
                        .domains
                        .iter()
                        .find(|d| d.id == self.page_domain)
                        .map(|d| d.label.as_str())
                        .unwrap_or("Domain"),
                )
                .show_ui(ui, |ui| {
                    for domain in manifest.domains {
                        ui.selectable_value(&mut self.page_domain, domain.id, domain.label);
                    }
                });
        }
        ui.horizontal_wrapped(|ui| {
            if widgets::primary(ui, "Approve daily discovery", true).clicked() {
                *action = Some(Action::ApprovePage);
            }
            if widgets::secondary(ui, "Check approved sources and pages").clicked() {
                self.request_check = true;
            }
        });
        for page in &self.pages {
            egui::CollapsingHeader::new(format!(
                "{} · {}",
                if page.enabled { "Approved" } else { "Disabled" },
                page.url
            ))
            .id_salt(("publication-page", page.id))
            .show(ui, |ui| {
                ui.label(format!("{} · {}", page.domain_id, page.last_status));
                if let Some(error) = &page.last_error {
                    ui.colored_label(AMBER, error);
                }
                if page.enabled && widgets::secondary(ui, "Disable this page").clicked() {
                    *action = Some(Action::DisablePage(page.id));
                }
            });
        }
        ui.separator();
        ui.label(RichText::new("Discovered official citations").font(heading(18.0)));
        self.reviewer_form(ui);
        if ui
            .add(TextEdit::singleline(&mut self.query).hint_text("Filter titles and URLs"))
            .accessible_name("Filter discovered citation titles and URLs")
            .changed()
        {
            self.link_page = 0;
        }
        if ui
            .checkbox(&mut self.show_dismissed, "Include dismissed citations")
            .changed()
        {
            self.link_page = 0;
        }
        let query = self.query.to_lowercase();
        let links = self
            .links
            .iter()
            .filter(|l| {
                (self.show_dismissed || !l.dismissed)
                    && format!("{} {}", l.title, l.url)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();
        let pages = links.len().div_ceil(20).max(1);
        self.link_page = self.link_page.min(pages - 1);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.link_page > 0, egui::Button::new("Previous citations"))
                .clicked()
            {
                self.link_page -= 1;
            }
            ui.label(format!(
                "{} citations · page {} / {pages}",
                links.len(),
                self.link_page + 1
            ));
            if ui
                .add_enabled(
                    self.link_page + 1 < pages,
                    egui::Button::new("Next citations"),
                )
                .clicked()
            {
                self.link_page += 1;
            }
        });
        for link in links.into_iter().skip(self.link_page * 20).take(20) {
            widgets::card().show(ui, |ui| {
                ui.strong(&link.title);
                ui.add(egui::Label::new(&link.url).wrap().selectable(true));
                widgets::muted(
                    ui,
                    if link.initial_scan {
                        "Initial inventory · publication/effectivity unconfirmed"
                    } else {
                        "Newly seen link · publication/effectivity unconfirmed"
                    },
                );
                if let Some(source) = self
                    .sources
                    .iter()
                    .find(|s| s.metadata.source_url == link.url)
                {
                    ui.label("Already saved in the local library");
                    if widgets::secondary(ui, "Review saved source").clicked() {
                        *action = Some(Action::Source(source.id));
                    }
                } else if widgets::secondary(ui, "Add downloaded legal reference…").clicked() {
                    *action = Some(Action::ImportLink(link.id));
                }
                if widgets::secondary(ui, "Assess rule impact…").clicked() {
                    *action = Some(Action::AssessLink(link.id));
                }
                if widgets::secondary(
                    ui,
                    if link.dismissed {
                        "Reopen citation"
                    } else {
                        "Dismiss citation"
                    },
                )
                .clicked()
                {
                    *action = Some(Action::ReviewLink(link.id, !link.dismissed));
                }
            });
        }
    }
    fn history_ui(&mut self, ui: &mut Ui) {
        widgets::muted(
            ui,
            "Append-only metadata, identity, adoption, dismissal and restoration decisions. Recorded identities are self-reported; snapshots and saved reports are retained.",
        );
        for version in &self.versions {
            ui.label(format!(
                "{} · {} · Acquisition provenance: {}",
                if version.active {
                    "Active"
                } else {
                    "Preserved"
                },
                version.metadata.raw_sha256,
                version.metadata.provenance.label()
            ));
        }
        for event in &self.history {
            egui::CollapsingHeader::new(format!(
                "#{} · {} · {} · {}",
                event.id, event.kind, event.reviewer.actor, event.reviewer.review_on
            ))
            .id_salt(("library-event", event.id))
            .show(ui, |ui| {
                ui.label(&event.reviewer.basis);
                widgets::muted(ui, &format!("Version: {}", event.content_sha256));
                if let Ok(json) = serde_json::to_string_pretty(&event.data) {
                    ui.add(egui::Label::new(json).wrap().selectable(true));
                }
            });
        }
    }
    fn adoption_review(&self) -> AdoptionReview {
        AdoptionReview {
            reviewer: self.reviewer.clone(),
            expected_active_sha256: self.active_hash.clone(),
            identity_confirmed: self.identity_confirmed,
            original_and_reading_compared: self.original_compared,
            effectivity_and_scope_reviewed: self.effectivity_reviewed,
        }
    }
    fn perform(
        &mut self,
        action: Action,
        catalog: &LibraryCatalog,
        directory: &Path,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        let id = self.source_id;
        let rule_action = matches!(action, Action::Revalidate(_));
        let mut refresh = false;
        match action {
            Action::Source(id) => {
                self.select_source(catalog, id)?;
                if !matches!(self.tab, Tab::Source | Tab::Compare | Tab::History) {
                    self.tab = Tab::Source;
                }
                self.scroll_top = true;
            }
            Action::SaveMetadata => {
                self.details.issued_on =
                    (!self.issued.trim().is_empty()).then(|| self.issued.trim().into());
                self.details.effective_from =
                    (!self.effective.trim().is_empty()).then(|| self.effective.trim().into());
                ui_result(catalog.edit_source_metadata(
                    id.ok_or("Choose a source.")?,
                    &self.active_hash,
                    &self.details,
                    &self.reviewer,
                ))?;
                self.message = Some(
                    "Reviewed metadata appended. Affected rule profiles now require revalidation."
                        .into(),
                );
                refresh = true;
            }
            Action::Identity(decision) => {
                ui_result(catalog.review_source_identity(
                    id.ok_or("Choose a source.")?,
                    &self.active_hash,
                    decision,
                    &self.reviewer,
                ))?;
                self.message =
                    Some("Source identity decision recorded for this exact version.".into());
                refresh = true;
            }
            Action::Compare => self.load_comparison(catalog)?,
            Action::Prepare => {
                let Some(Right::Candidate(change_id)) = self.right else {
                    return Err("Choose a downloaded candidate.".into());
                };
                if self.receiver.is_some() {
                    return Err("Wait for the current public reader.".into());
                }
                self.cancel.store(false, Ordering::Relaxed);
                let (sender, receiver) = mpsc::channel();
                self.receiver = Some(receiver);
                let directory = directory.to_path_buf();
                let cancel = self.cancel.clone();
                let ctx = ctx.clone();
                let ocr = self.use_ocr;
                std::thread::spawn(move || {
                    let result = (|| {
                        let catalog =
                            LibraryCatalog::open(&directory).map_err(|e| e.to_string())?;
                        if catalog.prepared_candidate(change_id).is_ok() {
                            return Ok(change_id);
                        }
                        let (change, raw) = catalog
                            .downloaded_candidate_bytes(change_id)
                            .map_err(|e| e.to_string())?;
                        let file = prepare_public_reference(raw, &change.mime, ocr, &cancel)?;
                        catalog
                            .prepare_candidate_reading(change_id, file)
                            .map_err(|e| e.to_string())?;
                        Ok(change_id)
                    })();
                    let _ = sender.send(result);
                    ctx.request_repaint();
                });
            }
            Action::Adopt => {
                let Some(Right::Candidate(change_id)) = self.right else {
                    return Err("Choose a downloaded candidate.".into());
                };
                if self.left_hash != self.active_hash || self.comparison.is_none() {
                    return Err("Compare this candidate against the current source first.".into());
                }
                ui_result(catalog.adopt_candidate(change_id, &self.adoption_review()))?;
                self.message=Some("Reviewed source adopted; previous snapshots retained. Open Rule revalidation to assess affected checks.".into());
                refresh = true;
            }
            Action::Dismiss => {
                let Some(Right::Candidate(change_id)) = self.right else {
                    return Err("Choose a downloaded candidate.".into());
                };
                ui_result(catalog.dismiss_candidate(change_id, &self.active_hash, &self.reviewer))?;
                self.message=Some("Candidate dismissed with the recorded no-impact basis. The downloaded bytes remain preserved.".into());
                refresh = true;
            }
            Action::Restore => {
                let Some(Right::Version(ref hash)) = self.right else {
                    return Err("Choose a preserved version.".into());
                };
                ui_result(catalog.restore_preserved_version(
                    id.ok_or("Choose a source.")?,
                    hash,
                    &self.adoption_review(),
                ))?;
                self.message =
                    Some("Reviewed version restored; adoption and source history retained.".into());
                refresh = true;
            }
            Action::Revalidate(unchanged_profile) => {
                let mut reviewer = ui_result(self.reviewer.validated())?;
                if let Some(reference) = &self.impact_reference {
                    reviewer.basis.push_str(&format!(
                        "\nRelated newly seen official citation: {reference}"
                    ));
                }
                let state = self
                    .rules
                    .iter()
                    .find(|r| r.dependency.rule_id == self.rule_id)
                    .ok_or("Choose a compiled rule.")?;
                ui_result(catalog.revalidate_rule(
                    &self.rule_id,
                    &RuleRevalidation {
                        expected_token: state.token.clone(),
                        unchanged_profile,
                        applicability_and_amendments_reviewed: self.amendments_reviewed,
                        test_reference: self.test_reference.clone(),
                        reviewer,
                        supersedes_update_assessment: if unchanged_profile
                            && self.withdraw_update_requirement
                        {
                            state.update_hold_id
                        } else {
                            None
                        },
                    },
                ))?;
                self.message = Some(
                    if unchanged_profile {
                        "Unchanged compiled profile revalidated for the exact current source state."
                    } else {
                        "Rule update requirement recorded. This profile remains held."
                    }
                    .into(),
                );
                refresh = true;
            }
            Action::ApprovePage => {
                ui_result(catalog.approve_publication_page(&self.page_url, &self.page_domain))?;
                self.message=Some("Official publication page approved. Its links will be inventoried during the next source check.".into());
                refresh = true;
            }
            Action::DisablePage(id) => {
                ui_result(catalog.disable_publication_page(id))?;
                self.message =
                    Some("Publication-page discovery disabled; previous scans retained.".into());
                refresh = true;
            }
            Action::ReviewLink(id, dismiss) => {
                ui_result(catalog.review_discovered_issuance(id, dismiss, &self.reviewer))?;
                self.message = Some("Citation review recorded.".into());
                refresh = true;
            }
            Action::ImportLink(id) => {
                let link = self
                    .links
                    .iter()
                    .find(|l| l.id == id)
                    .ok_or("This citation is unavailable.")?;
                self.import_request = Some(ManualSourceInput {
                    title: link.title.clone(),
                    official_url: link.url.clone(),
                    domains: vec![link.domain_id.clone()],
                    ..Default::default()
                });
                self.message=Some("Choose your downloaded public reference in Add legal source. Identity, issue dates and scope still require review.".into());
                self.open = false;
            }
            Action::AssessLink(id) => {
                let link = self
                    .links
                    .iter()
                    .find(|l| l.id == id)
                    .ok_or("This citation is unavailable.")?;
                self.impact_reference = Some(format!("{} · {}", link.title, link.url));
                self.reviewer.basis.clear();
                self.test_reference.clear();
                self.amendments_reviewed = false;
                self.withdraw_update_requirement = false;
                self.tab = Tab::Rules;
                self.scroll_top = true;
                self.message=Some("Select each affected rule, record the issuance's legal impact and assessment basis, then record the required rule review/update. Discovery does not establish legal effect.".into());
            }
            Action::AssessRule(id) => {
                self.rule_id = id;
                self.tab = Tab::Rules;
                self.scroll_top = true;
                self.amendments_reviewed = false;
                self.withdraw_update_requirement = false;
                self.test_reference.clear();
                self.reviewer.basis.clear();
                self.impact_reference = None;
            }
            Action::CompareRuleSource(key) => {
                let id = self
                    .sources
                    .iter()
                    .find(|s| s.metadata.key == key)
                    .ok_or("This rule's source is unavailable.")?
                    .id;
                self.select_source(catalog, id)?;
                self.tab = Tab::Compare;
                self.scroll_top = true;
                self.right = self
                    .changes
                    .first()
                    .map(|c| Right::Candidate(c.id))
                    .or_else(|| {
                        self.versions
                            .iter()
                            .find(|v| !v.active)
                            .map(|v| Right::Version(v.metadata.raw_sha256.clone()))
                    });
                if self.right.is_some() {
                    self.load_comparison(catalog)?;
                }
            }
            Action::Export => {
                let target = self.right.clone().ok_or("Choose a public original.")?;
                let (name, mime) = match &target {
                    Right::Candidate(id) => {
                        let (change, _) = ui_result(catalog.downloaded_candidate_bytes(*id))?;
                        (format!("source-candidate-{id}"), change.mime)
                    }
                    Right::Version(hash) => {
                        let metadata = &self
                            .versions
                            .iter()
                            .find(|v| &v.metadata.raw_sha256 == hash)
                            .ok_or("This version is unavailable.")?
                            .metadata;
                        (format!("source-{}", short(hash)), metadata.raw_mime.clone())
                    }
                };
                let extension = match mime.as_str() {
                    "application/pdf" => "pdf",
                    "text/html" => "html",
                    "text/plain" => "txt",
                    _ => "docx",
                };
                self.export = FileDialog::new()
                    .title("Save preserved public original")
                    .default_file_name(&format!("{name}.{extension}"))
                    .as_modal(true);
                self.export.save_file();
                self.export_right = Some(target);
            }
        }
        if refresh {
            self.load(catalog)?;
            if rule_action {
                self.amendments_reviewed = false;
                self.withdraw_update_requirement = false;
                self.reviewer.basis.clear();
            } else if let Some(id) = id {
                self.select_source(catalog, id)?;
            }
            self.changed = true;
        }
        ctx.request_repaint();
        Ok(())
    }
}
fn short(value: &str) -> String {
    value.chars().take(14).collect()
}
fn text_sections(ui: &mut Ui, title: &str, text: &str, chunk: &mut usize) {
    let boundaries = crate::text_navigation::chunk_boundaries(text);
    let count = boundaries.len().saturating_sub(1).max(1);
    *chunk = (*chunk).min(count - 1);
    ui.strong(title);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(*chunk > 0, egui::Button::new("Previous"))
            .clicked()
        {
            *chunk -= 1;
        }
        ui.label(format!("{} / {count}", *chunk + 1));
        if ui
            .add_enabled(*chunk + 1 < count, egui::Button::new("Next"))
            .clicked()
        {
            *chunk += 1;
        }
    });
    if let (Some(start), Some(end)) = (boundaries.get(*chunk), boundaries.get(*chunk + 1)) {
        ScrollArea::vertical()
            .id_salt(("complete-reference-text", title))
            .max_height(350.0)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(&text[*start..*end])
                        .wrap()
                        .selectable(true),
                );
            });
    }
}
