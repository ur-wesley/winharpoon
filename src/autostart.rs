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
    read_run_value().is_some()
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
    if status != ERROR_SUCCESS {
        Err(format!("RegOpenKeyExW failed: {status:?}"))
    } else {
        Ok(key)
    }
}

fn quote_exe_path(path: &str) -> String {
    if path.contains(' ') {
        format!("\"{path}\"")
    } else {
        path.to_string()
    }
}
