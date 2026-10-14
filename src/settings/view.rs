use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::config::{Config, ConfigValidationError};
use crate::hotkeys::HotkeyRegistrationResult;
use crate::icons::ui_icons;
use crate::modes::marks::{filled_entries, SharedMarks};
use crate::native_ui;
use crate::paths;
use crate::settings::controller::{
    binding_display_name, SettingsAction, SettingsController, SettingsTab,
};
use crate::ui::components::chord_button::{chord_binding_button, ChordButtonProps};

pub struct SettingsViewOutput {
    pub actions: Vec<SettingsAction>,
    pub capture_ui_used: bool,
}

pub fn render_settings(
    ui: &mut egui::Ui,
    controller: &SettingsController,
    state: &Arc<Mutex<AppState>>,
    _config: &Arc<Mutex<Config>>,
    marks: &SharedMarks,
) -> SettingsViewOutput {
    let mut actions = Vec::new();
    let mut capture_ui_used = false;
    let panel_fill = native_ui::GLASS_PANEL;
    let registrations = SettingsController::registration_map(state);
    let conflicts = state.lock().hotkey_conflicts;

    egui::Panel::bottom("settings_footer")
        .frame(
            egui::Frame::NONE
                .fill(panel_fill)
                .inner_margin(egui::Margin::symmetric(24, 12)),
        )
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if native_ui::secondary_button(ui, "Reset defaults").clicked() {
                    actions.push(SettingsAction::ResetDefaults);
                    capture_ui_used = true;
                }
                ui.add_space(8.0);
                if native_ui::primary_button(ui, "Save changes").clicked() {
                    actions.push(SettingsAction::Save);
                    capture_ui_used = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !controller.status.is_empty() {
                        let color = if controller.validation_errors.is_empty()
                            && !controller.status.starts_with("Failed")
                        {
                            native_ui::SUCCESS
                        } else {
                            native_ui::DANGER
                        };
                        ui.label(egui::RichText::new(&controller.status).color(color));
                    }
                });
            });
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(panel_fill)
                .inner_margin(egui::Margin::symmetric(24, 12)),
        )
        .show_inside(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("WinHarpoon")
                                .size(24.0)
                                .strong()
                                .color(native_ui::ACCENT),
                        );
                        ui.label(
                            egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .size(13.5)
                                .color(native_ui::TEXT_MUTED),
                        );
                    });
                    ui.label(
                        egui::RichText::new("Changes save and reload hotkeys automatically")
                            .size(13.5)
                            .color(native_ui::TEXT_MUTED),
                    );
                });
            });
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                for tab in [SettingsTab::Settings, SettingsTab::Marks] {
                    let selected = controller.active_tab == tab;
                    if ui.selectable_label(selected, tab.label()).clicked() {
                        actions.push(SettingsAction::SelectTab(tab));
                    }
                }
            });
            ui.add_space(12.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if controller.active_tab == SettingsTab::Marks {
                        render_marks_tab(ui, marks, &mut actions);
                    } else {
                        render_settings_tab(
                            ui,
                            controller,
                            &registrations,
                            conflicts,
                            &mut actions,
                            &mut capture_ui_used,
                        );
                    }

                    ui.add_space(12.0);
                });
        });

    SettingsViewOutput {
        actions,
        capture_ui_used,
    }
}

fn render_general_row(
    ui: &mut egui::Ui,
    controller: &SettingsController,
    actions: &mut Vec<SettingsAction>,
) {
    let mut autostart = controller.draft.general.autostart;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(220.0);
            ui.label(
                egui::RichText::new("Start with Windows")
                    .size(13.5)
                    .strong(),
            );
            native_ui::muted_label(ui, "Launch WinHarpoon automatically when you sign in");
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.checkbox(&mut autostart, "Enabled").changed() {
                actions.push(SettingsAction::SetAutostart(autostart));
            }
        });
    });
    ui.add_space(10.0);
    let config_path = paths::config_path();
    native_ui::muted_label(ui, &format!("Config: {}", config_path.display()));
    ui.add_space(6.0);
    if native_ui::secondary_button(ui, "Open config folder").clicked() {
        paths::open_config_folder();
    }
}

