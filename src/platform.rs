#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassBackdrop {
    Acrylic,
    Mica,
    None,
}

pub const MAIN_VIEWPORT_TITLE: &str = "WinHarpoon";
pub const SETTINGS_VIEWPORT_TITLE: &str = "WinHarpoon Settings";

#[cfg(windows)]
fn hwnd_from_frame(frame: &eframe::Frame) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;

    let handle = frame.window_handle().ok()?;
    let RawWindowHandle::Win32(win) = handle.as_raw() else {
        return None;
    };
    Some(HWND(win.hwnd.get() as *mut core::ffi::c_void))
}

#[cfg(windows)]
fn hwnd_from_title(title: &str) -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    use windows::core::PCWSTR;

    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { FindWindowW(None, PCWSTR(wide.as_ptr())) }.ok()?;
    if hwnd == HWND::default() {
        None
    } else {
        Some(hwnd)
    }
}

#[cfg(windows)]
fn resolve_main_hwnd(frame: Option<&eframe::Frame>) -> Option<windows::Win32::Foundation::HWND> {
    if let Some(frame) = frame {
        if let Some(hwnd) = hwnd_from_frame(frame) {
            return Some(hwnd);
        }
    }
    hwnd_from_title(MAIN_VIEWPORT_TITLE)
}

#[cfg(windows)]
fn maintain_borderless_overlay(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_NCRENDERING_POLICY, DWMWA_VISIBLE_FRAME_BORDER_THICKNESS,
        DWMNCRP_USEWINDOWSTYLE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION, WS_SYSMENU,
        WS_THICKFRAME,
    };

    let nc_policy = DWMNCRP_USEWINDOWSTYLE.0;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &nc_policy as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }

    let border_thickness: i32 = 0;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_VISIBLE_FRAME_BORDER_THICKNESS,
            &border_thickness as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }

    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let stripped = style & !(WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0);
        if stripped != style {
            SetWindowLongPtrW(hwnd, GWL_STYLE, stripped as _);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

#[cfg(windows)]
fn apply_dwm_glass_to_hwnd(hwnd: windows::Win32::Foundation::HWND, backdrop: GlassBackdrop) {
    use windows::Win32::Graphics::Dwm::{
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMSBT_MAINWINDOW,
        DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWCP_ROUND,
    };
    use windows::Win32::UI::Controls::MARGINS;

    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }

    let dark_mode: i32 = 1;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }

    let backdrop_type = match backdrop {
        GlassBackdrop::Acrylic => DWMSBT_TRANSIENTWINDOW,
        GlassBackdrop::Mica => DWMSBT_MAINWINDOW,
        GlassBackdrop::None => DWMSBT_NONE,
    };
    let backdrop_value = backdrop_type.0;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop_value as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }

    if backdrop != GlassBackdrop::None {
        let corner = DWMWCP_ROUND.0;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const i32 as *const _,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
}

#[cfg(windows)]
static OVERLAY_GLASS_APPLIED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
pub fn refresh_popup_glass(frame: Option<&eframe::Frame>, focused: bool) {
    let Some(hwnd) = resolve_main_hwnd(frame) else {
        return;
    };
    maintain_borderless_overlay(hwnd);
    let backdrop = if focused {
        GlassBackdrop::Acrylic
    } else {
        GlassBackdrop::None
    };
    apply_dwm_glass_to_hwnd(hwnd, backdrop);
    OVERLAY_GLASS_APPLIED.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(windows)]
pub fn reset_popup_glass() {
    OVERLAY_GLASS_APPLIED.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(windows))]
pub fn refresh_popup_glass(_frame: Option<&eframe::Frame>, _focused: bool) {}

#[cfg(not(windows))]
pub fn reset_popup_glass() {}

#[cfg(windows)]
pub fn apply_dwm_glass_for_title(title: &str, backdrop: GlassBackdrop) {
    let Some(hwnd) = hwnd_from_title(title) else {
        return;
    };
    apply_dwm_glass_to_hwnd(hwnd, backdrop);
}

#[cfg(not(windows))]
pub fn apply_dwm_glass_for_title(_title: &str, _backdrop: GlassBackdrop) {}

#[cfg(windows)]
pub fn monitor_work_area_at_physical_point(
    ctx: &eframe::egui::Context,
    physical_x: f64,
    physical_y: f64,
) -> eframe::egui::Rect {
    use eframe::egui;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    };

    let ppp = ctx
        .input(|i| i.viewport().native_pixels_per_point)
        .unwrap_or(1.0);

    let pt = POINT {
        x: physical_x as i32,
        y: physical_y as i32,
    };
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() } {
        let rect = info.rcWork;
        egui::Rect::from_min_max(
            egui::pos2(rect.left as f32 / ppp, rect.top as f32 / ppp),
            egui::pos2(rect.right as f32 / ppp, rect.bottom as f32 / ppp),
        )
    } else {
        ctx.input(|i| {
            i.viewport()
                .monitor_size
                .map(|size| egui::Rect::from_min_size(egui::Pos2::ZERO, size))
                .unwrap_or_else(|| ctx.content_rect())
        })
    }
}

#[cfg(not(windows))]
pub fn monitor_work_area_at_physical_point(
    ctx: &eframe::egui::Context,
    _physical_x: f64,
    _physical_y: f64,
) -> eframe::egui::Rect {
    use eframe::egui;
    ctx.input(|i| {
        i.viewport()
            .monitor_size
            .map(|size| egui::Rect::from_min_size(egui::Pos2::ZERO, size))
            .unwrap_or_else(|| ctx.content_rect())
    })
}


