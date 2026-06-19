use std::collections::{BTreeMap, HashMap};
use std::fs;

use serde::{Deserialize, Serialize};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VIRTUAL_KEY,
};

use crate::hotkeys::HotkeyAction;
use crate::log;
use crate::paths;

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
#[derive(Default)]
pub struct GeneralConfig {
    #[serde(default)]
    pub autostart: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub alt_double_click: bool,
    #[serde(default = "default_apps_scope")]
    pub alt_double_click_scope: String,
    #[serde(default = "default_apps_width")]
    pub width: f32,
    #[serde(default = "default_apps_height")]
    pub height: f32,
    #[serde(default = "default_apps_max_results")]
    pub max_results: usize,
}

fn default_true() -> bool {
    true
}

fn default_apps_scope() -> String {
    "anywhere".into()
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
            width: default_apps_width(),
            height: default_apps_height(),
            max_results: default_apps_max_results(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParsedHotkey {
    pub modifiers: u32,
    pub vk: u16,
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
    DuplicateBinding { chord: String, first: String, second: String },
    InvalidChord { label: String, chord: String, reason: String },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldChord {
    pub hold_modifiers: u32,
    pub trigger_vk: u16,
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
                Ok(text) => match toml::from_str(&text) {
                    Ok(cfg) => {
                        log::debug("config loaded from disk");
                        return cfg;
                    }
                    Err(err) => log::error(format!("config parse error: {err}")),
                },
                Err(err) => log::error(format!("config read error: {err}")),
            }
        } else {
            log::debug("config file missing, creating defaults");
        }
        let cfg = Self::default();
        let _ = cfg.save();
        cfg
    }

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
            let text = toml::to_string_pretty(self).expect("serialize config");
            fs::write(path, text)
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
            binding("mark_toggle", &self.hotkeys.mark_toggle, HotkeyAction::ToggleMark),
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
        if is_windows_reserved(&b.chord) {
            log::warn(format!(
                "warning: {} uses a Windows-reserved chord {}",
                b.label, b.chord
            ));
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

pub fn parse_chord(input: &str) -> Result<ParsedHotkey, String> {
    log::trace(format!("parse_chord: {input}"));
    let mut modifiers = MOD_NOREPEAT;
    let mut vk: Option<u16> = None;

    for part in input.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let upper = part.to_ascii_uppercase();
        match upper.as_str() {
            "WIN" | "SUPER" | "META" => modifiers |= MOD_WIN,
            "ALT" => modifiers |= MOD_ALT,
            "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
            "SHIFT" => modifiers |= MOD_SHIFT,
            _ => {
                if vk.is_some() {
                    return Err(format!("multiple keys in chord: {input}"));
                }
                vk = Some(parse_vk(part)?);
            }
        }
    }

    let vk = vk.ok_or_else(|| format!("missing key in chord: {input}"))?;
    let parsed = ParsedHotkey {
        modifiers: modifiers.0,
        vk,
    };
    log::trace(format!(
        "parse_chord ok: {input} -> mods=0x{:X} vk=0x{:X}",
        parsed.modifiers, parsed.vk
    ));
    Ok(parsed)
}

pub fn parse_hold_chord(input: &str) -> Result<HoldChord, String> {
    let parsed = parse_chord(input)?;
    let hold_modifiers = parsed.modifiers & !MOD_NOREPEAT.0;
    Ok(HoldChord {
        hold_modifiers,
        trigger_vk: parsed.vk,
    })
}

fn parse_vk(part: &str) -> Result<u16, String> {
    let upper = part.to_ascii_uppercase();
    if upper.len() == 1 {
        let c = upper.chars().next().unwrap();
        if c.is_ascii_alphanumeric() {
            return Ok(c as u16);
        }
    }
    if upper.len() == 2 && upper.chars().all(|c| c.is_ascii_digit()) {
        let n: u8 = upper.parse().map_err(|_| format!("bad digit key: {part}"))?;
        if (1..=9).contains(&n) {
            return Ok(0x30 + n as u16);
        }
    }
    if upper.len() == 2 && upper.starts_with('F') {
        let n: u8 = upper[1..]
            .parse()
            .map_err(|_| format!("bad function key: {part}"))?;
        if (1..=24).contains(&n) {
            return Ok(0x70 + (n as u16 - 1));
        }
    }

    let mapped = match upper.as_str() {
        "GRAVE" | "`" | "OEM_3" => 0xC0,
        "MINUS" | "DASH" | "-" => 0xBD,
        "EQUAL" | "=" => 0xBB,
        "LBRACKET" | "BRACKETLEFT" | "[" => 0xDB,
        "RBRACKET" | "BRACKETRIGHT" | "]" => 0xDD,
        "BACKSLASH" | "\\" => 0xDC,
        "SEMICOLON" | ";" => 0xBA,
        "QUOTE" | "'" => 0xDE,
        "COMMA" | "," => 0xBC,
        "PERIOD" | "." => 0xBE,
        "SLASH" | "/" => 0xBF,
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ESCAPE" | "ESC" => 0x1B,
        "BACK" | "BACKSPACE" => 0x08,
        "RETURN" | "ENTER" => 0x0D,
        "INSERT" => 0x2D,
        "DELETE" => 0x2E,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        _ => return Err(format!("unknown key: {part}")),
    };
    Ok(mapped)
}

pub fn format_vk(vk: u16) -> String {
    if (0x30..=0x39).contains(&vk) {
        return ((vk as u8) as char).to_string();
    }
    if (0x41..=0x5A).contains(&vk) {
        return ((vk as u8) as char).to_string();
    }
    if (0x70..=0x87).contains(&vk) {
        return format!("F{}", vk - 0x70 + 1);
    }
    match vk {
        0xC0 => "Grave".into(),
        0xBD => "Minus".into(),
        0xBB => "Equal".into(),
        0xDB => "BracketLeft".into(),
        0xDD => "BracketRight".into(),
        0x20 => "Space".into(),
        0x09 => "Tab".into(),
        0x1B => "Esc".into(),
        _ => format!("VK_{vk:04X}"),
    }
}

pub fn format_modifiers(mods: u32) -> String {
    let mods = HOT_KEY_MODIFIERS(mods);
    let mut parts = Vec::new();
    if (mods & MOD_WIN).0 != 0 {
        parts.push("Win");
    }
    if (mods & MOD_CONTROL).0 != 0 {
        parts.push("Ctrl");
    }
    if (mods & MOD_ALT).0 != 0 {
        parts.push("Alt");
    }
    if (mods & MOD_SHIFT).0 != 0 {
        parts.push("Shift");
    }
    parts.join("+")
}

fn is_windows_reserved(chord: &str) -> bool {
    matches!(
        chord.to_ascii_uppercase().as_str(),
        "WIN+L" | "WIN+D" | "WIN+E" | "WIN+R" | "WIN+I" | "WIN+X" | "CTRL+ALT+DEL"
    )
}

pub fn chord_from_vk_mods(vk: VIRTUAL_KEY, mods: u32) -> String {
    let key = format_vk(vk.0);
    let prefix = format_modifiers(mods);
    if prefix.is_empty() {
        key
    } else {
        format!("{prefix}+{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn default_bindings_are_unique_and_parseable() {
        let cfg = Config::default();
        let bindings = cfg.validate().expect("default hotkeys must not collide");
        assert_eq!(bindings.len(), 26);
        assert!(bindings.iter().all(|b| b.parsed.is_some()));
    }
}
