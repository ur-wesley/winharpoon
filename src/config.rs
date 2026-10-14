use std::collections::{BTreeMap, HashMap};
use std::fs;

use serde::{Deserialize, Serialize};

use crate::hotkeys::HotkeyAction;
use crate::log;
use crate::paths;

mod chords;

pub use chords::{chord_from_vk_mods, parse_chord, parse_hold_chord, HoldChord, ParsedHotkey};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    pub hotkeys: HotkeysConfig,
    pub launcher: LauncherConfig,
    #[serde(default)]
    pub apps: AppsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_true")]
    pub check_updates_on_startup: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            autostart: false,
            check_updates_on_startup: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeysConfig {
    pub launcher: String,
    pub same_app_next: String,
    pub same_app_prev: String,
    pub mark_next: String,
    pub mark_prev: String,
    #[serde(default = "default_marks_switcher")]
    pub marks_switcher: String,
    #[serde(default = "default_marks_switcher_next")]
    pub marks_switcher_next: String,
    #[serde(default = "default_marks_switcher_prev")]
    pub marks_switcher_prev: String,
    #[serde(default = "default_mark_toggle")]
    pub mark_toggle: String,
    #[serde(default)]
    pub mark: BTreeMap<String, String>,
    #[serde(default)]
    pub jump: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    pub width: f32,
    pub height: f32,
    pub max_results: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleTapModifier {
    Alt,
    Ctrl,
    Shift,
    Win,
}

impl DoubleTapModifier {
    pub fn parse(s: &str) -> Self {
        match s {
            "Ctrl" => Self::Ctrl,
            "Shift" => Self::Shift,
            "Win" => Self::Win,
            _ => Self::Alt,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alt => "Alt",
            Self::Ctrl => "Ctrl",
            Self::Shift => "Shift",
            Self::Win => "Win",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub alt_double_click: bool,
    #[serde(default = "default_apps_scope")]
    pub alt_double_click_scope: String,
    #[serde(default = "default_double_tap_key")]
    pub double_tap_key: String,
    #[serde(default = "default_apps_width")]
    pub width: f32,
    #[serde(default = "default_apps_height")]
    pub height: f32,
    #[serde(default = "default_apps_max_results")]
    pub max_results: usize,
}

impl AppsConfig {
    pub fn normalize(&mut self) {
        if self.alt_double_click_scope == "desktop_only" {
            self.alt_double_click_scope = "not_fullscreen".into();
        }
        if self.alt_double_click_scope != "not_fullscreen" {
            self.alt_double_click_scope = "anywhere".into();
        }
        self.double_tap_key = DoubleTapModifier::parse(&self.double_tap_key)
            .as_str()
            .to_string();
    }

    pub fn blocks_in_fullscreen(&self) -> bool {
        self.alt_double_click_scope == "not_fullscreen"
    }

    pub fn double_tap_modifier(&self) -> DoubleTapModifier {
        DoubleTapModifier::parse(&self.double_tap_key)
    }
}

fn default_true() -> bool {
    true
}

fn default_apps_scope() -> String {
    "anywhere".into()
}

fn default_double_tap_key() -> String {
    "Alt".into()
}

fn default_apps_width() -> f32 {
    440.0
}

fn default_apps_height() -> f32 {
    420.0
}

fn default_apps_max_results() -> usize {
    16
}

impl Default for AppsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            alt_double_click: true,
            alt_double_click_scope: default_apps_scope(),
            double_tap_key: default_double_tap_key(),
            width: default_apps_width(),
            height: default_apps_height(),
            max_results: default_apps_max_results(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub action: HotkeyAction,
    pub label: String,
    pub chord: String,
    pub parsed: Option<ParsedHotkey>,
}

#[derive(Debug, Clone)]
pub enum ConfigValidationError {
    DuplicateBinding {
        chord: String,
        first: String,
        second: String,
    },
    InvalidChord {
        label: String,
        chord: String,
        reason: String,
    },
}

fn default_marks_switcher() -> String {
    "Win+Alt+M".into()
}

fn default_marks_switcher_next() -> String {
    "Win+Alt+Right".into()
}

fn default_marks_switcher_prev() -> String {
    "Win+Alt+Left".into()
}

fn default_mark_toggle() -> String {
    "Win+Alt+Shift+M".into()
}

impl Default for Config {
    fn default() -> Self {
        let mut mark = BTreeMap::new();
        let mut jump = BTreeMap::new();
        for i in 1..=9 {
            mark.insert(i.to_string(), format!("Win+Alt+Shift+{i}"));
            jump.insert(i.to_string(), format!("Win+Alt+{i}"));
        }
        Self {
            general: GeneralConfig::default(),
            hotkeys: HotkeysConfig {
                launcher: "Win+K".into(),
                same_app_next: "Win+Alt+Grave".into(),
                same_app_prev: "Win+Alt+Shift+Grave".into(),
                mark_next: "Win+Alt+Period".into(),
                mark_prev: "Win+Alt+Comma".into(),
                marks_switcher: default_marks_switcher(),
                marks_switcher_next: default_marks_switcher_next(),
                marks_switcher_prev: default_marks_switcher_prev(),
                mark_toggle: default_mark_toggle(),
                mark,
                jump,
            },
            launcher: LauncherConfig {
                width: 440.0,
                height: 360.0,
                max_results: 12,
            },
            apps: AppsConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        paths::ensure_app_data();
        let path = paths::config_path();
        log::debug(format!("Config::load from {}", path.display()));
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(text) => match toml::from_str::<Self>(&text) {
                    Ok(mut cfg) => {
                        cfg.normalize();
                        log::debug("config loaded from disk");
                        return cfg;
                    }
                    Err(err) => {
                        // Usability: never silently destroy user config.
                        // Backup corrupt file and keep running on defaults.
                        log::error(format!("config parse error: {err}"));
                        paths::backup_corrupt_file(&path);
                        log::notify(
                            "WinHarpoon config error",
                            "Config was invalid — backup saved, using defaults.",
                        );
                    }
                },
                Err(err) => log::error(format!("config read error: {err}")),
            }
        } else {
            log::debug("config file missing, creating defaults");
        }
        let cfg = Self::default();
        // Only write defaults when no file existed; never overwrite a corrupt file we just backed up.
        if !path.exists() {
            let _ = cfg.save();
        }
        cfg
    }

    // Test stub returns Ok without touching self/disk; the real impl below uses both.
    #[cfg_attr(test, allow(clippy::unused_self, clippy::unnecessary_wraps))]
    pub fn save(&self) -> std::io::Result<()> {
        #[cfg(test)]
        {
            Ok(())
        }
        #[cfg(not(test))]
        {
            paths::ensure_app_data();
            let path = paths::config_path();
            log::debug(format!("Config::save to {}", path.display()));
            let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
            paths::atomic_write(&path, &text)
        }
    }

    pub fn normalize(&mut self) {
        self.apps.normalize();
        // Usability: clamp launcher geometry so a bad TOML edit can't trap focus in a 0px window.
        if !self.launcher.width.is_finite() || self.launcher.width < 200.0 {
            self.launcher.width = 200.0;
        } else if self.launcher.width > 1600.0 {
            self.launcher.width = 1600.0;
        }
        if !self.launcher.height.is_finite() || self.launcher.height < 120.0 {
            self.launcher.height = 120.0;
        } else if self.launcher.height > 1200.0 {
            self.launcher.height = 1200.0;
        }
        if self.launcher.max_results == 0 {
            self.launcher.max_results = 12;
        } else if self.launcher.max_results > 100 {
            self.launcher.max_results = 100;
        }
        if !self.apps.width.is_finite() || self.apps.width < 200.0 {
            self.apps.width = 440.0;
        } else if self.apps.width > 1600.0 {
            self.apps.width = 1600.0;
        }
        if !self.apps.height.is_finite() || self.apps.height < 120.0 {
            self.apps.height = 420.0;
        } else if self.apps.height > 1200.0 {
            self.apps.height = 1200.0;
        }
        if self.apps.max_results == 0 {
            self.apps.max_results = 16;
        } else if self.apps.max_results > 100 {
            self.apps.max_results = 100;
        }
    }

    pub fn bindings(&self) -> Vec<HotkeyBinding> {
        let mut out = vec![
            binding("launcher", &self.hotkeys.launcher, HotkeyAction::Launcher),
            binding(
                "same_app_next",
                &self.hotkeys.same_app_next,
                HotkeyAction::SameAppNext,
            ),
            binding(
                "same_app_prev",
                &self.hotkeys.same_app_prev,
                HotkeyAction::SameAppPrev,
            ),
            binding("mark_next", &self.hotkeys.mark_next, HotkeyAction::MarkNext),
            binding("mark_prev", &self.hotkeys.mark_prev, HotkeyAction::MarkPrev),
            binding(
                "mark_toggle",
                &self.hotkeys.mark_toggle,
                HotkeyAction::ToggleMark,
            ),
            binding(
                "marks_switcher_next",
                &self.hotkeys.marks_switcher_next,
                HotkeyAction::MarksSwitcherNext,
            ),
            binding(
                "marks_switcher_prev",
                &self.hotkeys.marks_switcher_prev,
                HotkeyAction::MarksSwitcherPrev,
            ),
        ];
        for slot in 1..=9 {
            let key = slot.to_string();
            if let Some(chord) = self.hotkeys.mark.get(&key) {
                out.push(binding(
                    &format!("mark_{slot}"),
                    chord,
                    HotkeyAction::Mark(slot),
                ));
            }
            if let Some(chord) = self.hotkeys.jump.get(&key) {
                out.push(binding(
                    &format!("jump_{slot}"),
                    chord,
                    HotkeyAction::Jump(slot),
                ));
            }
        }
        out
    }

    pub fn validate(&self) -> Result<Vec<HotkeyBinding>, Vec<ConfigValidationError>> {
        validate_bindings(&self.bindings())
    }

    pub fn validate_merged(
        &self,
        extra: &[HotkeyBinding],
    ) -> Result<Vec<HotkeyBinding>, Vec<ConfigValidationError>> {
        let mut bindings = self.bindings();
        bindings.extend(extra.iter().cloned());
        validate_bindings(&bindings)
    }

    pub fn set_binding_chord(&mut self, label: &str, chord: String) {
        log::debug(format!("set_binding_chord {label} -> {chord}"));
        match label {
            "launcher" => self.hotkeys.launcher = chord,
            "same_app_next" => self.hotkeys.same_app_next = chord,
            "same_app_prev" => self.hotkeys.same_app_prev = chord,
            "mark_next" => self.hotkeys.mark_next = chord,
            "mark_prev" => self.hotkeys.mark_prev = chord,
            "marks_switcher" => self.hotkeys.marks_switcher = chord,
            "marks_switcher_next" => self.hotkeys.marks_switcher_next = chord,
            "marks_switcher_prev" => self.hotkeys.marks_switcher_prev = chord,
            "mark_toggle" => self.hotkeys.mark_toggle = chord,
            _ if label.starts_with("mark_") => {
                if let Some(slot) = label.strip_prefix("mark_") {
                    if chord.trim().is_empty() {
                        self.hotkeys.mark.remove(slot);
                    } else {
                        self.hotkeys.mark.insert(slot.to_string(), chord);
                    }
                }
            }
            _ if label.starts_with("jump_") => {
                if let Some(slot) = label.strip_prefix("jump_") {
                    if chord.trim().is_empty() {
                        self.hotkeys.jump.remove(slot);
                    } else {
                        self.hotkeys.jump.insert(slot.to_string(), chord);
                    }
                }
            }
            _ => {
                log::debug(format!("set_binding_chord: unknown label {label}"));
            }
        }
    }
}

fn validate_bindings(
    bindings: &[HotkeyBinding],
) -> Result<Vec<HotkeyBinding>, Vec<ConfigValidationError>> {
    log::debug(format!("validate_bindings {} bindings", bindings.len()));
    let mut errors = Vec::new();
    let mut seen: HashMap<ParsedHotkey, String> = HashMap::new();

    for b in bindings {
        let Some(parsed) = &b.parsed else {
            if !b.chord.trim().is_empty() {
                errors.push(ConfigValidationError::InvalidChord {
                    label: b.label.clone(),
                    chord: b.chord.clone(),
                    reason: "could not parse chord".into(),
                });
            }
            continue;
        };
        if let Some(first) = seen.get(parsed) {
            errors.push(ConfigValidationError::DuplicateBinding {
                chord: b.chord.clone(),
                first: first.clone(),
                second: b.label.clone(),
            });
        } else {
            seen.insert(parsed.clone(), b.label.clone());
        }
        if chords::is_windows_reserved(&b.chord) {
            errors.push(ConfigValidationError::InvalidChord {
                label: b.label.clone(),
                chord: b.chord.clone(),
                reason: "Windows-reserved chord (OS will swallow it)".into(),
            });
        }
    }

    if errors.is_empty() {
        log::debug("binding validation ok");
        Ok(bindings.to_vec())
    } else {
        log::warn(format!("binding validation: {} errors", errors.len()));
        Err(errors)
    }
}

fn binding(label: &str, chord: &str, action: HotkeyAction) -> HotkeyBinding {
    let trimmed = chord.trim();
    let parsed = if trimmed.is_empty() {
        None
    } else {
        match parse_chord(trimmed) {
            Ok(p) => Some(p),
            Err(e) => {
                log::debug(format!("binding {label}: parse error for {trimmed}: {e}"));
                None
            }
        }
    };
    HotkeyBinding {
        action,
        label: label.to_string(),
        chord: chord.to_string(),
        parsed,
    }
}

#[cfg(test)]
mod tests {
    use super::{AppsConfig, Config, DoubleTapModifier};

    #[test]
    fn default_bindings_are_unique_and_parseable() {
        let cfg = Config::default();
        let bindings = cfg.validate().expect("default hotkeys must not collide");
        assert_eq!(bindings.len(), 26);
        assert!(bindings.iter().all(|b| b.parsed.is_some()));
    }

    #[test]
    fn apps_config_defaults_double_tap_key_to_alt() {
        let apps = AppsConfig::default();
        assert_eq!(apps.double_tap_key, "Alt");
        assert_eq!(apps.double_tap_modifier(), DoubleTapModifier::Alt);
    }

    #[test]
    fn apps_config_normalizes_legacy_desktop_only_scope() {
        let mut apps = AppsConfig {
            alt_double_click_scope: "desktop_only".into(),
            ..AppsConfig::default()
        };
        apps.normalize();
        assert_eq!(apps.alt_double_click_scope, "not_fullscreen");
        assert!(apps.blocks_in_fullscreen());
    }

    #[test]
    fn apps_config_clamps_invalid_double_tap_key() {
        let mut apps = AppsConfig {
            double_tap_key: "F12".into(),
            ..AppsConfig::default()
        };
        apps.normalize();
        assert_eq!(apps.double_tap_key, "Alt");
    }

    #[test]
    fn reserved_chord_is_validation_error() {
        let mut cfg = Config::default();
        cfg.hotkeys.launcher = "Win+L".into();
        let err = cfg.validate().expect_err("Win+L must be rejected");
        assert!(err.iter().any(|e| format!("{e:?}").contains("launcher")));
    }

    #[test]
    fn normalize_clamps_geometry_and_results() {
        let mut cfg = Config::default();
        cfg.launcher.width = 0.0;
        cfg.launcher.height = f32::INFINITY;
        cfg.launcher.max_results = 0;
        cfg.apps.width = 5000.0;
        cfg.apps.max_results = 500;
        cfg.normalize();
        assert!((cfg.launcher.width - 200.0).abs() < f32::EPSILON);
        assert!((cfg.launcher.height - 120.0).abs() < f32::EPSILON);
        assert_eq!(cfg.launcher.max_results, 12);
        assert!((cfg.apps.width - 1600.0).abs() < f32::EPSILON);
        assert_eq!(cfg.apps.max_results, 100);
    }

    #[test]
    fn set_binding_chord_empty_removes_slot() {
        let mut cfg = Config::default();
        assert!(cfg.hotkeys.mark.contains_key("1"));
        cfg.set_binding_chord("mark_1", String::new());
        assert!(!cfg.hotkeys.mark.contains_key("1"));
    }
}
