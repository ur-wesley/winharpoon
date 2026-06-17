#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod config;
mod hotkeys;
mod launcher;
mod log;
mod marks_switcher;
mod modes;
mod native_ui;
mod paths;
mod platform;
mod settings;
mod tray;
mod util;
mod window;

use std::sync::Arc;

use parking_lot::Mutex;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::CreateMutexW;
use crate::app::{dispatch_action, reload_hotkeys, AppState};
use crate::config::Config;
use crate::hotkeys::{report_config_errors, HotkeyManager};
use crate::modes::marks::shared_marks;
use crate::tray::init_tray;

const SINGLE_INSTANCE_MUTEX: &str = "WinHarpoon_SingleInstance";

fn main() {
    paths::ensure_app_data();
    log::init_toast();

    if handle_installer_flags() {
        return;
    }

    log::info("starting winharpoon");
    log::debug(&format!("app data dir: {}", paths::app_data_dir().display()));
    log::debug(&format!("log file: {}", paths::log_path().display()));
    util::release_stuck_modifier_keys();
    log::debug("released stuck modifier keys on startup");

    if !acquire_single_instance() {
        log::warn("second instance blocked by single-instance mutex");
        log::notify(
            "WinHarpoon",
            "WinHarpoon is already running in the system tray.",
        );
        return;
    }
    log::debug("single-instance mutex acquired");

    let config = Arc::new(Mutex::new(Config::load()));
    autostart::sync_from_config(config.lock().general.autostart);
    let marks = shared_marks();
    let state = Arc::new(Mutex::new(AppState::new(config.clone(), marks.clone())));
    marks_switcher::init(marks.clone(), config.clone());
    launcher::init(config.clone(), state.clone(), marks);
    log::debug("app state initialized");

    let bindings = match config.lock().validate() {
        Ok(bindings) => {
            log::debug(&format!("config validated, {} bindings", bindings.len()));
            bindings
        }
        Err(errors) => {
            log::warn(&format!(
                "config validation failed with {} errors, using raw bindings",
                errors.len()
            ));
            report_config_errors(&errors);
            config.lock().bindings()
        }
    };

    let hotkey_manager = HotkeyManager::register(&state, &bindings);
    let hotkeys = Arc::new(Mutex::new(hotkey_manager));
    let _tray = init_tray(state.clone());
    log::debug("tray icon initialized, entering message loop");

    hotkeys.lock().run_message_loop(
        state.clone(),
        |action, state| dispatch_action(action, state),
        |manager, state| {
            reload_hotkeys(manager, state);
        },
        || {
            crate::marks_switcher::hook::ensure_installed();
            crate::tray::drain_tray_events();
        },
    );
    log::info("message loop exited, shutting down");
    marks_switcher::hook::uninstall_hook();
}

fn handle_installer_flags() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--enable-autostart") {
        let mut config = Config::load();
        config.general.autostart = true;
        if let Err(err) = config.save() {
            log::error(&format!("failed to save config for autostart: {err}"));
            return true;
        }
        match autostart::apply(true) {
            Ok(()) => log::info("autostart enabled by installer"),
            Err(err) => log::error(&format!("failed to enable autostart: {err}")),
        }
        return true;
    }
    if args.iter().any(|arg| arg == "--disable-autostart") {
        let mut config = Config::load();
        config.general.autostart = false;
        let _ = config.save();
        match autostart::apply(false) {
            Ok(()) => log::info("autostart disabled by uninstaller"),
            Err(err) => log::error(&format!("failed to disable autostart: {err}")),
        }
        return true;
    }
    false
}

fn acquire_single_instance() -> bool {
    let name = util::wide(SINGLE_INSTANCE_MUTEX);
    unsafe {
        let handle = CreateMutexW(None, true, windows::core::PCWSTR(name.as_ptr()));
        if let Ok(handle) = handle {
            if windows::Win32::Foundation::GetLastError()
                == windows::Win32::Foundation::ERROR_ALREADY_EXISTS
            {
                log::debug("CreateMutexW: ERROR_ALREADY_EXISTS");
                let _ = CloseHandle(handle);
                return false;
            }
            log::debug(&format!("CreateMutexW handle: {:?}", handle.0));
            true
        } else {
            log::error("CreateMutexW failed");
            false
        }
    }
}
