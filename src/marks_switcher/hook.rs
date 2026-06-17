use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VK_CONTROL, VK_ESCAPE,
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
    VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, KillTimer, PostMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
};

use crate::config::{parse_chord, parse_hold_chord, Config, HoldChord};
use crate::hotkeys::{hotkey_hwnd, MARKS_POLL_TIMER_ID, WM_JUMP_KEY, WM_MARKS_KEY};
use crate::log;
use crate::marks_switcher::{ui_sender, SwitcherUiCommand};
use crate::modes::marks::{index_after_foreground, switcher_entries, SharedMarks};
use crate::window::focus;

const LLKHF_UP: u32 = 0x80;
const POLL_MS: u32 = 40;
const CONFIRM_GRACE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
struct JumpBinding {
    modifiers: u32,
    vk: u16,
    slot: u8,
}

#[derive(Debug, Clone, Default)]
struct TrackedModifiers {
    alt: bool,
    ctrl: bool,
    shift: bool,
    win: bool,
}

struct HookState {
    chord: HoldChord,
    marks: SharedMarks,
    jump_bindings: Vec<JumpBinding>,
    jump_keys_down: HashSet<u32>,
    mods: TrackedModifiers,
    active: bool,
    ignore_trigger_down: bool,
    activated_at: Option<Instant>,
    entries: Vec<crate::modes::marks::MarkEntry>,
    selected: usize,
}

static HOOK_STATE: Mutex<Option<HookState>> = Mutex::new(None);
static HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static SWITCHER_ACTIVE: AtomicBool = AtomicBool::new(false);
static TRIGGER_VK: AtomicU32 = AtomicU32::new(0x4D);
static HOLD_MODS: AtomicU32 = AtomicU32::new(MOD_WIN.0 | MOD_ALT.0);

