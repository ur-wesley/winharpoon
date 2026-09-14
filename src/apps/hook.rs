use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetDoubleClickTime, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LWIN, VK_MENU,
    VK_RCONTROL, VK_LSHIFT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT,
    WH_MOUSE_LL, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
};

use crate::config::{Config, DoubleTapModifier};
use crate::hotkeys::hotkey_hwnd;
use crate::log;
use crate::window::foreground_is_fullscreen;

use super::AppMenuAnchor;

struct HookConfig {
    enabled: bool,
    block_in_fullscreen: bool,
    modifier: DoubleTapModifier,
}

struct HookState {
    config: HookConfig,
    last_click: Option<Instant>,
    last_pos: (i32, i32),
}

static HOOK: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static STATE: Mutex<Option<HookState>> = Mutex::new(None);

struct ModTapTracker {
    mod_is_down: bool,
    press_time: Option<Instant>,
    last_tap_release: Option<Instant>,
}

static MOD_TAP_TRACKER: Mutex<ModTapTracker> = Mutex::new(ModTapTracker {
    mod_is_down: false,
    press_time: None,
    last_tap_release: None,
});

const MOD_DOUBLE_TAP_MS: u128 = 450;

pub fn reload(config: &Config) {
    let cfg = HookConfig {
        enabled: super::hook_enabled(config),
        block_in_fullscreen: config.apps.blocks_in_fullscreen(),
        modifier: config.apps.double_tap_modifier(),
    };
    if cfg.enabled {
        ensure_installed();
    }
    if let Some(state) = STATE.lock().as_mut() {
        state.config = cfg;
    }
}

pub fn ensure_installed() {
    if !HOOK.load(Ordering::Acquire).is_null() {
        return;
    }
    unsafe {
        let hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_proc),
            Some(GetModuleHandleW(None).unwrap().into()),
            0,
        );
        if let Ok(hook) = hook {
            HOOK.store(hook.0 as *mut _, Ordering::Release);
            *STATE.lock() = Some(HookState {
                config: HookConfig {
                    enabled: true,
                    block_in_fullscreen: false,
                    modifier: DoubleTapModifier::Alt,
                },
                last_click: None,
                last_pos: (0, 0),
            });
            log::debug("apps: mouse hook installed");
        } else {
            log::error("apps: mouse hook install failed");
        }
    }
}

pub fn uninstall() {
    let ptr = HOOK.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !ptr.is_null() {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(ptr));
        }
        *STATE.lock() = None;
        log::debug("apps: mouse hook uninstalled");
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(state) = STATE.lock().as_mut() {
            if state.config.enabled {
                let msg = wparam.0 as u32;
                let info = *(lparam.0 as *const MSLLHOOKSTRUCT);
                if msg == WM_LBUTTONDBLCLK {
                    maybe_open_menu(state, info.pt.x, info.pt.y);
                } else if msg == WM_LBUTTONDOWN {
                    handle_click(state, info.pt.x, info.pt.y);
                }
            }
        }
    }
    CallNextHookEx(Some(HHOOK(HOOK.load(Ordering::Acquire))), code, wparam, lparam)
}

pub fn try_alt_double_tap(vk: u32, key_up: bool) {
    let modifier = STATE
        .lock()
        .as_ref()
        .map_or(DoubleTapModifier::Alt, |s| s.config.modifier);

    let mut tracker = MOD_TAP_TRACKER.lock();

    if !is_modifier_vk(vk, modifier) {
        if !key_up {
            tracker.mod_is_down = false;
            tracker.press_time = None;
            tracker.last_tap_release = None;
        }
        return;
    }

    let enabled = STATE
        .lock()
        .as_ref()
        .is_some_and(|s| s.config.enabled);
    if !enabled || other_modifiers_held(modifier) {
        tracker.mod_is_down = false;
        tracker.press_time = None;
        tracker.last_tap_release = None;
        return;
    }

    let now = Instant::now();

    if key_up {
        tracker.mod_is_down = false;
        if let Some(press_time) = tracker.press_time.take() {
            if now.duration_since(press_time).as_millis() <= 350 {
                tracker.last_tap_release = Some(now);
            } else {
                tracker.last_tap_release = None;
            }
        } else {
            tracker.last_tap_release = None;
        }
    } else {
        if tracker.mod_is_down {
            tracker.press_time = None;
            tracker.last_tap_release = None;
            return;
        }

        tracker.mod_is_down = true;
        tracker.press_time = Some(now);

        if let Some(prev_release) = tracker.last_tap_release.take() {
            if now.duration_since(prev_release).as_millis() <= MOD_DOUBLE_TAP_MS {
                tracker.mod_is_down = true;
                tracker.press_time = None;
                tracker.last_tap_release = None;
                if let Some((x, y)) = cursor_pos() {
                    log::debug(format!(
                        "apps: {} double-tap at ({x},{y})",
                        modifier.as_str()
                    ));
                    post_app_menu(x, y);
                }
            }
        }
    }
}

