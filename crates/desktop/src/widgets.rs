use crate::theme::*;
use eframe::egui::{
    self, Align2, Color32, FontId, Frame, Margin, Painter, Rect, Response, RichText, Sense, Shape,
    Stroke, StrokeKind, Ui, pos2, vec2,
};

#[derive(Clone, Copy)]
pub enum Icon {
    Review,
    Document,
    Library,
    Reports,
    Settings,
    Lock,
    Arrow,
    Search,
}

pub fn paint_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let at = |x: f32, y: f32| {
        pos2(
            rect.left() + x * rect.width() / 24.0,
            rect.top() + y * rect.height() / 24.0,
        )
    };
    let stroke = Stroke::new((rect.width() / 15.0).max(1.3), color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    match icon {
        Icon::Review => {
            for (a, b) in [
                ((3.0, 8.0), (3.0, 4.0)),
                ((3.0, 4.0), (8.0, 4.0)),
                ((16.0, 4.0), (21.0, 4.0)),
                ((21.0, 4.0), (21.0, 8.0)),
                ((3.0, 16.0), (3.0, 20.0)),
                ((3.0, 20.0), (8.0, 20.0)),
                ((16.0, 20.0), (21.0, 20.0)),
                ((21.0, 16.0), (21.0, 20.0)),
                ((8.0, 10.0), (16.0, 10.0)),
                ((8.0, 14.0), (14.0, 14.0)),
            ] {
                line(a, b);
            }
        }
        Icon::Document | Icon::Reports => {
            painter.add(Shape::closed_line(
                vec![
                    at(6.0, 3.0),
                    at(14.0, 3.0),
                    at(19.0, 8.0),
                    at(19.0, 21.0),
                    at(6.0, 21.0),
                ],
                stroke,
            ));
            line((14.0, 3.0), (14.0, 8.0));
            line((14.0, 8.0), (19.0, 8.0));
            line((9.0, 12.0), (16.0, 12.0));
            line((9.0, 16.0), (15.0, 16.0));
            if matches!(icon, Icon::Reports) {
                line((9.0, 19.0), (13.0, 19.0));
            }
        }
        Icon::Library => {
            painter.add(Shape::closed_line(
                vec![
                    at(3.0, 5.0),
                    at(8.0, 4.0),
                    at(12.0, 6.0),
                    at(16.0, 4.0),
                    at(21.0, 5.0),
                    at(21.0, 20.0),
                    at(16.0, 19.0),
                    at(12.0, 21.0),
                    at(8.0, 19.0),
                    at(3.0, 20.0),
                ],
                stroke,
            ));
            line((12.0, 6.0), (12.0, 21.0));
        }
        Icon::Settings => {
            painter.circle_stroke(at(12.0, 12.0), rect.width() * 0.26, stroke);
            painter.circle_stroke(at(12.0, 12.0), rect.width() * 0.10, stroke);
            for a in [
                0.0_f32,
                std::f32::consts::FRAC_PI_4,
                std::f32::consts::FRAC_PI_2,
                3.0 * std::f32::consts::FRAC_PI_4,
                std::f32::consts::PI,
                5.0 * std::f32::consts::FRAC_PI_4,
                3.0 * std::f32::consts::FRAC_PI_2,
                7.0 * std::f32::consts::FRAC_PI_4,
            ] {
                line(
                    (12.0 + 6.5 * a.cos(), 12.0 + 6.5 * a.sin()),
                    (12.0 + 9.0 * a.cos(), 12.0 + 9.0 * a.sin()),
                );
            }
        }
        Icon::Lock => {
            painter.rect_stroke(
                Rect::from_min_max(at(5.0, 10.0), at(19.0, 21.0)),
                3,
                stroke,
                StrokeKind::Inside,
            );
            painter.add(Shape::line(
                vec![
                    at(8.0, 10.0),
                    at(8.0, 6.0),
                    at(10.0, 3.0),
                    at(14.0, 3.0),
                    at(16.0, 6.0),
                    at(16.0, 10.0),
                ],
                stroke,
            ));
            line((12.0, 14.0), (12.0, 17.0));
        }
        Icon::Arrow => {
            line((5.0, 12.0), (19.0, 12.0));
            line((14.0, 7.0), (19.0, 12.0));
            line((14.0, 17.0), (19.0, 12.0));
        }
        Icon::Search => {
            painter.circle_stroke(at(10.0, 10.0), rect.width() * 0.25, stroke);
            line((14.5, 14.5), (21.0, 21.0));
        }
    }
}

