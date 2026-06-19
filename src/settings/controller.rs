use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::app::AppState;
use crate::config::{Config, ConfigValidationError};
use crate::hotkeys::HotkeyRegistrationResult;
use crate::log;
use crate::settings::service::SettingsService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsAction {
    ResetDefaults,
    Save,
    SetAutostart(bool),
    SetAltDoubleClick(bool),
    SetAltDoubleClickScope(String),
    RebuildAppIndex,
    StartCapture(String),
    CancelCapture,
    FinishCapture { label: String, chord: String },
    ClearBinding(String),
}

pub struct SettingsController {
    pub visible: bool,
    pub draft: Config,
    pub capture_label: Option<String>,
    pub validation_errors: Vec<ConfigValidationError>,
    pub status: String,
}

impl SettingsController {
    pub fn new(config: &Config) -> Self {
        Self {
            visible: false,
            draft: config.clone(),
            capture_label: None,
            validation_errors: Vec::new(),
            status: String::new(),
        }
    }

    pub fn on_show(&mut self, config: &Config) {
        log::debug("settings show");
        self.visible = true;
        self.draft = config.clone();
        self.capture_label = None;
        self.validation_errors.clear();
        self.status.clear();
    }

    pub fn on_hide(&mut self) {
        if !self.visible {
            return;
        }
        log::debug("settings hide");
        self.visible = false;
        self.cancel_capture();
    }

    pub fn cancel_capture(&mut self) {
        if self.capture_label.is_none() {
            return;
        }
        log::debug(format!("cancel_capture: {:?}", self.capture_label));
        self.capture_label = None;
        crate::util::release_stuck_modifier_keys();
    }

    pub fn handle_action(
        &mut self,
        action: SettingsAction,
        config: &Arc<Mutex<Config>>,
    ) {
        match action {
            SettingsAction::ResetDefaults => {
                log::debug("settings reset defaults");
                self.draft = Config::default();
                self.persist_and_reload(config);
            }
            SettingsAction::Save => {
                log::debug("try_save");
                self.persist_and_reload(config);
            }
            SettingsAction::SetAutostart(enabled) => {
                self.draft.general.autostart = enabled;
                match SettingsService::apply_autostart(enabled) {
                    Ok(()) => {
                        if let Ok(status) = SettingsService::persist_general(&self.draft, config) {
                            self.validation_errors.clear();
                            self.status = status;
                            log::debug(format!(
                                "general settings saved (autostart={})",
                                self.draft.general.autostart
                            ));
                        }
                    }
                    Err(err) => {
                        self.draft.general.autostart = !enabled;
                        self.status = format!("Failed to update autostart: {err}");
                        log::error(format!("autostart toggle failed: {err}"));
                    }
                }
            }
            SettingsAction::SetAltDoubleClick(enabled) => {
                self.draft.apps.alt_double_click = enabled;
                self.persist_apps(config);
            }
            SettingsAction::SetAltDoubleClickScope(scope) => {
                self.draft.apps.alt_double_click_scope = scope;
                self.persist_apps(config);
            }
            SettingsAction::RebuildAppIndex => {
                crate::apps::refresh_async();
                self.status = "Rebuilding app index…".into();
            }
            SettingsAction::StartCapture(label) => {
                log::debug(format!("start capture for {label}"));
                self.cancel_capture();
                self.capture_label = Some(label);
            }
            SettingsAction::CancelCapture => {
                self.cancel_capture();
            }
            SettingsAction::FinishCapture { label, chord } => {
                log::debug(format!("capture binding {label} -> {chord}"));
                self.apply_binding(&label, chord, config);
                self.cancel_capture();
            }
            SettingsAction::ClearBinding(label) => {
                self.finish_binding(&label, String::new(), config);
            }
        }
    }

    pub fn registration_map(
        &self,
        state: &Arc<Mutex<AppState>>,
    ) -> HashMap<String, HotkeyRegistrationResult> {
        state
            .lock()
            .registration_results
            .iter()
            .map(|r| (r.label.clone(), r.clone()))
            .collect()
    }

    pub fn chord_for(&self, label: &str) -> String {
        match label {
            "launcher" => self.draft.hotkeys.launcher.clone(),
            "same_app_next" => self.draft.hotkeys.same_app_next.clone(),
            "same_app_prev" => self.draft.hotkeys.same_app_prev.clone(),
            "mark_next" => self.draft.hotkeys.mark_next.clone(),
            "mark_prev" => self.draft.hotkeys.mark_prev.clone(),
            "mark_toggle" => self.draft.hotkeys.mark_toggle.clone(),
            "marks_switcher" => self.draft.hotkeys.marks_switcher.clone(),
            "marks_switcher_next" => self.draft.hotkeys.marks_switcher_next.clone(),
            "marks_switcher_prev" => self.draft.hotkeys.marks_switcher_prev.clone(),
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

    pub fn binding_sections() -> Vec<BindingSection> {
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
                subtitle: "Hold to open the overlay, use next/prev to cycle, release to confirm",
                labels: vec!["marks_switcher", "marks_switcher_next", "marks_switcher_prev"],
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

    fn apply_binding(&mut self, label: &str, chord: String, config: &Arc<Mutex<Config>>) {
        log::debug(format!("apply_binding {label} -> {chord}"));
        self.draft.set_binding_chord(label, chord);
        self.persist_and_reload(config);
    }

    fn finish_binding(&mut self, label: &str, chord: String, config: &Arc<Mutex<Config>>) {
        self.apply_binding(label, chord, config);
        self.cancel_capture();
    }

    fn persist_apps(&mut self, config: &Arc<Mutex<Config>>) {
        match SettingsService::persist_apps(&self.draft, config) {
            Ok(status) => self.status = status,
            Err(err) => self.status = format!("Failed to save config: {err}"),
        }
    }

    fn persist_and_reload(&mut self, config: &Arc<Mutex<Config>>) {
        log::debug("persist_and_reload");
        self.validation_errors.clear();
        match SettingsService::persist_and_reload(&self.draft, config) {
            Ok((status, errors)) => {
                self.status = status;
                self.validation_errors = errors;
                if self.validation_errors.is_empty() {
                    log::debug("persist_and_reload: ok");
                } else {
                    log::warn(format!(
                        "persist_and_reload: {} validation warnings",
                        self.validation_errors.len()
                    ));
                }
            }
            Err(err) => {
                log::error(format!("persist_and_reload save failed: {err}"));
                self.status = format!("Failed to save config: {err}");
            }
        }
    }
}

pub struct BindingSection {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub labels: Vec<&'static str>,
}

pub fn binding_display_name(label: &str) -> String {
    match label {
        "launcher" => "Open launcher".into(),
        "same_app_next" => "Next window (same app)".into(),
        "same_app_prev" => "Previous window (same app)".into(),
        "mark_next" => "Next mark slot".into(),
        "mark_prev" => "Previous mark slot".into(),
        "mark_toggle" => "Toggle mark on current window".into(),
        "marks_switcher" => "Marked windows switcher".into(),
        "marks_switcher_next" => "Next marked window (overlay)".into(),
        "marks_switcher_prev" => "Previous marked window (overlay)".into(),
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