fn handle_click(state: &mut HookState, x: i32, y: i32) {
    if !state.config.enabled || !modifier_held(state.config.modifier) {
        state.last_click = None;
        return;
    }

    let now = Instant::now();
    let threshold = unsafe { GetDoubleClickTime() };
    let is_double = state
        .last_click
        .is_some_and(|t| now.duration_since(t).as_millis() <= threshold as u128)
        && (x - state.last_pos.0).abs() <= 8
        && (y - state.last_pos.1).abs() <= 8;

    state.last_click = Some(now);
    state.last_pos = (x, y);

    if is_double {
        state.last_click = None;
        post_app_menu(x, y);
    }
}

fn maybe_open_menu(state: &HookState, x: i32, y: i32) {
    if !state.config.enabled || !modifier_held(state.config.modifier) {
        return;
    }
    post_app_menu(x, y);
}

fn modifier_held(modifier: DoubleTapModifier) -> bool {
    unsafe fn down(vk: i32) -> bool {
        GetAsyncKeyState(vk) as u16 & 0x8000 != 0
    }
    unsafe {
        match modifier {
            DoubleTapModifier::Alt => {
                down(VK_MENU.0 as i32) || down(VK_LMENU.0 as i32) || down(VK_RMENU.0 as i32)
            }
            DoubleTapModifier::Ctrl => {
                down(VK_CONTROL.0 as i32)
                    || down(VK_LCONTROL.0 as i32)
                    || down(VK_RCONTROL.0 as i32)
            }
            DoubleTapModifier::Shift => {
                down(VK_SHIFT.0 as i32) || down(VK_LSHIFT.0 as i32) || down(VK_RSHIFT.0 as i32)
            }
            DoubleTapModifier::Win => {
                down(VK_LWIN.0 as i32) || down(VK_RWIN.0 as i32)
            }
        }
    }
}

fn is_modifier_vk(vk: u32, modifier: DoubleTapModifier) -> bool {
    match modifier {
        DoubleTapModifier::Alt => {
            matches!(vk, v if v == VK_MENU.0 as u32 || v == VK_LMENU.0 as u32 || v == VK_RMENU.0 as u32)
        }
        DoubleTapModifier::Ctrl => matches!(
            vk,
            v if v == VK_CONTROL.0 as u32 || v == VK_LCONTROL.0 as u32 || v == VK_RCONTROL.0 as u32
        ),
        DoubleTapModifier::Shift => matches!(
            vk,
            v if v == VK_SHIFT.0 as u32 || v == VK_LSHIFT.0 as u32 || v == VK_RSHIFT.0 as u32
        ),
        DoubleTapModifier::Win => {
            matches!(vk, v if v == VK_LWIN.0 as u32 || v == VK_RWIN.0 as u32)
        }
    }
}

fn other_modifiers_held(modifier: DoubleTapModifier) -> bool {
    unsafe fn down(vk: i32) -> bool {
        GetAsyncKeyState(vk) as u16 & 0x8000 != 0
    }
    unsafe {
        let alt = down(VK_MENU.0 as i32) || down(VK_LMENU.0 as i32) || down(VK_RMENU.0 as i32);
        let ctrl = down(VK_CONTROL.0 as i32)
            || down(VK_LCONTROL.0 as i32)
            || down(VK_RCONTROL.0 as i32);
        let shift =
            down(VK_SHIFT.0 as i32) || down(VK_LSHIFT.0 as i32) || down(VK_RSHIFT.0 as i32);
        let win = down(VK_LWIN.0 as i32) || down(VK_RWIN.0 as i32);

        match modifier {
            DoubleTapModifier::Alt => ctrl || shift || win,
            DoubleTapModifier::Ctrl => alt || shift || win,
            DoubleTapModifier::Shift => alt || ctrl || win,
            DoubleTapModifier::Win => alt || ctrl || shift,
        }
    }
}

fn cursor_pos() -> Option<(i32, i32)> {
    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_ok() {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}

fn post_app_menu(x: i32, y: i32) {
    if STATE
        .lock()
        .as_ref()
        .is_some_and(|s| s.config.block_in_fullscreen && foreground_is_fullscreen())
    {
        log::debug("apps: blocked while foreground is fullscreen");
        return;
    }

    let Some(hwnd) = hotkey_hwnd() else {
        return;
    };
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
        let _ = PostMessageW(
            Some(hwnd),
            crate::hotkeys::wm_app_menu(),
            WPARAM(x as usize),
            LPARAM(y as isize),
        );
    }
    log::debug(format!("apps: modifier double-click at ({x},{y})"));
}

pub fn dispatch_app_menu(_x: i32, _y: i32) {
    super::open_menu(AppMenuAnchor);
}
