use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use parking_lot::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

use crate::util;

static CACHE: LazyLock<Mutex<HashMap<PathBuf, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn process_display_name(exe_path: &Path) -> String {
    if let Some(cached) = CACHE.lock().get(exe_path).cloned() {
        return cached;
    }
    let name = query_process_name(exe_path);
    CACHE.lock().insert(exe_path.to_path_buf(), name.clone());
    name
}

fn query_process_name(exe_path: &Path) -> String {
    for field in ["FileDescription", "ProductName", "InternalName"] {
        if let Some(value) = unsafe { version_string(exe_path, field) } {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    format_exe_stem(exe_path)
}

fn format_exe_stem(exe_path: &Path) -> String {
    exe_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.replace(['-', '_'], " ")
                .split_whitespace()
                .map(title_word)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown".into())
}

fn title_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

unsafe fn version_string(exe_path: &Path, field: &str) -> Option<String> {
    let path = exe_path.to_str()?;
    let path_w = util::wide(path);
    let size = GetFileVersionInfoSizeW(PCWSTR(path_w.as_ptr()), None);
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    GetFileVersionInfoW(
        PCWSTR(path_w.as_ptr()),
        Some(0),
        size,
        data.as_mut_ptr().cast(),
    )
    .ok()?;

    let mut trans_ptr = std::ptr::null_mut();
    let mut trans_len = 0u32;
    if !VerQueryValueW(
        data.as_ptr().cast(),
        windows::core::w!("\\VarFileInfo\\Translation"),
        &mut trans_ptr,
        &mut trans_len,
    )
    .as_bool()
        || trans_ptr.is_null()
        || trans_len < 4
    {
        return None;
    }

    let trans = std::slice::from_raw_parts(trans_ptr.cast::<u16>(), (trans_len / 2) as usize);
    let subblock = format!(
        "\\StringFileInfo\\{:04x}{:04x}\\{field}",
        trans[0], trans[1]
    );
    let subblock_w = util::wide(&subblock);

    let mut value_ptr = std::ptr::null_mut();
    let mut value_len = 0u32;
    if !VerQueryValueW(
        data.as_ptr().cast(),
        PCWSTR(subblock_w.as_ptr()),
        &mut value_ptr,
        &mut value_len,
    )
    .as_bool()
        || value_ptr.is_null()
        || value_len < 2
    {
        return None;
    }

    let wide_len = (value_len).saturating_sub(1) as usize;
    let wide = std::slice::from_raw_parts(value_ptr.cast::<u16>(), wide_len);
    Some(util::from_wide(wide))
}
