use crate::{theme::*, widgets};
use complyph_core::{ReviewActionRequest, ReviewStatus, ReviewWorkflow, Workspace};
use eframe::egui::{self, RichText, Ui};
use egui_file_dialog::FileDialog;
use std::{collections::HashMap, path::PathBuf, sync::mpsc};

struct Draft {
    revision: i64,
    fingerprint: String,
    status: ReviewStatus,
    note: String,
    message: Option<String>,
    conflict: bool,
}

#[derive(Default)]
pub struct ReviewTools {
    reviewer: String,
    drafts: HashMap<(String, Option<String>), Draft>,
    picker: Option<FileDialog>,
    export_id: Option<i64>,
    export: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    message: Option<(bool, String)>,
}

impl ReviewTools {
    pub fn tick(&mut self, ctx: &egui::Context, workspace: Option<&Workspace>) {
        if let Some(picker) = &mut self.picker {
            picker.update(ctx);
            if let Some(path) = picker.take_picked()
                && let Some(id) = self.export_id.take()
            {
                let snapshot = workspace
                    .ok_or_else(|| "The workspace is unavailable.".to_string())
                    .and_then(|w| w.review_workflow(id).map_err(|e| e.to_string()));
                match snapshot {
                    Ok(snapshot) => {
                        let (sender, receiver) = mpsc::channel();
                        self.export = Some(receiver);
                        std::thread::spawn(move || {
                            let result = complyph_reports::export_pdf(&snapshot, &path)
                                .map(|()| path)
                                .map_err(|e| e.to_string());
                            let _ = sender.send(result);
                        });
                    }
                    Err(error) => self.message = Some((true, error)),
                }
            }
        }
        if let Some(receiver) = &self.export {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "The PDF export worker stopped. Try exporting again.".into(),
                )),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.message = Some(match result {
                    Ok(path) => (
                        false,
                        format!(
                            "PDF saved: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                    ),
                    Err(error) => (true, error),
                });
                self.export = None;
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
    }

    pub fn header(&mut self, ui: &mut Ui, workspace: &Workspace, state: &ReviewWorkflow) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.export.is_none(), egui::Button::new("Export PDF…"))
                .clicked()
            {
                let mut picker = FileDialog::new()
                    .title("Save review PDF — choose a new filename")
                    .default_size([760.0, 500.0])
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show_devices(false)
                    .show_removable_devices(false)
                    .show_hidden_option(false)
                    .default_file_name(&format!(
                        "ComplyPH-review-{}-{}.pdf",
                        state.saved.id,
                        chrono::Local::now().format("%Y%m%d-%H%M%S")
                    ))
                    .as_modal(true);
                picker.save_file();
                self.picker = Some(picker);
                self.export_id = Some(state.saved.id);
                self.message = None;
            }
            if ui.button("Copy full report JSON").clicked() {
                match workspace
                    .review_workflow(state.saved.id)
                    .map_err(|e| e.to_string())
                    .and_then(|s| serde_json::to_string_pretty(&s).map_err(|e| e.to_string()))
                {
                    Ok(json) => {
                        ui.ctx().copy_text(json);
                        self.message =
                            Some((false, "Saved review and tracking history copied.".into()));
                    }
                    Err(error) => self.message = Some((true, error)),
                }
            }
            if self.export.is_some() {
                ui.spinner();
                ui.label("Preparing PDF locally…");
            }
        });
        if let Some((error, message)) = &self.message {
            ui.colored_label(if *error { RED } else { GREEN }, message);
        }
        widgets::muted(
            ui,
            "Exports contain saved excerpts, citations and saved notes. Save pending edits first. Follow-up statuses never change the automated result.",
        );
    }

    pub fn document(&mut self, ui: &mut Ui, workspace: &Workspace, state: &ReviewWorkflow) {
        egui::CollapsingHeader::new("Document review notes")
            .id_salt(("document-notes", &state.review_fingerprint))
            .show(ui, |ui| {
                widgets::muted(ui, "Notes belong to this saved review. A new automated review has its own history.");
                self.editor(ui, workspace, state, None);
            });
    }

    pub fn finding(
        &mut self,
        ui: &mut Ui,
        workspace: &Workspace,
        state: &ReviewWorkflow,
        rule: &str,
    ) {
        ui.label(
            RichText::new(format!("Human follow-up: {}", state.status(rule).label())).strong(),
        );
        egui::CollapsingHeader::new("Reviewer notes and status")
            .id_salt(("finding-notes", &state.review_fingerprint, rule))
            .show(ui, |ui| self.editor(ui, workspace, state, Some(rule)));
    }

    fn editor(
        &mut self,
        ui: &mut Ui,
        workspace: &Workspace,
        state: &ReviewWorkflow,
        rule: Option<&str>,
    ) {
        ui.push_id((&state.review_fingerprint, rule), |ui| {
            let draft = self.drafts.entry((state.review_fingerprint.clone(), rule.map(str::to_owned)))
                .or_insert_with(|| Draft {
                    revision: state.revision(rule), fingerprint: state.review_fingerprint.clone(),
                    status: rule.map_or(ReviewStatus::Unresolved, |r| state.status(r)), note: String::new(), message: None, conflict: false,
                });
            ui.label("Reviewer name (self-reported)");
            ui.add(egui::TextEdit::singleline(&mut self.reviewer).char_limit(120).desired_width(f32::INFINITY));
            widgets::muted(ui, "Stored with each action; this is not a verified identity or digital signature.");
            if rule.is_some() {
                egui::ComboBox::from_id_salt("follow-up-status").selected_text(draft.status.label()).show_ui(ui, |ui| {
                    for status in ReviewStatus::ALL { ui.selectable_value(&mut draft.status, status, status.label()); }
                });
            }
            ui.label(if rule.is_some() { "Note or reason for status change (required)" } else { "Review note (required)" });
            ui.add(egui::TextEdit::multiline(&mut draft.note).desired_rows(3).desired_width(f32::INFINITY).char_limit(2000));
            if widgets::primary(ui, "Save reviewer update", !self.reviewer.trim().is_empty() && !draft.note.trim().is_empty() && !draft.conflict).clicked() {
                let request = ReviewActionRequest {
                    review_fingerprint: draft.fingerprint.clone(), rule_id: rule.map(str::to_owned), expected_revision: draft.revision,
                    reviewer: self.reviewer.clone(), note: draft.note.clone(), status: rule.map(|_| draft.status),
                };
                match workspace.append_review_action(state.saved.id, &request) {
                    Ok(event) => {
                        draft.revision = event.id; draft.note.clear();
                        draft.message = Some("Saved to local history. Add another note to correct or explain an earlier entry.".into());
                        ui.ctx().request_repaint();
                    }
                    Err(error) => {
                        draft.conflict = matches!(error, complyph_core::WorkspaceError::ReviewConflict);
                        draft.message = Some(error.to_string());
                    }
                }
            }
            if let Some(message) = &draft.message { ui.label(message); }
            if (draft.conflict || draft.revision != state.revision(rule)) && ui.button("Load latest status; keep my note").clicked() {
                draft.revision = state.revision(rule);
                draft.fingerprint = state.review_fingerprint.clone();
                draft.status = rule.map_or(ReviewStatus::Unresolved, |r| state.status(r));
                draft.conflict = false; draft.message = Some("Latest history loaded. Check the status and your note before saving.".into());
            }
            let events: Vec<_> = state.events.iter().filter(|e| e.rule_id.as_deref() == rule).collect();
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Saved history · {} {}", events.len(), if events.len() == 1 { "entry" } else { "entries" })).strong());
            if events.is_empty() { widgets::muted(ui, "No reviewer updates yet."); }
            // Keep long histories usable. Export always includes every saved event.
            let show_history = |ui: &mut Ui, events: &[&complyph_core::ReviewEvent]| {
                for event in events.iter().rev() {
                    ui.separator();
                    let time = chrono::DateTime::from_timestamp(event.created_at, 0)
                        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S %:z").to_string()).unwrap_or_default();
                    widgets::muted(ui, &format!("#{} · {} · {}", event.id, time, event.reviewer));
                    if let (Some(before), Some(after)) = (event.previous_status, event.new_status) { ui.label(format!("{} → {}", before.label(), after.label())); }
                    ui.add(egui::Label::new(&event.note).wrap().selectable(true));
                }
            };
            let split = events.len().saturating_sub(5);
            show_history(ui, &events[split..]);
            if split > 0 {
                egui::CollapsingHeader::new(format!("Earlier {} entries", split)).id_salt("earlier-history").show(ui, |ui| show_history(ui, &events[..split]));
            }
        });
    }
}
