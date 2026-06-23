use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::log;
use crate::paths;

use super::enumerate;
use super::AppEntry;

#[derive(Debug, Serialize, Deserialize)]
struct AppsCacheFile {
    entries: Vec<AppEntry>,
}

pub fn load_or_scan() -> Vec<AppEntry> {
    let path = paths::apps_cache_path();
    if path.exists() {
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(cache) = toml::from_str::<AppsCacheFile>(&text) {
                if !cache.entries.is_empty() {
                    log::debug(format!("apps: loaded {} from cache", cache.entries.len()));
                    return cache.entries;
                }
            }
        }
    }
    let entries = enumerate::scan_all();
    save_cache(&path, &entries);
    entries
}

pub fn save_cache(path: &Path, entries: &[AppEntry]) {
    #[cfg(test)]
    {
        let _ = (path, entries);
    }
    #[cfg(not(test))]
    {
        paths::ensure_app_data();
        let file = AppsCacheFile {
            entries: entries.to_vec(),
        };
        if let Ok(text) = toml::to_string(&file) {
            let _ = fs::write(path, text);
        }
    }
}