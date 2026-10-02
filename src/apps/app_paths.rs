use std::path::PathBuf;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ,
};

use crate::log;
use crate::util;

use super::enumerate::{entry_id, expand_env};
use super::{AppEntry, AppSource};

const SUBKEY_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths";

pub fn scan() -> Vec<AppEntry> {
    let mut out = Vec::new();
    for &hive in &[HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        walk_hive(hive, &mut out);
    }
    log::debug(format!("apps: App Paths yielded {} items", out.len()));
    out
}

fn walk_hive(hive: HKEY, out: &mut Vec<AppEntry>) {
    let subkey_wide = util::wide(SUBKEY_PATH);
    let mut key = HKEY::default();
    let res = unsafe {
        RegOpenKeyExW(
            hive,
            PCWSTR(subkey_wide.as_ptr()),
            Some(0),
            KEY_READ,
            &mut key,
        )
    };
    if res.is_err() {
        return;
    }

    let mut index = 0u32;
    loop {
        let mut name_buf = [0u16; 256];
        let mut name_len: u32 = u32::try_from(name_buf.len()).unwrap_or(u32::MAX);
        let written = unsafe {
            RegEnumKeyExW(
                key,
                index,
                Some(PWSTR(name_buf.as_mut_ptr())),
                &mut name_len,
                None,
                None,
                None,
                None,
            )
        };
        if written.is_err() {
            break;
        }
        let name_cap = u32::try_from(name_buf.len()).unwrap_or(0);
        if name_len == 0 || name_len > name_cap {
            index = index.saturating_add(1);
            continue;
        }

        let end = usize::try_from(name_len).unwrap_or(0).min(name_buf.len());
        let name = util::from_wide(name_buf.get(..end).unwrap_or_default());
        if !name.is_empty() {
            if let Some(entry) = read_entry(key, &name) {
                out.push(entry);
            }
        }

        index = index.saturating_add(1);
    }

    let _ = unsafe { RegCloseKey(key) };
}

fn read_entry(parent: HKEY, name: &str) -> Option<AppEntry> {
    let sub_name_wide = util::wide(name);
    let mut sub = HKEY::default();
    let res = unsafe {
        RegOpenKeyExW(
            parent,
            PCWSTR(sub_name_wide.as_ptr()),
            Some(0),
            KEY_READ,
            &mut sub,
        )
    };
    if res.is_err() {
        return None;
    }

    let exe_path = read_default_string(sub)?;
    let _ = unsafe { RegCloseKey(sub) };

    if exe_path.is_empty() {
        return None;
    }

    let exe_path_clean = strip_quotes(&exe_path);
    // Registry values may contain unexpanded `%VAR%` paths; expand so the
    // target is a real path and shares identity with other sources.
    let exe_path_buf = PathBuf::from(expand_env(&exe_path_clean));

    let display_name = {
        let desc = file_description(&exe_path_buf);
        if desc.trim().is_empty() {
            name.trim_end_matches(".exe")
                .trim_end_matches(".EXE")
                .to_string()
        } else {
            desc.trim().to_string()
        }
    };

    if display_name.is_empty() {
        return None;
    }

    // Same identity scheme as the other sources so the same exe merges
    // instead of showing up twice (Start Menu shortcut + App Paths).
    let id = entry_id(&exe_path_buf, None, "");
    let exe_name = exe_path_buf
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let search_label = format!("{display_name} {exe_name} {name}");

    Some(AppEntry {
        id,
        name: display_name,
        target: exe_path_buf,
        args: String::new(),
        source_lnk: PathBuf::new(),
        search_label,
        aumid: None,
        source: AppSource::AppPath,
    })
}

fn read_default_string(key: HKEY) -> Option<String> {
    let mut buf = vec![0u16; 2048];
    let mut size: u32 = u32::try_from(buf.len().saturating_mul(2)).unwrap_or(u32::MAX);
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            PCWSTR::null(),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast::<core::ffi::c_void>()),
            Some(&mut size),
        )
    };
    if status.is_err() || size < 2 {
        return None;
    }
    let char_count = usize::try_from(size)
        .unwrap_or(0)
        .saturating_div(2)
        .saturating_sub(1);
    Some(util::from_wide(
        buf.get(..char_count.min(buf.len())).unwrap_or_default(),
    ))
}

fn file_description(exe: &std::path::Path) -> String {
    if !exe.is_file() {
        return String::new();
    }
    read_version_info_string(exe, "FileDescription").unwrap_or_default()
}

fn read_version_info_string(exe: &std::path::Path, field: &str) -> Option<String> {
    let path_wide = util::wide(&exe.to_string_lossy());
    let mut handle = 0u32;
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(path_wide.as_ptr()), Some(&mut handle)) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; usize::try_from(size).unwrap_or(0)];
    let ok = unsafe {
        GetFileVersionInfoW(
            PCWSTR(path_wide.as_ptr()),
            Some(handle),
            size,
            data.as_mut_ptr().cast::<core::ffi::c_void>(),
        )
    };
    if ok.is_err() {
        return None;
    }
    let mut lang_info_ptr = std::ptr::null_mut();
    let mut lang_info_len = 0u32;
    let query = util::wide("\\VarFileInfo\\Translation");
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr().cast::<core::ffi::c_void>(),
            PCWSTR(query.as_ptr()),
            &mut lang_info_ptr,
            &mut lang_info_len,
        )
    };
    if !ok.as_bool() || lang_info_ptr.is_null() || lang_info_len < 4 {
        return None;
    }
    // `VerQueryValueW` reports the translation block size in bytes; one
    // entry is two `u16` (language + code page).
    let entry_len = usize::try_from(lang_info_len.saturating_div(2)).unwrap_or(0);
    let translations =
        unsafe { std::slice::from_raw_parts(lang_info_ptr.cast::<u16>(), entry_len) };
    let [lang_raw, code_raw] = *translations.first_chunk::<2>()?;
    let lang = u16::from_le(lang_raw);
    let code_page = u16::from_le(code_raw);
    let sub_block = format!("StringFileInfo\\{lang:04x}{code_page:04x}\\{field}");
    let sub_wide = util::wide(&sub_block);
    let mut value_ptr = std::ptr::null_mut();
    let mut value_len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr().cast::<core::ffi::c_void>(),
            PCWSTR(sub_wide.as_ptr()),
            &mut value_ptr,
            &mut value_len,
        )
    };
    if !ok.as_bool() || value_ptr.is_null() {
        return None;
    }
    let chars = unsafe {
        std::slice::from_raw_parts(
            value_ptr.cast::<u16>(),
            usize::try_from(value_len).unwrap_or(0),
        )
    };
    let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    Some(String::from_utf16_lossy(
        chars.get(..end).unwrap_or_default(),
    ))
}

fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(inner) = trimmed.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return inner.to_string();
    }
    if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|t| t.strip_suffix('\''))
    {
        return inner.to_string();
    }
    trimmed.to_string()
}
