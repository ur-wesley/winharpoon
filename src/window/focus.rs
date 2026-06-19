use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOP, SW_RESTORE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
};

use crate::log;

#[derive(Debug, Clone)]
pub struct StackSnapshot {
    pub foreground: Option<isize>,
    pub z_order: Vec<isize>,
}

pub fn capture_stack_snapshot() -> StackSnapshot {
    let mut z_order = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_visible_hwnd),
            LPARAM(&mut z_order as *mut _ as isize),
        );
    }
    let foreground = unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            None
        } else {
            Some(fg.0 as isize)
        }
    };
    StackSnapshot {
        foreground,
        z_order,
    }
}

pub fn restore_stack_snapshot(snapshot: &StackSnapshot) {
    restore_z_order(&snapshot.z_order);
    if let Some(hwnd) = snapshot.foreground {
        focus_window_impl(hwnd, false);
    }
}

pub fn preview_window(hwnd_raw: isize) {
    let hwnd = HWND(hwnd_raw as *mut _);
    if hwnd.0.is_null() {
        return;
    }
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

pub fn focus_window(hwnd_raw: isize) -> bool {
    focus_window_impl(hwnd_raw, true)
}

fn restore_z_order(z_order: &[isize]) {
    unsafe {
        for &hwnd_raw in z_order.iter().rev() {
            let hwnd = HWND(hwnd_raw as *mut _);
            if hwnd.0.is_null() || !IsWindowVisible(hwnd).as_bool() {
                continue;
            }
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

unsafe extern "system" fn enum_visible_hwnd(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if hwnd.0.is_null() || !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let list = &mut *(lparam.0 as *mut Vec<isize>);
    list.push(hwnd.0 as isize);
    BOOL(1)
}

fn alt_physically_held() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LMENU, VK_MENU, VK_RMENU,
    };

    unsafe fn key_down(vk: i32) -> bool {
        GetAsyncKeyState(vk) as u16 & 0x8000 != 0
    }

    unsafe {
        key_down(VK_MENU.0 as i32) || key_down(VK_LMENU.0 as i32) || key_down(VK_RMENU.0 as i32)
    }
}

fn focus_window_impl(hwnd_raw: isize, log_result: bool) -> bool {
    if log_result {
        log::debug(format!("focus_window hwnd={hwnd_raw}"));
    }
    let hwnd = HWND(hwnd_raw as *mut _);
    if hwnd.0.is_null() {
        log::warn("focus_window: null hwnd");
        return false;
    }
    unsafe {
        if IsIconic(hwnd).as_bool() {
            log::debug(format!("focus_window: restoring minimized hwnd={hwnd_raw}"));
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let current = GetCurrentThreadId();

        let attached_fg =
            fg_thread != 0 && AttachThreadInput(current, fg_thread, true).as_bool();
        let attached_target = target_thread != 0
            && target_thread != current
            && AttachThreadInput(current, target_thread, true).as_bool();

        if !alt_physically_held() {
            keybd_event(0x12, 0, KEYBD_EVENT_FLAGS(0), 0);
            keybd_event(0x12, 0, KEYEVENTF_KEYUP, 0);
        }

        let ok = SetForegroundWindow(hwnd).as_bool() || BringWindowToTop(hwnd).is_ok();
        if log_result {
            log::debug(format!("focus_window hwnd={hwnd_raw} ok={ok}"));
        }

        if attached_target {
            let _ = AttachThreadInput(current, target_thread, false);
        }
        if attached_fg {
            let _ = AttachThreadInput(current, fg_thread, false);
        }
        ok
    }
}