fn render_apps_row(
    ui: &mut egui::Ui,
    controller: &SettingsController,
    actions: &mut Vec<SettingsAction>,
) {
    let modifier = controller.draft.apps.double_tap_key.as_str();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(format!("{modifier} + double-click"))
                    .size(13.5)
                    .strong(),
            );
            native_ui::muted_label(
                ui,
                "Or double-tap the chosen modifier — opens centered on the active monitor",
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut enabled = controller.draft.apps.alt_double_click;
            if ui.checkbox(&mut enabled, "Enabled").changed() {
                actions.push(SettingsAction::SetAltDoubleClick(enabled));
            }
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Double-tap key");
        let mut key = controller.draft.apps.double_tap_key.clone();
        egui::ComboBox::from_id_salt("apps_double_tap_key")
            .selected_text(&key)
            .show_ui(ui, |ui| {
                for option in ["Alt", "Ctrl", "Shift", "Win"] {
                    if ui
                        .selectable_value(&mut key, option.to_string(), option)
                        .clicked()
                    {
                        actions.push(SettingsAction::SetDoubleTapKey(key.clone()));
                    }
                }
            });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Scope");
        let mut scope = controller.draft.apps.alt_double_click_scope.clone();
        let scope_label = match scope.as_str() {
            "not_fullscreen" => "Not when fullscreen",
            _ => "Anywhere",
        };
        egui::ComboBox::from_id_salt("apps_scope")
            .selected_text(scope_label)
            .show_ui(ui, |ui| {
                for (value, label) in [
                    ("anywhere", "Anywhere"),
                    ("not_fullscreen", "Not when fullscreen"),
                ] {
                    if ui
                        .selectable_value(&mut scope, value.to_string(), label)
                        .clicked()
                    {
                        actions.push(SettingsAction::SetAltDoubleClickScope(scope.clone()));
                    }
                }
            });
    });
    ui.add_space(8.0);
    native_ui::muted_label(
        ui,
        &format!("Favorites: {}", paths::favorites_path().display()),
    );
    ui.add_space(8.0);
    if native_ui::secondary_button(ui, "Rebuild app index").clicked() {
        actions.push(SettingsAction::RebuildAppIndex);
    }
}

fn render_settings_tab(
    ui: &mut egui::Ui,
    controller: &SettingsController,
    registrations: &HashMap<String, HotkeyRegistrationResult>,
    conflicts: usize,
    actions: &mut Vec<SettingsAction>,
    capture_ui_used: &mut bool,
) {
    native_ui::section_frame().show(ui, |ui| {
        native_ui::section_heading(ui, "General", "Startup and background behavior");
        ui.add_space(10.0);
        render_general_row(ui, controller, actions);
    });
    ui.add_space(12.0);

    native_ui::section_frame().show(ui, |ui| {
        native_ui::section_heading(
            ui,
            "Apps",
            "Alt+double-click menu and installed program search",
        );
        ui.add_space(10.0);
        render_apps_row(ui, controller, actions);
    });
    ui.add_space(12.0);

    for section in SettingsController::binding_sections() {
        native_ui::section_frame().show(ui, |ui| {
            native_ui::section_heading(ui, section.title, section.subtitle);
            ui.add_space(10.0);

            for label in section.labels {
                render_binding_row(
                    ui,
                    controller,
                    label,
                    registrations,
                    actions,
                    capture_ui_used,
                );
                ui.add_space(4.0);
            }
        });
        ui.add_space(12.0);
    }

    if !controller.validation_errors.is_empty() {
        ui.add_space(8.0);
        render_validation_errors(ui, &controller.validation_errors);
    }

    if conflicts > 0 {
        ui.add_space(8.0);
        native_ui::section_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                native_ui::badge(ui, "Conflict", native_ui::WARNING);
                ui.label(
                    egui::RichText::new(format!(
                        "{conflicts} hotkey conflict(s) — remap conflicting bindings"
                    ))
                    .color(native_ui::WARNING),
                );
            });
        });
    }
}

fn render_marks_tab(ui: &mut egui::Ui, marks: &SharedMarks, actions: &mut Vec<SettingsAction>) {
    native_ui::section_frame().show(ui, |ui| {
        native_ui::section_heading(
            ui,
            "Marked windows",
            "Jump to, reorder, or remove numbered marks",
        );
        ui.add_space(10.0);
        render_marks_section(ui, marks, actions);
    });
}

