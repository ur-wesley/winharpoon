use eframe::egui;

use crate::icons::ui_icons::{self, TrayIconKind};
use crate::native_ui;

pub const ROW_HEIGHT: f32 = 26.0;
pub const SLOT_ROW_HEIGHT: f32 = 22.0;
const ROW_LEFT_PADDING: f32 = 10.0;
const ROW_ICON_WIDTH: f32 = 22.0;
const ROW_ICON_TEXT_GAP: f32 = 4.0;
const ROW_TEXT_LEFT: f32 = ROW_LEFT_PADDING + ROW_ICON_WIDTH + ROW_ICON_TEXT_GAP;
const SLOT_BADGE_WIDTH: f32 = 16.0;

pub struct MenuRowProps<'a> {
    pub label: &'a str,
    pub icon: Option<TrayIconKind>,
    pub accent: Option<egui::Color32>,
    pub height: f32,
}

pub fn menu_row(ui: &mut egui::Ui, props: &MenuRowProps<'_>) -> egui::Response {
    let text_color = props.accent.unwrap_or(egui::Color32::from_rgb(220, 224, 234));
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), props.height),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(
                rect.expand2(egui::vec2(-3.0, 0.0)),
                6,
                native_ui::POPUP_CARD_HOVER,
            );
        }
        if let Some(kind) = props.icon {
            let icon_color = props.accent.unwrap_or(native_ui::ACCENT);
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + ROW_LEFT_PADDING, rect.min.y),
                egui::vec2(ROW_ICON_WIDTH, props.height),
            );
            ui_icons::paint_tray_icon(ui.painter(), icon_rect, kind, 14.0, icon_color);
        }
        let text_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + ROW_TEXT_LEFT, rect.min.y),
            egui::pos2(rect.max.x - 4.0, rect.max.y),
        );
        native_ui::tray_menu_clipped_label(ui, text_rect, props.label, 12.0, text_color);
    }
    response
}

pub struct SlotRowProps<'a> {
    pub slot: u8,
    pub label: &'a str,
    pub filled: bool,
}

pub fn slot_row(ui: &mut egui::Ui, props: &SlotRowProps<'_>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), SLOT_ROW_HEIGHT),
        if props.filled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    if ui.is_rect_visible(rect) {
        if props.filled && response.hovered() {
            ui.painter().rect_filled(
                rect.expand2(egui::vec2(-3.0, 0.0)),
                5,
                native_ui::POPUP_CARD_HOVER,
            );
        }
        let badge_rect = egui::Rect::from_center_size(
            rect.left_center() + egui::vec2(ROW_LEFT_PADDING + SLOT_BADGE_WIDTH * 0.5, 0.0),
            egui::vec2(SLOT_BADGE_WIDTH, SLOT_BADGE_WIDTH),
        );
        let badge_fill = if props.filled {
            native_ui::ACCENT.gamma_multiply(0.22)
        } else {
            native_ui::POPUP_CARD
        };
        let badge_stroke = if props.filled {
            native_ui::ACCENT.gamma_multiply(0.5)
        } else {
            native_ui::BORDER.gamma_multiply(0.6)
        };
        ui.painter().rect(
            badge_rect,
            4.0,
            badge_fill,
            egui::Stroke::new(1.0, badge_stroke),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            props.slot.to_string(),
            egui::FontId::proportional(10.0),
            if props.filled {
                native_ui::ACCENT
            } else {
                native_ui::TEXT_DIM
            },
        );
        let text_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + ROW_TEXT_LEFT, rect.min.y),
            egui::pos2(rect.max.x - 4.0, rect.max.y),
        );
        let text_color = if props.filled {
            egui::Color32::from_rgb(210, 214, 226)
        } else {
            native_ui::TEXT_DIM
        };
        native_ui::tray_menu_clipped_label(ui, text_rect, props.label, 11.0, text_color);
    }
    response
}
