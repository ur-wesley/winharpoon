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
    Some(HWND(crate::win_cast::raw_to_mut_c_void(win.hwnd.get())))
}

#[cfg(windows)]
fn hwnd_from_title(title: &str) -> Option<windows::Win32::Foundation::HWND> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

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
        DwmSetWindowAttribute, DWMNCRP_USEWINDOWSTYLE, DWMWA_NCRENDERING_POLICY,
        DWMWA_VISIBLE_FRAME_BORDER_THICKNESS,
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
            std::ptr::addr_of!(nc_policy).cast::<core::ffi::c_void>(),
            crate::win_cast::size_of_u32::<i32>(),
        );
    }

    let border_thickness: i32 = 0;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_VISIBLE_FRAME_BORDER_THICKNESS,
            std::ptr::addr_of!(border_thickness).cast::<core::ffi::c_void>(),
            crate::win_cast::size_of_u32::<i32>(),
        );
    }

    unsafe {
        let style = crate::win_cast::long_ptr_to_u32(GetWindowLongPtrW(hwnd, GWL_STYLE));
        let stripped = style & !(WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0);
        if stripped != style {
            SetWindowLongPtrW(hwnd, GWL_STYLE, crate::win_cast::u32_to_long_ptr(stripped));
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
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMSBT_NONE,
        DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
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
            std::ptr::addr_of!(dark_mode).cast::<core::ffi::c_void>(),
            crate::win_cast::size_of_u32::<i32>(),
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
            std::ptr::addr_of!(backdrop_value).cast::<core::ffi::c_void>(),
            crate::win_cast::size_of_u32::<i32>(),
        );
    }

    if backdrop != GlassBackdrop::None {
        let corner = DWMWCP_ROUND.0;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                std::ptr::addr_of!(corner).cast::<core::ffi::c_void>(),
                crate::win_cast::size_of_u32::<i32>(),
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
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let ppp = ctx
        .input(|i| i.viewport().native_pixels_per_point)
        .unwrap_or(1.0);

    let pt = POINT {
        x: crate::win_cast::f64_to_i32(physical_x),
        y: crate::win_cast::f64_to_i32(physical_y),
    };
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: crate::win_cast::size_of_u32::<MONITORINFO>(),
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() } {
        let rect = info.rcWork;
        egui::Rect::from_min_max(
            egui::pos2(
                crate::win_cast::px_to_f32(rect.left) / ppp,
                crate::win_cast::px_to_f32(rect.top) / ppp,
            ),
            egui::pos2(
                crate::win_cast::px_to_f32(rect.right) / ppp,
                crate::win_cast::px_to_f32(rect.bottom) / ppp,
            ),
        )
    } else {
        ctx.input(|i| {
            i.viewport().monitor_size.map_or_else(
                || ctx.content_rect(),
                |size| egui::Rect::from_min_size(egui::Pos2::ZERO, size),
            )
        })
    }
}

#[cfg(not(windows))]
pub fn monitor_work_area_at_physical_point(
    ctx: &eframe::egui::Context,
    _physical_x: f64,
    _physical_y: f64,
) -> eframe::egui::Rect {
    fallback_monitor_rect(ctx)
}

#[cfg(windows)]
pub fn active_monitor_work_area(ctx: &eframe::egui::Context) -> eframe::egui::Rect {
    // Focus-first placement: center popups on the monitor with the current
    // focus, not always the main monitor.
    // Priority: foreground window center -> cursor -> fallback.
    if let Some((x, y)) = foreground_center_physical() {
        return monitor_work_area_at_physical_point(ctx, f64::from(x), f64::from(y));
    }
    if let Some((x, y)) = cursor_physical_pos() {
        return monitor_work_area_at_physical_point(ctx, f64::from(x), f64::from(y));
    }
    fallback_monitor_rect(ctx)
}

#[cfg(not(windows))]
pub fn active_monitor_work_area(ctx: &eframe::egui::Context) -> eframe::egui::Rect {
    fallback_monitor_rect(ctx)
}

