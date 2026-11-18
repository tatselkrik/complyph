use crate::{theme::*, widgets};
use complyph_checks::{Applicability, Context, Outcome};
use complyph_core::{Document, SavedExtraction, SavedReview, Workspace};
use complyph_library::LibraryCatalog;
use eframe::egui::{self, RichText, Ui};

#[derive(Default)]
pub struct CheckView {
    pub visible: bool,
    context: Context,
    result: Option<SavedReview>,
    error: Option<String>,
    editing: bool,
}

pub enum ReviewAction {
    Passage {
        hash: String,
        evidence: complyph_checks::Evidence,
    },
    Provision {
        review: Box<SavedReview>,
        finding: usize,
    },
}

pub fn approved_source(catalog: &LibraryCatalog) -> Result<(), String> {
    let source = catalog
        .snapshot_by_key(complyph_checks::SOURCE_KEY)
        .map_err(|e| e.to_string())?;
    if source.metadata.provenance != complyph_library::Provenance::AgencyDownload
        || !complyph_checks::source_is_approved(&source.raw, &source.text)
    {
        return Err("These rules require their pinned, verified agency reference. Restore the matching library before running them.".into());
    }
    Ok(())
}

impl CheckView {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn set_review(&mut self, review: SavedReview) {
        self.visible = true;
        self.editing = false;
        self.context = review.review.context.clone();
        self.context.operative_terms_confirmed = false;
        self.result = Some(review);
        self.error = None;
        self.editing = false;
    }
    pub fn edit_scope(&mut self) {
        self.visible = true;
        self.editing = true;
    }
    pub fn show(
        &mut self,
        ui: &mut Ui,
        workspace: &Workspace,
        catalog: &LibraryCatalog,
        document: &Document,
        saved: Option<&SavedExtraction>,
    ) -> Option<ReviewAction> {
        if catalog
            .reference_has_downloaded_changes(complyph_checks::SOURCE_KEY)
            .unwrap_or(false)
        {
            ui.colored_label(AMBER,"An online publication change is saved for this rule source. These checks still cite the preserved version; review source and rule currency before relying on the findings. See Legal library → Source updates and downloaded history.");
        }
        if !self.visible {
            widgets::card().show(ui,|ui| {
                ui.label(RichText::new("Findings").font(heading(20.0)));
                widgets::muted(ui,"Read the document on the left, then confirm the checking scope. Findings and their supporting provisions will appear here.");
                if widgets::secondary(ui,"Set review scope").clicked(){self.edit_scope();}
            });
            return None;
        }
        if self.result.is_some() {
            if ui
                .button(if self.editing {
                    "Hide scope form"
                } else {
                    "Review or change scope"
                })
                .clicked()
            {
                self.editing = !self.editing;
            }
            widgets::muted(
                ui,
                "Select a finding or its passage to locate the evidence in the document.",
            );
        }
        if self.editing || self.result.is_none() {
            widgets::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Three focused labour checks").font(heading(21.0)));
            widgets::muted(ui,"Confirm applicability for the period you are reviewing. The app cannot establish coverage from the document or a job title.");
            scope(ui,"Normal daily working hours", &mut self.context.normal_hours,
                "Covered adult employment under Articles 82–85; standard eight-hour arrangement; stated hours are net working time, not a shift including unpaid meals. Excludes compressed/flexible schedules and special-sector or health-personnel arrangements.");
            scope(ui,"Ordinary-day overtime", &mut self.context.ordinary_overtime,
                "Covered employment under Articles 82 and 87. Ordinary working days beyond eight hours only; the figure is an additional premium on the regular hourly wage. Holiday, rest-day and special arrangements require separate review.");
            scope(ui,"Night shift differential", &mut self.context.night_differential,
                "Covered employment under Article 86 and Book III, Rule II; premium is additional to regular hourly wage for each hour 22:00–06:00. Check exclusions, including government, managerial/staff, field/unsupervised, domestic/personal service, qualifying paid-by-results work and retail/service establishments with not more than five workers.");
            widgets::muted(ui,"For all rules, establish Article 82 exclusions (including managerial, field, dependent family, domestic/personal service and qualifying paid-by-results workers), applicable amendments, contracts and CBAs. Leave scope unestablished when unsure.");
            ui.add_space(10.0);
            ui.checkbox(&mut self.context.operative_terms_confirmed,"I compared the extracted text with the original; these are operative policy terms, not examples or quotations.");
            widgets::muted(ui,"Supported statements use explicit numeric hours, an additional ordinary-day premium, or a night differential with its full time window. Other wording stays unresolved.");
            if widgets::primary(ui,"Run and save local review",self.context.operative_terms_confirmed && saved.is_some_and(|s|s.report.status==complyph_extract::Status::Ready)).clicked() {
                self.result=None;
                let result=(|| {
                    let saved=saved.ok_or("Wait for this document to finish reading.")?;
                    approved_source(catalog)?;
                    let current=complyph_core::document_snapshot(&document.path).map_err(|e|e.to_string())?;
                    if current.sha256!=saved.sha256 { return Err("The original changed. Read it again before checking.".into()); }
                    let review=complyph_checks::review(&saved.report,&saved.sha256,self.context.clone());
                    workspace.save_review(document.id,review).map_err(|e|e.to_string())
                })();
                match result { Ok(review)=>{self.result=Some(review);self.error=None;},Err(error)=>self.error=Some(error) }
            }
            if let Some(error)=&self.error {ui.colored_label(RED,error);}
        });
        }
        if let Some(result) = &self.result {
            ui.add_space(12.0);
            return show_review(ui, result, saved.map(|s| s.sha256.as_str()));
        }
        None
    }
}
fn scope(ui: &mut Ui, title: &str, value: &mut Applicability, explanation: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).strong());
    egui::ComboBox::from_id_salt(title)
        .selected_text(value.label())
        .width(ui.available_width().clamp(180.0, 330.0))
        .show_ui(ui, |ui| {
            for option in [
                Applicability::Unknown,
                Applicability::Covered,
                Applicability::OutsideScope,
            ] {
                ui.selectable_value(value, option, option.label());
            }
        });
    widgets::muted(ui, explanation);
}
pub fn show_review(
    ui: &mut Ui,
    saved: &SavedReview,
    current_hash: Option<&str>,
) -> Option<ReviewAction> {
    let mut action = None;
    let can_jump = current_hash == Some(saved.review.document_sha256.as_str());
    widgets::card().show(ui,|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(format!("Findings · review #{}",saved.id)).font(heading(20.0)));
        widgets::muted(ui,"Three supported numeric comparisons. No overall compliance verdict is produced.");
        if current_hash.is_some() && !can_jump {
            ui.colored_label(AMBER,"This document has changed since the saved review. Evidence quotes remain available; passage navigation is disabled. Run a new review for this version.");
        }else if !can_jump {widgets::muted(ui,"Open the connected review and wait for text reading to navigate document passages.");}
        for (index,finding) in saved.review.findings.iter().enumerate() {
            ui.add_space(12.0);ui.separator();ui.add_space(8.0);
            let can_select=can_jump && !finding.evidence.is_empty();
            let title=egui::Button::new(RichText::new(&finding.title).strong().size(16.0)).frame(false).wrap();
            if ui.add_enabled(can_select,title).on_hover_text("Jump to the first saved passage in the document").clicked(){
                action=Some(ReviewAction::Passage{hash:saved.review.document_sha256.clone(),evidence:finding.evidence[0].clone()});
            }
            let color=match finding.outcome {Outcome::PotentialIssue=>RED,Outcome::StatedTermMeetsMinimum=>GREEN,_=>AMBER};
            ui.colored_label(color,finding.outcome.label());
            ui.label(&finding.explanation);
            let scope=[saved.review.context.normal_hours,saved.review.context.ordinary_overtime,saved.review.context.night_differential].get(index).copied().unwrap_or_default();
            widgets::muted(ui,&format!("Confirmed scope: {}",scope.label()));
            for evidence in &finding.evidence {
                if ui.add_enabled(can_jump,egui::Button::new(format!("Show passage · {}",evidence.location.label())).small().wrap()).clicked(){
                    action=Some(ReviewAction::Passage{hash:saved.review.document_sha256.clone(),evidence:evidence.clone()});
                }
                ui.add(egui::Label::new(&evidence.text).wrap().selectable(true));
            }
            ui.label(RichText::new(&finding.article).small().strong());
            if ui.button("Open supporting provision offline").clicked(){
                action=Some(ReviewAction::Provision{review:Box::new(saved.clone()),finding:index});
            }
        }
        ui.add_space(12.0);
        egui::CollapsingHeader::new("Scope and limitations").id_salt(("limits",saved.id)).show(ui,|ui|{ui.label(&saved.review.limitation);});
        egui::CollapsingHeader::new("Snapshot and rule identifiers").id_salt(("hashes",saved.id)).show(ui,|ui| {
            for text in [format!("Rules: {}",saved.review.engine),format!("Document SHA-256: {}",saved.review.document_sha256),format!("Source SHA-256: {}",saved.review.source_sha256),format!("Reading-text SHA-256: {}",saved.review.source_text_sha256)] {ui.add(egui::Label::new(text).wrap().selectable(true));}
        });
        if ui.button("Copy report JSON").clicked() && let Ok(json)=serde_json::to_string_pretty(saved) {ui.ctx().copy_text(json);}
    });
    action
}
