use eframe::egui;

use crate::native_ui;

pub struct OverlayPanelHeaderProps<'a> {
    pub title: &'a str,
    pub trailing: &'a str,
}

pub fn overlay_panel_header(ui: &mut egui::Ui, props: &OverlayPanelHeaderProps<'_>) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(props.title)
                .size(12.0)
                .strong()
                .color(native_ui::ACCENT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(props.trailing)
                    .size(10.5)
                    .color(native_ui::TEXT_MUTED),
            );
        });
    });
}
