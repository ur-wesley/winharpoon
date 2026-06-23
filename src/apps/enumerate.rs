use std::collections::HashMap;
use std::path::{Path, PathBuf};

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Com::{
    CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER,
};
use windows::Win32::System::Com::STGM;
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};

use crate::log;
use crate::util;

use super::{app_paths_scan, apps_folder_scan, AppEntry, AppSource};

pub fn start_menu_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(pd) = std::env::var_os("ProgramData") {
        roots.push(PathBuf::from(pd).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(app) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(app).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    roots
}

pub fn scan_all() -> Vec<AppEntry> {
    let mut by_key: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<AppEntry> = Vec::new();

    merge_into(&mut out, &mut by_key, apps_folder_scan());
    merge_into(&mut out, &mut by_key, scan_start_menu());
    merge_into(&mut out, &mut by_key, app_paths_scan());

    out.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    log::debug(format!("apps: indexed {} programs (merged)", out.len()));
    out
}

fn merge_into(out: &mut Vec<AppEntry>, by_key: &mut HashMap<String, usize>, incoming: Vec<AppEntry>) {
    for entry in incoming {
        let key = entry.id.clone();
        if let Some(&idx) = by_key.get(&key) {
            if let Some(existing) = out.get_mut(idx) {
                if entry.name.len() > existing.name.len() {
                    existing.name = entry.name;
                }
                if existing.aumid.is_none() && entry.aumid.is_some() {
                    existing.aumid = entry.aumid;
                }
                if existing.target.as_os_str().is_empty() && !entry.target.as_os_str().is_empty() {
                    existing.target = entry.target;
                }
                if existing.source_lnk.as_os_str().is_empty() && !entry.source_lnk.as_os_str().is_empty() {
                    existing.source_lnk = entry.source_lnk;
                }
                if existing.args.is_empty() && !entry.args.is_empty() {
                    existing.args = entry.args;
                }
            }
            continue;
        }
        by_key.insert(key, out.len());
        out.push(entry);
    }
}

fn scan_start_menu() -> Vec<AppEntry> {
    let roots = start_menu_roots();
    let mut links = Vec::new();
    for root in &roots {
        collect_links(root, &mut links);
    }
    log::debug(format!("apps: found {} shortcuts", links.len()));

    let mut out: Vec<AppEntry> = Vec::new();
    for lnk in links {
        let Some((target, args)) = resolve_lnk(&lnk) else {
            continue;
        };
        if target.as_os_str().is_empty() {
            continue;
        }
        let name = lnk
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("App")
            .to_string();
        let aumid = extract_aumid_from_args(&args);
        let id = entry_id(&target, aumid.as_deref(), &args);
        let exe_name = target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let parent = lnk
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let aumid_str = aumid.clone().unwrap_or_default();
        let search_label = format!("{name} {exe_name} {parent} {aumid_str}");

        let source = if aumid.is_some() {
            AppSource::AppsFolder
        } else {
            AppSource::StartMenu
        };

        out.push(AppEntry {
            id,
            name,
            target,
            args,
            source_lnk: lnk,
            search_label,
            aumid,
            source,
        });
    }
    out
}

fn extract_aumid_from_args(args: &str) -> Option<String> {
    let lower = args.to_ascii_lowercase();
    let marker = "shell:appsfolder\\";
    let idx = lower.find(marker)?;
    let rest = &args[idx + marker.len()..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '"')
        .unwrap_or(rest.len());
    let candidate = rest[..end].trim().to_string();
    if candidate.is_empty() || !candidate.contains('!') {
        None
    } else {
        Some(candidate)
    }
}

fn collect_links(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_links(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            out.push(path);
        }
    }
}

pub(crate) fn entry_id(target: &Path, aumid: Option<&str>, args: &str) -> String {
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

pub(crate) fn fnv1a(s: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn resolve_lnk(path: &Path) -> Option<(PathBuf, String)> {
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link.cast().ok()?;
        let wide = util::wide(&path.to_string_lossy());
        file.Load(PCWSTR(wide.as_ptr()), STGM(0)).ok()?;

        let mut target_buf = [0u16; MAX_PATH as usize];
        link.GetPath(
            &mut target_buf,
            std::ptr::null_mut(),
            SLGP_RAWPATH.0 as u32,
        )
        .ok()?;
        let target = PathBuf::from(util::from_wide(&target_buf));
        if target.as_os_str().is_empty() {
            return None;
        }

        let mut args_buf = [0u16; 2048];
        link.GetArguments(&mut args_buf).ok()?;
        let args = util::from_wide(&args_buf);
        Some((target, args))
    }
}