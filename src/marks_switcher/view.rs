use eframe::egui;

use crate::icons::IconCache;
use crate::marks_switcher::panel::MarksSwitcherController;
use crate::native_ui;
use crate::ui::components::overlay_panel::{overlay_panel_header, OverlayPanelHeaderProps};
use crate::ui::components::selection_bar::primary_list_text_color;

pub struct MarksSwitcherViewOutput {
    pub content_size: egui::Vec2,
}

pub fn render_marks_switcher(
    ui: &mut egui::Ui,
    controller: &MarksSwitcherController,
    icon_cache: &mut IconCache,
    ctx: &egui::Context,
) -> MarksSwitcherViewOutput {
    let selected = controller.selected;
    let entries = controller.entries.clone();
    // Full row width (uncapped) drives the scrollable inner content; the outer
    // panel width is capped so it never exceeds the viewport and clips the border.
    let row_width = native_ui::marks_row_width(entries.len());
    let max_content_width = ctx
        .input(|i| i.viewport().monitor_size.map(|s| s.x * 0.9))
        .unwrap_or(1200.0);
    let cards_width = native_ui::marks_row_outer_width(entries.len(), max_content_width);

    let ((), panel_rect) = native_ui::render_overlay_shell(ui, |ui| {
        // cards_width already floored (220) and capped to the monitor.
        ui.set_max_width(cards_width);
        ui.set_width(cards_width);

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
            .max_width(cards_width)
            .show(ui, |ui| {
                if row_width > 0.0 {
                    ui.set_width(row_width);
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
                                                let exe_path = entry.window.as_ref()
                                                    .map_or(&entry.identity.exe, |w| &w.exe_path);
                                                if let Some(texture) =
                                                    icon_cache.file_icon(
                                                        ctx,
                                                        exe_path,
                                                        24,
                                                    )
                                                {
                                                    native_ui::list_icon(
                                                        ui, texture, 24.0,
                                                    );
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                    let process_name = entry.window.as_ref().map_or_else(
                                        || {
                                            entry.identity.exe.file_name().map_or_else(
                                                || "Unknown".to_string(),
                                                |s| s.to_string_lossy().into_owned(),
                                            )
                                        },
                                        |win| win.process_name.clone(),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&process_name)
                                                .size(11.0)
                                                .strong()
                                                .color(primary_list_text_color(
                                                    is_selected,
                                                )),
                                        )
                                        .truncate()
                                        .wrap_mode(egui::TextWrapMode::Truncate),
                                    );
                                    if entry.window.is_none() {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new("(not running)")
                                                    .size(9.0)
                                                    .color(native_ui::TEXT_MUTED),
                                            )
                                            .truncate()
                                            .wrap_mode(egui::TextWrapMode::Truncate),
                                        );
                                    }
                                });
                            });
                        });
                        if is_selected && row_width > cards_width {
                            ui.scroll_to_rect(
                                card_response.rect,
                                Some(egui::Align::Center),
                            );
                        }
                    }
                });
            });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new("Tips:")
                    .size(9.0)
                    .strong()
                    .color(native_ui::TEXT_MUTED),
            );
            ui.label(
                // ASCII only: the overlay font lacks arrow/bullet glyphs (tofu squares).
                egui::RichText::new("[Del] remove | [Shift+Left/Right] move")
                    .size(9.0)
                    .color(native_ui::TEXT_MUTED),
            );
        });
    });

    let content_size = if panel_rect.is_positive() {
        panel_rect.size()
    } else {
        egui::Vec2::ZERO
    };

    MarksSwitcherViewOutput { content_size }
}
