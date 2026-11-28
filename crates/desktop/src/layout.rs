//! Logical-point layout; the native backend applies the monitor's DPI scale.
use eframe::egui::{self, Rect, Ui, pos2};

pub const MIN_WINDOW: [f32; 2] = [640.0, 480.0];

#[derive(Clone, Copy, Debug)]
pub struct DesktopLayout {
    pub sidebar: Rect,
    pub header: Rect,
    pub content: Rect,
    pub compact_navigation: bool,
    pub split_review: bool,
}

impl DesktopLayout {
    pub fn new(rect: Rect) -> Self {
        let compact_navigation = rect.width() < 1000.0 || rect.height() < 650.0;
        let sidebar_width = if compact_navigation { 72.0 } else { 232.0 };
        let margin = if compact_navigation { 16.0 } else { 30.0 };
        let sidebar =
            Rect::from_min_max(rect.min, pos2(rect.left() + sidebar_width, rect.bottom()));
        let header = Rect::from_min_max(
            pos2(sidebar.right(), rect.top()),
            pos2(
                rect.right(),
                rect.top() + if compact_navigation { 56.0 } else { 66.0 },
            ),
        );
        let content = Rect::from_min_max(
            pos2(sidebar.right() + margin, header.bottom() + 18.0),
            pos2(rect.right() - margin, rect.bottom() - 16.0),
        );
        Self {
            sidebar,
            header,
            content,
            compact_navigation,
            split_review: content.width() >= 900.0 && content.height() >= 540.0,
        }
    }
}

/// Constrain the whole window, including controls above/below an inner reader.
pub fn window<'a>(ctx: &egui::Context, window: egui::Window<'a>) -> egui::Window<'a> {
    let bounds = ctx.content_rect().shrink(12.0);
    window
        .constrain_to(bounds)
        .min_width(280.0_f32.min(bounds.width()))
        .max_width(bounds.width())
        .max_height(bounds.height())
        .vscroll(true)
}

/// Give labels and their edits separate rows when a metadata grid would be narrow.
pub fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) -> egui::Response {
    let label = ui.label(label);
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    )
    .labelled_by(label.id)
}

pub fn multiline(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    rows: usize,
    limit: usize,
) -> egui::Response {
    let label = ui.label(label);
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_width(f32::INFINITY)
            .desired_rows(rows)
            .char_limit(limit),
    )
    .labelled_by(label.id)
}

pub fn review_split(ui: &Ui) -> bool {
    DesktopLayout::new(ui.ctx().content_rect()).split_review
}

pub fn native_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title("ComplyPH")
        .with_app_id("ph.complyph.desktop")
        .with_inner_size([1280.0, 860.0])
        .with_min_inner_size(MIN_WINDOW)
}

pub fn remaining_height(ui: &Ui) -> f32 {
    (ui.ctx().content_rect().bottom() - ui.cursor().top() - 16.0)
        .clamp(180.0, ui.ctx().content_rect().height().max(180.0))
}
