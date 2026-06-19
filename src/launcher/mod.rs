pub mod controller;
pub mod panel;
mod view;

use std::sync::mpsc::Sender;
use std::sync::{Arc, Once, OnceLock};
use std::thread;

use egui::Context;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::apps::controller::AppMenuAnchor;
use crate::config::Config;
use crate::log;
use crate::modes::marks::SharedMarks;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCommand {
    Launcher,
    Settings,
    TrayMenu,
    AppMenu,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrayClickInfo {
    pub click_x: f64,
    pub click_y: f64,
    pub rect_x: f64,
    pub rect_y: f64,
    pub rect_w: u32,
    pub rect_h: u32,
}

static INIT: Once = Once::new();
static COMMAND_TX: OnceLock<Sender<UiCommand>> = OnceLock::new();
static UI_CTX: OnceLock<Context> = OnceLock::new();
static PENDING_TRAY_MENU: Mutex<Option<TrayClickInfo>> = Mutex::new(None);
static PENDING_APP_MENU: Mutex<Option<AppMenuAnchor>> = Mutex::new(None);

pub fn init(config: Arc<Mutex<Config>>, state: Arc<Mutex<AppState>>, marks: SharedMarks) {
    INIT.call_once(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = COMMAND_TX.set(tx);
        thread::spawn(move || crate::ui::app::run_ui(config, state, marks, rx));
        log::debug("ui thread started");
    });
}

pub(crate) fn register_context(ctx: &Context) {
    let _ = UI_CTX.set(ctx.clone());
    log::debug("ui context registered");
}

pub(crate) fn take_pending_tray_menu() -> Option<TrayClickInfo> {
    PENDING_TRAY_MENU.lock().take()
}

pub(crate) fn take_pending_app_menu() -> Option<AppMenuAnchor> {
    PENDING_APP_MENU.lock().take()
}

fn signal(command: UiCommand) {
    if let Some(tx) = COMMAND_TX.get() {
        log::debug(format!("ui command signaled: {command:?}"));
        let _ = tx.send(command);
    } else {
        log::warn(format!("ui command before init: {command:?}"));
    }
    if let Some(ctx) = UI_CTX.get() {
        ctx.request_repaint();
    }
}

pub fn open() {
    signal(UiCommand::Launcher);
}

pub fn open_settings() {
    signal(UiCommand::Settings);
}

pub fn open_tray_menu(info: TrayClickInfo) {
    *PENDING_TRAY_MENU.lock() = Some(info);
    signal(UiCommand::TrayMenu);
}

pub fn open_app_menu(anchor: AppMenuAnchor) {
    *PENDING_APP_MENU.lock() = Some(anchor);
    signal(UiCommand::AppMenu);
}

pub fn ui_context() -> Option<&'static Context> {
    UI_CTX.get()
}
