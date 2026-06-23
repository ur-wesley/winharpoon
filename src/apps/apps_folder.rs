use std::collections::HashMap;
use std::path::PathBuf;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IEnumIDList, IShellFolder, IShellItem, SHCreateItemFromIDList, SHCreateItemFromParsingName,
    SHCONTF_FOLDERS, SHCONTF_INCLUDEHIDDEN, SHCONTF_NONFOLDERS, SIGDN,
    SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
};

use crate::log;
use crate::util;

use super::{AppEntry, AppSource};

pub fn scan() -> Vec<AppEntry> {
    let mut out: Vec<AppEntry> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    let root = match parse_apps_folder() {
        Some(r) => r,
        None => {
            log::warn("apps: failed to bind shell:AppsFolder");
            return out;
        }
    };

    let folder: IShellFolder = match root.cast() {
        Ok(f) => f,
        Err(_) => {
            log::warn("apps: failed to cast AppsFolder to IShellFolder");
            return out;
        }
    };

    let mut enum_id_list: Option<IEnumIDList> = None;
    let flags: u32 = (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32;
    if unsafe { folder.EnumObjects(HWND(std::ptr::null_mut()), flags, &mut enum_id_list) }.is_err() {
        log::warn("apps: EnumObjects failed");
        return out;
    }
    let enum_id_list = match enum_id_list {
        Some(e) => e,
        None => return out,
    };

    loop {
        let mut pidl_arr: [*mut ITEMIDLIST; 1] = [std::ptr::null_mut()];
        let mut fetched: u32 = 0;
        let hr = unsafe { enum_id_list.Next(&mut pidl_arr, Some(&mut fetched)) };
        if hr.is_err() || fetched == 0 {
            break;
        }
        let pidl = pidl_arr[0];
        if pidl.is_null() {
            break;
        }

        let item: Option<IShellItem> = unsafe { SHCreateItemFromIDList(pidl as *const _) }.ok();
        if let Some(item) = item {
            if let Some(entry) = build_entry(&item) {
                let key = if let Some(a) = &entry.aumid {
                    format!("aumid:{a}")
                } else {
                    format!("exe:{}", entry.target.to_string_lossy().to_ascii_lowercase())
                };
                if let Some(&idx) = seen.get(&key) {
                    if let Some(existing) = out.get_mut(idx) {
                        if entry.name.len() > existing.name.len() {
                            existing.name = entry.name;
                        }
                    }
                } else {
                    seen.insert(key, out.len());
                    out.push(entry);
                }
            }
        }

        unsafe { CoTaskMemFree(Some(pidl as *const _)) };
    }

    log::debug(format!("apps: AppsFolder yielded {} items", out.len()));
    out
}

fn parse_apps_folder() -> Option<IShellItem> {
    let wide = util::wide("shell:AppsFolder");
    unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok() }
}

fn build_entry(item: &IShellItem) -> Option<AppEntry> {
    let name = read_display_name(item, SIGDN_NORMALDISPLAY)?;
    let name = name.trim().to_string();
    if name.is_empty() || name.starts_with('@') {
        return None;
    }

    let parsing = read_display_name(item, SIGDN_DESKTOPABSOLUTEPARSING).unwrap_or_default();

    let (aumid, target) = classify(&parsing);

    let id = make_id(target.as_path(), aumid.as_deref(), "");

    let exe_name = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let aumid_str = aumid.clone().unwrap_or_default();
    let search_label = format!("{name} {exe_name} {aumid_str}");

    Some(AppEntry {
        id,
        name,
        target,
        args: String::new(),
        source_lnk: PathBuf::new(),
        search_label,
        aumid,
        source: AppSource::AppsFolder,
    })
}

fn classify(parsing: &str) -> (Option<String>, PathBuf) {
    if parsing.is_empty() {
        return (None, PathBuf::new());
    }
    if parsing.contains('!') {
        return (Some(parsing.to_string()), PathBuf::new());
    }
    if parsing.to_ascii_lowercase().ends_with(".exe") {
        return (None, PathBuf::from(parsing));
    }
    (None, PathBuf::new())
}

fn read_display_name(item: &IShellItem, sigdn: SIGDN) -> Option<String> {
    unsafe {
        let ptr = item.GetDisplayName(sigdn).ok()?;
        if ptr.is_null() {
            return None;
        }
        let owned = ptr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(ptr.0 as *const _));
        if owned.is_empty() {
            None
        } else {
            Some(owned)
        }
    }
}

fn make_id(target: &std::path::Path, aumid: Option<&str>, args: &str) -> String {
    let key = if let Some(a) = aumid {
        format!("aumid:{a}")
    } else {
        format!(
            "exe:{}|{}",
            target.to_string_lossy().to_ascii_lowercase(),
            args.trim()
        )
    };
    format!("{:x}", fnv1a(&key))
}

fn fnv1a(s: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}