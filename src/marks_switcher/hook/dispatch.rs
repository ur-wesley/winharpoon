//! Switcher key dispatch: hook callback, binding handlers, message posts.
//!
//! Split from `hook/mod.rs` to keep modules under the 600-line gate.

use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT};

use crate::hotkeys::{post_hotkey_message, wm_jump_key, wm_launcher_key, wm_marks_key};
use crate::log;
use crate::marks_switcher::SwitcherUiCommand;
use crate::modes::marks::switcher_entries;

use super::modifiers::{
    extra_mods_pressed_for, hold_mods_match_modifiers, is_hold_modifier_vk, should_handle_trigger,
    sync_tracked_modifiers, tracked_extra_mods, tracked_mods_match, update_tracked_modifiers,
};
use super::state::HookState;
use super::{
    cancel_active, cycle, is_switcher_active, send_ui, HOLD_MODS, HOOK_HANDLE, HOOK_STATE,
    TRIGGER_VK,
};

const LLKHF_UP: u32 = 0x80;
const LLKHF_INJECTED: u32 = 0x10;

fn is_jump_vk(state: &HookState, vk: u32) -> bool {
    state.jump_bindings.iter().any(|b| u32::from(b.vk) == vk)
}

fn post_jump(slot: u8) {
    let _ = post_hotkey_message(wm_jump_key(), WPARAM(usize::from(slot)), LPARAM(0));
}

fn handle_switcher_manage_key(state: &mut HookState, vk: u32, key_up: bool) -> bool {
    const VK_BACK: u32 = 0x08;
    const VK_DELETE: u32 = 0x2E;

    if !is_switcher_active() {
        return false;
    }
    if key_up {
        return false;
    }

    // 1. Handle deletion (Delete or Backspace)
    if vk == VK_DELETE || vk == VK_BACK {
        if let Some(entry) = state.entries.get(state.selected) {
            let slot = entry.slot;
            let new_entries = {
                let mut marks_guard = state.marks.lock();
                marks_guard.store.slots.remove(&slot.to_string());
                let _ = marks_guard.store.save();
                switcher_entries(&marks_guard.store)
            };
            if new_entries.is_empty() {
                cancel_active(state);
            } else {
                state.entries = new_entries;
                state.selected = state.selected.min(state.entries.len().saturating_sub(1));
                send_ui(SwitcherUiCommand::Show {
                    entries: state.entries.clone(),
                    selected: state.selected,
                });
            }
        }
        return true;
    }

    // 2. Handle move/swap (Shift + Nav Key)
    sync_tracked_modifiers(&mut state.mods);
    if state.mods.shift {
        if let Some(binding) = state
            .switcher_nav_bindings
            .iter()
            .find(|b| u32::from(b.vk) == vk)
        {
            if let Some(entry) = state.entries.get(state.selected) {
                let slot = entry.slot;
                let earlier = binding.delta < 0;
                let mut marks_guard = state.marks.lock();
                if marks_guard.store.move_mark_slot(slot, earlier) {
                    let new_entries = switcher_entries(&marks_guard.store);
                    if let Some(new_pos) = new_entries.iter().position(|e| e.slot == slot) {
                        state.selected = new_pos;
                    }
                    state.entries = new_entries;
                    send_ui(SwitcherUiCommand::Show {
                        entries: state.entries.clone(),
                        selected: state.selected,
                    });
                }
            }
            return true;
        }
    }

    false
}

fn handle_switcher_nav_key(state: &mut HookState, vk: u32, key_up: bool) {
    if !is_switcher_active() {
        return;
    }
    if key_up {
        state.switcher_nav_keys_down.remove(&vk);
        return;
    }
    if !state
        .switcher_nav_bindings
        .iter()
        .any(|b| u32::from(b.vk) == vk)
    {
        return;
    }
    sync_tracked_modifiers(&mut state.mods);
    if state.switcher_nav_keys_down.contains(&vk) {
        return;
    }
    let delta = state.switcher_nav_bindings.iter().find_map(|binding| {
        if u32::from(binding.vk) != vk {
            return None;
        }
        let mods = binding.modifiers;
        if (tracked_mods_match(&state.mods, mods) && !tracked_extra_mods(&state.mods, mods))
            || (hold_mods_match_modifiers(mods) && !extra_mods_pressed_for(mods))
        {
            Some(binding.delta)
        } else {
            None
        }
    });
    if let Some(delta) = delta {
        state.switcher_nav_keys_down.insert(vk);
        log::trace(format!(
            "marks_switcher nav vk=0x{vk:X} delta={delta} mods={:?}",
            state.mods
        ));
        cycle(state, delta);
    }
}

fn handle_jump_key(state: &mut HookState, vk: u32, key_up: bool) -> Option<u8> {
    if key_up {
        state.jump_keys_down.remove(&vk);
        return None;
    }
    if !is_jump_vk(state, vk) {
        return None;
    }
    sync_tracked_modifiers(&mut state.mods);
    if state.jump_keys_down.contains(&vk) {
        return None;
    }
    let slot = state.jump_bindings.iter().find_map(|binding| {
        if u32::from(binding.vk) != vk {
            return None;
        }
        let mods = binding.modifiers;
        if (tracked_mods_match(&state.mods, mods) && !tracked_extra_mods(&state.mods, mods))
            || (hold_mods_match_modifiers(mods) && !extra_mods_pressed_for(mods))
        {
            Some(binding.slot)
        } else {
            None
        }
    });
    if let Some(slot) = slot {
        state.jump_keys_down.insert(vk);
        log::trace(format!(
            "marks_switcher hook jump vk=0x{vk:X} slot={slot} mods={:?}",
            state.mods
        ));
        Some(slot)
    } else {
        None
    }
}

