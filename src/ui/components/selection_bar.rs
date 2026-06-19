use eframe::egui;

use crate::native_ui;


pub fn primary_list_text_color(keyboard_highlight: bool) -> egui::Color32 {
    if keyboard_highlight {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(220, 224, 234)
    }
}

pub fn paint_hover_card_highlight(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 8.0, native_ui::POPUP_CARD_HOVER);
    painter.rect_stroke(
        rect,
        8,
        egui::Stroke::new(1.0, native_ui::GLASS_BORDER),
        egui::StrokeKind::Inside,
    );
}
