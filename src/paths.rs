use std::path::PathBuf;

use crate::log;

pub fn app_data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "winharpoon")
        .map_or_else(|| PathBuf::from("."), |d| d.data_dir().to_path_buf())
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

/// Atomic write: tmp file + rename so crashes/races never leave half-written TOML.
/// Usability: prevents corrupt config/marks/favorites. Security: reduces TOCTOU window.
pub fn atomic_write(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

pub fn backup_corrupt_file(path: &std::path::Path) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let backup = path.with_extension(format!("corrupt.{stamp}.bak"));
    let _ = std::fs::rename(path, &backup);
    log::warn(format!(
        "backed up corrupt file {} to {}",
        path.display(),
        backup.display()
    ));
}

pub fn open_config_folder() {
    let path = app_data_dir();
    log::debug(format!("open_config_folder: {}", path.display()));
    // Absolute path avoids PATH hijack (explorer.exe spoof).
    let explorer = std::env::var("SystemRoot")
        .map_or_else(|_| r"C:\Windows\explorer.exe".into(), |r| format!("{r}\\explorer.exe"));
    let _ = std::process::Command::new(explorer).arg(path).spawn();
}
