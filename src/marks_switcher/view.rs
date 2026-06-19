use eframe::egui;

use crate::icons::IconCache;
use crate::marks_switcher::panel::MarksSwitcherController;
use crate::native_ui;
use crate::ui::components::overlay_panel::{overlay_panel_header, OverlayPanelHeaderProps};
use crate::ui::components::selection_bar::primary_list_text_color;

pub struct MarksSwitcherViewOutput {
    pub content_size: egui::Vec2,
}

fn marks_cards_width(count: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let count = count as f32;
    count * native_ui::MARKS_CARD_WIDTH + (count - 1.0) * native_ui::MARKS_CARD_GAP
}

pub fn render_marks_switcher(
    ui: &mut egui::Ui,
    controller: &MarksSwitcherController,
    icon_cache: &mut IconCache,
    ctx: &egui::Context,
) -> MarksSwitcherViewOutput {
    let selected = controller.selected;
    let entries = controller.entries.clone();
    let cards_width = marks_cards_width(entries.len());
    let max_content_width = ctx
        .input(|i| i.viewport().monitor_size.map(|s| s.x * 0.9))
        .unwrap_or(1200.0);

    let (_, panel_rect) = native_ui::render_overlay_shell(ui, |ui| {
        let content_width = cards_width.max(220.0);
        ui.set_max_width(content_width);
        ui.set_width(content_width);

        overlay_panel_header(
            ui,
            &OverlayPanelHeaderProps {
                title: "Marked windows",
                trailing: "Release to switch",
            },
        );
        ui.add_space(6.0);

        egui::ScrollArea::horizontal()
            .id_salt("marks_switcher_cards")
            .auto_shrink([true, true])
            .max_width(cards_width.min(max_content_width))
            .show(ui, |ui| {
                if cards_width > 0.0 {
                    ui.set_width(cards_width);
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = native_ui::MARKS_CARD_GAP;
                    let card_size = egui::vec2(
                        native_ui::MARKS_CARD_WIDTH,
                        native_ui::MARKS_CARD_HEIGHT,
                    );
                    for (idx, entry) in entries.iter().enumerate() {
                        let is_selected = idx == selected;
                        let (card_rect, card_response) = ui.allocate_exact_size(
                            card_size,
                            egui::Sense::hover(),
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(card_rect), |ui| {
                            ui.set_width(card_size.x);
                            ui.set_min_width(card_size.x);
                            ui.set_max_width(card_size.x);
                            native_ui::overlay_card_frame(is_selected, false).show(ui, |ui| {
                                ui.set_width(native_ui::MARKS_CARD_INNER_WIDTH);
                                ui.set_min_width(native_ui::MARKS_CARD_INNER_WIDTH);
                                ui.set_max_width(native_ui::MARKS_CARD_INNER_WIDTH);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        native_ui::badge(
                                            ui,
                                            &format!("{}", entry.slot),
                                            native_ui::ACCENT,
                                        );
                                        ui.add_space(6.0);
                                        native_ui::icon_slot(
                                            ui,
                                            egui::vec2(24.0, 24.0),
                                            |ui| {
                                                if let Some(win) = &entry.window {
                                                    if let Some(texture) =
                                                        icon_cache.file_icon(
                                                            ctx,
                                                            &win.exe_path,
                                                            24,
                                                        )
                                                    {
                                                        native_ui::list_icon(
                                                            ui, texture, 24.0,
                                                        );
                                                    }
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                    if let Some(win) = &entry.window {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&win.title)
                                                    .size(11.0)
                                                    .strong()
                                                    .color(primary_list_text_color(
                                                        is_selected,
                                                    )),
                                            )
                                            .truncate()
                                            .wrap_mode(egui::TextWrapMode::Truncate),
                                        );
                                    }
                                });
                            });
                        });
                        if is_selected && cards_width > max_content_width {
                            ui.scroll_to_rect(
                                card_response.rect,
                                Some(egui::Align::Center),
                            );
                        }
                    }
                });
            });
    });

    let content_size = if panel_rect.is_positive() {
        panel_rect.size()
    } else {
        egui::Vec2::ZERO
    };

    MarksSwitcherViewOutput { content_size }
}
