pub mod controller;
pub mod panel;
mod view;

use std::sync::Arc;

use parking_lot::Mutex;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tray_icon::{MouseButton, MouseButtonState};

use crate::app::AppState;
use crate::launcher::{open_tray_menu, TrayClickInfo};
use crate::log;

pub fn init_tray(state: &Arc<Mutex<AppState>>) -> Option<TrayIcon> {
    log::debug("init_tray");
    let tooltip = tray_tooltip(state);

    let builder = TrayIconBuilder::new().with_tooltip(tooltip);
    let builder = if let Some(icon) = tray_icon() {
        builder.with_icon(icon)
    } else {
        log::warn("tray icon unavailable, continuing without icon");
        builder
    };

    match builder.build() {
        Ok(tray) => {
            tray.set_show_menu_on_left_click(false);
            log::debug("tray icon created");
            Some(tray)
        }
        Err(err) => {
            log::error(format!("tray icon build failed: {err:?}"));
            None
        }
    }
}

pub fn drain_tray_events() {
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click {
            button,
            button_state,
            rect,
            position,
            ..
        } = event
        {
            if button_state != MouseButtonState::Up {
                continue;
            }
            if !matches!(button, MouseButton::Left | MouseButton::Right) {
                continue;
            }
            log::debug(format!("tray click at ({}, {})", position.x, position.y));
            open_tray_menu(TrayClickInfo {
                click_x: position.x,
                click_y: position.y,
                rect_x: rect.position.x,
                rect_y: rect.position.y,
                rect_w: rect.size.width,
                rect_h: rect.size.height,
            });
        }
    }
}

fn tray_tooltip(state: &Arc<Mutex<AppState>>) -> String {
    let conflicts = state.lock().hotkey_conflicts;
    if conflicts > 0 {
        format!("WinHarpoon ({conflicts} hotkey conflicts)")
    } else {
        "WinHarpoon".into()
    }
}

fn tray_icon() -> Option<Icon> {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(icon) = Icon::from_path(exe, None) {
            return Some(icon);
        }
    }
    icon_from_bytes(include_bytes!("../../assets/winharpoon.ico"))
}

fn icon_from_bytes(bytes: &[u8]) -> Option<Icon> {
    let image = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).ok()
}
