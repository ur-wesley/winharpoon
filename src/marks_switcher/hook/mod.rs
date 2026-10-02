use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, WH_KEYBOARD_LL,
};

use crate::config::{parse_chord, parse_hold_chord, Config, HoldChord};
use crate::hotkeys::{hotkey_hwnd, MARKS_POLL_TIMER_ID};
use crate::log;
use crate::marks_switcher::{ui_sender, SwitcherUiCommand};
use crate::modes::marks::SharedMarks;
use crate::window::focus;

mod dispatch;
mod modifiers;
mod state;

use dispatch::keyboard_proc;
use modifiers::{
    hold_mods_match, should_handle_trigger, sync_tracked_modifiers, trigger_physically_down,
};
use state::{
    jump_bindings_from_config, switcher_nav_bindings_from_config, sync_chord_atomics, HookState,
    LauncherBinding, TrackedModifiers,
};

const POLL_MS: u32 = 40;

static HOOK_STATE: Mutex<Option<HookState>> = Mutex::new(None);
static HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static SWITCHER_ACTIVE: AtomicBool = AtomicBool::new(false);
static TRIGGER_VK: AtomicU32 = AtomicU32::new(0x4D);
static HOLD_MODS: AtomicU32 = AtomicU32::new(MOD_WIN.0 | MOD_ALT.0);

fn set_switcher_active(active: bool) {
    SWITCHER_ACTIVE.store(active, Ordering::Relaxed);
}

pub fn install(marks: SharedMarks, config: &Arc<Mutex<Config>>) {
    init_state(marks, config);
    install_hook();
}

pub fn init_state(marks: SharedMarks, config: &Arc<Mutex<Config>>) {
    let config_guard = config.lock();
    let chord = parse_hold_chord(&config_guard.hotkeys.marks_switcher).unwrap_or(HoldChord {
        hold_modifiers: MOD_WIN.0 | MOD_ALT.0,
        trigger_vk: 0x4D,
    });
    let jump_bindings = jump_bindings_from_config(&config_guard);
    let switcher_nav_bindings = switcher_nav_bindings_from_config(&config_guard);
    let launcher_binding = parse_chord(&config_guard.hotkeys.launcher)
        .ok()
        .map(|parsed| LauncherBinding {
            modifiers: parsed.modifiers & !MOD_NOREPEAT.0,
            vk: parsed.vk,
        });
    drop(config_guard);
    {
        let mut guard = HOOK_STATE.lock();
        sync_chord_atomics(&chord);
        *guard = Some(HookState {
            chord,
            marks,
            jump_bindings,
            switcher_nav_bindings,
            launcher_binding,
            jump_keys_down: HashSet::new(),
            switcher_nav_keys_down: HashSet::new(),
            mods: TrackedModifiers::default(),
            active: false,
            ignore_trigger_down: false,
            trigger_released_since_activate: false,
            last_trigger_down: false,
            entries: Vec::new(),
            selected: 0,
        });
    }
}

pub fn ensure_installed() {
    install_hook();
}

pub fn reload_chord(config: &Config) {
    let jump_bindings = jump_bindings_from_config(config);
    let switcher_nav_bindings = switcher_nav_bindings_from_config(config);
    let launcher_binding =
        parse_chord(&config.hotkeys.launcher)
            .ok()
            .map(|parsed| LauncherBinding {
                modifiers: parsed.modifiers & !MOD_NOREPEAT.0,
                vk: parsed.vk,
            });
    let mut guard = HOOK_STATE.lock();
    let Some(state) = guard.as_mut() else {
        return;
    };
    if let Ok(chord) = parse_hold_chord(&config.hotkeys.marks_switcher) {
        if state.active {
            cancel_active(state);
        }
        sync_chord_atomics(&chord);
        state.chord = chord;
    } else {
        log::warn("marks_switcher chord parse failed on reload");
    }
    state.jump_bindings = jump_bindings;
    state.switcher_nav_bindings = switcher_nav_bindings;
    state.launcher_binding = launcher_binding;
    log::debug("marks_switcher hook config reloaded");
}

pub fn dispatch_jump(slot: u8) {
    let marks = {
        let guard = HOOK_STATE.lock();
        let Some(state) = guard.as_ref() else {
            return;
        };
        state.marks.clone()
    };
    let mut guard = marks.lock();
    if guard.store.jump_slot(slot) {
        guard.touch_slot(slot);
        log::debug(format!("hook jump slot {slot}: ok"));
    } else {
        log::warn(format!("hook jump slot {slot}: missed"));
    }
}