pub fn icon(ui: &mut Ui, kind: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_icon(ui.painter(), rect, kind, color);
}

pub fn logo(painter: &Painter, rect: Rect, color: Color32) {
    let center = rect.center() - vec2(rect.width() * 0.07, rect.height() * 0.07);
    let radius = rect.width() * 0.30;
    painter.circle_stroke(center, radius, Stroke::new(2.4, color));
    painter.line_segment(
        [
            center + vec2(radius * 0.72, radius * 0.72),
            rect.right_bottom() - vec2(3.0, 3.0),
        ],
        Stroke::new(3.0, color),
    );
    for offset in [-3.5, 2.5] {
        painter.line_segment(
            [center + vec2(-5.0, offset), center + vec2(5.0, offset)],
            Stroke::new(1.7, color),
        );
    }
}

pub fn card() -> Frame {
    Frame::new()
        .fill(IVORY)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(Margin::same(22))
}

pub fn eyebrow(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(heading(11.0)).color(MUTED));
}

pub fn title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(heading(32.0)).color(NAVY));
}

pub fn muted(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(MUTED));
}

pub fn primary(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).font(heading(15.0)).color(CREAM))
            .fill(if enabled { NAVY } else { MUTED })
            .stroke(Stroke::NONE)
            .corner_radius(8)
            .min_size(vec2(166.0, 43.0)),
    )
}

pub fn secondary(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).font(heading(14.0)))
            .fill(IVORY)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(8)
            .min_size(vec2(100.0, 38.0)),
    )
}

pub fn pill(ui: &mut Ui, text: &str, foreground: Color32, background: Color32, dot: bool) {
    Frame::new()
        .fill(background)
        .corner_radius(18)
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if dot {
                    let (rect, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.0, foreground);
                }
                ui.label(RichText::new(text).font(heading(11.5)).color(foreground));
            });
        });
}

pub fn navigation(
    ui: &mut Ui,
    text: &str,
    kind: Icon,
    selected: bool,
    count: Option<usize>,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click());
    let hovered = response.hovered();
    if selected || hovered {
        ui.painter()
            .rect_filled(rect, 8, if selected { NAVY_LIGHT } else { NAVY });
    }
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(rect.left(), rect.top() + 12.0), vec2(3.0, 24.0)),
            2,
            CREAM,
        );
    }
    let color = if selected || hovered {
        IVORY
    } else {
        NAV_MUTED
    };
    paint_icon(
        ui.painter(),
        Rect::from_center_size(pos2(rect.left() + 25.0, rect.center().y), vec2(21.0, 21.0)),
        kind,
        color,
    );
    ui.painter().text(
        pos2(rect.left() + 48.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        if selected {
            heading(15.5)
        } else {
            FontId::proportional(15.5)
        },
        color,
    );
    if let Some(count) = count.filter(|c| *c > 0) {
        let badge =
            Rect::from_center_size(pos2(rect.right() - 23.0, rect.center().y), vec2(27.0, 24.0));
        ui.painter().rect_filled(badge, 6, NAVY_LIGHT);
        ui.painter().text(
            badge.center(),
            Align2::CENTER_CENTER,
            count.to_string(),
            heading(12.0),
            CREAM,
        );
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 8, Stroke::new(1.5, CREAM), StrokeKind::Inside);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
