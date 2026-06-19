use std::path::PathBuf;

use crate::log;

pub fn app_data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "winharpoon")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("config.toml")
}

pub fn marks_path() -> PathBuf {
    app_data_dir().join("marks.toml")
}

pub fn favorites_path() -> PathBuf {
    app_data_dir().join("favorites.toml")
}

pub fn apps_cache_path() -> PathBuf {
    app_data_dir().join("apps_cache.toml")
}

pub fn log_path() -> PathBuf {
    app_data_dir().join("winharpoon.log")
}

pub fn ensure_app_data() {
    let dir = app_data_dir();
    log::trace(format!("ensure_app_data: {}", dir.display()));
    let _ = std::fs::create_dir_all(dir);
}

pub fn open_config_folder() {
    let path = app_data_dir();
    log::debug(format!("open_config_folder: {}", path.display()));
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}
