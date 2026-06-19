use std::path::PathBuf;

use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    GWL_EXSTYLE, GW_OWNER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use crate::log;
use crate::util;
use crate::window::process_name;

const MAX_TITLE_LEN: usize = 200;
const MIN_WINDOW_WIDTH: i32 = 80;
const MIN_WINDOW_HEIGHT: i32 = 40;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub exe_path: PathBuf,
    pub exe_name: String,
    pub process_name: String,
}

pub fn get_foreground_window() -> Option<WindowInfo> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            log::trace("get_foreground_window: null hwnd");
            return None;
        }
        let info = window_info(hwnd);
        if let Some(ref w) = info {
            log::trace(format!(
                "get_foreground_window: hwnd={} title={} process={}",
                w.hwnd, w.title, w.process_name
            ));
        }
        info
    }
}

pub fn enumerate_windows(exclude_hwnd: Option<isize>) -> Vec<WindowInfo> {
    log::trace(format!("enumerate_windows exclude={exclude_hwnd:?}"));
    let mut windows = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut windows as *mut _ as isize),
        );
    }
    if let Some(ex) = exclude_hwnd {
        windows.retain(|w: &WindowInfo| w.hwnd != ex);
    }
    windows.sort_by_key(|a| a.title.to_lowercase());
    log::debug(format!("enumerate_windows: {} windows", windows.len()));
    windows
}

unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<WindowInfo>);
    if let Some(info) = window_info(hwnd) {
        list.push(info);
    }
    BOOL(1)
}

unsafe fn window_info(hwnd: HWND) -> Option<WindowInfo> {
    if hwnd.0.is_null() || !is_switchable_window(hwnd) {
        return None;
    }

    let len = GetWindowTextLengthW(hwnd);
    if len == 0 {
        return None;
    }

    let mut title_buf = vec![0u16; len as usize + 1];
    GetWindowTextW(hwnd, &mut title_buf);
    let title = util::from_wide(&title_buf);
    if !is_reasonable_title(&title) {
        log::trace(format!("skip hwnd={:?}: bad title len={}", hwnd.0, title.len()));
        return None;
    }

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return None;
    }

    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    let res = QueryFullProcessImageNameW(
        process,
        PROCESS_NAME_FORMAT(0),
        windows::core::PWSTR(buf.as_mut_ptr()),
        &mut size,
    );
    let _ = CloseHandle(process);
    res.ok()?;
    let exe_path = PathBuf::from(util::from_wide(&buf[..size as usize]));
    let exe_name = exe_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".into());

    if exe_name.eq_ignore_ascii_case("winharpoon.exe") {
        return None;
    }

    let process_name = process_name::process_display_name(&exe_path);

    Some(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        exe_path,
        exe_name,
        process_name,
    })
}

unsafe fn is_switchable_window(hwnd: HWND) -> bool {
    if !IsWindowVisible(hwnd).as_bool() {
        return false;
    }

    let mut cloaked = 0u32;
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        (&mut cloaked as *mut u32).cast(),
        std::mem::size_of::<u32>() as u32,
    )
    .is_ok()
        && cloaked != 0
    {
        return false;
    }

    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if (ex_style & WS_EX_TOOLWINDOW.0) != 0 && (ex_style & WS_EX_APPWINDOW.0) == 0 {
        return false;
    }

    let owner = GetWindow(hwnd, GW_OWNER).ok();
    if owner.is_some_and(|o| !o.0.is_null()) && (ex_style & WS_EX_APPWINDOW.0) == 0 {
        return false;
    }

    if !IsIconic(hwnd).as_bool() {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 || width < MIN_WINDOW_WIDTH || height < MIN_WINDOW_HEIGHT {
            return false;
        }
    }

    true
}

fn is_reasonable_title(title: &str) -> bool {
    let trimmed = title.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_TITLE_LEN {
        return false;
    }
    if trimmed.lines().count() > 2 {
        return false;
    }
    if trimmed.matches('@').count() > 4 {
        return false;
    }
    if trimmed.starts_with("npm list") || trimmed.starts_with("pnpm list") || trimmed.starts_with("yarn list")
    {
        return false;
    }
    true
}
