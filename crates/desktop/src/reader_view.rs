use crate::{reader, text_navigation, theme::*, widgets};
use complyph_core::{Document, SavedExtraction, WorkspacePaths};
use complyph_extract::Status;
use eframe::egui::{self, RichText, ScrollArea, Ui};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};

type Reply = (u64, Result<SavedExtraction, String>);

enum State {
    Idle,
    Reading,
    Finished(SavedExtraction),
    Failed(String),
}

pub struct ReaderView {
    state: State,
    generation: u64,
    cancel: Option<reader::Cancel>,
    sender: Sender<Reply>,
    receiver: Receiver<Reply>,
    unit: usize,
    chunk: usize,
    search: String,
    passage: Option<text_navigation::Passage>,
    scroll_to_passage: bool,
    jump_error: Option<String>,
}

impl Default for ReaderView {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            state: State::Idle,
            generation: 0,
            cancel: None,
            sender,
            receiver,
            unit: 0,
            chunk: 0,
            search: String::new(),
            passage: None,
            scroll_to_passage: false,
            jump_error: None,
        }
    }
}

impl ReaderView {
    pub fn current(&self) -> Option<&SavedExtraction> {
        match &self.state {
            State::Finished(saved) => Some(saved),
            _ => None,
        }
    }

    pub fn clear(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.state = State::Idle;
        self.passage = None;
        self.scroll_to_passage = false;
        self.jump_error = None;
    }

    pub fn start(
        &mut self,
        paths: WorkspacePaths,
        document: Document,
        force: bool,
        ctx: &egui::Context,
    ) {
        self.clear();
        self.unit = 0;
        self.chunk = 0;
        self.search.clear();
        self.state = State::Reading;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        let generation = self.generation;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = reader::read_document(&paths, &document, force, &cancel);
            let _ = sender.send((generation, result));
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self) {
        while let Ok((generation, result)) = self.receiver.try_recv() {
            if generation == self.generation {
                self.cancel = None;
                self.state = match result {
                    Ok(saved) => State::Finished(saved),
                    Err(error) => State::Failed(error),
                };
            }
        }
    }

