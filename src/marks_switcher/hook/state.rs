//! Switcher hook state: bindings, tracked modifiers, and the session.
//!
//! Split from `hook/mod.rs` to keep modules under the 600-line gate.

use std::collections::HashSet;
use std::sync::atomic::Ordering;

use windows::Win32::UI::Input::KeyboardAndMouse::MOD_NOREPEAT;

use crate::config::{parse_chord, Config, HoldChord};
use crate::log;
use crate::modes::marks::{MarkEntry, SharedMarks};

#[derive(Debug, Clone)]
pub(super) struct JumpBinding {
    pub(super) modifiers: u32,
    pub(super) vk: u16,
    pub(super) slot: u8,
}

#[derive(Debug, Clone, Default)]
pub(super) struct TrackedModifiers {
    pub(super) alt: bool,
    pub(super) ctrl: bool,
    pub(super) shift: bool,
    pub(super) win: bool,
}

#[derive(Debug, Clone)]
pub(super) struct SwitcherNavBinding {
    pub(super) modifiers: u32,
    pub(super) vk: u16,
    pub(super) delta: i32,
}

#[derive(Debug, Clone)]
pub(super) struct LauncherBinding {
    pub(super) modifiers: u32,
    pub(super) vk: u16,
}

pub(super) struct HookState {
    pub(super) chord: HoldChord,
    pub(super) marks: SharedMarks,
    pub(super) jump_bindings: Vec<JumpBinding>,
    pub(super) switcher_nav_bindings: Vec<SwitcherNavBinding>,
    pub(super) launcher_binding: Option<LauncherBinding>,
    pub(super) jump_keys_down: HashSet<u32>,
    pub(super) switcher_nav_keys_down: HashSet<u32>,
    pub(super) mods: TrackedModifiers,
    pub(super) active: bool,
    pub(super) ignore_trigger_down: bool,
    pub(super) trigger_released_since_activate: bool,
    pub(super) last_trigger_down: bool,
    pub(super) entries: Vec<MarkEntry>,
    pub(super) selected: usize,
}

pub(super) fn jump_bindings_from_config(config: &Config) -> Vec<JumpBinding> {
    let mut out = Vec::new();
    for slot in 1..=9 {
        let Some(chord) = config.hotkeys.jump.get(&slot.to_string()) else {
            continue;
        };
        let trimmed = chord.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(parsed) = parse_chord(trimmed) else {
            log::warn(format!("jump_{slot} chord parse failed: {trimmed}"));
            continue;
        };
        out.push(JumpBinding {
            modifiers: parsed.modifiers & !MOD_NOREPEAT.0,
            vk: parsed.vk,
            slot,
        });
    }
    out
}

pub(super) fn switcher_nav_bindings_from_config(config: &Config) -> Vec<SwitcherNavBinding> {
    let mut out = Vec::new();
    for (chord, delta) in [
        (&config.hotkeys.marks_switcher_next, 1),
        (&config.hotkeys.marks_switcher_prev, -1),
    ] {
        let trimmed = chord.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(parsed) = parse_chord(trimmed) else {
            log::warn(format!("marks_switcher nav chord parse failed: {trimmed}"));
            continue;
        };
        out.push(SwitcherNavBinding {
            modifiers: parsed.modifiers & !MOD_NOREPEAT.0,
            vk: parsed.vk,
            delta,
        });
    }
    out
}

pub(super) fn sync_chord_atomics(chord: &HoldChord) {
    super::TRIGGER_VK.store(u32::from(chord.trigger_vk), Ordering::Relaxed);
    super::HOLD_MODS.store(chord.hold_modifiers, Ordering::Relaxed);
}
