use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::icons::ui_icons::TrayIconKind;
use crate::native_ui;
use crate::tray::controller::TrayAction;
use crate::ui::components::menu_row::{menu_row, slot_row, MenuRowProps, SlotRowProps, ROW_HEIGHT};
use crate::ui::components::overlay_panel::{overlay_panel_header, OverlayPanelHeaderProps};

const MENU_WIDTH: f32 = 220.0;

pub struct TrayMenuViewOutput {
    pub action: Option<TrayAction>,
    pub content_size: egui::Vec2,
}

pub fn render_tray_menu(
    ui: &mut egui::Ui,
    state: &Arc<Mutex<AppState>>,
) -> TrayMenuViewOutput {
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

    let (_, panel_rect) = native_ui::render_overlay_shell(ui, |ui| {
        ui.set_width(MENU_WIDTH);
        overlay_panel_header(
            ui,
            &OverlayPanelHeaderProps {
                title: "WinHarpoon",
                trailing: &trailing,
            },
        );
        ui.add_space(4.0);

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
            let subtitle = if *filled {
                label.as_str()
            } else {
                "Empty"
            };
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
        content_size = egui::vec2(MENU_WIDTH, panel_rect.height());
    }

    TrayMenuViewOutput {
        action,
        content_size,
    }
}