    pub fn jump_to(&mut self, hash: &str, evidence: &complyph_checks::Evidence) {
        let result = self
            .current()
            .ok_or_else(|| "Wait for this document to finish reading.".to_owned())
            .and_then(|saved| text_navigation::locate(saved, hash, evidence));
        match result {
            Ok(passage) => {
                self.unit = passage.unit;
                self.search.clear();
                if let Some(saved) = self.current() {
                    self.chunk = text_navigation::chunk_at(
                        &text_navigation::chunk_boundaries(&saved.report.units[self.unit].text),
                        passage.ranges[0].start,
                    );
                }
                self.passage = Some(passage);
                self.scroll_to_passage = true;
                self.jump_error = None;
            }
            Err(error) => {
                self.passage = None;
                self.jump_error = Some(error);
            }
        }
    }
    /// Returns true when the user requests a fresh read.
    pub fn show(&mut self, ui: &mut Ui, pane_height: f32) -> bool {
        let mut reread = false;
        widgets::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Document text").font(heading(19.0)));
                widgets::pill(ui, "On this computer", GREEN, GREEN_TINT, false);
            });
            ui.add_space(6.0);
            if let Some(error)=&self.jump_error {ui.colored_label(AMBER,error);}
            match &self.state {
                State::Idle => { widgets::muted(ui, "Choose a document to read its text locally."); }
                State::Reading => {
                    ui.horizontal(|ui| { ui.spinner(); ui.label("Reading locally…"); });
                    widgets::muted(ui, "The original is unchanged. No document content is sent online.");
                    if widgets::secondary(ui, "Cancel reading").clicked() {
                        self.clear();
                        self.state = State::Failed("Reading cancelled. The original is unchanged.".into());
                    }
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                }
                State::Failed(error) => {
                    ui.colored_label(RED, error);
                    reread = widgets::secondary(ui, "Read again").clicked();
                }
                State::Finished(saved) => {
                    let report = &saved.report;
                    if let Some(error)=&self.jump_error {ui.colored_label(RED,error);}
                    if let Some(passage)=&mut self.passage {
                        ui.horizontal_wrapped(|ui| {
                            widgets::pill(ui,"Finding passage",AMBER,AMBER_TINT,false);
                            if passage.ranges.len()>1 {
                                ui.label(format!("{} identical matches · {} selected",passage.ranges.len(),passage.active+1));
                                if ui.button("Next match").clicked(){passage.active=(passage.active+1)%passage.ranges.len();self.scroll_to_passage=true;}
                            }
                            if ui.button("Show selected passage").clicked(){self.scroll_to_passage=true;}
                        });
                        if self.scroll_to_passage {
                            self.unit=passage.unit;self.search.clear();
                            self.chunk=text_navigation::chunk_at(&text_navigation::chunk_boundaries(&report.units[self.unit].text),passage.ranges[passage.active].start);
                        }
                    }
                    ui.horizontal_wrapped(|ui| {
                        let ready = report.status == Status::Ready;
                        widgets::pill(ui, report.status.label(), if ready { GREEN } else { AMBER }, if ready { GREEN_TINT } else { AMBER_TINT }, false);
                        ui.label(RichText::new(format!("{} · {} text bytes", super::app::format_date(saved.read_at), report.text_bytes())).size(11.5).color(MUTED));
                        reread = widgets::secondary(ui, "Read again").clicked();
                    });
                    ui.label(&report.summary);
                    widgets::muted(ui, "Saved text stays in the private workspace. File contents are checked against the cache whenever opened here; use Read again after editing.");
                    if !report.units.is_empty() && report.text_bytes() > 0 {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label("Find text");
                            if ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search this document…").desired_width(230.0)).changed() {
                                self.chunk = 0;
                            }
                        });
                        let query = self.search.to_lowercase();
                        let matches: Vec<usize> = report.units.iter().enumerate()
                            .filter(|(_, unit)| query.is_empty() || unit.text.to_lowercase().contains(&query))
                            .map(|(index, _)| index).collect();
                        if matches.is_empty() {
                            ui.label("No matching locations.");
                        } else {
                            if !matches.contains(&self.unit) { self.unit = matches[0]; self.chunk = 0; }
                            let current = matches.iter().position(|&index| index == self.unit).unwrap_or(0);
                            let unit = &report.units[self.unit];
                            ui.horizontal_wrapped(|ui| {
                                if ui.add_enabled(current > 0, egui::Button::new("Previous")).clicked() { self.unit = matches[current - 1]; self.chunk = 0; }
                                if ui.add_enabled(current + 1 < matches.len(), egui::Button::new("Next")).clicked() { self.unit = matches[current + 1]; self.chunk = 0; }
                                ui.label(format!("{} of {} {}", current + 1, matches.len(), if query.is_empty() { "locations" } else { "matching locations" }));
                            });
                            let unit = if self.unit < report.units.len() { &report.units[self.unit] } else { unit };
                            ui.label(RichText::new(unit.location.label()).font(heading(13.0)).color(NAVY));
                            // UTF-8-safe display chunks keep one unusually long paragraph/page responsive.
                            let boundaries = text_navigation::chunk_boundaries(&unit.text);
                            let count = boundaries.len().saturating_sub(1).max(1);
                            self.chunk = self.chunk.min(count - 1);
                            if count > 1 {
                                ui.horizontal(|ui| {
                                    if ui.add_enabled(self.chunk > 0, egui::Button::new("Earlier text")).clicked() { self.chunk -= 1; }
                                    if ui.add_enabled(self.chunk + 1 < count, egui::Button::new("More text")).clicked() { self.chunk += 1; }
                                    ui.label(format!("Display section {} / {count} · same original location", self.chunk + 1));
                                });
                            }
                            let start=boundaries[self.chunk];let end=boundaries[self.chunk+1];
                            let ranges=self.passage.as_ref().filter(|p|p.unit==self.unit).map(|p|text_navigation::clip_ranges(&p.ranges,start..end)).unwrap_or_default();
                            let scroll=self.passage.as_ref().filter(|p|p.unit==self.unit && self.scroll_to_passage).map(|p|p.ranges[p.active].start.saturating_sub(start));
                            self.scroll_to_passage=false;
                            let text = if unit.text.is_empty() { "No selectable text on this page. It may be blank or require OCR." }
                                else { &unit.text[boundaries[self.chunk]..boundaries[self.chunk + 1]] };
                            egui::Frame::new().fill(IVORY).stroke(egui::Stroke::new(1.0, BORDER))
                                .corner_radius(8).inner_margin(14).show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ScrollArea::vertical().id_salt(("private-reader", self.generation, self.unit, self.chunk))
                                    .max_height((pane_height-310.0).max(180.0)).min_scrolled_height(160.0).show(ui, |ui| {
                                    ui.set_min_height(150.0);
                                    text_navigation::highlighted_label(ui,text,&ranges,scroll);
                                });
                            });
                        }
                    }
                    ui.add_space(8.0);
                    for warning in &report.warnings {
                        ui.label(RichText::new(warning).size(11.5).color(MUTED));
                    }
                }
            }
        });
        reread
    }
}

impl Drop for ReaderView {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