fn handle_launcher_key(state: &mut HookState, vk: u32, key_up: bool) -> bool {
    if key_up {
        return false;
    }
    let Some(binding) = &state.launcher_binding else {
        return false;
    };
    if u32::from(binding.vk) != vk {
        return false;
    }
    sync_tracked_modifiers(&mut state.mods);
    let mods = binding.modifiers;
    if (tracked_mods_match(&state.mods, mods) && !tracked_extra_mods(&state.mods, mods))
        || (hold_mods_match_modifiers(mods) && !extra_mods_pressed_for(mods))
    {
        log::trace(format!(
            "marks_switcher hook launcher vk=0x{vk:X} mods={:?}",
            state.mods
        ));
        true
    } else {
        false
    }
}

fn post_launcher() {
    let _ = post_hotkey_message(wm_launcher_key(), WPARAM(0), LPARAM(0));
}

fn should_swallow_key(state: &HookState, vk: u32, _key_up: bool) -> bool {
    let trigger_vk = TRIGGER_VK.load(Ordering::Relaxed);
    if vk == u32::from(VK_ESCAPE.0) {
        return is_switcher_active();
    }
    if vk == trigger_vk {
        if is_switcher_active() {
            return true;
        }
        if should_handle_trigger(state, &state.chord) {
            return true;
        }
    }
    if let Some(binding) = &state.launcher_binding {
        if u32::from(binding.vk) == vk {
            let mods = binding.modifiers;
            if (tracked_mods_match(&state.mods, mods) && !tracked_extra_mods(&state.mods, mods))
                || (hold_mods_match_modifiers(mods) && !extra_mods_pressed_for(mods))
            {
                return true;
            }
        }
    }
    if is_switcher_active() {
        if is_jump_vk(state, vk) {
            return true;
        }
        if state
            .switcher_nav_bindings
            .iter()
            .any(|b| u32::from(b.vk) == vk)
        {
            return true;
        }
        // Swallow Delete (0x2E) and Backspace (0x08) keys
        if vk == 0x2E || vk == 0x08 {
            return true;
        }
    }
    false
}

pub(super) unsafe extern "system" fn keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut swallow = false;
    if code >= 0 {
        let kbd = *crate::win_cast::lparam_to_const_ptr::<KBDLLHOOKSTRUCT>(lparam);
        let vk = kbd.vkCode;
        let key_up = (kbd.flags.0 & LLKHF_UP) != 0;
        if vk == 0x59 || vk == 0x11 || vk == 0x10 || vk == 0xA2 || vk == 0xA0 {
            log::trace(format!("keyboard_proc: vk=0x{vk:X} key_up={key_up}"));
        }
        let injected = (kbd.flags.0 & LLKHF_INJECTED) != 0;

        let mut jump_slot = None;
        let mut trigger_launcher = false;

        {
            let mut guard = HOOK_STATE.lock();
            if let Some(state) = guard.as_mut() {
                update_tracked_modifiers(&mut state.mods, vk, key_up);
                jump_slot = handle_jump_key(state, vk, key_up);
                trigger_launcher = handle_launcher_key(state, vk, key_up);

                let managed = handle_switcher_manage_key(state, vk, key_up);
                if !managed {
                    handle_switcher_nav_key(state, vk, key_up);
                }

                if !injected {
                    crate::apps::hook::try_alt_double_tap(vk, key_up);
                }

                swallow = should_swallow_key(state, vk, key_up);
            }
        }

        if let Some(slot) = jump_slot {
            post_jump(slot);
        }
        if trigger_launcher {
            post_launcher();
        }

        if should_forward_key(vk, key_up) {
            let _ = post_hotkey_message(
                wm_marks_key(),
                WPARAM(usize::try_from(vk).unwrap_or(0)),
                LPARAM(isize::from(key_up)),
            );
        }
    }

    if swallow {
        LRESULT(1)
    } else {
        CallNextHookEx(
            Some(HHOOK(HOOK_HANDLE.load(Ordering::SeqCst))),
            code,
            wparam,
            lparam,
        )
    }
}

fn should_forward_key(vk: u32, key_up: bool) -> bool {
    let trigger_vk = TRIGGER_VK.load(Ordering::Relaxed);
    let hold_mods = HOLD_MODS.load(Ordering::Relaxed);
    let res = if vk == u32::from(VK_ESCAPE.0) {
        is_switcher_active()
    } else if vk == trigger_vk {
        true
    } else if is_hold_modifier_vk(vk, hold_mods) {
        is_switcher_active() || !key_up
    } else {
        false
    };
    if vk == 0x59 || vk == 0x11 || vk == 0x10 || vk == 0xA2 || vk == 0xA0 {
        log::trace(format!(
            "should_forward_key: vk=0x{vk:X} key_up={key_up} trigger_vk=0x{trigger_vk:X} res={res}"
        ));
    }
    res
}
