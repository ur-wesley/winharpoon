use std::fs;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::config::{parse_chord, HotkeyBinding};
use crate::hotkeys::HotkeyAction;
use crate::log;
use crate::paths;

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

    pub fn save(&self) -> std::io::Result<()> {
        #[cfg(test)]
        {
            Ok(())
        }
        #[cfg(not(test))]
        {
            paths::ensure_app_data();
            fs::write(paths::favorites_path(), toml::to_string_pretty(self).expect("serialize"))
        }
    }

    pub fn toggle(&mut self, id: &str, target: &str) -> bool {
        if let Some(idx) = self.favorites.iter().position(|f| f.id == id) {
            self.favorites.remove(idx);
            let _ = self.save();
            false
        } else {
            self.favorites.push(FavoriteEntry {
                id: id.to_string(),
                hotkey: String::new(),
                target: target.to_string(),
            });
            let _ = self.save();
            true
        }
    }

    pub fn set_hotkey(&mut self, id: &str, hotkey: String) -> bool {
        let Some(entry) = self.favorites.iter_mut().find(|f| f.id == id) else {
            return false;
        };
        entry.hotkey = hotkey;
        let _ = self.save();
        true
    }

    pub fn remap_to(&mut self, entries: &[AppEntry]) {
        if self.favorites.is_empty() {
            return;
        }
        let known_ids: std::collections::HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        let mut changed = false;
        for fav in self.favorites.iter_mut() {
            if known_ids.contains(fav.id.as_str()) {
                continue;
            }
            let old_target = fav.target.to_ascii_lowercase();
            if old_target.is_empty() {
                continue;
            }
            if let Some(match_entry) = entries.iter().find(|e| {
                e.target.to_string_lossy().to_ascii_lowercase() == old_target
            }) {
                log::debug(format!(
                    "apps: remapped favorite {} -> {}",
                    fav.id, match_entry.id
                ));
                fav.id = match_entry.id.clone();
                changed = true;
            }
        }
        if changed {
            let _ = self.save();
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
