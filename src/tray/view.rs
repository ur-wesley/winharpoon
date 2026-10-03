use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::icons::ui_icons::TrayIconKind;
use crate::native_ui;
use crate::tray::controller::TrayAction;
use crate::ui::components::menu_row::{menu_row, slot_row, MenuRowProps, SlotRowProps, ROW_HEIGHT};
use crate::ui::components::overlay_panel::{overlay_panel_header, OverlayPanelHeaderProps};

const MENU_WIDTH: f32 = 200.0;

pub struct TrayMenuViewOutput {
    pub action: Option<TrayAction>,
    pub content_size: egui::Vec2,
}

pub fn render_tray_menu(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) -> TrayMenuViewOutput {
    let conflicts = state.lock().hotkey_conflicts;
    let slot_labels: Vec<(u8, String, bool)> = {
        let state_guard = state.lock();
        let marks = state_guard.marks.lock();
        (1..=9)
            .map(|slot| {
                let label = marks.store.slot_label(slot);
                let filled = label != "empty";
                (slot, label, filled)
            })
            .collect()
    };

    let mut action = None;
    let mut content_size = egui::Vec2::ZERO;
    let trailing = if conflicts > 0 {
        format!("{conflicts} conflicts")
    } else {
        String::new()
    };

    let ((), panel_rect) =
        native_ui::render_overlay_shell_with(ui, native_ui::tray_panel_frame(), |ui| {
            ui.set_width(MENU_WIDTH);
            overlay_panel_header(
                ui,
                &OverlayPanelHeaderProps {
                    title: "WinHarpoon",
                    trailing: &trailing,
                },
            );
            ui.add_space(2.0);

            let update_state = {
                let state_guard = state.lock();
                state_guard.update_state.clone()
            };

            if let crate::updater::UpdateState::Available { version, .. } = &update_state {
                let label = format!("Update to v{version}");
                if menu_row(
                    ui,
                    &MenuRowProps {
                        label: &label,
                        icon: Some(TrayIconKind::Update),
                        accent: Some(native_ui::SUCCESS),
                        height: ROW_HEIGHT,
                    },
                )
                .clicked()
                {
                    action = Some(TrayAction::UpdateApp);
                }
                native_ui::tray_menu_divider(ui);
            }

            if menu_row(
                ui,
                &MenuRowProps {
                    label: "Settings",
                    icon: Some(TrayIconKind::Settings),
                    accent: None,
                    height: ROW_HEIGHT,
                },
            )
            .clicked()
            {
                action = Some(TrayAction::Settings);
            }

            native_ui::tray_menu_divider(ui);
            native_ui::tray_menu_section_label(ui, "Marked slots");

            for (slot, label, filled) in &slot_labels {
                let subtitle = if *filled { label.as_str() } else { "Empty" };
                let response = slot_row(
                    ui,
                    &SlotRowProps {
                        slot: *slot,
                        label: subtitle,
                        filled: *filled,
                    },
                );
                if *filled && response.clicked() {
                    action = Some(TrayAction::JumpSlot(*slot));
                }
            }

            native_ui::tray_menu_divider(ui);

            if menu_row(
                ui,
                &MenuRowProps {
                    label: "Open config folder",
                    icon: Some(TrayIconKind::Folder),
                    accent: None,
                    height: ROW_HEIGHT,
                },
            )
            .clicked()
            {
                action = Some(TrayAction::ConfigFolder);
            }
            if menu_row(
                ui,
                &MenuRowProps {
                    label: "Reload config",
                    icon: Some(TrayIconKind::Reload),
                    accent: None,
                    height: ROW_HEIGHT,
                },
            )
            .clicked()
            {
                action = Some(TrayAction::Reload);
            }
            if menu_row(
                ui,
                &MenuRowProps {
                    label: "Rescan apps",
                    icon: Some(TrayIconKind::Reload),
                    accent: None,
                    height: ROW_HEIGHT,
                },
            )
            .clicked()
            {
                action = Some(TrayAction::RescanApps);
            }

            native_ui::tray_menu_divider(ui);

            if menu_row(
                ui,
                &MenuRowProps {
                    label: "Quit",
                    icon: Some(TrayIconKind::Quit),
                    accent: Some(native_ui::DANGER),
                    height: ROW_HEIGHT,
                },
            )
            .clicked()
            {
                action = Some(TrayAction::Quit);
            }
        });

    if panel_rect.is_positive() {
        // Report the measured panel size (content + frame chrome), not the
        // inner content width: the viewport is sized from this and a narrow
        // value clips the panel's right edge.
        content_size = panel_rect.size();
    }

    TrayMenuViewOutput {
        action,
        content_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The viewport is sized from `content_size`: it must cover the whole
    /// panel (content + frame chrome). A hardcoded inner width clips the
    /// panel's right edge and misplaces the screen-edge clamp.
    /// The viewport is sized from `content_size`: it must cover the whole
    /// panel (content + frame chrome). A hardcoded inner width clips the
    /// panel's right edge and misplaces the screen-edge clamp.
    #[test]
    fn tray_content_size_covers_panel_chrome() {
        let ctx = egui::Context::default();
        crate::icons::ui_icons::init(&ctx);
        let state = std::sync::Arc::new(parking_lot::Mutex::new(AppState::new(
            std::sync::Arc::new(parking_lot::Mutex::new(crate::config::Config::default())),
            crate::modes::marks::shared_marks(),
            crate::apps::shared_favorites(),
        )));
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut size = egui::Vec2::ZERO;
        let _ = ctx.run_ui(raw, |ui| {
            size = render_tray_menu(ui, &state).content_size;
        });
        assert!(
            size.x > MENU_WIDTH,
            "content width {} must exceed inner width {MENU_WIDTH}",
            size.x
        );
        assert!(size.y > 0.0, "content height must be measured");
    }
}
