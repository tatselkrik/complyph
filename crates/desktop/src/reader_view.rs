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
    Finished(Box<SavedExtraction>),
    Failed(String),
}

pub struct ReaderView {
    ocr_settings: complyph_extract::OcrSettings,
    format: Option<complyph_extract::Format>,
    request_ocr: bool,
    ocr_running: bool,
    retained: bool,
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
            ocr_settings: Default::default(),
            format: None,
            request_ocr: false,
            ocr_running: false,
            retained: false,
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
    pub fn is_retained(&self) -> bool {
        self.retained
    }
    pub fn open_retained(&mut self, saved: SavedExtraction) {
        self.clear();
        self.format = Some(saved.report.format);
        self.ocr_settings = saved
            .report
            .ocr
            .as_ref()
            .and_then(|o| o.processing.as_ref())
            .map(|p| p.settings.clone())
            .unwrap_or_default();
        self.retained = true;
        self.unit = 0;
        self.chunk = 0;
        self.search.clear();
        self.state = State::Finished(Box::new(saved));
    }
    pub fn take_ocr_request(&mut self) -> bool {
        std::mem::take(&mut self.request_ocr)
    }
    pub fn start_ocr(&mut self, paths: WorkspacePaths, document: Document, ctx: &egui::Context) {
        self.start_mode(paths, document, true, Some(self.ocr_settings.clone()), ctx);
    }
    pub fn current(&self) -> Option<&SavedExtraction> {
        match &self.state {
            State::Finished(saved) => Some(saved),
            _ => None,
        }
    }

