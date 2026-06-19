use std::fs::OpenOptions;
use std::io::{self, Write};
use std::sync::OnceLock;

use winrt_toast_reborn::{register, Text, Toast, ToastManager};

use crate::paths;
use crate::util;

const APP_ID: &str = "WinHarpoon.App";

static TOAST_APP_ID: OnceLock<String> = OnceLock::new();

pub fn init_toast() {
    let icon = std::env::current_exe()
        .ok()
        .filter(|path| path.is_absolute());

    match register(
        APP_ID,
        "WinHarpoon",
        icon.as_deref(),
    ) {
        Ok(()) => debug(format!("toast AUMID registered: {APP_ID}")),
        Err(error) => warn(format!("toast AUMID registration failed: {error}")),
    }

    let app_id = if set_process_aumid(APP_ID) {
        APP_ID.to_string()
    } else {
        warn("SetCurrentProcessExplicitAppUserModelID failed, using PowerShell AUMID");
        ToastManager::POWERSHELL_AUM_ID.to_string()
    };

    let _ = TOAST_APP_ID.set(app_id.clone());
    debug(format!("toast manager ready ({app_id})"));
}

pub fn trace(message: impl AsRef<str>) {
    log_line("TRACE", message);
}

pub fn debug(message: impl AsRef<str>) {
    log_line("DEBUG", message);
}

pub fn info(message: impl AsRef<str>) {
    log_line("INFO", message);
}

pub fn warn(message: impl AsRef<str>) {
    log_line("WARN", message);
}

pub fn error(message: impl AsRef<str>) {
    log_line("ERROR", message);
}

fn log_line(level: &str, message: impl AsRef<str>) {
    let line = format!(
        "[{}] [{level}] {}",
        timestamp(),
        message.as_ref()
    );
    let _ = io::stderr().write_all(line.as_bytes());
    let _ = io::stderr().write_all(b"\n");

    let path = paths::log_path();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

pub fn notify(title: &str, body: &str) {
    info(format!("NOTIFY {title}: {body}"));

    let Some(app_id) = TOAST_APP_ID.get() else {
        warn("notify called before init_toast");
        return;
    };

    let manager = ToastManager::new(app_id);
    let mut toast = Toast::new();
    toast.text1(title).text2(Text::new(body));

    if let Err(error) = manager.show(&toast) {
        warn(format!("toast show failed: {error}"));
    }
}

fn set_process_aumid(app_id: &str) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let wide = util::wide(app_id);
    unsafe { SetCurrentProcessExplicitAppUserModelID(PCWSTR(wide.as_ptr())).is_ok() }
}
