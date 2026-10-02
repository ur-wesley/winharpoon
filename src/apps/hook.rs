use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT,
    WH_MOUSE_LL, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
};

use crate::config::{Config, DoubleTapModifier};
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
        let Ok(module) = GetModuleHandleW(None) else {
            log::error("apps: mouse hook install failed: no module handle");
            return;
        };
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(module.into()), 0);
        if let Ok(hook) = hook {
            HOOK.store(hook.0, Ordering::Release);
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
                let msg = crate::win_cast::wparam_to_u32(wparam);
                let info = *crate::win_cast::lparam_to_const_ptr::<MSLLHOOKSTRUCT>(lparam);
                if msg == WM_LBUTTONDBLCLK {
                    maybe_open_menu(state, info.pt.x, info.pt.y);
                } else if msg == WM_LBUTTONDOWN {
                    handle_click(state, info.pt.x, info.pt.y);
                }
            }
        }
    }
    CallNextHookEx(
        Some(HHOOK(HOOK.load(Ordering::Acquire))),
        code,
        wparam,
        lparam,
    )
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

    let enabled = STATE.lock().as_ref().is_some_and(|s| s.config.enabled);
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
        .is_some_and(|t| now.duration_since(t).as_millis() <= u128::from(threshold))
        && x.saturating_sub(state.last_pos.0).abs() <= 8
        && y.saturating_sub(state.last_pos.1).abs() <= 8;

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

/// Virtual-key group (base key plus left/right variants) for a tap modifier.
///
/// Shared by the held-state query and the vk classification below so the
/// four groups exist in exactly one place. Win has no third variant; its
/// slot repeats `VK_RWIN`, which is harmless for `contains`/`any` checks.
fn modifier_vk_group(modifier: DoubleTapModifier) -> [u16; 3] {
    match modifier {
        DoubleTapModifier::Alt => [VK_MENU.0, VK_LMENU.0, VK_RMENU.0],
        DoubleTapModifier::Ctrl => [VK_CONTROL.0, VK_LCONTROL.0, VK_RCONTROL.0],
        DoubleTapModifier::Shift => [VK_SHIFT.0, VK_LSHIFT.0, VK_RSHIFT.0],
        DoubleTapModifier::Win => [VK_LWIN.0, VK_RWIN.0, VK_RWIN.0],
    }
}

fn modifier_held(modifier: DoubleTapModifier) -> bool {
    crate::util::any_vk_down(&modifier_vk_group(modifier))
}

fn is_modifier_vk(vk: u32, modifier: DoubleTapModifier) -> bool {
    modifier_vk_group(modifier).contains(&u16::try_from(vk).unwrap_or(0))
}

fn other_modifiers_held(modifier: DoubleTapModifier) -> bool {
    use crate::util::any_vk_down;
    let alt = any_vk_down(&modifier_vk_group(DoubleTapModifier::Alt));
    let ctrl = any_vk_down(&modifier_vk_group(DoubleTapModifier::Ctrl));
    let shift = any_vk_down(&modifier_vk_group(DoubleTapModifier::Shift));
    let win = any_vk_down(&modifier_vk_group(DoubleTapModifier::Win));

    match modifier {
        DoubleTapModifier::Alt => ctrl || shift || win,
        DoubleTapModifier::Ctrl => alt || shift || win,
        DoubleTapModifier::Shift => alt || ctrl || win,
        DoubleTapModifier::Win => alt || ctrl || shift,
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

    let _ = crate::hotkeys::post_hotkey_message(
        crate::hotkeys::wm_app_menu(),
        WPARAM(crate::win_cast::i32_to_usize_bits(x)),
        LPARAM(crate::win_cast::i32_to_isize_bits(y)),
    );
    log::debug(format!("apps: modifier double-click at ({x},{y})"));
}

pub fn dispatch_app_menu(_x: i32, _y: i32) {
    super::open_menu(AppMenuAnchor);
}