    pub fn clear(&mut self) {
        self.ocr_settings = Default::default();
        self.format = None;
        self.retained = false;
        self.request_ocr = false;
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
        self.start_mode(paths, document, force, None, ctx);
    }
    fn start_mode(
        &mut self,
        paths: WorkspacePaths,
        document: Document,
        force: bool,
        options: Option<complyph_extract::OcrSettings>,
        ctx: &egui::Context,
    ) {
        self.clear();
        self.format = Some(document.kind.format());
        if let Some(options) = &options {
            self.ocr_settings = options.clone();
        }
        self.request_ocr = false;
        self.ocr_running = options.is_some();
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
            let result = if let Some(options) = options {
                reader::read_document_ocr_with_settings(&paths, &document, options, &cancel)
            } else {
                reader::read_document(&paths, &document, force, &cancel)
            };
            let _ = sender.send((generation, result));
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self) {
        while let Ok((generation, result)) = self.receiver.try_recv() {
            if generation == self.generation {
                self.cancel = None;
                self.state = match result {
                    Ok(saved) => {
                        self.ocr_settings = saved
                            .report
                            .ocr
                            .as_ref()
                            .and_then(|o| o.processing.as_ref())
                            .map(|p| p.settings.clone())
                            .unwrap_or_default();
                        State::Finished(Box::new(saved))
                    }
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
                    ui.horizontal(|ui| { ui.spinner(); ui.label(if self.ocr_running { "Recognizing pages locally…" } else { "Reading locally…" }); });
                    if self.ocr_running { widgets::muted(ui,"Up to 20 PDF pages per run; processing stops at 120 seconds. No OCR service or document upload is used."); }
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
                    if !self.retained && self.format==Some(complyph_extract::Format::Pdf) {
                        ocr_controls(ui,&mut self.ocr_settings);
                        if ui.button("Try local OCR").clicked(){self.request_ocr=true;}
                    }
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
                        if self.retained {
                            widgets::pill(ui,"Retained reading",AMBER,AMBER_TINT,false);
                        } else {
                            reread = widgets::secondary(ui, "Read again").clicked();
                        }
                    });
                    ui.label(&report.summary);
                    let blocking=report.warnings.iter().filter(|w|!routine_warning(w)).collect::<Vec<_>>();
                    if !blocking.is_empty(){
                        widgets::pill(ui,"Manual review needed",AMBER,AMBER_TINT,false);
                        ui.colored_label(AMBER,format!("{} reading warning(s) prevent numeric conclusions.",blocking.len()));
                        egui::CollapsingHeader::new("Inspect reading warnings").show(ui,|ui|{for warning in blocking {ui.colored_label(AMBER,warning);}});
                    }
                    if !self.retained && report.format == complyph_extract::Format::Pdf && matches!(report.status, Status::Ready | Status::NeedsOcr | Status::Unreadable) {
                        ocr_controls(ui,&mut self.ocr_settings);
                        if ui.button(if report.ocr.is_some() { "Run local OCR again" } else { "Run local OCR" }).clicked() { self.request_ocr = true; }
                        widgets::muted(ui,"Recognize every PDF page offline. OCR can misread confident-looking numbers; compare the result with the original.");
                    }
                    widgets::muted(ui, if self.retained {
                        "Immutable saved reading for this file version. The current original is not opened or rechecked here. Use Review current file for a new assessment."
                    } else {
                        "Saved readings stay in the private workspace. File contents are checked against the cache whenever opened here; use Read again after editing."
                    });
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
                            if let Some(info)=&report.reading {for detail in info.details.iter().filter(|d|d.location==unit.location){widgets::muted(ui,&detail.message);}}
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
                            let uncertain=report.ocr.as_ref().and_then(|o|o.pages.get(self.unit));
                            if let Some(quality) = uncertain {
                                ui.colored_label(AMBER,format!("OCR page {} · {} words · {} low-confidence words",quality.page,quality.words,quality.uncertain.len()));
                                widgets::muted(ui,"OCR highlights mark low recognition confidence; a selected finding highlights its evidence instead. Check all numbers even when unflagged.");
                                if let Some(processing)=&quality.processing {widgets::muted(ui,&processing.label());}
                            }
                            let ranges=self.passage.as_ref().filter(|p|p.unit==self.unit).map(|p|text_navigation::clip_ranges(&p.ranges,start..end)).unwrap_or_else(|| {
                                let flags:Vec<_>=uncertain.map(|p|p.uncertain.iter().map(|w|w.start..w.end).collect()).unwrap_or_default();
                                text_navigation::clip_ranges(&flags,start..end)
                            });
                            let scroll=self.passage.as_ref().filter(|p|p.unit==self.unit && self.scroll_to_passage).map(|p|p.ranges[p.active].start.saturating_sub(start));
                            self.scroll_to_passage=false;
                            let text = if unit.text.is_empty() { "No selectable text on this page. It may be blank or require OCR." }
                                else { &unit.text[boundaries[self.chunk]..boundaries[self.chunk + 1]] };
                            let text_frame=egui::Frame::new().fill(IVORY).stroke(egui::Stroke::new(1.0, BORDER))
                                .corner_radius(8).inner_margin(14).show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ScrollArea::vertical().id_salt(("private-reader", self.generation, self.unit, self.chunk))
                                    .max_height((pane_height-310.0).max(180.0)).min_scrolled_height(160.0).show(ui, |ui| {
                                    ui.set_min_height(150.0);
                                    text_navigation::highlighted_label(ui,text,&ranges,scroll);
                                });
                            });
                            if scroll.is_some() {
                                // The inner text viewport may itself be below
                                // the outer pane's clip after controls wrap.
                                ui.scroll_to_rect(text_frame.response.rect,Some(egui::Align::Center));
                            }
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

fn routine_warning(warning: &str) -> bool {
    [
        complyph_extract::PDF_LAYOUT_WARNING,
        complyph_extract::DOCX_LAYOUT_WARNING,
        complyph_extract::LEGACY_DOCX_LAYOUT_WARNING,
        complyph_extract::OCR_WARNING,
    ]
    .contains(&warning)
}
fn ocr_controls(ui: &mut Ui, settings: &mut complyph_extract::OcrSettings) {
    egui::CollapsingHeader::new("OCR options").show(ui,|ui|{
        ui.label("Rotate page images clockwise");
        egui::ComboBox::from_id_salt("ocr-rotation").selected_text(settings.rotation.label()).show_ui(ui,|ui|{for rotation in complyph_extract::OcrRotation::ALL {ui.selectable_value(&mut settings.rotation,rotation,rotation.label());}});
        ui.label("Reading layout");
        egui::ComboBox::from_id_salt("ocr-layout").selected_text(settings.layout.label()).show_ui(ui,|ui|{for layout in complyph_extract::OcrLayout::ALL {ui.selectable_value(&mut settings.layout,layout,layout.label());}});
        ui.checkbox(&mut settings.enhance_scan,"Enhance faint scans and straighten slight skew");
        widgets::muted(ui,"Options apply to every page. The PDF's own page rotation is respected first. Enhancement adjusts pixels; it does not correct words or numbers. Scattered-text mode does not establish reading order and requires manual review.");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use complyph_extract::{Format, Location, Report, TextUnit};

    #[test]
    fn passage_jump_scrolls_the_outer_pane_to_the_text_viewport() {
        let location = Location::PdfPage { page: 1 };
        let evidence = complyph_checks::Evidence {
            location: location.clone(),
            text: "Regular meal period is 60 minutes of time-off".into(),
            hundredths: Some(6000),
        };
        let saved = SavedExtraction {
            sha256: "synthetic".into(),
            read_at: 0,
            report: Report {
                reading: None,
                engine: complyph_extract::ENGINE.into(),
                ocr: None,
                format: Format::Pdf,
                status: Status::Ready,
                summary: "PDF text read locally. Page numbers refer to the original PDF.".into(),
                units: vec![TextUnit {
                    location,
                    text: format!(
                        "{}{}.",
                        "Fictional introductory paragraph.\n".repeat(30),
                        evidence.text
                    ),
                }],
                warnings: vec![complyph_extract::PDF_LAYOUT_WARNING.into()],
            },
        };
        let mut view = ReaderView::default();
        view.state = State::Finished(Box::new(saved));
        let ctx = egui::Context::default();
        let mut offset = 0.0;
        let mut frame = |view: &mut ReaderView, time: f64| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(440.0, 520.0),
                    )),
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    let shown = ScrollArea::vertical()
                        .id_salt("outer-reader-test")
                        .max_height(500.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            view.show(ui, 500.0);
                        });
                    offset = shown.state.clamped_offset().y;
                },
            );
            output.textures_delta.clear();
            offset
        };
        let before = frame(&mut view, 0.0);
        view.jump_to("synthetic", &evidence);
        let mut after = 0.0;
        for i in 1..=12 {
            after = frame(&mut view, i as f64 * 0.1);
        }
        assert!(view.passage.is_some());
        assert!(
            after > before + 20.0,
            "The passage viewport remained below the visible pane: before={before}, after={after}"
        );
    }
}
