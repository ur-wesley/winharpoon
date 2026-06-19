use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::apps::controller::{icon_path, AppMenuAction, AppMenuController};
use crate::apps::favorites::SharedFavorites;
use crate::apps::is_ready;
use crate::icons::IconCache;
use crate::native_ui;
use crate::ui::components::overlay_panel::{overlay_panel_header, OverlayPanelHeaderProps};
use crate::ui::components::search_list_row::{
    searchable_list_row, RowHighlight, SearchableListRowProps, LIST_ICON_SIZE,
};

pub struct AppMenuViewOutput {
    pub actions: Vec<AppMenuAction>,
}

pub fn render_app_menu(
    ui: &mut egui::Ui,
    controller: &mut AppMenuController,
    _favorites: &SharedFavorites,
    _state: &Arc<Mutex<AppState>>,
    icon_cache: &mut IconCache,
    open_frames: u32,
    scroll_to: bool,
) -> AppMenuViewOutput {
    let ctx = ui.ctx().clone();
    let mut actions = Vec::new();
    let selected_row = controller.selection.selected;

    native_ui::render_popup_layout(
        ui,
        "app_menu_hints",
        &[
            ("↑↓", "Navigate"),
            ("Enter", "Launch"),
            ("Ctrl+D", "Favorite"),
            ("Esc", "Close"),
        ],
        |ui| {
            ui.set_width(ui.available_width());
            overlay_panel_header(
                ui,
                &OverlayPanelHeaderProps {
                    title: "Apps",
                    trailing: &format!("{} programs", controller.trailing_count()),
                },
            );
            ui.add_space(4.0);
            let r = native_ui::overlay_search_bar(ui, &mut controller.query, "Search programs…");
            if open_frames <= 2 {
                r.request_focus();
            }
            ui.add_space(6.0);

            if ctx.input(|i| {
                (i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp))
                    && !i.modifiers.ctrl
            }) && !controller.rows.is_empty()
            {
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                    actions.push(AppMenuAction::Navigate(1));
                } else {
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                    actions.push(AppMenuAction::Navigate(-1));
                }
            }

            egui::ScrollArea::vertical()
                .id_salt("app_menu_list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let list_width = ui.available_width();
                    ui.set_width(list_width);
                    ui.with_layout(
                        egui::Layout::top_down(egui::Align::Min).with_cross_justify(true),
                        |ui| {
                            ui.set_width(list_width);
                            if controller.rows.is_empty() {
                                ui.label(
                                    egui::RichText::new(if is_ready() {
                                        "No matching programs"
                                    } else {
                                        "Indexing programs…"
                                    })
                                    .color(native_ui::TEXT_MUTED),
                                );
                                return;
                            }
                            for (row_i, row) in controller.rows.iter().enumerate() {
                                let keyboard_highlight =
                                    controller.selection.hovered.is_none() && row_i == selected_row;
                                let row_result = searchable_list_row(
                                    ui,
                                    &SearchableListRowProps {
                                        icon: icon_cache
                                            .file_icon(
                                                &ctx,
                                                icon_path(&row.entry),
                                                LIST_ICON_SIZE as u32,
                                            ),
                                        title: &row.entry.name,
                                        highlight: if keyboard_highlight {
                                            RowHighlight::Keyboard
                                        } else {
                                            RowHighlight::None
                                        },
                                        active_border: false,
                                        scroll_to: scroll_to && keyboard_highlight,
                                        is_favorite: row.favorite,
                                        mark_slot: None,
                                    },
                                );
                                if row_result.highlight == RowHighlight::Hover {
                                    actions.push(AppMenuAction::Hover(Some(row_i)));
                                }
                                if row_result.response.clicked() {
                                    actions.push(AppMenuAction::Launch(row_i));
                                }
                                ui.add_space(2.0);
                            }
                        },
                    );
                });
        },
    );

    if controller.query != controller.last_query {
        actions.push(AppMenuAction::SetQuery);
    }

    if ctx.input(|i| {
        i.key_pressed(egui::Key::D)
            && i.modifiers.ctrl
            && !i.modifiers.shift
            && !i.modifiers.alt
    }) && controller.capture_hotkey_id.is_none()
    {
        let active = controller.selection.active_row();
        if let Some(row) = controller.rows.get(active) {
            actions.push(AppMenuAction::ToggleFavorite(row.entry.id.clone()));
        }
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && controller.capture_hotkey_id.is_none() {
        let active = controller.selection.active_row();
        if controller.rows.get(active).is_some() {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            actions.push(AppMenuAction::Launch(active));
        }
    }

    AppMenuViewOutput { actions }
}
