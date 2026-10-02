//! Physical modifier tracking for the switcher hook.
//!
//! Split from `hook/mod.rs` to keep modules under the 600-line gate. All
//! key-state queries go through `util::is_vk_down` / `util::any_vk_down`.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
    VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};

use crate::config::HoldChord;
use crate::util::{any_vk_down, is_vk_down};

use super::state::{HookState, TrackedModifiers};

pub(super) fn trigger_physically_down(chord: &HoldChord) -> bool {
    is_vk_down(chord.trigger_vk)
}

pub(super) fn update_tracked_modifiers(mods: &mut TrackedModifiers, vk: u32, key_up: bool) {
    let down = !key_up;
    match vk {
        0x12 | 0xA4 | 0xA5 => mods.alt = down,
        0x11 | 0xA2 | 0xA3 => mods.ctrl = down,
        0x10 | 0xA0 | 0xA1 => mods.shift = down,
        0x5B | 0x5C => mods.win = down,
        _ => {}
    }
}

pub(super) fn tracked_mods_match(mods: &TrackedModifiers, required: u32) -> bool {
    if (required & MOD_WIN.0) != 0 && !mods.win {
        return false;
    }
    if (required & MOD_ALT.0) != 0 && !mods.alt {
        return false;
    }
    if (required & MOD_SHIFT.0) != 0 && !mods.shift {
        return false;
    }
    if (required & MOD_CONTROL.0) != 0 && !mods.ctrl {
        return false;
    }
    true
}

pub(super) fn tracked_extra_mods(mods: &TrackedModifiers, required: u32) -> bool {
    if (required & MOD_SHIFT.0) == 0 && mods.shift {
        return true;
    }
    if (required & MOD_CONTROL.0) == 0 && mods.ctrl {
        return true;
    }
    false
}

pub(super) fn sync_tracked_modifiers(mods: &mut TrackedModifiers) {
    mods.alt = any_vk_down(&[VK_MENU.0, VK_LMENU.0, VK_RMENU.0]);
    mods.ctrl = any_vk_down(&[VK_CONTROL.0, VK_LCONTROL.0, VK_RCONTROL.0]);
    mods.shift = any_vk_down(&[VK_SHIFT.0, VK_LSHIFT.0, VK_RSHIFT.0]);
    mods.win = any_vk_down(&[VK_LWIN.0, VK_RWIN.0]);
}

pub(super) fn hold_mods_match_modifiers(modifiers: u32) -> bool {
    let mods = modifiers;
    if (mods & MOD_WIN.0) != 0 && !any_vk_down(&[VK_LWIN.0, VK_RWIN.0]) {
        return false;
    }
    if (mods & MOD_ALT.0) != 0 && !any_vk_down(&[VK_MENU.0, VK_LMENU.0, VK_RMENU.0]) {
        return false;
    }
    if (mods & MOD_SHIFT.0) != 0 && !any_vk_down(&[VK_SHIFT.0, VK_LSHIFT.0, VK_RSHIFT.0]) {
        return false;
    }
    if (mods & MOD_CONTROL.0) != 0 && !any_vk_down(&[VK_CONTROL.0, VK_LCONTROL.0, VK_RCONTROL.0]) {
        return false;
    }
    true
}

pub(super) fn hold_mods_match(chord: &HoldChord) -> bool {
    hold_mods_match_modifiers(chord.hold_modifiers)
}

pub(super) fn extra_mods_pressed_for(modifiers: u32) -> bool {
    let mods = modifiers;
    if (mods & MOD_SHIFT.0) == 0 && any_vk_down(&[VK_SHIFT.0, VK_LSHIFT.0, VK_RSHIFT.0]) {
        return true;
    }
    if (mods & MOD_CONTROL.0) == 0 && any_vk_down(&[VK_CONTROL.0, VK_LCONTROL.0, VK_RCONTROL.0]) {
        return true;
    }
    false
}

pub(super) fn extra_mods_pressed(chord: &HoldChord) -> bool {
    extra_mods_pressed_for(chord.hold_modifiers)
}

pub(super) fn is_hold_modifier_vk(vk: u32, hold_modifiers: u32) -> bool {
    if (hold_modifiers & MOD_WIN.0) != 0 && matches!(vk, 0x5B | 0x5C) {
        return true;
    }
    if (hold_modifiers & MOD_ALT.0) != 0 && matches!(vk, 0x12 | 0xA4 | 0xA5) {
        return true;
    }
    if (hold_modifiers & MOD_SHIFT.0) != 0 && matches!(vk, 0x10 | 0xA0 | 0xA1) {
        return true;
    }
    if (hold_modifiers & MOD_CONTROL.0) != 0 && matches!(vk, 0x11 | 0xA2 | 0xA3) {
        return true;
    }
    false
}

pub(super) fn should_handle_trigger(state: &HookState, chord: &HoldChord) -> bool {
    (tracked_mods_match(&state.mods, chord.hold_modifiers)
        && !tracked_extra_mods(&state.mods, chord.hold_modifiers))
        || (hold_mods_match(chord) && !extra_mods_pressed(chord))
}
