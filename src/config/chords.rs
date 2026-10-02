//! Hotkey chord parsing and formatting (`Win+Alt+M` <-> `VIRTUAL_KEY` + modifiers).
//!
//! Split from `config.rs` to keep modules under the 600-line gate. The parent
//! module re-exports the public API, so `crate::config::parse_chord` paths
//! keep working.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VIRTUAL_KEY,
};

use crate::log;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParsedHotkey {
    pub modifiers: u32,
    pub vk: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldChord {
    pub hold_modifiers: u32,
    pub trigger_vk: u16,
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
        if let Some(c) = upper.chars().next() {
            if c.is_ascii_alphanumeric() {
                let code = u32::from(c);
                return u16::try_from(code).map_err(|_| format!("bad key (out of range): {part}"));
            }
        }
    }
    if upper.len() == 2 && upper.chars().all(|c| c.is_ascii_digit()) {
        let n: u8 = upper
            .parse()
            .map_err(|_| format!("bad digit key: {part}"))?;
        if (1..=9).contains(&n) {
            return Ok(0x30_u16.saturating_add(u16::from(n)));
        }
    }
    if upper.len() == 2 && upper.starts_with('F') {
        let digits = upper
            .get(1..)
            .ok_or_else(|| format!("bad function key: {part}"))?;
        let n: u8 = digits
            .parse()
            .map_err(|_| format!("bad function key: {part}"))?;
        if (1..=24).contains(&n) {
            return Ok(0x70_u16.saturating_add(u16::from(n).saturating_sub(1)));
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

fn format_vk(vk: u16) -> String {
    if (0x30..=0x39).contains(&vk) {
        return char::from_u32(u32::from(vk)).unwrap_or('?').to_string();
    }
    if (0x41..=0x5A).contains(&vk) {
        return char::from_u32(u32::from(vk)).unwrap_or('?').to_string();
    }
    if (0x70..=0x87).contains(&vk) {
        return format!("F{}", vk.saturating_sub(0x70).saturating_add(1));
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

fn format_modifiers(mods: u32) -> String {
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

pub(super) fn is_windows_reserved(chord: &str) -> bool {
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
    use super::parse_chord;

    #[test]
    fn parse_chord_rejects_multiple_keys() {
        assert!(parse_chord("Ctrl+A+B").is_err());
    }

    #[test]
    fn parse_chord_rejects_missing_key() {
        assert!(parse_chord("Win+Alt").is_err());
    }

    #[test]
    fn parse_chord_rejects_unknown_key() {
        assert!(parse_chord("Win+Frobnicate").is_err());
    }
}