fn install_hook() {
    if !HOOK_HANDLE.load(Ordering::SeqCst).is_null() {
        return;
    }
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            log::error("marks_switcher hook install failed: no module handle");
            return;
        };
        let instance: HINSTANCE = module.into();
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(instance), 0);
        match hook {
            Ok(h) => {
                HOOK_HANDLE.store(h.0, Ordering::SeqCst);
                log::debug("marks_switcher WH_KEYBOARD_LL installed");
            }
            Err(err) => log::error(format!("marks_switcher hook install failed: {err:?}")),
        }
    }
}

pub fn is_switcher_active() -> bool {
    SWITCHER_ACTIVE.load(Ordering::Relaxed)
}

pub fn cancel_if_active() {
    let mut guard = HOOK_STATE.lock();
    if let Some(state) = guard.as_mut() {
        if state.active {
            cancel_active(state);
        }
    }
}

pub fn dispatch_key(vk: u32, key_up: bool) {
    // Security: keystrokes at trace only — debug logs persist VK timing to disk.
    log::trace(format!(
        "marks_switcher dispatch_key vk=0x{vk:X} key_up={key_up}"
    ));
    if key_up {
        let confirm = {
            let mut guard = HOOK_STATE.lock();
            let Some(state) = guard.as_mut() else {
                return;
            };
            let chord = state.chord.clone();
            if vk == u32::from(chord.trigger_vk) {
                state.ignore_trigger_down = false;
                if state.active {
                    state.trigger_released_since_activate = true;
                }
            }
            if should_confirm_release(state, &chord) {
                log::trace(format!("marks_switcher confirm on key up vk=0x{vk:X}"));
                let selected = state.selected;
                let entries = state.entries.clone();
                let marks = state.marks.clone();
                cancel_active(state);
                Some((selected, entries, marks))
            } else {
                None
            }
        };
        if let Some((selected, entries, marks)) = confirm {
            focus_selected(&marks, &entries, selected);
        }
        return;
    }

    let mut guard = HOOK_STATE.lock();
    let Some(state) = guard.as_mut() else {
        return;
    };
    let chord = state.chord.clone();

    if vk == u32::from(chord.trigger_vk) && should_handle_trigger(state, &chord) {
        if state.ignore_trigger_down && trigger_physically_down(&chord) {
            log::debug("marks_switcher: trigger held from prior session, release first");
            return;
        }
        state.ignore_trigger_down = false;
        let shift_in_hold = (chord.hold_modifiers & MOD_SHIFT.0) != 0;
        let backward = !shift_in_hold && state.mods.shift;
        if state.active {
            cycle(state, if backward { -1 } else { 1 });
        } else {
            activate(state);
        }
    } else if vk == u32::from(VK_ESCAPE.0) && state.active {
        log::debug("marks_switcher cancelled via Esc");
        cancel_active(state);
    }
}

pub fn poll_active() {
    let confirm = {
        let mut guard = HOOK_STATE.lock();
        let Some(state) = guard.as_mut() else {
            return;
        };
        if state.active {
            sync_tracked_modifiers(&mut state.mods);
            let trig_down = trigger_physically_down(&state.chord);
            if trig_down {
                if !state.last_trigger_down {
                    log::debug("marks_switcher poll: trigger key pressed (cycle)");
                    let shift_in_hold = (state.chord.hold_modifiers & MOD_SHIFT.0) != 0;
                    let backward = !shift_in_hold && state.mods.shift;
                    cycle(state, if backward { -1 } else { 1 });
                    state.last_trigger_down = true;
                }
            } else if state.last_trigger_down {
                log::debug("marks_switcher poll: trigger key released");
                state.last_trigger_down = false;
                state.trigger_released_since_activate = true;
            }
        }
        if should_confirm_release(state, &state.chord) {
            log::debug("marks_switcher poll: chord released");
            let selected = state.selected;
            let entries = state.entries.clone();
            let marks = state.marks.clone();
            cancel_active(state);
            Some((selected, entries, marks))
        } else {
            None
        }
    };
    if let Some((selected, entries, marks)) = confirm {
        focus_selected(&marks, &entries, selected);
    }
}

