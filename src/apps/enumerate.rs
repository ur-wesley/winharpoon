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

use super::AppEntry;

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

pub fn dir_mtime(roots: &[PathBuf]) -> u64 {
    roots
        .iter()
        .filter_map(|root| root.metadata().ok()?.modified().ok())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
        .max()
        .unwrap_or(0)
}

pub fn scan_all() -> Vec<AppEntry> {
    let roots = start_menu_roots();
    let mut links = Vec::new();
    for root in &roots {
        collect_links(root, &mut links);
    }
    log::debug(format!("apps: found {} shortcuts", links.len()));

    let mut by_target: HashMap<String, AppEntry> = HashMap::new();
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
        let id = entry_id(&target, &args);
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
        let search_label = format!("{name} {exe_name} {parent}");
        let entry = AppEntry {
            id: id.clone(),
            name,
            target,
            args,
            source_lnk: lnk,
            search_label,
        };
        by_target.entry(id).or_insert(entry);
    }
    let mut out: Vec<_> = by_target.into_values().collect();
    out.sort_by_key(|a| a.name.to_ascii_lowercase());
    log::debug(format!("apps: indexed {} programs", out.len()));
    out
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

fn entry_id(target: &Path, args: &str) -> String {
    let key = format!(
        "{}|{}",
        target.to_string_lossy().to_ascii_lowercase(),
        args.trim()
    );
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
