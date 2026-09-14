use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
};

use crate::log;
use crate::util;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "WinHarpoon";

pub fn is_enabled() -> bool {
    let Some(value) = read_run_value() else {
        return false;
    };
    // Usability+security: only report enabled when the Run value actually points to us.
    // A foreign value under our name is hijack/confusion — don't claim it.
    current_quoted_exe().is_ok_and(|ours| normalize(&value) == normalize(&ours))
}

/// True when a Run value exists but doesn't point at this exe (possible hijack or stale path).
pub fn has_foreign_value() -> bool {
    let Some(value) = read_run_value() else {
        return false;
    };
    if value.trim().is_empty() {
        return false;
    }
    current_quoted_exe().map_or(true, |ours| normalize(&value) != normalize(&ours))
}

pub fn apply(enabled: bool) -> Result<(), String> {
    if enabled {
        let exe = std::env::current_exe().map_err(|err| err.to_string())?;
        let path = exe.display().to_string();
        log::debug(format!("autostart enable: {path}"));
        write_run_value(&path)
    } else {
        log::debug("autostart disable");
        delete_run_value()
    }
}

pub fn sync_from_config(enabled: bool) {
    if has_foreign_value() {
        log::warn("autostart Run value points elsewhere, not overwriting silently");
        log::notify(
            "WinHarpoon",
            "Autostart registry entry points to another app — check Settings.",
        );
        return;
    }
    let active = is_enabled();
    if active == enabled {
        log::trace(format!("autostart already synced (enabled={enabled})"));
        return;
    }
    match apply(enabled) {
        Ok(()) => log::debug(format!("autostart synced to {enabled}")),
        Err(err) => log::warn(format!("autostart sync failed: {err}")),
    }
}

fn read_run_value() -> Option<String> {
    unsafe {
        let key = open_run_key(KEY_QUERY_VALUE).ok()?;
        let name = util::wide(VALUE_NAME);
        let mut kind = REG_SZ;
        let mut size = 0u32;
        let status = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind as *mut _),
            None,
            Some(&mut size),
        );
        if status != ERROR_SUCCESS || size < 2 {
            let _ = RegCloseKey(key);
            return None;
        }

        let wchar_count = (size as usize / 2).max(1);
        let mut buffer = vec![0u16; wchar_count];
        let status = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind as *mut _),
            Some(buffer.as_mut_ptr() as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if status != ERROR_SUCCESS {
            return None;
        }

        let value = util::from_wide(&buffer);
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    }
}

fn write_run_value(path: &str) -> Result<(), String> {
    let quoted = quote_exe_path(path);
    unsafe {
        let key = open_run_key(KEY_SET_VALUE)?;
        let name = util::wide(VALUE_NAME);
        let data = util::wide(&quoted);
        let byte_len = (data.len() * 2) as u32;
        let status = RegSetValueExW(
            key,
            PCWSTR(name.as_ptr()),
            Some(0),
            REG_SZ,
            Some(std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                byte_len as usize,
            )),
        );
        let _ = RegCloseKey(key);
        if status != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW failed: {status:?}"));
        }
        Ok(())
    }
}

fn delete_run_value() -> Result<(), String> {
    unsafe {
        let key = open_run_key(KEY_SET_VALUE)?;
        let name = util::wide(VALUE_NAME);
        let status = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
        let _ = RegCloseKey(key);
        if status != ERROR_SUCCESS {
            return Err(format!("RegDeleteValueW failed: {status:?}"));
        }
        Ok(())
    }
}

unsafe fn open_run_key(
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Result<HKEY, String> {
    let subkey = util::wide(RUN_KEY);
    let mut key = HKEY::default();
    let status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        PCWSTR(subkey.as_ptr()),
        Some(0),
        access,
        &mut key,
    );
    if status == ERROR_SUCCESS {
        Ok(key)
    } else {
        Err(format!("RegOpenKeyExW failed: {status:?}"))
    }
}

fn quote_exe_path(path: &str) -> String {
    // Strip embedded quotes to block registry-value injection, then quote.
    let clean: String = path.chars().filter(|&c| c != '"').collect();
    if clean.contains(' ') {
        format!("\"{clean}\"")
    } else {
        clean
    }
}

fn current_quoted_exe() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(quote_exe_path(&exe.display().to_string()))
}

fn normalize(s: &str) -> String {
    s.trim().trim_matches('"').replace('/', "\\").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{normalize, quote_exe_path};

    #[test]
    fn quote_adds_quotes_only_with_spaces() {
        assert_eq!(quote_exe_path(r"C:\app\win.exe"), r"C:\app\win.exe");
        assert_eq!(quote_exe_path(r"C:\my app\win.exe"), r#""C:\my app\win.exe""#);
    }

    #[test]
    fn quote_strips_embedded_quotes() {
        assert_eq!(quote_exe_path("C:\\evil\"bar.exe"), r"C:\evilbar.exe");
    }

    #[test]
    fn normalize_canonicalizes_run_values() {
        assert_eq!(normalize("\"C:/APP/win.EXE\" "), r"c:\app\win.exe");
    }
}
