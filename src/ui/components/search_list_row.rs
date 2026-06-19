use eframe::egui;

use crate::native_ui;
use super::selection_bar::{paint_hover_card_highlight, primary_list_text_color};

pub const LIST_ICON_SIZE: f32 = 20.0;
const LIST_ROW_HEIGHT: f32 = 28.0;
const ICON_TEXT_GAP: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowHighlight {
    None,
    Keyboard,
    Hover,
}

pub struct SearchableListRowProps<'a> {
    pub icon: Option<&'a egui::TextureHandle>,
    pub title: &'a str,
    pub highlight: RowHighlight,
    pub active_border: bool,
    pub scroll_to: bool,
    pub is_favorite: bool,
    pub mark_slot: Option<u8>,
}

pub struct SearchableListRowResponse {
    pub response: egui::Response,
    pub highlight: RowHighlight,
}

pub fn searchable_list_row(
    ui: &mut egui::Ui,
    props: &SearchableListRowProps<'_>,
) -> SearchableListRowResponse {
    let keyboard_highlight = props.highlight == RowHighlight::Keyboard;
    let row_width = ui.available_width();
    let frame = native_ui::overlay_card_frame(keyboard_highlight, props.active_border);
    ui.set_width(row_width);
    let card_response = frame.show(ui, |ui| {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width, LIST_ROW_HEIGHT),
            egui::Sense::click(),
        );
        if ui.is_rect_visible(rect) {
            let mut text_left = rect.min.x;
            if let Some(texture) = props.icon {
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.min.x + LIST_ICON_SIZE * 0.5, rect.center().y),
                    egui::vec2(LIST_ICON_SIZE, LIST_ICON_SIZE),
                );
                ui.put(
                    icon_rect,
                    egui::Image::new(texture)
                        .fit_to_exact_size(egui::vec2(LIST_ICON_SIZE, LIST_ICON_SIZE))
                        .corner_radius(4),
                );
                text_left = rect.min.x + LIST_ICON_SIZE + ICON_TEXT_GAP;
            }
            let mut text_right = rect.max.x;
            if props.is_favorite {
                let star_size = 12.0;
                let star_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.max.x - star_size * 0.5 - 6.0, rect.center().y),
                    egui::vec2(star_size, star_size),
                );
                text_right = star_rect.min.x - 4.0;
                let icon = egui_material_icons::icons::ICON_STAR;
                ui.painter().text(
                    star_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    icon.codepoint,
                    egui::FontId::new(star_size, icon.font_family()),
                    native_ui::WARNING,
                );
            }
            if let Some(slot) = props.mark_slot {
                let badge_rect = egui::Rect::from_min_max(
                    egui::pos2(text_right - 32.0, rect.min.y),
                    egui::pos2(text_right - 8.0, rect.max.y),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(badge_rect), |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        native_ui::badge(ui, &slot.to_string(), native_ui::ACCENT);
                    });
                });
                text_right -= 36.0;
            }
            let text_rect = egui::Rect::from_min_max(
                egui::pos2(text_left, rect.min.y),
                egui::pos2(text_right, rect.max.y),
            );
            native_ui::tray_menu_clipped_label(
                ui,
                text_rect,
                props.title,
                12.0,
                primary_list_text_color(keyboard_highlight),
            );
        }
        response
    });

    let hovered = card_response.response.hovered();
    let highlight = if hovered {
        RowHighlight::Hover
    } else {
        props.highlight
    };
    let highlighted = highlight != RowHighlight::None;

    if hovered && !keyboard_highlight {
        paint_hover_card_highlight(ui.painter(), card_response.response.rect);
    }

    if highlighted
        && props.scroll_to {
            ui.scroll_to_rect(card_response.response.rect, Some(egui::Align::Center));
        }

    SearchableListRowResponse {
        response: card_response.response,
        highlight,
    }
}
