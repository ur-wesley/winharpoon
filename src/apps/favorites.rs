use std::fs;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::config::{parse_chord, HotkeyBinding};
use crate::hotkeys::HotkeyAction;
use crate::log;
use crate::paths;

use super::enumerate::normalize_target;
use super::AppEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteEntry {
    pub id: String,
    #[serde(default)]
    pub hotkey: String,
    #[serde(default)]
    pub target: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FavoritesStore {
    #[serde(default)]
    pub favorites: Vec<FavoriteEntry>,
}

pub type SharedFavorites = Arc<Mutex<FavoritesStore>>;

pub fn shared_favorites() -> SharedFavorites {
    Arc::new(Mutex::new(FavoritesStore::load()))
}

impl FavoritesStore {
    pub fn load() -> Self {
        paths::ensure_app_data();
        let path = paths::favorites_path();
        if path.exists() {
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(store) = toml::from_str(&text) {
                    return store;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        #[cfg(test)]
        {
            let _ = self;
        }
        #[cfg(not(test))]
        {
            paths::ensure_app_data();
            let _ = paths::atomic_write(
                &paths::favorites_path(),
                &toml::to_string_pretty(self).expect("serialize"),
            );
        }
    }

    pub fn toggle(&mut self, id: &str, target: &str) -> bool {
        if let Some(idx) = self.favorites.iter().position(|f| f.id == id) {
            self.favorites.remove(idx);
            self.save();
            false
        } else {
            self.favorites.push(FavoriteEntry {
                id: id.to_string(),
                hotkey: String::new(),
                target: target.to_string(),
            });
            self.save();
            true
        }
    }

    pub fn set_hotkey(&mut self, id: &str, hotkey: String) -> bool {
        let Some(entry) = self.favorites.iter_mut().find(|f| f.id == id) else {
            return false;
        };
        entry.hotkey = hotkey;
        self.save();
        true
    }

    pub fn remap_to(&mut self, entries: &[AppEntry]) {
        if self.favorites.is_empty() {
            return;
        }
        let known_ids: std::collections::HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        let mut changed = false;
        for fav in &mut self.favorites {
            if known_ids.contains(fav.id.as_str()) {
                continue;
            }
            let old_target = fav.target.trim();
            if old_target.is_empty() {
                continue;
            }
            // Compare canonical targets so favorites survive entry-ID
            // changes (e.g. `%VAR%` expansion, case normalization).
            let old_key = normalize_target(std::path::Path::new(old_target));
            if let Some(match_entry) = entries
                .iter()
                .find(|e| normalize_target(&e.target) == old_key)
            {
                log::debug(format!(
                    "apps: remapped favorite {} -> {}",
                    fav.id, match_entry.id
                ));
                fav.id.clone_from(&match_entry.id);
                changed = true;
            }
        }
        if changed {
            self.save();
        }
    }

    pub fn hotkey_bindings(&self) -> Vec<HotkeyBinding> {
        self.favorites
            .iter()
            .enumerate()
            .filter_map(|(i, fav)| {
                let trimmed = fav.hotkey.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let parsed = parse_chord(trimmed).ok()?;
                Some(HotkeyBinding {
                    action: HotkeyAction::LaunchFavorite(i),
                    label: format!("favorite_{}", fav.id),
                    chord: trimmed.to_string(),
                    parsed: Some(parsed),
                })
            })
            .collect()
    }
}

pub struct AppIndexRef<'a> {
    pub by_id: std::collections::HashMap<String, &'a AppEntry>,
}

impl<'a> AppIndexRef<'a> {
    pub fn new(entries: &'a [AppEntry]) -> Self {
        Self {
            by_id: entries.iter().map(|e| (e.id.clone(), e)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FavoritesStore;

    #[test]
    fn toggle_adds_and_removes() {
        let mut s = FavoritesStore::default();
        assert!(s.toggle("id1", r"C:\a.exe"));
        assert_eq!(s.favorites.len(), 1);
        assert!(!s.toggle("id1", r"C:\a.exe"));
        assert!(s.favorites.is_empty());
    }

    #[test]
    fn hotkey_bindings_skips_empty_and_bad() {
        let mut s = FavoritesStore::default();
        s.toggle("id1", r"C:\a.exe");
        s.toggle("id2", r"C:\b.exe");
        s.set_hotkey("id1", "Win+K".into());
        s.set_hotkey("id2", "not a chord!!!".into());
        assert!(!s.set_hotkey("missing", "Win+J".into()));
        let b = s.hotkey_bindings();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].chord, "Win+K");
    }
}
