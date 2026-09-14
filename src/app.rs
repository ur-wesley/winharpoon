use std::sync::Arc;

use parking_lot::Mutex;

use crate::config::Config;
use crate::hotkeys::{post_reload, HotkeyManager, HotkeyRegistrationResult, HotkeyAction};
use crate::launcher;
use crate::log;
use crate::apps::{self, SharedFavorites};
use crate::modes::marks::{ToggleMarkResult, SharedMarks};
use crate::modes::same_app;

pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub marks: SharedMarks,
    pub favorites: SharedFavorites,
    pub hotkey_conflicts: usize,
    pub registration_results: Vec<HotkeyRegistrationResult>,
}

impl AppState {
    pub fn new(config: Arc<Mutex<Config>>, marks: SharedMarks, favorites: SharedFavorites) -> Self {
        log::debug("AppState::new");
        Self {
            config,
            marks,
            favorites,
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
    let favorite_bindings = state.lock().favorites.lock().hotkey_bindings();
    if let Err(errors) = config.validate_merged(&favorite_bindings) {
        log::warn(format!("reload validation: {} errors", errors.len()));
        crate::hotkeys::report_config_errors(&errors);
    }
    let mut all = config.bindings();
    all.extend(favorite_bindings);
    log::debug(format!("reload applying {} bindings", all.len()));
    manager.reload(state, &all);
    let config = state.lock().config.lock().clone();
    crate::marks_switcher::reload_hook(&config);
    apps::hook::reload(&config);
}

pub fn dispatch_action(action: HotkeyAction, state: &Arc<Mutex<AppState>>) {
    log::debug(format!("dispatch_action: {action:?}"));
    match action {
        HotkeyAction::Launcher => launcher::open(),
        HotkeyAction::SameAppNext | HotkeyAction::SameAppPrev => {
            let forward = action == HotkeyAction::SameAppNext;
            match same_app::cycle_same_app(forward) {
                same_app::CycleResult::Cycled => {}
                same_app::CycleResult::NoForeground => {
                    log::notify("WinHarpoon", "No window to cycle from");
                }
                same_app::CycleResult::SingleWindow(app) => {
                    log::notify("WinHarpoon", &format!("Only one {app} window"));
                }
            }
        }
        HotkeyAction::MarkNext => {
            let state_guard = state.lock();
            let mut marks = state_guard.marks.lock();
            if !marks.cycle_mark(true) {
                log::notify("WinHarpoon", "No marked windows to cycle");
            }
        }
        HotkeyAction::MarkPrev => {
            let state_guard = state.lock();
            let mut marks = state_guard.marks.lock();
            if !marks.cycle_mark(false) {
                log::notify("WinHarpoon", "No marked windows to cycle");
            }
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
                log::info(format!("marked slot {slot}: {}", id.display_label()));
            } else {
                log::notify("WinHarpoon", "No window to mark");
            }
        }
        HotkeyAction::Jump(slot) => {
            let state_guard = state.lock();
            let marks = state_guard.marks.lock();
            if marks.store.jump_slot(slot) {
                log::debug(format!("jump slot {slot}: ok"));
            } else {
                log::notify("WinHarpoon", &format!("Slot {slot} is empty or closed"));
            }
        }
        HotkeyAction::MarksSwitcherNext | HotkeyAction::MarksSwitcherPrev => {}
        HotkeyAction::LaunchFavorite(idx) => {
            let state_guard = state.lock();
            let favs = state_guard.favorites.lock();
            if !apps::launch_favorite_index(idx, &favs) {
                log::notify("WinHarpoon", "Could not launch favorite");
            }
        }
    }
}
