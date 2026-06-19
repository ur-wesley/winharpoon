use eframe::egui;

use crate::native_ui;

pub struct ChordButtonProps<'a> {
    pub chord: &'a str,
    pub capturing: bool,
}

pub fn chord_binding_button(ui: &mut egui::Ui, props: &ChordButtonProps<'_>) -> egui::Response {
    let button_text = if props.capturing {
        "Press keys…".to_string()
    } else if props.chord.trim().is_empty() {
        "Click to bind".to_string()
    } else {
        props.chord.to_string()
    };

    let chord_button = egui::Button::new(
        egui::RichText::new(button_text)
            .monospace()
            .strong()
            .color(if props.capturing {
                native_ui::ACCENT
            } else if props.chord.trim().is_empty() {
                native_ui::TEXT_DIM
            } else {
                egui::Color32::WHITE
            }),
    )
    .fill(if props.capturing {
        native_ui::ACCENT.gamma_multiply(0.15)
    } else {
        native_ui::GLASS_INSET
    })
    .stroke(egui::Stroke::new(
        if props.capturing { 1.5 } else { 1.0 },
        if props.capturing {
            native_ui::ACCENT
        } else {
            native_ui::BORDER
        },
    ))
    .corner_radius(8)
    .min_size(egui::vec2(160.0, 34.0));

    ui.add(chord_button)
}
