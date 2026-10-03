use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::app::AppState;
use crate::log;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum UpdateState {
    #[default]
    None,
    Checking,
    Available {
        version: String,
        download_url: String,
        release_notes: String,
    },
    Downloading {
        progress: f32,
    },
    Downloaded {
        installer_path: PathBuf,
    },
    Error(String),
}

#[derive(Debug)]
struct ReleaseInfo {
    version: String,
    download_url: String,
    release_notes: String,
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let latest_clean = latest.trim_start_matches('v').trim();
    let current_clean = current.trim_start_matches('v').trim();

    let latest_parts: Vec<&str> = latest_clean.split('.').collect();
    let current_parts: Vec<&str> = current_clean.split('.').collect();

    let len = std::cmp::max(latest_parts.len(), current_parts.len());
    for (latest_part, current_part) in latest_parts
        .iter()
        .chain(std::iter::repeat(&""))
        .zip(current_parts.iter().chain(std::iter::repeat(&"")))
        .take(len)
    {
        let latest_num = latest_part.parse::<u32>().unwrap_or(0);
        let current_num = current_part.parse::<u32>().unwrap_or(0);

        if latest_num > current_num {
            return true;
        } else if latest_num < current_num {
            return false;
        }
    }
    false
}

fn is_installer_exe(name: &str, require_setup: bool) -> bool {
    let is_exe = std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if !is_exe {
        return false;
    }
    if require_setup && !name.contains("Setup") {
        return false;
    }
    true
}

fn find_download_url(assets: &[serde_json::Value], require_setup: bool) -> Option<String> {
    assets.iter().find_map(|asset| {
        let name = asset.get("name")?.as_str()?;
        if !is_installer_exe(name, require_setup) {
            return None;
        }
        asset
            .get("browser_download_url")?
            .as_str()
            .map(str::to_string)
    })
}

fn do_check_updates() -> Result<Option<ReleaseInfo>, String> {
    let url = "https://api.github.com/repos/ur-wesley/winharpoon/releases/latest";
    let response = ureq::get(url)
        .set("User-Agent", "winharpoon-updater")
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing tag_name in release".to_string())?;

    let body = json.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let latest_version = tag_name.trim_start_matches('v');
    let current_version = env!("CARGO_PKG_VERSION");

    if is_newer(latest_version, current_version) {
        let assets = json
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing assets array in release".to_string())?;

        let download_url = find_download_url(assets, true)
            .or_else(|| find_download_url(assets, false))
            .ok_or_else(|| "No installer executable (.exe) found in release assets".to_string())?;

        Ok(Some(ReleaseInfo {
            version: latest_version.to_string(),
            download_url,
            release_notes: body.to_string(),
        }))
    } else {
        Ok(None)
    }
}

pub fn check_for_updates(state: Arc<Mutex<AppState>>, manual: bool) {
    log::info("checking for updates");
    std::thread::spawn(move || {
        {
            let mut state_guard = state.lock();
            state_guard.update_state = UpdateState::Checking;
        }

        if let Some(ctx) = crate::launcher::ui_context() {
            ctx.request_repaint();
        }

        match do_check_updates() {
            Ok(Some(release)) => {
                log::info(format!("new version available: {}", release.version));
                {
                    let mut state_guard = state.lock();
                    state_guard.update_state = UpdateState::Available {
                        version: release.version.clone(),
                        download_url: release.download_url,
                        release_notes: release.release_notes,
                    };
                }

                log::notify(
                    "WinHarpoon Update",
                    &format!(
                        "Version {} is available. Open Settings or Tray to update.",
                        release.version
                    ),
                );
            }
            Ok(None) => {
                log::info("running latest version");
                {
                    let mut state_guard = state.lock();
                    state_guard.update_state = UpdateState::None;
                }
                if manual {
                    log::notify("WinHarpoon Update", "You are running the latest version.");
                }
            }
            Err(err) => {
                log::error(format!("update check failed: {err}"));
                {
                    let mut state_guard = state.lock();
                    state_guard.update_state = UpdateState::Error(err.clone());
                }
                if manual {
                    log::notify(
                        "WinHarpoon Update",
                        &format!("Failed to check for updates: {err}"),
                    );
                }
            }
        }

        if let Some(ctx) = crate::launcher::ui_context() {
            ctx.request_repaint();
        }
    });
}

fn download_file(url: &str, state: &Arc<Mutex<AppState>>) -> Result<PathBuf, String> {
    let response = ureq::get(url)
        .set("User-Agent", "winharpoon-updater")
        .call()
        .map_err(|e| format!("Failed to connect to download URL: {e}"))?;

    let total_size = response
        .header("Content-Length")
        .and_then(|len| len.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = response.into_reader();
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("WinHarpoon-Setup-Update.exe");

    let _ = std::fs::remove_file(&file_path);

    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create installer temp file: {e}"))?;

    let mut buffer = [0; 8192];
    let mut downloaded: u64 = 0;

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Error reading download stream: {e}"))?;

        if bytes_read == 0 {
            break;
        }

        let Some(chunk) = buffer.get(..bytes_read) else {
            return Err("Download buffer slice out of bounds".to_string());
        };
        file.write_all(chunk)
            .map_err(|e| format!("Error writing download file: {e}"))?;

        downloaded = downloaded.saturating_add(u64::try_from(bytes_read).unwrap_or(0));

        if total_size > 0 {
            // no exact std conversion u64 -> f32 exists
            #[allow(clippy::as_conversions)]
            let progress = (downloaded as f32) / (total_size as f32);
            let mut state_guard = state.lock();
            state_guard.update_state = UpdateState::Downloading { progress };

            if let Some(ctx) = crate::launcher::ui_context() {
                ctx.request_repaint();
            }
        }
    }

    Ok(file_path)
}

// update flow must hand off to the installer and terminate this process
#[allow(clippy::exit)]
pub fn start_download_and_install(state: Arc<Mutex<AppState>>, download_url: String) {
    log::info(format!("starting update download from {download_url}"));
    std::thread::spawn(move || {
        {
            let mut state_guard = state.lock();
            state_guard.update_state = UpdateState::Downloading { progress: 0.0 };
        }

        if let Some(ctx) = crate::launcher::ui_context() {
            ctx.request_repaint();
        }

        match download_file(&download_url, &state) {
            Ok(temp_path) => {
                log::info("installer downloaded successfully");
                {
                    let mut state_guard = state.lock();
                    state_guard.update_state = UpdateState::Downloaded {
                        installer_path: temp_path.clone(),
                    };
                }

                if let Some(ctx) = crate::launcher::ui_context() {
                    ctx.request_repaint();
                }

                log::info(format!(
                    "spawning installer {} and exiting app",
                    temp_path.display()
                ));
                match std::process::Command::new(&temp_path).spawn() {
                    Ok(_) => {
                        log::info("installer process spawned, exiting WinHarpoon");
                        std::process::exit(0);
                    }
                    Err(err) => {
                        let err_msg = format!("Failed to run installer executable: {err}");
                        log::error(&err_msg);
                        let mut state_guard = state.lock();
                        state_guard.update_state = UpdateState::Error(err_msg);
                    }
                }
            }
            Err(err) => {
                log::error(format!("update download failed: {err}"));
                {
                    let mut state_guard = state.lock();
                    state_guard.update_state = UpdateState::Error(err);
                }
            }
        }

        if let Some(ctx) = crate::launcher::ui_context() {
            ctx.request_repaint();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("v1.1.0", "1.0.0"));
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }
}