fn render_marks_section(ui: &mut egui::Ui, marks: &SharedMarks, actions: &mut Vec<SettingsAction>) {
    let entries = {
        let guard = marks.lock();
        filled_entries(&guard.store)
    };
    ui.horizontal(|ui| {
        native_ui::muted_label(ui, &format!("{} of 9 slots used", entries.len()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if native_ui::small_button(ui, "Clear all").clicked() {
                actions.push(SettingsAction::ClearMarks);
            }
        });
    });
    ui.add_space(8.0);
    if entries.is_empty() {
        native_ui::muted_label(
            ui,
            "No marked windows yet. Use Toggle mark or Win+Alt+Shift+1…9.",
        );
        return;
    }
    for entry in entries {
        let title = entry.window.as_ref().map_or_else(
            || entry.identity.display_label(),
            |w| {
                if w.title.trim().is_empty() {
                    w.process_name.clone()
                } else {
                    format!("{} — {}", w.process_name, w.title)
                }
            },
        );
        let live = entry.window.is_some();
        ui.horizontal(|ui| {
            native_ui::badge(ui, &format!("Slot {}", entry.slot), native_ui::ACCENT);
            ui.vertical(|ui| {
                ui.set_min_width(220.0);
                ui.label(egui::RichText::new(title).size(13.0).strong());
                native_ui::muted_label(
                    ui,
                    if live {
                        "window found"
                    } else {
                        "stale — window closed"
                    },
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if native_ui::small_button(ui, "Unmark").clicked() {
                    actions.push(SettingsAction::UnmarkSlot(entry.slot));
                }
                if native_ui::small_button(ui, "Show").clicked() {
                    actions.push(SettingsAction::JumpToSlot(entry.slot));
                }
                if native_ui::icon_small_button(
                    ui,
                    ui_icons::move_up_label(16.0, native_ui::TEXT_DIM),
                )
                .clicked()
                {
                    actions.push(SettingsAction::MoveMarkSlot {
                        slot: entry.slot,
                        up: true,
                    });
                }
                if native_ui::icon_small_button(
                    ui,
                    ui_icons::move_down_label(16.0, native_ui::TEXT_DIM),
                )
                .clicked()
                {
                    actions.push(SettingsAction::MoveMarkSlot {
                        slot: entry.slot,
                        up: false,
                    });
                }
            });
        });
        ui.add_space(6.0);
    }
}

fn render_binding_row(
    ui: &mut egui::Ui,
    controller: &SettingsController,
    label: &str,
    registrations: &HashMap<String, HotkeyRegistrationResult>,
    actions: &mut Vec<SettingsAction>,
    capture_ui_used: &mut bool,
) {
    let chord = controller.chord_for(label);
    let capturing = controller.capture_label.as_deref() == Some(label);
    let display_name = binding_display_name(label);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(220.0);
            ui.label(egui::RichText::new(display_name).size(13.5).strong());
            native_ui::muted_label(ui, label);
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if native_ui::small_button(ui, "Clear").clicked() {
                actions.push(SettingsAction::ClearBinding(label.to_string()));
                *capture_ui_used = true;
            }

            if chord_binding_button(
                ui,
                &ChordButtonProps {
                    chord: &chord,
                    capturing,
                },
            )
            .clicked()
            {
                actions.push(SettingsAction::StartCapture(label.to_string()));
                *capture_ui_used = true;
            }
        });
    });

    if capturing {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            native_ui::muted_label(
                ui,
                "Press the key combination. For Win+… chords, edit config.toml directly. Esc or click outside cancels.",
            );
        });
    }

    if let Some(result) = registrations.get(label) {
        if !result.success {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.colored_label(
                    native_ui::DANGER,
                    format!(
                        "Conflict with {} — {}",
                        result.chord,
                        result.error.clone().unwrap_or_default()
                    ),
                );
            });
        }
    }
}

fn render_validation_errors(ui: &mut egui::Ui, errors: &[ConfigValidationError]) {
    native_ui::section_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("Validation errors")
                .strong()
                .color(native_ui::DANGER),
        );
        ui.add_space(6.0);
        for err in errors {
            let text = match err {
                ConfigValidationError::DuplicateBinding {
                    chord,
                    first,
                    second,
                } => format!("Duplicate {chord}: {first} and {second}"),
                ConfigValidationError::InvalidChord {
                    label,
                    chord,
                    reason,
                } => format!("Invalid {label} ({chord}): {reason}"),
            };
            ui.colored_label(native_ui::DANGER, text);
        }
    });
}
