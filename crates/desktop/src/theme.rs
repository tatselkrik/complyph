use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};
use std::sync::Arc;

pub const NAVY: Color32 = Color32::from_rgb(20, 43, 73);
pub const NAVY_DEEP: Color32 = Color32::from_rgb(15, 33, 57);
pub const NAVY_LIGHT: Color32 = Color32::from_rgb(43, 64, 91);
pub const CREAM: Color32 = Color32::from_rgb(246, 241, 231);
pub const IVORY: Color32 = Color32::from_rgb(255, 252, 246);
pub const BORDER: Color32 = Color32::from_rgb(221, 214, 200);
pub const MUTED: Color32 = Color32::from_rgb(82, 97, 116);
pub const NAV_MUTED: Color32 = Color32::from_rgb(180, 192, 207);
pub const GREEN: Color32 = Color32::from_rgb(50, 98, 74);
pub const GREEN_TINT: Color32 = Color32::from_rgb(232, 239, 230);
pub const AMBER: Color32 = Color32::from_rgb(139, 92, 20);
pub const AMBER_TINT: Color32 = Color32::from_rgb(248, 237, 214);
pub const RED: Color32 = Color32::from_rgb(163, 54, 54);
pub const RED_TINT: Color32 = Color32::from_rgb(248, 232, 226);

pub fn heading(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Lato bold".into()))
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Lato".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/Lato-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "Lato bold".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/Lato-Bold.ttf"
        ))),
    );
    let mut proportional = fonts.families[&FontFamily::Proportional].clone();
    proportional.insert(0, "Lato".into());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional.clone());
    proportional.insert(0, "Lato bold".into());
    fonts
        .families
        .insert(FontFamily::Name("Lato bold".into()), proportional);
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Light);

    let mut style = egui::Style::default();
    style.text_styles.insert(TextStyle::Heading, heading(24.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(15.0));
    style.text_styles.insert(TextStyle::Button, heading(15.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.0));
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 10.0);
    style.spacing.interact_size = egui::vec2(40.0, 36.0);
    style.spacing.indent = 18.0;
    style.animation_time = 0.12;
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = None;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, NAVY);
    style.visuals.text_edit_bg_color = Some(IVORY);
    style.visuals.panel_fill = CREAM;
    style.visuals.window_fill = IVORY;
    style.visuals.extreme_bg_color = IVORY;
    style.visuals.faint_bg_color = CREAM;
    style.visuals.window_corner_radius = CornerRadius::same(12);
    style.visuals.window_stroke = Stroke::new(1.0, BORDER);
    style.visuals.selection.bg_fill = NAVY;
    style.visuals.selection.stroke = Stroke::new(1.0, CREAM);
    style.visuals.hyperlink_color = NAVY;
    for visual in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        visual.corner_radius = CornerRadius::same(7);
        visual.fg_stroke = Stroke::new(1.3, NAVY);
        visual.bg_stroke = Stroke::new(1.0, BORDER);
    }
    style.visuals.widgets.inactive.bg_fill = IVORY;
    style.visuals.widgets.inactive.weak_bg_fill = IVORY;
    style.visuals.widgets.hovered.bg_fill = CREAM;
    style.visuals.widgets.hovered.weak_bg_fill = CREAM;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.2, NAVY);
    style.visuals.widgets.active.bg_fill = BORDER;
    style.visuals.widgets.active.weak_bg_fill = BORDER;
    ctx.set_global_style(style);
}