fn jump_bindings_from_config(config: &Config) -> Vec<JumpBinding> {
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
            log::warn(&format!("jump_{slot} chord parse failed: {trimmed}"));
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

fn sync_chord_atomics(chord: &HoldChord) {
    TRIGGER_VK.store(chord.trigger_vk as u32, Ordering::Relaxed);
    HOLD_MODS.store(chord.hold_modifiers, Ordering::Relaxed);
}

fn set_switcher_active(active: bool) {
    SWITCHER_ACTIVE.store(active, Ordering::Relaxed);
}

pub fn install(marks: SharedMarks, config: Arc<Mutex<Config>>) {
    init_state(marks, config);
    install_hook();
}

pub fn init_state(marks: SharedMarks, config: Arc<Mutex<Config>>) {
    let config_guard = config.lock();
    let chord = parse_hold_chord(&config_guard.hotkeys.marks_switcher)
        .unwrap_or_else(|_| HoldChord {
            hold_modifiers: MOD_WIN.0 | MOD_ALT.0,
            trigger_vk: 0x4D,
        });
    let jump_bindings = jump_bindings_from_config(&config_guard);
    drop(config_guard);
    {
        let mut guard = HOOK_STATE.lock();
        sync_chord_atomics(&chord);
        *guard = Some(HookState {
            chord,
            marks,
            jump_bindings,
            jump_keys_down: HashSet::new(),
            mods: TrackedModifiers::default(),
            active: false,
            ignore_trigger_down: false,
            activated_at: None,
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
    if marks.lock().store.jump_slot(slot) {
        log::debug(&format!("hook jump slot {slot}: ok"));
    } else {
        log::warn(&format!("hook jump slot {slot}: missed"));
    }
}

fn install_hook() {
    if !HOOK_HANDLE.load(Ordering::SeqCst).is_null() {
        return;
    }
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(instance), 0);
        match hook {
            Ok(h) => {
                HOOK_HANDLE.store(h.0, Ordering::SeqCst);
                log::debug("marks_switcher WH_KEYBOARD_LL installed");
            }
            Err(err) => log::error(&format!("marks_switcher hook install failed: {err:?}")),
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
    if key_up {
        let confirm = {
            let mut guard = HOOK_STATE.lock();
            let Some(state) = guard.as_mut() else {
                return;
            };
            let chord = state.chord.clone();
            if vk == chord.trigger_vk as u32 {
                state.ignore_trigger_down = false;
            }
            if state.active && is_hold_modifier_vk(vk, chord.hold_modifiers) && may_confirm(state) {
                log::debug(&format!("marks_switcher modifier up vk=0x{vk:X}"));
                let selected = state.selected;
                let entries = state.entries.clone();
                cancel_active(state);
                Some((selected, entries))
            } else {
                None
            }
        };
        if let Some((selected, entries)) = confirm {
            focus_selected(&entries, selected);
        }
        return;
    }

    let mut guard = HOOK_STATE.lock();
    let Some(state) = guard.as_mut() else {
        return;
    };
    let chord = state.chord.clone();

    if vk == chord.trigger_vk as u32 && should_handle_trigger(&chord) {
        if state.ignore_trigger_down && trigger_physically_down(&chord) {
            log::debug("marks_switcher: trigger held from prior session, release first");
            return;
        }
        state.ignore_trigger_down = false;
        let backward = unsafe { shift_held() };
        if backward {
            if state.active {
                cycle(state, -1);
            } else {
                activate(state);
                cycle(state, -1);
            }
        } else if state.active {
            cycle(state, 1);
        } else {
            activate(state);
        }
    } else if vk == VK_ESCAPE.0 as u32 && state.active {
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
        if state.active && !hold_mods_match(&state.chord) && may_confirm(state) {
            log::debug("marks_switcher poll: hold modifiers released");
            let selected = state.selected;
            let entries = state.entries.clone();
            cancel_active(state);
            Some((selected, entries))
        } else {
            None
        }
    };
    if let Some((selected, entries)) = confirm {
        focus_selected(&entries, selected);
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
    state.activated_at = None;
    state.entries.clear();
    set_switcher_active(false);
    stop_poll_timer();
    send_ui(SwitcherUiCommand::Hide);
}

fn focus_selected(entries: &[crate::modes::marks::MarkEntry], selected: usize) {
    let Some(entry) = entries.get(selected) else {
        return;
    };
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
        log::warn(&format!(
            "marks_switcher confirm: window not found for {}",
            entry.identity.display_label()
        ));
    }
}

fn should_focus_target(target_hwnd: isize) -> bool {
    if let Some(our_hwnd) = hotkey_hwnd() {
        if our_hwnd.0 as isize == target_hwnd {
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
    let entries = {
        let guard = state.marks.lock();
        switcher_entries(&guard.store)
    };
    if entries.is_empty() {
        log::debug("marks_switcher: no available marks");
        log::notify("WinHarpoon", "No marked windows");
        return;
    }
    let selected = index_after_foreground(&entries);
    state.entries = entries;
    state.selected = selected;
    state.active = true;
    state.ignore_trigger_down = false;
    state.activated_at = Some(Instant::now());
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
        (state.selected + 1) % len
    } else if state.selected == 0 {
        len - 1
    } else {
        state.selected - 1
    };
    send_ui(SwitcherUiCommand::SetSelected(state.selected));
}

fn may_confirm(state: &HookState) -> bool {
    state
        .activated_at
        .is_none_or(|t| t.elapsed() >= CONFIRM_GRACE)
}

fn trigger_physically_down(chord: &HoldChord) -> bool {
    unsafe { GetAsyncKeyState(chord.trigger_vk as i32) as u16 & 0x8000 != 0 }
}

fn update_tracked_modifiers(mods: &mut TrackedModifiers, vk: u32, key_up: bool) {
    let down = !key_up;
    match vk {
        0x12 | 0xA4 | 0xA5 => mods.alt = down,
        0x11 | 0xA2 | 0xA3 => mods.ctrl = down,
        0x10 | 0xA0 | 0xA1 => mods.shift = down,
        0x5B | 0x5C => mods.win = down,
        _ => {}
    }
}

fn tracked_mods_match(mods: &TrackedModifiers, required: u32) -> bool {
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

fn tracked_extra_mods(mods: &TrackedModifiers, required: u32) -> bool {
    if (required & MOD_SHIFT.0) == 0 && mods.shift {
        return true;
    }
    if (required & MOD_CONTROL.0) == 0 && mods.ctrl {
        return true;
    }
    false
}

fn is_jump_vk(state: &HookState, vk: u32) -> bool {
    state.jump_bindings.iter().any(|b| b.vk as u32 == vk)
}

fn sync_tracked_modifiers(mods: &mut TrackedModifiers) {
    mods.alt = modifier_physically_held(VK_MENU.0 as i32)
        || modifier_physically_held(VK_LMENU.0 as i32)
        || modifier_physically_held(VK_RMENU.0 as i32);
    mods.ctrl = modifier_physically_held(VK_CONTROL.0 as i32)
        || modifier_physically_held(VK_LCONTROL.0 as i32)
        || modifier_physically_held(VK_RCONTROL.0 as i32);
    mods.shift = modifier_physically_held(VK_SHIFT.0 as i32)
        || modifier_physically_held(VK_LSHIFT.0 as i32)
        || modifier_physically_held(VK_RSHIFT.0 as i32);
    mods.win = modifier_physically_held(VK_LWIN.0 as i32)
        || modifier_physically_held(VK_RWIN.0 as i32);
}

fn modifier_physically_held(vk: i32) -> bool {
    unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
}

fn hold_mods_match_modifiers(modifiers: u32) -> bool {
    unsafe fn key_down(vk: i32) -> bool {
        GetAsyncKeyState(vk) as u16 & 0x8000 != 0
    }

    let mods = modifiers;
    unsafe {
        if (mods & MOD_WIN.0) != 0
            && !key_down(VK_LWIN.0 as i32)
            && !key_down(VK_RWIN.0 as i32)
        {
            return false;
        }
        if (mods & MOD_ALT.0) != 0
            && !key_down(VK_MENU.0 as i32)
            && !key_down(VK_LMENU.0 as i32)
            && !key_down(VK_RMENU.0 as i32)
        {
            return false;
        }
        if (mods & MOD_SHIFT.0) != 0
            && !key_down(VK_SHIFT.0 as i32)
            && !key_down(VK_LSHIFT.0 as i32)
            && !key_down(VK_RSHIFT.0 as i32)
        {
            return false;
        }
        if (mods & MOD_CONTROL.0) != 0
            && !key_down(VK_CONTROL.0 as i32)
            && !key_down(VK_LCONTROL.0 as i32)
            && !key_down(VK_RCONTROL.0 as i32)
        {
            return false;
        }
    }
    true
}

fn hold_mods_match(chord: &HoldChord) -> bool {
    hold_mods_match_modifiers(chord.hold_modifiers)
}

fn extra_mods_pressed_for(modifiers: u32) -> bool {
    unsafe fn key_down(vk: i32) -> bool {
        GetAsyncKeyState(vk) as u16 & 0x8000 != 0
    }

    let mods = modifiers;
    unsafe {
        if (mods & MOD_SHIFT.0) == 0
            && (key_down(VK_SHIFT.0 as i32)
                || key_down(VK_LSHIFT.0 as i32)
                || key_down(VK_RSHIFT.0 as i32))
        {
            return true;
        }
        if (mods & MOD_CONTROL.0) == 0
            && (key_down(VK_CONTROL.0 as i32)
                || key_down(VK_LCONTROL.0 as i32)
                || key_down(VK_RCONTROL.0 as i32))
        {
            return true;
        }
    }
    false
}

fn extra_mods_pressed(chord: &HoldChord) -> bool {
    extra_mods_pressed_for(chord.hold_modifiers)
}

fn post_jump(slot: u8) {
    let Some(hwnd) = hotkey_hwnd() else {
        return;
    };
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_JUMP_KEY,
            WPARAM(slot as usize),
            LPARAM(0),
        );
    }
}

fn handle_jump_key(vk: u32, key_up: bool) {
    let mut guard = HOOK_STATE.lock();
    let Some(state) = guard.as_mut() else {
        return;
    };
    if key_up {
        state.jump_keys_down.remove(&vk);
        return;
    }
    if !is_jump_vk(state, vk) {
        return;
    }
    sync_tracked_modifiers(&mut state.mods);
    if state.jump_keys_down.contains(&vk) {
        return;
    }
    let slot = state.jump_bindings.iter().find_map(|binding| {
        if binding.vk as u32 != vk {
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
        log::debug(&format!(
            "marks_switcher hook jump vk=0x{vk:X} slot={slot} mods={:?}",
            state.mods
        ));
        drop(guard);
        post_jump(slot);
    }
}

fn is_hold_modifier_vk(vk: u32, hold_modifiers: u32) -> bool {
    if (hold_modifiers & MOD_WIN.0) != 0 && matches!(vk, 0x5B | 0x5C) {
        return true;
    }
    if (hold_modifiers & MOD_ALT.0) != 0 && matches!(vk, 0x12 | 0xA4 | 0xA5) {
        return true;
    }
    if (hold_modifiers & MOD_SHIFT.0) != 0 && matches!(vk, 0x10 | 0xA0 | 0xA1) {
        return true;
    }
    false
}

unsafe fn shift_held() -> bool {
    GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000 != 0
}

fn should_handle_trigger(chord: &HoldChord) -> bool {
    hold_mods_match(chord) && !extra_mods_pressed(chord)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kbd.vkCode;
        let key_up = (kbd.flags.0 & LLKHF_UP) != 0;
        {
            let mut guard = HOOK_STATE.lock();
            if let Some(state) = guard.as_mut() {
                update_tracked_modifiers(&mut state.mods, vk, key_up);
            }
        }
        handle_jump_key(vk, key_up);
        if should_forward_key(vk, key_up) {
            if let Some(hwnd) = hotkey_hwnd() {
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_MARKS_KEY,
                    WPARAM(vk as usize),
                    LPARAM(if key_up { 1 } else { 0 }),
                );
            }
        }
    }

    CallNextHookEx(Some(HHOOK(HOOK_HANDLE.load(Ordering::SeqCst))), code, wparam, lparam)
}

fn should_forward_key(vk: u32, key_up: bool) -> bool {
    let trigger_vk = TRIGGER_VK.load(Ordering::Relaxed);
    let hold_mods = HOLD_MODS.load(Ordering::Relaxed);
    if vk == VK_ESCAPE.0 as u32 {
        return is_switcher_active();
    }
    if vk == trigger_vk {
        return true;
    }
    if is_hold_modifier_vk(vk, hold_mods) {
        return is_switcher_active() || !key_up;
    }
    false
}
