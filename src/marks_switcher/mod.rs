pub mod hook;
pub mod ui;

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use crate::config::Config;
use crate::modes::marks::{MarkEntry, SharedMarks};

#[derive(Debug, Clone)]
pub enum SwitcherUiCommand {
    Show {
        entries: Vec<MarkEntry>,
        selected: usize,
    },
    SetSelected(usize),
    Hide,
}

static UI_TX: OnceLock<Sender<SwitcherUiCommand>> = OnceLock::new();
static UI_RX: OnceLock<Mutex<Option<Receiver<SwitcherUiCommand>>>> = OnceLock::new();

pub fn init(marks: SharedMarks, config: Arc<Mutex<Config>>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = UI_TX.set(tx);
    let _ = UI_RX.set(Mutex::new(Some(rx)));
    hook::install(marks, config);
    log::debug("marks_switcher initialized");
}

pub fn ui_sender() -> Option<&'static Sender<SwitcherUiCommand>> {
    UI_TX.get()
}

pub fn take_ui_receiver() -> Option<Receiver<SwitcherUiCommand>> {
    UI_RX.get()?.lock().take()
}

pub fn reload_hook(config: &Config) {
    hook::reload_chord(config);
}

pub fn cancel_if_active() {
    hook::cancel_if_active();
}

use crate::log;
