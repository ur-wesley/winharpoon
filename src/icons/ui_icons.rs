use egui::{Color32, Painter, Rect, RichText, Ui};
use egui_material_icons::icon_text;
use egui_material_icons::icons::*;

pub fn init(ctx: &egui::Context) {
    egui_material_icons::initialize(ctx);
}

#[derive(Clone, Copy)]
pub enum TrayIconKind {
    Settings,
    Folder,
    Reload,
    Quit,
}

pub fn search_label(size: f32, color: Color32) -> RichText {
    icon_text(ICON_SEARCH).size(size).color(color)
}

pub fn search_icon(ui: &mut Ui, size: f32, color: Color32) {
    ui.label(search_label(size, color));
}

fn tray_material_icon(kind: TrayIconKind) -> egui_material_icons::MaterialIcon {
    match kind {
        TrayIconKind::Settings => ICON_SETTINGS,
        TrayIconKind::Folder => ICON_FOLDER_OPEN,
        TrayIconKind::Reload => ICON_REFRESH,
        TrayIconKind::Quit => ICON_POWER_SETTINGS_NEW,
    }
}

pub fn paint_tray_icon(painter: &Painter, rect: Rect, kind: TrayIconKind, size: f32, color: Color32) {
    let icon = tray_material_icon(kind);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon.codepoint,
        egui::FontId::new(size, icon.font_family()),
        color,
    );
}


