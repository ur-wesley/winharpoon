use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::{request_reload, AppState};
use crate::config::{Config, ConfigValidationError};
use crate::hotkeys::HotkeyRegistrationResult;
use crate::log;
use crate::native_ui;
use crate::paths;

pub struct SettingsPanel {
    pub visible: bool,
    draft: Config,
    capture_label: Option<String>,
    validation_errors: Vec<ConfigValidationError>,
    status: String,
}

impl SettingsPanel {
    pub fn new(config: &Config) -> Self {
        Self {
            visible: false,
            draft: config.clone(),
            capture_label: None,
            validation_errors: Vec::new(),
            status: String::new(),
        }
    }

    pub fn show(&mut self, config: &Config) {
        log::debug("settings show");
        self.visible = true;
        self.draft = config.clone();
        self.capture_label = None;
        self.validation_errors.clear();
        self.status.clear();
    }

    pub fn hide(&mut self, ctx: &egui::Context) {
        if !self.visible {
            return;
        }
        log::debug("settings hide");
        self.visible = false;
        self.cancel_capture();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        crate::util::release_stuck_modifier_keys();
    }

    pub fn update(
        &mut self,
        ui: &mut egui::Ui,
        state: &Arc<Mutex<AppState>>,
        config: &Arc<Mutex<Config>>,
    ) {
        let ctx = ui.ctx().clone();
        native_ui::apply_theme(&ctx);
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(&ctx);
            return;
        }

        let mut capture_ui_used = false;
        let panel_fill = egui::Color32::from_rgb(20, 22, 28);

        egui::Panel::bottom("settings_footer")
            .frame(
                egui::Frame::NONE
                    .fill(panel_fill)
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    if native_ui::secondary_button(ui, "Reset defaults").clicked() {
                        log::debug("settings reset defaults");
                        self.draft = Config::default();
                        self.persist_and_reload(config);
                        capture_ui_used = true;
                    }
                    ui.add_space(8.0);
                    if native_ui::primary_button(ui, "Save changes").clicked() {
                        self.try_save(config);
                        capture_ui_used = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !self.status.is_empty() {
                            let color = if self.validation_errors.is_empty()
                                && !self.status.starts_with("Failed")
                            {
                                native_ui::SUCCESS
                            } else {
                                native_ui::DANGER
                            };
                            ui.label(egui::RichText::new(&self.status).color(color));
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
                            self.general_row(ui, config);
                        });
                        ui.add_space(12.0);

                        let registrations = self.registration_map(state);

                        for section in Self::sections() {
                            native_ui::section_frame().show(ui, |ui| {
                                native_ui::section_heading(ui, section.title, section.subtitle);
                                ui.add_space(10.0);

                                for label in section.labels {
                                    self.binding_row(
                                        ui,
                                        label,
                                        &registrations,
                                        &mut capture_ui_used,
                                        config,
                                    );
                                    ui.add_space(4.0);
                                }
                            });
                            ui.add_space(12.0);
                        }

                        if !self.validation_errors.is_empty() {
                            ui.add_space(8.0);
                            native_ui::section_frame().show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("Validation errors")
                                        .strong()
                                        .color(native_ui::DANGER),
                                );
                                ui.add_space(6.0);
                                for err in &self.validation_errors {
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

                        let conflicts = state.lock().hotkey_conflicts;
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

        if let Some(label) = self.capture_label.clone() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Escape) {
                    self.cancel_capture();
                    return;
                }
                for event in &i.events {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        ..
                    } = event
                    {
                        if matches!(key, egui::Key::Escape) {
                            continue;
                        }
                        let vk = egui_key_to_vk(*key);
                        if vk == 0 || crate::util::is_modifier_vk(vk) {
                            continue;
                        }
                        let mods = crate::util::keyboard_modifiers();
                        let chord = crate::config::chord_from_vk_mods(
                            windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk as u16),
                            mods,
                        );
                        log::debug(&format!("capture binding {label} -> {chord}"));
                        self.finish_binding(label.as_str(), chord, config);
                    }
                }
            });

            if ctx.input(|i| i.pointer.any_click()) && !capture_ui_used {
                self.cancel_capture();
            }
        }
    }
}

