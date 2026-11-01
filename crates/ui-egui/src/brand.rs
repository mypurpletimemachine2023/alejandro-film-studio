//! Original ACA wordmark for the independent Alejandro Creative Academy derivative.

/// Draw the wordmark and report its bounds without embedding upstream trademark assets.
pub fn paint_wordmark(ui: &egui::Ui, left_center: egui::Pos2, height: f32, _dark: bool) -> Option<egui::Rect> {
    Some(ui.painter().text(left_center, egui::Align2::LEFT_CENTER, "ALEJANDRO Creative Academy", egui::FontId::proportional(height), egui::Color32::from_rgb(217, 181, 103)))
}