pub fn uninstall_hook() {
    let ptr = HOOK_HANDLE.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !ptr.is_null() {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(ptr));
        }
        log::debug("marks_switcher hook uninstalled");
    }
    stop_poll_timer();
}

fn start_poll_timer() {
    let Some(hwnd) = hotkey_hwnd() else {
        return;
    };
    unsafe {
        let _ = SetTimer(Some(hwnd), MARKS_POLL_TIMER_ID, POLL_MS, None);
    }
}

fn stop_poll_timer() {
    let Some(hwnd) = hotkey_hwnd() else {
        return;
    };
    unsafe {
        let _ = KillTimer(Some(hwnd), MARKS_POLL_TIMER_ID);
    }
}

fn cancel_active(state: &mut HookState) {
    if !state.active {
        return;
    }
    log::debug("marks_switcher cancel_active");
    state.active = false;
    state.ignore_trigger_down = trigger_physically_down(&state.chord);
    state.trigger_released_since_activate = false;
    state.last_trigger_down = false;
    state.jump_keys_down.clear();
    state.switcher_nav_keys_down.clear();
    state.entries.clear();
    set_switcher_active(false);
    stop_poll_timer();
    send_ui(SwitcherUiCommand::Hide);
}

fn focus_selected(
    marks: &SharedMarks,
    entries: &[crate::modes::marks::MarkEntry],
    selected: usize,
) {
    let Some(entry) = entries.get(selected) else {
        return;
    };
    marks.lock().touch_slot(entry.slot);
    if let Some(win) = &entry.window {
        if should_focus_target(win.hwnd) {
            focus::focus_window(win.hwnd);
        }
        return;
    }
    let windows = crate::window::enumerate_windows(None);
    if let Some(target) = crate::window::identity::resolve_identity(&entry.identity, &windows) {
        if should_focus_target(target.hwnd) {
            focus::focus_window(target.hwnd);
        }
    } else {
        log::warn(format!(
            "marks_switcher confirm: window not found for {}",
            entry.identity.display_label()
        ));
    }
}

fn should_focus_target(target_hwnd: isize) -> bool {
    if let Some(our_hwnd) = hotkey_hwnd() {
        if crate::win_cast::hwnd_to_raw(our_hwnd) == target_hwnd {
            return false;
        }
    }
    true
}

fn send_ui(cmd: SwitcherUiCommand) {
    if let Some(tx) = ui_sender() {
        let _ = tx.send(cmd);
    }
    if let Some(ctx) = crate::launcher::ui_context() {
        ctx.request_repaint();
    }
}

fn activate(state: &mut HookState) {
    let (entries, selected) = {
        let mut guard = state.marks.lock();
        let entries = guard.switcher_entries();
        let selected = guard.initial_selected_index(&entries);
        (entries, selected)
    };
    if entries.is_empty() {
        log::debug("marks_switcher: no available marks");
        log::notify("WinHarpoon", "No marked windows");
        return;
    }
    state.entries = entries;
    state.selected = selected;
    state.active = true;
    state.ignore_trigger_down = false;
    state.trigger_released_since_activate = false;
    state.last_trigger_down = true;
    set_switcher_active(true);
    start_poll_timer();
    send_ui(SwitcherUiCommand::Show {
        entries: state.entries.clone(),
        selected: state.selected,
    });
}

fn cycle(state: &mut HookState, delta: i32) {
    if state.entries.is_empty() {
        return;
    }
    let len = state.entries.len();
    state.selected = if delta > 0 {
        let next = state.selected.saturating_add(1);
        if next >= len {
            0
        } else {
            next
        }
    } else if state.selected == 0 {
        len.saturating_sub(1)
    } else {
        state.selected.saturating_sub(1)
    };
    send_ui(SwitcherUiCommand::SetSelected(state.selected));
}

fn should_confirm_release(state: &HookState, chord: &HoldChord) -> bool {
    let mods_match = hold_mods_match(chord);
    let trig_down = trigger_physically_down(chord);
    log::debug(format!(
        "should_confirm_release: active={}, trigger_released={}, mods_match={}, trig_down={}",
        state.active, state.trigger_released_since_activate, mods_match, trig_down
    ));
    if !state.active {
        return false;
    }
    if !mods_match && !trig_down {
        return true;
    }
    if !state.trigger_released_since_activate {
        return false;
    }
    if mods_match || trig_down {
        return false;
    }
    true
}