struct BindingSection {
    title: &'static str,
    subtitle: &'static str,
    labels: Vec<&'static str>,
}

impl SettingsPanel {
    fn general_row(&mut self, ui: &mut egui::Ui, config: &Arc<Mutex<Config>>) {
        let mut autostart = self.draft.general.autostart;
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
                    self.draft.general.autostart = autostart;
                    match crate::autostart::apply(autostart) {
                        Ok(()) => self.persist_general(config),
                        Err(err) => {
                            self.draft.general.autostart = !autostart;
                            self.status = format!("Failed to update autostart: {err}");
                            log::error(&format!("autostart toggle failed: {err}"));
                        }
                    }
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

    fn persist_general(&mut self, config: &Arc<Mutex<Config>>) {
        self.validation_errors.clear();
        match self.draft.save() {
            Ok(()) => {
                *config.lock() = self.draft.clone();
                self.status = if self.draft.general.autostart {
                    "Autostart enabled.".into()
                } else {
                    "Autostart disabled.".into()
                };
                log::debug(&format!(
                    "general settings saved (autostart={})",
                    self.draft.general.autostart
                ));
            }
            Err(err) => {
                log::error(&format!("general settings save failed: {err}"));
                self.status = format!("Failed to save config: {err}");
            }
        }
    }

    fn registration_map(
        &self,
        state: &Arc<Mutex<AppState>>,
    ) -> std::collections::HashMap<String, HotkeyRegistrationResult> {
        state
            .lock()
            .registration_results
            .iter()
            .map(|r| (r.label.clone(), r.clone()))
            .collect()
    }

    fn chord_for(&self, label: &str) -> String {
        match label {
            "launcher" => self.draft.hotkeys.launcher.clone(),
            "same_app_next" => self.draft.hotkeys.same_app_next.clone(),
            "same_app_prev" => self.draft.hotkeys.same_app_prev.clone(),
            "mark_next" => self.draft.hotkeys.mark_next.clone(),
            "mark_prev" => self.draft.hotkeys.mark_prev.clone(),
            "mark_toggle" => self.draft.hotkeys.mark_toggle.clone(),
            "marks_switcher" => self.draft.hotkeys.marks_switcher.clone(),
            other if other.starts_with("mark_") => self
                .draft
                .hotkeys
                .mark
                .get(other.strip_prefix("mark_").unwrap_or(""))
                .cloned()
                .unwrap_or_default(),
            other if other.starts_with("jump_") => self
                .draft
                .hotkeys
                .jump
                .get(other.strip_prefix("jump_").unwrap_or(""))
                .cloned()
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn sections() -> Vec<BindingSection> {
        vec![
            BindingSection {
                title: "Launcher",
                subtitle: "Open the window switcher overlay",
                labels: vec!["launcher"],
            },
            BindingSection {
                title: "Window switching",
                subtitle: "Cycle between windows of the same application",
                labels: vec!["same_app_next", "same_app_prev"],
            },
            BindingSection {
                title: "Marks switcher",
                subtitle: "Hold Win+Alt to open the overlay, Tab to cycle, release to switch",
                labels: vec!["marks_switcher"],
            },
            BindingSection {
                title: "Marks",
                subtitle: "Assign and jump to numbered window marks",
                labels: vec![
                    "mark_toggle",
                    "mark_next",
                    "mark_prev",
                    "mark_1",
                    "mark_2",
                    "mark_3",
                    "mark_4",
                    "mark_5",
                    "mark_6",
                    "mark_7",
                    "mark_8",
                    "mark_9",
                    "jump_1",
                    "jump_2",
                    "jump_3",
                    "jump_4",
                    "jump_5",
                    "jump_6",
                    "jump_7",
                    "jump_8",
                    "jump_9",
                ],
            },
        ]
    }

    fn cancel_capture(&mut self) {
        if self.capture_label.is_some() {
            log::debug(&format!("cancel_capture: {:?}", self.capture_label));
        }
        self.capture_label = None;
        crate::util::release_stuck_modifier_keys();
    }

    fn apply_binding(&mut self, label: &str, chord: String, config: &Arc<Mutex<Config>>) {
        log::debug(&format!("apply_binding {label} -> {chord}"));
        self.draft.set_binding_chord(label, chord);
        self.persist_and_reload(config);
    }

    fn finish_binding(&mut self, label: &str, chord: String, config: &Arc<Mutex<Config>>) {
        self.apply_binding(label, chord, config);
        self.cancel_capture();
    }

    fn persist_and_reload(&mut self, config: &Arc<Mutex<Config>>) {
        log::debug("persist_and_reload");
        self.validation_errors.clear();
        match self.draft.save() {
            Ok(()) => {
                *config.lock() = self.draft.clone();
                request_reload();
                match self.draft.validate() {
                    Ok(_) => {
                        self.status = "Binding saved and reloaded.".into();
                        log::debug("persist_and_reload: ok");
                    }
                    Err(errors) => {
                        self.validation_errors = errors;
                        self.status = "Saved and reloaded with validation warnings.".into();
                        log::warn(&format!(
                            "persist_and_reload: {} validation warnings",
                            self.validation_errors.len()
                        ));
                    }
                }
            }
            Err(err) => {
                log::error(&format!("persist_and_reload save failed: {err}"));
                self.status = format!("Failed to save config: {err}");
            }
        }
    }

    fn try_save(&mut self, config: &Arc<Mutex<Config>>) {
        log::debug("try_save");
        self.persist_and_reload(config);
    }

    fn binding_row(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        registrations: &std::collections::HashMap<String, HotkeyRegistrationResult>,
        capture_ui_used: &mut bool,
        config: &Arc<Mutex<Config>>,
    ) {
        let chord = self.chord_for(label);
        let capturing = self.capture_label.as_deref() == Some(label);
        let display_name = binding_display_name(label);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(220.0);
                ui.label(egui::RichText::new(display_name).size(13.5).strong());
                native_ui::muted_label(ui, label);
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if native_ui::small_button(ui, "Clear").clicked() {
                    self.finish_binding(label, String::new(), config);
                    *capture_ui_used = true;
                }

                let button_text = if capturing {
                    "Press keys…".to_string()
                } else if chord.trim().is_empty() {
                    "Click to bind".to_string()
                } else {
                    chord.clone()
                };

                let chord_button = egui::Button::new(
                    egui::RichText::new(button_text)
                        .monospace()
                        .strong()
                        .color(if capturing {
                            native_ui::ACCENT
                        } else if chord.trim().is_empty() {
                            native_ui::TEXT_DIM
                        } else {
                            egui::Color32::WHITE
                        }),
                )
                .fill(if capturing {
                    native_ui::ACCENT.gamma_multiply(0.15)
                } else {
                    egui::Color32::from_rgb(18, 20, 26)
                })
                .stroke(egui::Stroke::new(
                    if capturing { 1.5 } else { 1.0 },
                    if capturing {
                        native_ui::ACCENT
                    } else {
                        native_ui::BORDER
                    },
                ))
                .corner_radius(8)
                .min_size(egui::vec2(160.0, 34.0));

                if ui.add(chord_button).clicked() {
                    log::debug(&format!("start capture for {label}"));
                    self.cancel_capture();
                    self.capture_label = Some(label.to_string());
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
}

fn binding_display_name(label: &str) -> String {
    match label {
        "launcher" => "Open launcher".into(),
        "same_app_next" => "Next window (same app)".into(),
        "same_app_prev" => "Previous window (same app)".into(),
        "mark_next" => "Next mark slot".into(),
        "mark_prev" => "Previous mark slot".into(),
        "mark_toggle" => "Toggle mark on current window".into(),
        "marks_switcher" => "Marked windows switcher".into(),
        "mark_1" => "Set mark 1".into(),
        "mark_2" => "Set mark 2".into(),
        "mark_3" => "Set mark 3".into(),
        "mark_4" => "Set mark 4".into(),
        "mark_5" => "Set mark 5".into(),
        "mark_6" => "Set mark 6".into(),
        "mark_7" => "Set mark 7".into(),
        "mark_8" => "Set mark 8".into(),
        "mark_9" => "Set mark 9".into(),
        "jump_1" => "Jump to mark 1".into(),
        "jump_2" => "Jump to mark 2".into(),
        "jump_3" => "Jump to mark 3".into(),
        "jump_4" => "Jump to mark 4".into(),
        "jump_5" => "Jump to mark 5".into(),
        "jump_6" => "Jump to mark 6".into(),
        "jump_7" => "Jump to mark 7".into(),
        "jump_8" => "Jump to mark 8".into(),
        "jump_9" => "Jump to mark 9".into(),
        _ => label.to_string(),
    }
}

fn egui_key_to_vk(key: egui::Key) -> u32 {
    match key {
        egui::Key::A => 0x41,
        egui::Key::B => 0x42,
        egui::Key::C => 0x43,
        egui::Key::D => 0x44,
        egui::Key::E => 0x45,
        egui::Key::F => 0x46,
        egui::Key::G => 0x47,
        egui::Key::H => 0x48,
        egui::Key::I => 0x49,
        egui::Key::J => 0x4A,
        egui::Key::K => 0x4B,
        egui::Key::L => 0x4C,
        egui::Key::M => 0x4D,
        egui::Key::N => 0x4E,
        egui::Key::O => 0x4F,
        egui::Key::P => 0x50,
        egui::Key::Q => 0x51,
        egui::Key::R => 0x52,
        egui::Key::S => 0x53,
        egui::Key::T => 0x54,
        egui::Key::U => 0x55,
        egui::Key::V => 0x56,
        egui::Key::W => 0x57,
        egui::Key::X => 0x58,
        egui::Key::Y => 0x59,
        egui::Key::Z => 0x5A,
        egui::Key::Num0 => 0x30,
        egui::Key::Num1 => 0x31,
        egui::Key::Num2 => 0x32,
        egui::Key::Num3 => 0x33,
        egui::Key::Num4 => 0x34,
        egui::Key::Num5 => 0x35,
        egui::Key::Num6 => 0x36,
        egui::Key::Num7 => 0x37,
        egui::Key::Num8 => 0x38,
        egui::Key::Num9 => 0x39,
        egui::Key::F1 => 0x70,
        egui::Key::F2 => 0x71,
        egui::Key::F3 => 0x72,
        egui::Key::F4 => 0x73,
        egui::Key::F5 => 0x74,
        egui::Key::F6 => 0x75,
        egui::Key::F7 => 0x76,
        egui::Key::F8 => 0x77,
        egui::Key::F9 => 0x78,
        egui::Key::F10 => 0x79,
        egui::Key::F11 => 0x7A,
        egui::Key::F12 => 0x7B,
        egui::Key::Space => 0x20,
        egui::Key::Tab => 0x09,
        egui::Key::Backspace => 0x08,
        egui::Key::Enter => 0x0D,
        egui::Key::Escape => 0x1B,
        egui::Key::ArrowLeft => 0x25,
        egui::Key::ArrowUp => 0x26,
        egui::Key::ArrowRight => 0x27,
        egui::Key::ArrowDown => 0x28,
        egui::Key::OpenBracket => 0xDB,
        egui::Key::CloseBracket => 0xDD,
        egui::Key::Backtick => 0xC0,
        egui::Key::Minus => 0xBD,
        egui::Key::Equals => 0xBB,
        _ => 0,
    }
}