#[cfg(not(windows))]
fn fallback_monitor_rect(ctx: &eframe::egui::Context) -> eframe::egui::Rect {
    use eframe::egui;
    ctx.input(|i| {
        i.viewport().monitor_size.map_or_else(
            || ctx.content_rect(),
            |size| egui::Rect::from_min_size(egui::Pos2::ZERO, size),
        )
    })
}

#[cfg(windows)]
fn fallback_monitor_rect(ctx: &eframe::egui::Context) -> eframe::egui::Rect {
    use eframe::egui;
    ctx.input(|i| {
        i.viewport().monitor_size.map_or_else(
            || ctx.content_rect(),
            |size| egui::Rect::from_min_size(egui::Pos2::ZERO, size),
        )
    })
}

#[cfg(windows)]
fn cursor_physical_pos() -> Option<(i32, i32)> {
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut pt = windows::Win32::Foundation::POINT::default();
    unsafe { GetCursorPos(&mut pt).ok()? };
    Some((pt.x, pt.y))
}

#[cfg(windows)]
fn foreground_center_physical() -> Option<(i32, i32)> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut rect = windows::Win32::Foundation::RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }
        // Ignore our own hidden overlay parked off-screen.
        if rect.left <= -10_000 && rect.top <= -10_000 {
            return None;
        }
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return None;
        }
        Some((
            i32::midpoint(rect.left, rect.right),
            i32::midpoint(rect.top, rect.bottom),
        ))
    }
}

/// Center `size` in `work_area`, clamped inside with a small margin so the
/// popup never hangs off-screen. Pure math — unit tested.
pub fn center_in_work_area(
    work_area: eframe::egui::Rect,
    size: eframe::egui::Vec2,
) -> eframe::egui::Pos2 {
    use eframe::egui;
    const MARGIN: f32 = 8.0;
    let mut pos = egui::pos2(
        work_area.center().x - size.x / 2.0,
        work_area.center().y - size.y / 2.0,
    );
    // If the popup is larger than the work area, pin to the top-left margin.
    if size.x + MARGIN * 2.0 >= work_area.width() {
        pos.x = work_area.min.x + MARGIN;
    } else {
        pos.x = pos
            .x
            .clamp(work_area.min.x + MARGIN, work_area.max.x - size.x - MARGIN);
    }
    if size.y + MARGIN * 2.0 >= work_area.height() {
        pos.y = work_area.min.y + MARGIN;
    } else {
        pos.y = pos
            .y
            .clamp(work_area.min.y + MARGIN, work_area.max.y - size.y - MARGIN);
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::center_in_work_area;

    #[test]
    fn centers_popup_in_secondary_monitor_work_area() {
        // Secondary monitor at x=[1920,3840]: popup must center there,
        // not on the main monitor at x=0.
        let work_area = eframe::egui::Rect::from_min_max(
            eframe::egui::pos2(1920.0, 0.0),
            eframe::egui::pos2(3840.0, 1080.0),
        );
        let pos = center_in_work_area(work_area, eframe::egui::vec2(480.0, 420.0));
        assert!((pos.x - (1920.0 + (1920.0 - 480.0) / 2.0)).abs() < 0.01);
        assert!((pos.y - ((1080.0 - 420.0) / 2.0)).abs() < 0.01);
        assert!(pos.x >= 1920.0);
    }

    #[test]
    fn oversized_popup_pins_to_work_area_margin() {
        let work_area = eframe::egui::Rect::from_min_max(
            eframe::egui::pos2(1920.0, 0.0),
            eframe::egui::pos2(2560.0, 1080.0),
        );
        let pos = center_in_work_area(work_area, eframe::egui::vec2(2000.0, 1200.0));
        assert!((pos.x - 1928.0).abs() < 0.01);
        assert!((pos.y - 8.0).abs() < 0.01);
    }
}
