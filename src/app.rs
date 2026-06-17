use std::sync::Arc;

use parking_lot::Mutex;

use crate::config::Config;
use crate::hotkeys::{post_reload, HotkeyManager, HotkeyRegistrationResult, HotkeyAction};
use crate::launcher;
use crate::log;
use crate::modes::marks::{ToggleMarkResult, SharedMarks};
use crate::modes::same_app;

pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub marks: SharedMarks,
    pub hotkey_conflicts: usize,
    pub registration_results: Vec<HotkeyRegistrationResult>,
}

impl AppState {
    pub fn new(config: Arc<Mutex<Config>>, marks: SharedMarks) -> Self {
        log::debug("AppState::new");
        Self {
            config,
            marks,
            hotkey_conflicts: 0,
            registration_results: Vec::new(),
        }
    }
}

pub fn request_reload() {
    log::debug("request_reload");
    post_reload();
}

pub fn reload_hotkeys(manager: &mut HotkeyManager, state: &Arc<Mutex<AppState>>) {
    log::debug("reload_hotkeys");
    let config = state.lock().config.lock().clone();
    if let Err(errors) = config.validate() {
        log::warn(&format!("reload validation: {} errors", errors.len()));
        crate::hotkeys::report_config_errors(&errors);
    }
    let bindings = config.bindings();
    log::debug(&format!("reload applying {} bindings", bindings.len()));
    manager.reload(state, &bindings);
    let config = state.lock().config.lock().clone();
    crate::marks_switcher::reload_hook(&config);
}

pub fn dispatch_action(action: HotkeyAction, state: &Arc<Mutex<AppState>>) {
    log::debug(&format!("dispatch_action: {action:?}"));
    match action {
        HotkeyAction::Launcher => launcher::open(),
        HotkeyAction::SameAppNext => same_app::cycle_same_app(true),
        HotkeyAction::SameAppPrev => same_app::cycle_same_app(false),
        HotkeyAction::MarkNext => {
            let state_guard = state.lock();
            let mut marks = state_guard.marks.lock();
            let ok = marks.cycle_mark(true);
            log::debug(&format!("mark cycle next: {ok}"));
        }
        HotkeyAction::MarkPrev => {
            let state_guard = state.lock();
            let mut marks = state_guard.marks.lock();
            let ok = marks.cycle_mark(false);
            log::debug(&format!("mark cycle prev: {ok}"));
        }
        HotkeyAction::ToggleMark => {
            crate::marks_switcher::cancel_if_active();
            let state_guard = state.lock();
            let mut marks = state_guard.marks.lock();
            match marks.store.toggle_mark() {
                ToggleMarkResult::Marked { slot, app } => {
                    log::notify(
                        "WinHarpoon",
                        &format!("{app} — marked slot {slot}"),
                    );
                }
                ToggleMarkResult::Unmarked { slot, app } => {
                    log::notify(
                        "WinHarpoon",
                        &format!("{app} — unmarked slot {slot}"),
                    );
                }
                ToggleMarkResult::NoForeground => {
                    log::notify("WinHarpoon", "No window to mark");
                }
                ToggleMarkResult::AllSlotsFull { app } => {
                    log::notify(
                        "WinHarpoon",
                        &format!("{app} — all 9 mark slots are full"),
                    );
                }
            }
        }
        HotkeyAction::Mark(slot) => {
            crate::marks_switcher::cancel_if_active();
            let state_guard = state.lock();
            let mut marks = state_guard.marks.lock();
            if let Some(id) = marks.store.mark_slot(slot) {
                log::info(&format!("marked slot {slot}: {}", id.display_label()));
            } else {
                log::warn(&format!("mark slot {slot}: no foreground window"));
            }
        }
        HotkeyAction::Jump(slot) => {
            let state_guard = state.lock();
            let marks = state_guard.marks.lock();
            if marks.store.jump_slot(slot) {
                log::debug(&format!("jump slot {slot}: ok"));
            } else {
                log::warn(&format!("jump slot {slot}: missed"));
            }
        }
    }
}
