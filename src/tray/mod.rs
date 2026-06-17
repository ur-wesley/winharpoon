pub mod ui;

use std::sync::Arc;

use parking_lot::Mutex;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tray_icon::{MouseButton, MouseButtonState};

use crate::app::AppState;
use crate::launcher::{open_tray_menu, TrayClickInfo};
use crate::log;

pub fn init_tray(state: Arc<Mutex<AppState>>) -> TrayIcon {
    log::debug("init_tray");
    let icon = tray_icon().expect("tray icon");
    let tooltip = tray_tooltip(&state);

    let tray = TrayIconBuilder::new()
        .with_tooltip(tooltip)
        .with_icon(icon)
        .build()
        .expect("tray icon");

    tray.set_show_menu_on_left_click(false);
    log::debug("tray icon created");
    let _ = state;
    tray
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
            log::debug(&format!(
                "tray click at ({}, {})",
                position.x, position.y
            ));
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
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = ((y * SIZE + x) * 4) as usize;
            let edge = x == 0 || y == 0 || x == SIZE - 1 || y == SIZE - 1;
            let inner = x >= 8 && x <= 23 && y >= 8 && y <= 23;
            if edge || inner {
                rgba[i] = 99;
                rgba[i + 1] = 140;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).ok()
}
