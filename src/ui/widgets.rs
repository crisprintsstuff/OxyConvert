//! Custom UI widgets and badge chips matching OxyBrush.

use egui::{Color32, CornerRadius, Margin, Response, RichText, Stroke, Ui, pos2, vec2};

/// Accent clay color used in OxyBrush
pub const OXY_CLAY_ACCENT: Color32 = Color32::from_rgb(0xE0, 0x7A, 0x4F);

/// Render a badge chip with background color and text.
pub fn badge_chip(ui: &mut Ui, text: &str, bg: Color32, fg: Color32) {
    let padding = vec2(6.0, 2.0);
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), egui::FontId::proportional(11.0), fg);
    let size = galley.size() + padding * 2.0;
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
    ui.painter().galley(rect.min + padding, galley, fg);
}

/// A button styled as an OxyBrush primary action button.
pub fn primary_button(ui: &mut Ui, label: &str, accent: Color32) -> Response {
    let button = egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
        .fill(accent)
        .corner_radius(CornerRadius::same(4))
        .min_size(vec2(120.0, 28.0));

    ui.add(button)
}

/// Draw a subtle horizontal separator with title.
pub fn section_header(ui: &mut Ui, title: &str, accent: Color32) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().color(accent));
        let rect = ui.available_rect_before_wrap();
        let y = rect.center().y;
        ui.painter().line_segment(
            [pos2(rect.min.x + 8.0, y), pos2(rect.max.x - 4.0, y)],
            Stroke::new(1.0, Color32::from_gray(50)),
        );
    });
    ui.add_space(2.0);
}

/// Alert / notification banner.
pub fn alert_box(ui: &mut Ui, text: &str, is_error: bool) {
    let (bg, border, fg) = if is_error {
        (
            Color32::from_rgb(50, 18, 18),
            Color32::from_rgb(180, 50, 50),
            Color32::from_rgb(255, 180, 180),
        )
    } else {
        (
            Color32::from_rgb(18, 40, 24),
            Color32::from_rgb(50, 150, 80),
            Color32::from_rgb(180, 240, 200),
        )
    };

    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if is_error { "⚠" } else { "✔" })
                        .color(fg)
                        .strong(),
                );
                ui.label(RichText::new(text).color(fg));
            });
        });
}
