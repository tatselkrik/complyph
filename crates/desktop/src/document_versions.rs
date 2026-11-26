//! Native document history, retained-text comparison and separate historical review streams.
use crate::{reader_view::ReaderView, theme::*, widgets};
use complyph_core::{
    SavedReview, Workspace, WorkspacePaths,
    document_comparison::{CompareRequest, DocumentComparison, FindingChange},
    document_history::{DocumentHistory, RetainedReading, RevisionLinkRequest},
};
use eframe::egui::{self, RichText, ScrollArea, Ui};
use egui_file_dialog::{FileDialog, Filter};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
enum Tab {
    #[default]
    History,
    Compare,
}
#[derive(Clone, Copy)]
enum ExportKind {
    Pdf,
    Json,
}
enum Job {
    Comparison(Box<DocumentComparison>),
    Imported(crate::document_cli::RevisionImport),
    Exported(String),
}
enum Action {
    Refresh,
    Link,
    Unlink(i64),
    Compare,
    Export(ExportKind),
    OpenReading(i64),
    OpenReview(i64),
    CurrentFile(i64, Option<i64>),
    Passage(bool, complyph_checks::Evidence),
}
pub enum VersionAction {
    Reading(Box<RetainedReading>),
    Review(Box<SavedReview>),
    CurrentFile {
        document_id: i64,
        context: Option<complyph_checks::Context>,
    },
}
struct Candidate {
    id: i64,
    group: i64,
    name: String,
}
#[derive(Clone)]
struct Choice {
    id: i64,
    version: i64,
    document_id: i64,
    label: String,
}
pub struct DocumentVersions {
    open: bool,
    document_id: Option<i64>,
    history: Option<DocumentHistory>,
    candidates: Vec<Candidate>,
    link_to: Option<i64>,
    choices: Vec<Choice>,
    tab: Tab,
    scroll_top: bool,
    left: Option<i64>,
    right: Option<i64>,
    left_review: Option<i64>,
    right_review: Option<i64>,
    comparison: Option<Box<DocumentComparison>>,
    left_reader: ReaderView,
    right_reader: ReaderView,
    full_text: bool,
    diff_page: usize,
    show_unchanged: bool,
    import: FileDialog,
    export: FileDialog,
    export_kind: ExportKind,
    work: Option<Receiver<Result<Job, String>>>,
    cancel: Arc<AtomicBool>,
    message: Option<String>,
    changed: bool,
}
impl Default for DocumentVersions {
    fn default() -> Self {
        Self {
            open: false,
            document_id: None,
            history: None,
            candidates: vec![],
            link_to: None,
            choices: vec![],
            tab: Tab::History,
            scroll_top: true,
            left: None,
            right: None,
            left_review: None,
            right_review: None,
            comparison: None,
            left_reader: ReaderView::default(),
            right_reader: ReaderView::default(),
            full_text: false,
            diff_page: 0,
            show_unchanged: false,
            import: FileDialog::new()
                .title("Choose a revised PDF or Word document")
                .add_file_filter(
                    "PDF and Word",
                    Filter::new(|p| complyph_core::DocumentKind::from_path(p).is_some()),
                )
                .default_file_filter("PDF and Word")
                .show_all_files_filter(false)
                .as_modal(true),
            export: FileDialog::new()
                .title("Export comparison — choose a new filename")
                .as_modal(true),
            export_kind: ExportKind::Pdf,
            work: None,
            cancel: Arc::new(AtomicBool::new(false)),
            message: None,
            changed: false,
        }
    }
}
impl Drop for DocumentVersions {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl DocumentVersions {
    pub fn clear(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.work = None;
        self.open = false;
        self.history = None;
        self.comparison = None;
        self.left_reader.clear();
        self.right_reader.clear();
        self.choices.clear();
        self.candidates.clear();
        self.document_id = None;
    }
    pub fn open(&mut self, workspace: &Workspace, id: i64) {
        self.clear();
        self.changed = true;
        self.document_id = Some(id);
        self.tab = Tab::History;
        self.scroll_top = true;
        self.message = None;
        self.left = None;
        self.right = None;
        self.comparison = None;
        match self.load(workspace) {
            Ok(()) => self.open = true,
            Err(e) => {
                self.message = Some(e);
                self.open = true;
            }
        }
    }
    fn load(&mut self, workspace: &Workspace) -> Result<(), String> {
        let id = self.document_id.ok_or("Choose a document.")?;
        let history = workspace.document_history(id).map_err(|e| e.to_string())?;
        self.candidates.clear();
        for document in workspace.documents().map_err(|e| e.to_string())? {
            let candidate = workspace
                .document_history(document.id)
                .map_err(|e| e.to_string())?;
            if candidate.group_id != history.group_id && candidate.documents.len() == 1 {
                self.candidates.push(Candidate {
                    id: document.id,
                    group: candidate.group_id,
                    name: document.name,
                });
            }
        }
        self.choices = history
            .versions
            .iter()
            .flat_map(|v| {
                v.readings.iter().map(move |r| Choice {
                    id: r.id,
                    version: v.id,
                    document_id: v.document_id,
                    label: format!(
                        "Version #{} · {} · {} · {}{}",
                        v.id,
                        v.name,
                        short(&v.document_sha256),
                        r.status.label(),
                        if r.ocr { " · OCR" } else { "" }
                    ),
                })
            })
            .collect();
        self.history = Some(history);
        if !self.choices.iter().any(|c| Some(c.id) == self.right) {
            self.right = self.choices.last().map(|c| c.id);
        }
        if !self.choices.iter().any(|c| Some(c.id) == self.left) {
            let version = self
                .choices
                .iter()
                .find(|c| Some(c.id) == self.right)
                .map(|c| c.version);
            self.left = self
                .choices
                .iter()
                .rev()
                .find(|c| Some(c.version) != version)
                .or_else(|| self.choices.iter().rev().find(|c| Some(c.id) != self.right))
                .map(|c| c.id)
                .or(self.right);
        }
        self.left_review = self.default_review(self.left);
        self.right_review = self.default_review(self.right);
        Ok(())
    }
    fn default_review(&self, id: Option<i64>) -> Option<i64> {
        self.history
            .as_ref()?
            .versions
            .iter()
            .flat_map(|v| v.reviews.iter())
            .rev()
            .find(|r| r.reading_id == id && id.is_some())
            .map(|r| r.id)
    }
    fn choices_changed(&mut self) {
        self.comparison = None;
        self.full_text = false;
        self.diff_page = 0;
        self.left_reader.clear();
        self.right_reader.clear();
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        workspace: &Workspace,
        paths: &WorkspacePaths,
    ) -> (bool, Option<VersionAction>) {
        if let Some(result) = self.work.as_ref().and_then(|r| r.try_recv().ok()) {
            self.work = None;
            match result {
                Ok(Job::Comparison(comparison)) => {
                    self.left_reader
                        .open_retained(comparison.left.reading.saved.clone());
                    self.right_reader
                        .open_retained(comparison.right.reading.saved.clone());
                    self.comparison = Some(comparison);
                    self.message = None;
                }
                Ok(Job::Imported(imported)) => {
                    self.changed = true;
                    let previous = self.right;
                    match self.load(workspace) {
                        Ok(()) => {
                            self.right = Some(imported.reading_id);
                            self.left = previous.filter(|id| Some(*id) != self.right).or(self.left);
                            self.left_review = self.default_review(self.left);
                            self.right_review = self.default_review(self.right);
                            if self.left == self.right {
                                self.left = self
                                    .choices
                                    .iter()
                                    .rev()
                                    .find(|c| Some(c.id) != self.right)
                                    .map(|c| c.id)
                                    .or(self.right);
                                self.left_review = self.default_review(self.left);
                            }
                            self.choices_changed();
                            self.tab = Tab::Compare;
                            self.scroll_top = true;
                            self.message = Some(format!(
                                "Revision linked and reading retained for document #{}. {}. No review was started; confirm the revised text and scope before checking.",
                                imported.document_id,
                                imported.status.label()
                            ));
                        }
                        Err(e) => self.message = Some(e),
                    }
                }
                Ok(Job::Exported(message)) => self.message = Some(message),
                Err(e) => self.message = Some(e),
            }
        }
        if self.work.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let mut external = None;
        if self.open {
            let mut open = self.open;
            let mut action = None;
            egui::Window::new("Document versions").open(&mut open)
                .default_size([1080.0,680.0]).max_width((ctx.content_rect().width()-24.0).max(600.0))
                .max_height((ctx.content_rect().height()-24.0).max(400.0))
                .anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui|{
                ui.horizontal_wrapped(|ui|{
                    let old=self.tab;
                    ui.selectable_value(&mut self.tab,Tab::History,"Version history");
                    ui.selectable_value(&mut self.tab,Tab::Compare,"Compare versions");
                    if old!=self.tab {self.scroll_top=true;}
                    if ui.button("Refresh history").clicked(){action=Some(Action::Refresh);}
                    if self.work.is_some(){ui.spinner();ui.label("Processing locally…");}
                });
                widgets::muted(ui,"Retained text and review histories stay on this computer. Original files stay in place. Revision links are chosen explicitly.");
                if let Some(message)=&self.message{ui.colored_label(AMBER,message);}
                ui.separator();
                let mut area=ScrollArea::vertical().id_salt(("document-version-content",self.tab))
                    .max_height((ui.available_height()-8.0).max(280.0));
                if std::mem::take(&mut self.scroll_top){area=area.vertical_scroll_offset(0.0);}
                area.show(ui,|ui|{
                    ui.add_enabled_ui(self.work.is_none(),|ui|{
                        match self.tab {
                            Tab::History=>self.history_ui(ui,&mut action),
                            Tab::Compare=>self.compare_ui(ui,&mut action),
                        }
                    });
                });
            });
            if !open && self.open {
                self.cancel.store(true, Ordering::Relaxed);
            }
            self.open = open;
            if let Some(action) = action {
                match self.perform(action, ctx, workspace, paths) {
                    Ok(result) => external = result,
                    Err(e) => self.message = Some(e),
                }
            }
        }
        self.import.update(ctx);
        if let Some(path) = self.import.take_picked()
            && let (Some(id), Some(history)) = (self.document_id, self.history.as_ref())
        {
            let group = history.group_id;
            let paths = paths.clone();
            self.launch(ctx, move |cancel| {
                crate::document_cli::add_revision(&paths, id, &path, group, &cancel)
                    .map(Job::Imported)
            });
        }
        self.export.update(ctx);
        if let Some(path) = self.export.take_picked()
            && let Some(comparison) = self.comparison.clone()
        {
            let kind = self.export_kind;
            self.launch(ctx, move |_| {
                match kind {
                    ExportKind::Pdf => complyph_reports::export_comparison_pdf(&comparison, &path),
                    ExportKind::Json => {
                        complyph_reports::export_comparison_json(&comparison, &path)
                    }
                }
                .map_err(|e| e.to_string())?;
                Ok(Job::Exported(format!(
                    "Comparison saved locally: {}",
                    path.display()
                )))
            });
        }
        (std::mem::take(&mut self.changed), external)
    }
    fn launch(
        &mut self,
        ctx: &egui::Context,
        run: impl FnOnce(Arc<AtomicBool>) -> Result<Job, String> + Send + 'static,
    ) {
        if self.work.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        self.work = Some(receiver);
        self.message = None;
        std::thread::spawn(move || {
            let _ = sender.send(run(cancel));
            ctx.request_repaint();
        });
    }
    fn history_ui(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        let Some(history) = &self.history else {
            widgets::muted(ui, "Choose a document to open its history.");
            return;
        };
        ui.label(RichText::new(&history.title).font(heading(19.0)));
        ui.label(format!(
            "{} linked files · {} retained file versions",
            history.documents.len(),
            history.versions.len()
        ));
        ui.horizontal_wrapped(|ui| {
            if widgets::primary(ui, "Add revised document…", true).clicked() {
                self.import.pick_file();
            }
            if let Some(id) = self.document_id
                && ui.button("Review current file").clicked()
            {
                *action = Some(Action::CurrentFile(id, None));
            }
        });
        widgets::muted(
            ui,
            "A changed file at the same path is captured when read again. Identical bytes in the same file entry reuse its version; different OCR/reader results retain separate readings. Importing never starts a review.",
        );
        egui::CollapsingHeader::new("Link an existing document as a revision").show(ui,|ui|{
            egui::ComboBox::from_id_salt("revision-candidate").selected_text(self.candidates.iter().find(|c|Some(c.id)==self.link_to).map(|c|c.name.as_str()).unwrap_or("Choose a standalone document")).show_ui(ui,|ui|{
                for c in &self.candidates {ui.selectable_value(&mut self.link_to,Some(c.id),&c.name);}
            });
            if ui.add_enabled(self.link_to.is_some(),egui::Button::new("Link selected revision")).clicked(){*action=Some(Action::Link);}
            widgets::muted(ui,"Only standalone entries appear here. Linking does not transfer notes, statuses or legal conclusions. Unlinking preserves each file's history.");
        });
        if history.documents.len() > 1 {
            egui::CollapsingHeader::new("Linked files and link corrections").show(ui, |ui| {
                for d in &history.documents {
                    ui.push_id(d.document_id, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(&d.name);
                            if ui.button("Unlink file; keep its history").clicked() {
                                *action = Some(Action::Unlink(d.document_id));
                            }
                        });
                    });
                }
            });
        }
        ui.separator();
        if history.versions.is_empty() {
            widgets::muted(
                ui,
                "Read this document first to retain its text. Earlier reviews whose full text was never saved cannot reconstruct a missing original.",
            );
        }
        for version in history.versions.iter().rev() {
            egui::CollapsingHeader::new(format!("Version #{} · {} · {}",version.id,version.name,short(&version.document_sha256)))
                .id_salt(("document-version",version.id)).default_open(true).show(ui,|ui|{
                widgets::muted(ui,&format!("File SHA-256: {}",version.document_sha256));
                widgets::muted(ui,&format!("First captured: {}. This is not a signing/effectivity date.",crate::app::format_date(version.captured_at)));
                if version.readings.is_empty(){ui.colored_label(AMBER,"Historical review only. Its complete reading was not retained; a changed or missing original cannot reconstruct it.");}
                for reading in &version.readings {
                    ui.push_id(reading.id,|ui|{ui.horizontal_wrapped(|ui|{
                        ui.label(format!("Reading #{} · {} · {} · {} text bytes{}",reading.id,reading.engine,reading.status.label(),reading.text_bytes,if reading.ocr{" · OCR"}else{""}));
                        if ui.button("Read saved text").clicked(){*action=Some(Action::OpenReading(reading.id));}
                        if ui.button("Use as revised side").clicked(){
                            self.right=Some(reading.id);self.right_review=version.reviews.iter().rev().find(|r|r.reading_id==Some(reading.id)).map(|r|r.id);
                            self.tab=Tab::Compare;self.scroll_top=true;self.comparison=None;
                        }
                    });});
                    widgets::muted(ui,&format!("Reading SHA-256: {}",reading.reading_sha256));
                }
                for review in version.reviews.iter().rev(){
                    ui.horizontal_wrapped(|ui|{
                        ui.label(format!("Review #{} · {} · {}",review.id,crate::app::format_date(review.created_at),
                            review.reading_id.map(|id|format!("reading #{id}")).unwrap_or_else(||"exact reading not recorded".into())));
                        if ui.button("Open saved review and notes").clicked(){*action=Some(Action::OpenReview(review.id));}
                    });
                }
            });
        }
    }
    fn selectors(&mut self, ui: &mut Ui) {
        let old = (self.left, self.right, self.left_review, self.right_review);
        for is_left in [true, false] {
            let (reading, review) = if is_left {
                (&mut self.left, &mut self.left_review)
            } else {
                (&mut self.right, &mut self.right_review)
            };
            ui.label(if is_left {
                "Earlier selected version"
            } else {
                "Revised selected version"
            });
            let before = *reading;
            egui::ComboBox::from_id_salt(("version-choice", is_left))
                .selected_text(
                    self.choices
                        .iter()
                        .find(|c| Some(c.id) == *reading)
                        .map(|c| c.label.as_str())
                        .unwrap_or("Choose retained reading"),
                )
                .width(ui.available_width().min(900.0))
                .show_ui(ui, |ui| {
                    for c in &self.choices {
                        ui.selectable_value(reading, Some(c.id), &c.label);
                    }
                });
            let selected = self.choices.iter().find(|c| Some(c.id) == *reading);
            let version = selected.and_then(|c| {
                self.history
                    .as_ref()?
                    .versions
                    .iter()
                    .find(|v| v.id == c.version)
            });
            if before != *reading {
                *review = version.and_then(|v| {
                    v.reviews
                        .iter()
                        .rev()
                        .find(|r| r.reading_id == *reading)
                        .map(|r| r.id)
                });
            }
            egui::ComboBox::from_id_salt(("version-review", is_left))
                .selected_text(
                    review
                        .map(|id| format!("Saved review #{id}"))
                        .unwrap_or_else(|| "Text only · no saved review selected".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(review, None, "Text only · no saved review selected");
                    if let Some(v) = version {
                        for r in v.reviews.iter().rev() {
                            ui.selectable_value(
                                review,
                                Some(r.id),
                                format!(
                                    "Review #{} · {} · {}",
                                    r.id,
                                    crate::app::format_date(r.created_at),
                                    if r.reading_id == *reading {
                                        "exact reading"
                                    } else if r.reading_id.is_none() {
                                        "legacy reading not recorded"
                                    } else {
                                        "different reading of this file"
                                    }
                                ),
                            );
                        }
                    }
                });
        }
        if old != (self.left, self.right, self.left_review, self.right_review) {
            self.choices_changed();
        }
    }
    fn compare_ui(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        self.selectors(ui);
        ui.horizontal_wrapped(|ui| {
            let ready = self.left.is_some()
                && self.right.is_some()
                && (self.left != self.right || self.left_review != self.right_review);
            if widgets::primary(ui, "Compare selected versions", ready).clicked() {
                *action = Some(Action::Compare);
            }
            if let Some(choice) = self.choices.iter().find(|c| Some(c.id) == self.right) {
                if ui.button("Review revised file").clicked() {
                    *action = Some(Action::CurrentFile(choice.document_id, None));
                }
                if ui
                    .add_enabled(
                        self.left_review.is_some(),
                        egui::Button::new("Use earlier scope for a fresh review…"),
                    )
                    .clicked()
                {
                    *action = Some(Action::CurrentFile(choice.document_id, self.left_review));
                }
            }
        });
        widgets::muted(
            ui,
            "Fresh reviews require new text/OCR confirmations. Earlier notes and resolved statuses are never copied. Only matching rule, source, scope and reading-method bases support a direct finding comparison.",
        );
        let Some(comparison) = &self.comparison else {
            return;
        };
        ui.separator();
        widgets::muted(
            ui,
            &format!(
                "Comparison captured {}. Refresh to include later notes or statuses.",
                crate::app::format_date(comparison.captured_at)
            ),
        );
        if comparison.reading_only_change {
            ui.colored_label(AMBER,"Same original file bytes; different saved reading. This is not evidence of a document edit.");
        } else if comparison.same_file_bytes {
            widgets::muted(
                ui,
                "The selected file bytes are identical. Review or follow-up changes do not establish a document edit.",
            );
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Export comparison PDF…").clicked() {
                *action = Some(Action::Export(ExportKind::Pdf));
            }
            if ui.button("Export full comparison JSON…").clicked() {
                *action = Some(Action::Export(ExportKind::Json));
            }
            ui.checkbox(&mut self.full_text, "Read complete retained texts");
        });
        widgets::muted(
            ui,
            "PDF includes displayed differences and both selected review histories. JSON also contains the complete retained texts. Exports are private local files; existing files are never overwritten.",
        );
        if self.full_text {
            let height = (ui.available_height() - 10.0).clamp(300.0, 550.0);
            ui.columns(2, |columns| {
                columns[0].label(RichText::new(&comparison.left.reading.name).strong());
                columns[1].label(RichText::new(&comparison.right.reading.name).strong());
                columns[0].push_id("earlier-comparison-reader", |ui| {
                    self.left_reader.show(ui, height);
                });
                columns[1].push_id("revised-comparison-reader", |ui| {
                    self.right_reader.show(ui, height);
                });
            });
            return;
        }
        ui.label(RichText::new("Text differences").font(heading(18.0)));
        widgets::muted(ui, &comparison.text.detail);
        if comparison.text.available {
            ui.label(format!(
                "+{} added · −{} removed · {} unchanged lines",
                comparison.text.added_lines.unwrap_or(0),
                comparison.text.removed_lines.unwrap_or(0),
                comparison.text.unchanged_lines.unwrap_or(0)
            ));
        }
        if comparison.text.display_truncated {
            ui.colored_label(AMBER,"Difference display is truncated. Inspect the complete retained texts and originals; full text is included in JSON.");
        }
        let rows = paired_rows(&comparison.text.lines);
        let pages = rows.len().div_ceil(60).max(1);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.diff_page > 0,
                    egui::Button::new("Previous differences"),
                )
                .clicked()
            {
                self.diff_page -= 1;
            }
            ui.label(format!(
                "Difference section {} / {pages}",
                self.diff_page + 1
            ));
            if ui
                .add_enabled(
                    self.diff_page + 1 < pages,
                    egui::Button::new("Next differences"),
                )
                .clicked()
            {
                self.diff_page += 1;
            }
        });
        let width = (ui.available_width() - 24.0) / 2.0;
        egui::Grid::new("private-document-diff")
            .num_columns(2)
            .min_col_width(width)
            .max_col_width(width)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Earlier");
                ui.strong("Revised");
                ui.end_row();
                for row in rows.iter().skip(self.diff_page * 60).take(60) {
                    diff_cell(ui, row.old, true);
                    diff_cell(ui, row.new, false);
                    ui.end_row();
                }
            });
        ui.add_space(12.0);
        ui.separator();
        ui.label(RichText::new("Findings and separate follow-up").font(heading(18.0)));
        ui.checkbox(&mut self.show_unchanged, "Include unchanged findings");
        if comparison.findings.is_empty() {
            widgets::muted(
                ui,
                "Select a saved review on both sides to compare findings. A text edit alone is not a new legal assessment.",
            );
        }
        for finding in &comparison.findings {
            if finding.change == FindingChange::Unchanged && !self.show_unchanged {
                continue;
            }
            egui::CollapsingHeader::new(format!("{} · {}", finding.title, finding.change.label()))
                .id_salt(("compared-finding", &finding.rule_id))
                .default_open(finding.change != FindingChange::Unchanged)
                .show(ui, |ui| {
                    for reason in &finding.reasons {
                        ui.colored_label(AMBER, reason);
                    }
                    ui.columns(2, |columns| {
                        for (index, column) in columns.iter_mut().enumerate() {
                            let is_left = index == 0;
                            let (side, outcome, status) = if is_left {
                                (&comparison.left, finding.left_outcome, finding.left_status)
                            } else {
                                (
                                    &comparison.right,
                                    finding.right_outcome,
                                    finding.right_status,
                                )
                            };
                            column.push_id((is_left, &finding.rule_id), |ui| {
                                ui.label(if is_left {
                                    "Earlier review"
                                } else {
                                    "Revised review"
                                });
                                ui.label(outcome.map(|o| o.label()).unwrap_or("Not assessed"));
                                ui.strong(format!(
                                    "Human follow-up: {}",
                                    status.map(|s| s.label()).unwrap_or("Not recorded")
                                ));
                                if let Some(review) = &side.review {
                                    if ui
                                        .button(format!(
                                            "Open review #{} and its notes",
                                            review.workflow.saved.id
                                        ))
                                        .clicked()
                                    {
                                        *action =
                                            Some(Action::OpenReview(review.workflow.saved.id));
                                    }
                                    if let Some(f) = review
                                        .workflow
                                        .saved
                                        .review
                                        .findings
                                        .iter()
                                        .find(|f| f.rule_id == finding.rule_id)
                                    {
                                        for evidence in &f.evidence {
                                            ui.label(format!(
                                                "{} · {}",
                                                evidence.location.label(),
                                                evidence.text
                                            ));
                                            if ui.button("Show passage in retained text").clicked()
                                            {
                                                *action = Some(Action::Passage(
                                                    is_left,
                                                    evidence.clone(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    });
                });
        }
        widgets::muted(ui, &comparison.limitation);
    }
    fn perform(
        &mut self,
        action: Action,
        ctx: &egui::Context,
        workspace: &Workspace,
        paths: &WorkspacePaths,
    ) -> Result<Option<VersionAction>, String> {
        match action {
            Action::Refresh => {
                self.load(workspace)?;
                self.choices_changed();
                self.message = Some(
                    "History refreshed. Compare again to capture current saved follow-up.".into(),
                );
            }
            Action::Link => {
                let id = self.document_id.ok_or("Choose a base document.")?;
                let history = self.history.as_ref().ok_or("Refresh document history.")?;
                let candidate = self
                    .candidates
                    .iter()
                    .find(|c| Some(c.id) == self.link_to)
                    .ok_or("Choose a standalone revised file.")?;
                let revised_id = candidate.id;
                let previous = self.right;
                workspace
                    .link_document_revision(&RevisionLinkRequest {
                        document_id: id,
                        revised_document_id: candidate.id,
                        expected_group_id: history.group_id,
                        expected_revision_group_id: candidate.group,
                    })
                    .map_err(|e| e.to_string())?;
                self.changed = true;
                self.load(workspace)?;
                self.right = self
                    .choices
                    .iter()
                    .rev()
                    .find(|c| c.document_id == revised_id)
                    .map(|c| c.id)
                    .or(self.right);
                self.left = previous.filter(|id| Some(*id) != self.right).or(self.left);
                self.left_review = self.default_review(self.left);
                self.right_review = self.default_review(self.right);
                self.choices_changed();
                self.tab = Tab::Compare;
                self.scroll_top = true;
                self.message = Some(
                    "Revision linked. Each review and its notes/statuses remain separate.".into(),
                );
            }
            Action::Unlink(id) => {
                let group = self
                    .history
                    .as_ref()
                    .ok_or("Refresh document history.")?
                    .group_id;
                workspace
                    .unlink_document_revision(id, group)
                    .map_err(|e| e.to_string())?;
                self.changed = true;
                self.load(workspace)?;
                self.choices_changed();
                self.message =
                    Some("File unlinked. Its versions, reports and notes are preserved.".into());
            }
            Action::Compare => {
                let request = CompareRequest {
                    left_reading_id: self.left.ok_or("Choose the earlier reading.")?,
                    right_reading_id: self.right.ok_or("Choose the revised reading.")?,
                    left_review_id: self.left_review,
                    right_review_id: self.right_review,
                };
                let paths = paths.clone();
                self.diff_page = 0;
                self.launch(ctx, move |_| {
                    Workspace::open(&paths)
                        .and_then(|w| w.compare_document_versions(&request))
                        .map(|c| Job::Comparison(Box::new(c)))
                        .map_err(|e| e.to_string())
                });
            }
            Action::Export(kind) => {
                self.export_kind = kind;
                self.export = FileDialog::new()
                    .title("Export private comparison — new filename")
                    .default_file_name(&format!(
                        "ComplyPH-comparison-{}.{}",
                        chrono::Local::now().format("%Y%m%d-%H%M%S"),
                        match kind {
                            ExportKind::Pdf => "pdf",
                            ExportKind::Json => "json",
                        }
                    ))
                    .as_modal(true);
                self.export.save_file();
            }
            Action::OpenReading(id) => {
                let reading = workspace.retained_reading(id).map_err(|e| e.to_string())?;
                self.open = false;
                return Ok(Some(VersionAction::Reading(Box::new(reading))));
            }
            Action::OpenReview(id) => {
                let workflow = workspace.review_workflow(id).map_err(|e| e.to_string())?;
                let expected = self
                    .history
                    .as_ref()
                    .and_then(|h| {
                        h.versions
                            .iter()
                            .find(|v| v.reviews.iter().any(|r| r.id == id))
                    })
                    .map(|v| v.id);
                if expected.is_none()
                    || workflow.document_version.as_ref().map(|v| v.version_id) != expected
                {
                    return Err(
                        "The saved review changed. Refresh document history before opening it."
                            .into(),
                    );
                }
                let review = workflow.saved;
                self.open = false;
                return Ok(Some(VersionAction::Review(Box::new(review))));
            }
            Action::CurrentFile(document_id, from_review) => {
                let context = from_review
                    .map(|id| {
                        workspace
                            .review_workflow(id)
                            .map(|r| r.saved.review.context)
                    })
                    .transpose()
                    .map_err(|e| e.to_string())?;
                self.open = false;
                return Ok(Some(VersionAction::CurrentFile {
                    document_id,
                    context,
                }));
            }
            Action::Passage(left, evidence) => {
                let comparison = self
                    .comparison
                    .as_ref()
                    .ok_or("Compare the versions first.")?;
                let side = if left {
                    &comparison.left
                } else {
                    &comparison.right
                };
                if left {
                    self.left_reader
                        .jump_to(&side.reading.saved.sha256, &evidence);
                } else {
                    self.right_reader
                        .jump_to(&side.reading.saved.sha256, &evidence);
                }
                self.full_text = true;
                self.scroll_top = true;
            }
        }
        Ok(None)
    }
}
fn short(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

struct PairedRow<'a> {
    old: Option<&'a complyph_core::document_comparison::DifferenceLine>,
    new: Option<&'a complyph_core::document_comparison::DifferenceLine>,
}
// Align contiguous replacement blocks by line order; retain the exact locations/text.
fn paired_rows(lines: &[complyph_core::document_comparison::DifferenceLine]) -> Vec<PairedRow<'_>> {
    let mut rows = vec![];
    let mut index = 0;
    while index < lines.len() {
        if matches!(lines[index].kind.as_str(), "added" | "removed") {
            let start = index;
            while index < lines.len() && matches!(lines[index].kind.as_str(), "added" | "removed") {
                index += 1;
            }
            let removed = lines[start..index]
                .iter()
                .filter(|r| r.kind == "removed")
                .collect::<Vec<_>>();
            let added = lines[start..index]
                .iter()
                .filter(|r| r.kind == "added")
                .collect::<Vec<_>>();
            for i in 0..removed.len().max(added.len()) {
                rows.push(PairedRow {
                    old: removed.get(i).copied(),
                    new: added.get(i).copied(),
                });
            }
        } else {
            rows.push(PairedRow {
                old: Some(&lines[index]),
                new: Some(&lines[index]),
            });
            index += 1;
        }
    }
    rows
}
fn diff_cell(
    ui: &mut Ui,
    row: Option<&complyph_core::document_comparison::DifferenceLine>,
    earlier: bool,
) {
    if let Some(row) = row {
        let location = if earlier {
            &row.old_location
        } else {
            &row.new_location
        };
        let label = location.as_ref().map(|l| l.label()).unwrap_or_default();
        ui.label(
            RichText::new(format!("{label}\n{}", row.text)).background_color(
                match row.kind.as_str() {
                    "removed" => RED_TINT,
                    "added" => GREEN_TINT,
                    _ => IVORY,
                },
            ),
        );
    } else {
        ui.label("");
    }
}
