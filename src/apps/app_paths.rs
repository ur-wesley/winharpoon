use std::path::PathBuf;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegEnumKeyExW,
    RegGetValueW, RegOpenKeyExW, RRF_RT_REG_SZ,
};

use crate::log;
use crate::util;

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
        let mut name_len: u32 = name_buf.len() as u32;
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
        if name_len == 0 || name_len > name_buf.len() as u32 {
            index += 1;
            continue;
        }

        let name = util::from_wide(&name_buf[..name_len as usize]);
        if !name.is_empty() {
            if let Some(entry) = read_entry(key, &name) {
                out.push(entry);
            }
        }

        index += 1;
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
    let exe_path_buf = PathBuf::from(&exe_path_clean);

    let display_name = {
        let desc = file_description(&exe_path_buf);
        if !desc.trim().is_empty() {
            desc.trim().to_string()
        } else {
            name.trim_end_matches(".exe")
                .trim_end_matches(".EXE")
                .to_string()
        }
    };

    if display_name.is_empty() {
        return None;
    }

    let id = format!(
        "{:x}",
        fnv1a(&format!(
            "apppath:{}",
            exe_path_clean.to_ascii_lowercase()
        ))
    );
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
    let mut size: u32 = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            PCWSTR::null(),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    if status.is_err() || size < 2 {
        return None;
    }
    let char_count = (size as usize / 2).saturating_sub(1);
    Some(util::from_wide(&buf[..char_count]))
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
    let mut data = vec![0u8; size as usize];
    let ok = unsafe {
        GetFileVersionInfoW(
            PCWSTR(path_wide.as_ptr()),
            Some(handle),
            size,
            data.as_mut_ptr() as *mut _,
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
            data.as_ptr() as *const _,
            PCWSTR(query.as_ptr()),
            &mut lang_info_ptr,
            &mut lang_info_len,
        )
    };
    if !ok.as_bool() || lang_info_ptr.is_null() || lang_info_len < 4 {
        return None;
    }
    let translations = unsafe {
        std::slice::from_raw_parts(lang_info_ptr as *const u16, (lang_info_len as usize) * 2)
    };
    if translations.len() < 2 {
        return None;
    }
    let lang = u16::from_le(translations[0]);
    let code_page = u16::from_le(translations[1]);
    let sub_block = format!(
        "StringFileInfo\\{:04x}{:04x}\\{}",
        lang, code_page, field
    );
    let sub_wide = util::wide(&sub_block);
    let mut value_ptr = std::ptr::null_mut();
    let mut value_len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr() as *const _,
            PCWSTR(sub_wide.as_ptr()),
            &mut value_ptr,
            &mut value_len,
        )
    };
    if !ok.as_bool() || value_ptr.is_null() {
        return None;
    }
    let chars =
        unsafe { std::slice::from_raw_parts(value_ptr as *const u16, value_len as usize) };
    let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    Some(String::from_utf16_lossy(&chars[..end]))
}

fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}