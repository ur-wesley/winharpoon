use eframe::egui;

use crate::icons::IconCache;
use crate::launcher::controller::{LauncherAction, LauncherController};
use crate::modes::marks::SharedMarks;
use crate::native_ui;
use crate::ui::components::overlay_panel::overlay_panel_header;
use crate::ui::components::search_list_row::{
    searchable_list_row, RowHighlight, SearchableListRowProps, LIST_ICON_SIZE,
};

pub struct LauncherViewOutput {
    pub actions: Vec<LauncherAction>,
}

pub fn render_launcher(
    ui: &mut egui::Ui,
    controller: &mut LauncherController,
    filtered: &[usize],
    _marks: &SharedMarks,
    icon_cache: &mut IconCache,
    open_frames: u32,
    scroll_to: bool,
) -> LauncherViewOutput {
    let ctx = ui.ctx().clone();
    let mut actions = Vec::new();

    let selected_row = controller.selection.selected;
    let mut launch_hwnd = None;

    native_ui::render_popup_layout(
        ui,
        "launcher_hints",
        &[
            ("↑↓", "Navigate"),
            ("Enter", "Switch"),
            ("Ctrl+M", "Toggle mark"),
            ("Ctrl+Shift+↑↓", "Reorder mark"),
            ("Esc", "Close"),
        ],
        |ui| {
            ui.set_width(ui.available_width());
            overlay_panel_header(
                ui,
                &crate::ui::components::overlay_panel::OverlayPanelHeaderProps {
                    title: "WinHarpoon",
                    trailing: &format!("{} windows", controller.windows.len()),
                },
            );
            ui.add_space(2.0);

            let response =
                native_ui::overlay_search_bar(ui, &mut controller.query, "Search windows…");
            if open_frames <= 3 || response.gained_focus() || controller.query.is_empty() {
                response.request_focus();
            }

            if ctx.input(|i| {
                (i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp))
                    && !i.modifiers.ctrl
            }) && !filtered.is_empty()
            {
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                    actions.push(LauncherAction::Navigate(1));
                } else {
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                    actions.push(LauncherAction::Navigate(-1));
                }
            }

            ui.add_space(6.0);

            let list_width = ui.available_width();
            egui::ScrollArea::vertical()
                .id_salt("launcher_list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(list_width);
                    ui.with_layout(
                        egui::Layout::top_down(egui::Align::Min).with_cross_justify(true),
                        |ui| {
                            ui.set_width(list_width);
                            if filtered.is_empty() {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(12.0);
                                    ui.label(
                                        egui::RichText::new("No matching windows")
                                            .size(11.5)
                                            .color(native_ui::TEXT_MUTED),
                                    );
                                });
                                return;
                            }

                            for (row, win_idx) in filtered.iter().enumerate() {
                                let win = &controller.windows[*win_idx];
                                let is_fg = controller.foreground_hwnd == Some(win.hwnd);
                                let keyboard_highlight =
                                    controller.selection.hovered.is_none() && row == selected_row;

                                let mark_slot = {
                                    let marks_guard = _marks.lock();
                                    let identity = crate::window::identity::WindowIdentity::from_window(win);
                                    marks_guard.store.find_slot(&identity)
                                };

                                let title_lower = win.title.to_lowercase();
                                let process_lower = win.process_name.to_lowercase();
                                let display_title = if title_lower.starts_with(&process_lower)
                                    || title_lower.ends_with(&process_lower)
                                {
                                    win.title.clone()
                                } else {
                                    format!("{} - {}", win.process_name, win.title)
                                };

                                let row_result = searchable_list_row(
                                    ui,
                                    &SearchableListRowProps {
                                        icon: icon_cache
                                            .file_icon(&ctx, &win.exe_path, LIST_ICON_SIZE as u32),
                                        title: &display_title,
                                        highlight: if keyboard_highlight {
                                            RowHighlight::Keyboard
                                        } else {
                                            RowHighlight::None
                                        },
                                        active_border: is_fg,
                                        scroll_to: scroll_to && keyboard_highlight,
                                        mark_slot,
                                        is_favorite: false,
                                    },
                                );

                                if row_result.highlight == RowHighlight::Hover {
                                    actions.push(LauncherAction::Hover(Some(row)));
                                }

                                if row_result.response.clicked() {
                                    launch_hwnd = Some(win.hwnd);
                                }
                                ui.add_space(2.0);
                            }
                        },
                    );
                });
        },
    );

    if let Some(hwnd) = launch_hwnd {
        actions.push(LauncherAction::Commit(hwnd));
    }

    if ctx.input(|i| {
        i.key_pressed(egui::Key::M)
            && i.modifiers.ctrl
            && !i.modifiers.shift
            && !i.modifiers.alt
    }) {
        ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::M));
        actions.push(LauncherAction::ToggleMark);
    }

    if controller.active_window(filtered).is_some() {
        if ctx.input(|i| {
            i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl && i.modifiers.shift
        }) {
            ctx.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::ArrowUp,
                );
            });
            actions.push(LauncherAction::MoveMarkSlot { up: true });
        } else if ctx.input(|i| {
            i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl && i.modifiers.shift
        }) {
            ctx.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::ArrowDown,
                );
            });
            actions.push(LauncherAction::MoveMarkSlot { up: false });
        }
    }

    if controller.query != controller.last_query {
        actions.push(LauncherAction::SetQuery);
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Some(win) = controller.active_window(filtered) {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            actions.push(LauncherAction::Commit(win.hwnd));
        }
    }

    LauncherViewOutput { actions }
}
