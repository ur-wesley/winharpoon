mod cache;
pub mod controller;
mod enumerate;
mod favorites;
pub mod hook;
mod launch;
pub mod panel;
pub mod search;
mod view;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

use crate::config::Config;
use crate::log;
use crate::paths;

pub use favorites::{shared_favorites, AppIndexRef, SharedFavorites};
pub use controller::AppMenuAnchor;

use cache::load_or_scan;
use enumerate::{dir_mtime, start_menu_roots, scan_all};
use favorites::FavoritesStore as Store;
use launch::launch;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub target: std::path::PathBuf,
    pub args: String,
    pub source_lnk: std::path::PathBuf,
    pub search_label: String,
}

struct AppIndex {
    entries: Vec<AppEntry>,
    mtime: u64,
}

static INDEX: RwLock<Option<AppIndex>> = RwLock::new(None);
static REFRESHING: AtomicBool = AtomicBool::new(false);
static LAST_MTIME_CHECK: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let entries = load_or_scan();
        let mtime = dir_mtime(&start_menu_roots());
        *INDEX.write() = Some(AppIndex { entries, mtime });
        log::debug("apps: index ready");
        unsafe {
            CoUninitialize();
        }
    });
}

pub fn entries() -> Vec<AppEntry> {
    INDEX.read().as_ref().map(|i| i.entries.clone()).unwrap_or_default()
}

pub fn is_ready() -> bool {
    INDEX.read().is_some()
}

pub fn refresh_async() {
    if REFRESHING.swap(true, Ordering::AcqRel) {
        return;
    }
    thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let roots = start_menu_roots();
        let mtime = dir_mtime(&roots);
        let entries = scan_all();
        cache::save_cache(&paths::apps_cache_path(), mtime, &entries);
        *INDEX.write() = Some(AppIndex { entries, mtime });
        REFRESHING.store(false, Ordering::Release);
        log::debug("apps: index refreshed");
        unsafe {
            CoUninitialize();
        }
    });
}

pub fn maybe_refresh() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = LAST_MTIME_CHECK.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 60 {
        return;
    }
    LAST_MTIME_CHECK.store(now, Ordering::Relaxed);

    let roots = start_menu_roots();
    let mtime = dir_mtime(&roots);
    let stale = INDEX
        .read()
        .as_ref()
        .is_none_or(|i| i.mtime != mtime);
    if stale {
        refresh_async();
    }
}

pub fn launch_by_id(id: &str, favorites: &Store) -> bool {
    let entries = entries();
    let index = AppIndexRef::new(&entries);
    if let Some(entry) = index.by_id.get(id) {
        return launch(entry);
    }
    let _ = favorites;
    log::warn(format!("apps: unknown id {id}"));
    false
}

pub fn launch_favorite_index(idx: usize, favorites: &Store) -> bool {
    let Some(fav) = favorites.favorites.get(idx) else {
        return false;
    };
    launch_by_id(&fav.id, favorites)
}

pub fn hook_enabled(config: &Config) -> bool {
    config.apps.enabled && config.apps.alt_double_click
}

pub fn open_menu(anchor: AppMenuAnchor) {
    crate::launcher::open_app_menu(anchor);
}
