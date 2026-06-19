use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::config::{Config, ConfigValidationError};
use crate::hotkeys::HotkeyRegistrationResult;
use crate::native_ui;
use crate::paths;
use crate::settings::controller::{
    binding_display_name, SettingsAction, SettingsController,
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
) -> SettingsViewOutput {
    let mut actions = Vec::new();
    let mut capture_ui_used = false;
    let panel_fill = native_ui::GLASS_PANEL;
    let registrations = controller.registration_map(state);
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
                    ui.label(
                        egui::RichText::new("WinHarpoon")
                            .size(24.0)
                            .strong()
                            .color(native_ui::ACCENT),
                    );
                    ui.label(
                        egui::RichText::new("Changes save and reload hotkeys automatically")
                            .size(13.5)
                            .color(native_ui::TEXT_MUTED),
                    );
                });
            });
            ui.add_space(16.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    native_ui::section_frame().show(ui, |ui| {
                        native_ui::section_heading(
                            ui,
                            "General",
                            "Startup and background behavior",
                        );
                        ui.add_space(10.0);
                        render_general_row(ui, controller, &mut actions);
                    });
                    ui.add_space(12.0);

                    native_ui::section_frame().show(ui, |ui| {
                        native_ui::section_heading(
                            ui,
                            "Apps",
                            "Alt+double-click menu and installed program search",
                        );
                        ui.add_space(10.0);
                        render_apps_row(ui, controller, &mut actions);
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
                                    &registrations,
                                    &mut actions,
                                    &mut capture_ui_used,
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
            native_ui::muted_label(
                ui,
                "Launch WinHarpoon automatically when you sign in",
            );
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

fn render_apps_row(ui: &mut egui::Ui, controller: &SettingsController, actions: &mut Vec<SettingsAction>) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Alt + double-click").size(13.5).strong());
            native_ui::muted_label(
                ui,
                "Or double-tap Alt — opens centered on the active monitor",
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
        ui.label("Scope");
        let mut scope = controller.draft.apps.alt_double_click_scope.clone();
        egui::ComboBox::from_id_salt("apps_scope")
            .selected_text(&scope)
            .show_ui(ui, |ui| {
                for s in ["anywhere", "desktop_only"] {
                    if ui.selectable_value(&mut scope, s.to_string(), s).clicked() {
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
