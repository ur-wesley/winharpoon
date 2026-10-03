use egui::{Color32, Painter, Rect, RichText, Ui};
use egui_material_icons::icon_text;
use egui_material_icons::icons::{
    ICON_FOLDER_OPEN, ICON_POWER_SETTINGS_NEW, ICON_REFRESH, ICON_SEARCH, ICON_SETTINGS,
    ICON_SYSTEM_UPDATE_ALT,
};

pub fn init(ctx: &egui::Context) {
    egui_material_icons::initialize(ctx);
}

#[derive(Clone, Copy)]
pub enum TrayIconKind {
    Settings,
    Folder,
    Reload,
    Quit,
    Update,
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
        TrayIconKind::Update => ICON_SYSTEM_UPDATE_ALT,
    }
}

pub fn paint_tray_icon(
    painter: &Painter,
    rect: Rect,
    kind: TrayIconKind,
    size: f32,
    color: Color32,
) {
    // Optical correction: the icon font's glyph ink sits below the line-box
    // center, so box-centering reads as "icons sit low" next to text labels.
    const OPTICAL_DY: f32 = -1.0;
    let icon = tray_material_icon(kind);
    painter.text(
        egui::pos2(rect.center().x, rect.center().y + OPTICAL_DY),
        egui::Align2::CENTER_CENTER,
        icon.codepoint,
        egui::FontId::new(size, icon.font_family()),
        color,
    );
}
