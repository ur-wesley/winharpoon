mod app_paths;
mod apps_folder;
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
use enumerate::scan_all;
use favorites::FavoritesStore as Store;
use launch::launch;

const REFRESH_TTL_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AppSource {
    #[default]
    StartMenu,
    AppsFolder,
    AppPath,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub target: std::path::PathBuf,
    pub args: String,
    pub source_lnk: std::path::PathBuf,
    pub search_label: String,
    #[serde(default)]
    pub aumid: Option<String>,
    #[serde(default)]
    pub source: AppSource,
}

struct AppIndex {
    entries: Vec<AppEntry>,
    scanned_at: u64,
}

static INDEX: RwLock<Option<AppIndex>> = RwLock::new(None);
static REFRESHING: AtomicBool = AtomicBool::new(false);
static LAST_REFRESH_CHECK: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    thread::spawn(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let entries = load_or_scan();
        shared_favorites().lock().remap_to(&entries);
        let scanned_at = now_secs();
        *INDEX.write() = Some(AppIndex { entries, scanned_at });
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
        let entries = scan_all();
        shared_favorites().lock().remap_to(&entries);
        cache::save_cache(&paths::apps_cache_path(), &entries);
        let scanned_at = now_secs();
        *INDEX.write() = Some(AppIndex { entries, scanned_at });
        LAST_REFRESH_CHECK.store(scanned_at, Ordering::Relaxed);
        REFRESHING.store(false, Ordering::Release);
        log::debug("apps: index refreshed");
        unsafe {
            CoUninitialize();
        }
    });
}

pub fn maybe_refresh() {
    let now = now_secs();
    let last = LAST_REFRESH_CHECK.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 60 {
        return;
    }
    LAST_REFRESH_CHECK.store(now, Ordering::Relaxed);

    let stale = INDEX
        .read()
        .as_ref()
        .is_none_or(|i| now.saturating_sub(i.scanned_at) > REFRESH_TTL_SECS);
    if stale {
        refresh_async();
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

pub(crate) fn app_paths_scan() -> Vec<AppEntry> {
    app_paths::scan()
}

pub(crate) fn apps_folder_scan() -> Vec<AppEntry> {
    apps_folder::scan()
}
