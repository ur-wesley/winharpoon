use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::log;
use crate::paths;
use crate::window::identity::{resolve_identity, WindowIdentity};
use crate::window::{enumerate_windows, focus, get_foreground_window, WindowInfo};

#[derive(Debug, Clone)]
pub struct MarkEntry {
    pub slot: u8,
    pub identity: WindowIdentity,
    pub window: Option<WindowInfo>,
}

pub fn filled_entries(store: &MarksStore) -> Vec<MarkEntry> {
    let windows = enumerate_windows(None);
    (1..=9)
        .filter_map(|slot| {
            let identity = store.slots.get(&slot.to_string())?;
            let window = resolve_identity(identity, &windows).cloned();
            Some(MarkEntry {
                slot,
                identity: identity.clone(),
                window,
            })
        })
        .collect()
}

pub fn switcher_entries(store: &MarksStore) -> Vec<MarkEntry> {
    filled_entries(store)
        .into_iter()
        .filter(|entry| entry.window.is_some())
        .collect()
}

pub fn index_after_foreground(entries: &[MarkEntry]) -> usize {
    if entries.is_empty() {
        return 0;
    }
    let fg = get_foreground_window();
    let current_idx = fg.and_then(|fg_win| {
        entries.iter().position(|e| {
            e.window
                .as_ref()
                .is_some_and(|w| w.hwnd == fg_win.hwnd)
        })
    });
    current_idx.unwrap_or_default()
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MarksStore {
    #[serde(default)]
    pub slots: BTreeMap<String, WindowIdentity>,
}

impl MarksStore {
    pub fn load() -> Self {
        paths::ensure_app_data();
        let path = paths::marks_path();
        log::debug(format!("MarksStore::load from {}", path.display()));
        if path.exists() {
            if let Ok(text) = fs::read_to_string(path) {
                if let Ok(store) = toml::from_str(&text) {
                    log::debug("marks loaded from disk");
                    return store;
                }
                log::warn("marks parse failed, using empty store");
            } else {
                log::warn("marks read failed, using empty store");
            }
        } else {
            log::debug("marks file missing, using empty store");
        }
        Self::default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        #[cfg(test)]
        {
            Ok(())
        }
        #[cfg(not(test))]
        {
            paths::ensure_app_data();
            let path = paths::marks_path();
            log::debug(format!("MarksStore::save to {} ({} slots)", path.display(), self.slots.len()));
            let text = toml::to_string_pretty(self).expect("serialize marks");
            fs::write(path, text)
        }
    }

    pub fn mark_slot(&mut self, slot: u8) -> Option<WindowIdentity> {
        log::debug(format!("mark_slot {slot}"));
        let current = get_foreground_window()?;
        let identity = WindowIdentity::from_window(&current);
        self.slots.insert(slot.to_string(), identity.clone());
        let _ = self.save();
        log::info(format!("marked slot {slot}: {}", identity.display_label()));
        Some(identity)
    }

    pub fn jump_slot(&self, slot: u8) -> bool {
        log::debug(format!("jump_slot {slot}"));
        let Some(identity) = self.slots.get(&slot.to_string()) else {
            log::warn(format!("jump_slot {slot}: empty"));
            return false;
        };
        let windows = enumerate_windows(None);
        let Some(target) = resolve_identity(identity, &windows) else {
            log::warn(format!("jump_slot {slot}: window not found for {}", identity.display_label()));
            return false;
        };
        let ok = focus::focus_window(target.hwnd);
        log::debug(format!("jump_slot {slot}: focus ok={ok}"));
        ok
    }

    pub fn slot_label(&self, slot: u8) -> String {
        self.slots
            .get(&slot.to_string())
            .map(|id| id.display_label())
            .unwrap_or_else(|| "empty".into())
    }

    pub fn find_slot(&self, identity: &WindowIdentity) -> Option<u8> {
        (1..=9).find(|slot| {
            self.slots
                .get(&slot.to_string())
                .is_some_and(|stored| stored == identity)
        })
    }

    pub fn toggle_mark(&mut self) -> ToggleMarkResult {
        let Some(current) = get_foreground_window() else {
            log::warn("toggle_mark: no foreground window");
            return ToggleMarkResult::NoForeground;
        };
        self.toggle_mark_for(&current)
    }

    pub fn toggle_mark_for(&mut self, window: &WindowInfo) -> ToggleMarkResult {
        let identity = WindowIdentity::from_window(window);
        let app = window.process_name.clone();

        if let Some(slot) = self.find_slot(&identity) {
            self.slots.remove(&slot.to_string());
            let _ = self.save();
            log::info(format!("unmarked slot {slot}: {app}"));
            return ToggleMarkResult::Unmarked { slot, app };
        }

        for slot in 1..=9 {
            let key = slot.to_string();
            if let std::collections::btree_map::Entry::Vacant(e) = self.slots.entry(key) {
                e.insert(identity);
                let _ = self.save();
                log::info(format!("marked slot {slot}: {app}"));
                return ToggleMarkResult::Marked { slot, app };
            }
        }

        log::warn("toggle_mark: all slots full");
        ToggleMarkResult::AllSlotsFull { app }
    }

    pub fn move_mark_slot(&mut self, slot: u8, earlier: bool) -> bool {
        let filled: Vec<u8> = (1..=9)
            .filter(|s| self.slots.contains_key(&s.to_string()))
            .collect();
        let Some(pos) = filled.iter().position(|&s| s == slot) else {
            return false;
        };
        let swap_with = if earlier {
            if pos == 0 {
                return false;
            }
            filled[pos - 1]
        } else if pos + 1 >= filled.len() {
            return false;
        } else {
            filled[pos + 1]
        };
        self.swap_slots(slot, swap_with);
        true
    }

    fn swap_slots(&mut self, a: u8, b: u8) {
        let key_a = a.to_string();
        let key_b = b.to_string();
        let val_a = self.slots.remove(&key_a);
        let val_b = self.slots.remove(&key_b);
        if let Some(v) = val_a {
            self.slots.insert(key_b, v);
        }
        if let Some(v) = val_b {
            self.slots.insert(key_a, v);
        }
        let _ = self.save();
    }
}

#[derive(Debug, Clone)]
pub enum ToggleMarkResult {
    Marked { slot: u8, app: String },
    Unmarked { slot: u8, app: String },
    NoForeground,
    AllSlotsFull { app: String },
}

pub struct MarksState {
    pub store: MarksStore,
    pub cycle_index: usize,
}

impl MarksState {
    pub fn new() -> Self {
        log::debug("MarksState::new");
        Self {
            store: MarksStore::load(),
            cycle_index: 0,
        }
    }

    pub fn cycle_mark(&mut self, forward: bool) -> bool {
        log::debug(format!("cycle_mark forward={forward}"));
        let filled: Vec<u8> = (1..=9)
            .filter(|slot| self.store.slots.contains_key(&slot.to_string()))
            .collect();
        if filled.is_empty() {
            log::debug("cycle_mark: no filled slots");
            return false;
        }
        if forward {
            self.cycle_index = (self.cycle_index + 1) % filled.len();
        } else if self.cycle_index == 0 {
            self.cycle_index = filled.len() - 1;
        } else {
            self.cycle_index -= 1;
        }
        let slot = filled[self.cycle_index];
        log::debug(format!("cycle_mark: slot {slot} (index {})", self.cycle_index));
        self.store.jump_slot(slot)
    }
}

pub type SharedMarks = Arc<Mutex<MarksState>>;

pub fn shared_marks() -> SharedMarks {
    log::debug("shared_marks");
    Arc::new(Mutex::new(MarksState::new()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn identity(label: &str) -> WindowIdentity {
        WindowIdentity {
            exe: PathBuf::from(format!("C:\\apps\\{label}.exe")),
            title: label.into(),
        }
    }

    #[test]
    fn move_mark_slot_swaps_with_neighbor() {
        let mut store = MarksStore::default();
        store.slots.insert("1".into(), identity("a"));
        store.slots.insert("3".into(), identity("b"));
        store.slots.insert("5".into(), identity("c"));

        assert!(store.move_mark_slot(3, true));
        assert_eq!(store.slots.get("1").map(|i| i.title.as_str()), Some("b"));
        assert_eq!(store.slots.get("3").map(|i| i.title.as_str()), Some("a"));

        assert!(store.move_mark_slot(3, false));
        assert_eq!(store.slots.get("3").map(|i| i.title.as_str()), Some("c"));
        assert_eq!(store.slots.get("5").map(|i| i.title.as_str()), Some("a"));
    }

    #[test]
    fn move_mark_slot_noop_at_edges() {
        let mut store = MarksStore::default();
        store.slots.insert("2".into(), identity("only"));

        assert!(!store.move_mark_slot(2, true));
        assert!(!store.move_mark_slot(2, false));
        assert!(!store.move_mark_slot(9, true));
    }
}
