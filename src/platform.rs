#[cfg(windows)]
pub fn cover_monitor_at_physical_point(
    ctx: &eframe::egui::Context,
    physical_x: f64,
    physical_y: f64,
) {
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
        let pos = egui::pos2(rect.left as f32 / ppp, rect.top as f32 / ppp);
        let size = egui::vec2(
            (rect.right - rect.left) as f32 / ppp,
            (rect.bottom - rect.top) as f32 / ppp,
        );
        apply_overlay_geometry(ctx, pos, size);
    } else {
        cover_active_monitor(ctx);
    }
}

#[cfg(not(windows))]
pub fn cover_monitor_at_physical_point(ctx: &eframe::egui::Context, _physical_x: f64, _physical_y: f64) {
    cover_active_monitor(ctx);
}

#[cfg(windows)]
pub fn cover_active_monitor(ctx: &eframe::egui::Context) {
    use eframe::egui;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let ppp = ctx
        .input(|i| i.viewport().native_pixels_per_point)
        .unwrap_or(1.0);

    let fallback_size = || {
        ctx.input(|i| {
            i.viewport()
                .monitor_size
                .unwrap_or_else(|| ctx.content_rect().size())
        })
    };

    let mut pt = POINT::default();
    let (pos, size) = if unsafe { GetCursorPos(&mut pt).is_ok() } {
        let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() } {
            let rect = info.rcWork;
            (
                egui::pos2(rect.left as f32 / ppp, rect.top as f32 / ppp),
                egui::vec2(
                    (rect.right - rect.left) as f32 / ppp,
                    (rect.bottom - rect.top) as f32 / ppp,
                ),
            )
        } else {
            (egui::Pos2::ZERO, fallback_size())
        }
    } else {
        (egui::Pos2::ZERO, fallback_size())
    };

    apply_overlay_geometry(ctx, pos, size);
}

#[cfg(windows)]
fn apply_overlay_geometry(
    ctx: &eframe::egui::Context,
    pos: eframe::egui::Pos2,
    size: eframe::egui::Vec2,
) {
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Transparent(true));
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::OuterPosition(pos));
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::InnerSize(size));
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
}

#[cfg(windows)]
pub fn apply_dwm_transparency(frame: &eframe::Frame) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
    use windows::Win32::UI::Controls::MARGINS;

    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(win.hwnd.get() as *mut core::ffi::c_void);
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}

#[cfg(not(windows))]
pub fn cover_active_monitor(ctx: &eframe::egui::Context) {
    use eframe::egui;
    let size = ctx.input(|i| {
        i.viewport()
            .monitor_size
            .unwrap_or_else(|| ctx.screen_rect().size())
    });
    ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
}

#[cfg(not(windows))]
pub fn apply_dwm_transparency(_frame: &eframe::Frame) {}
