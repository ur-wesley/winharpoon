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
    let merged = merge_sources(vec![
        apps_folder_scan(),
        scan_start_menu(),
        app_paths_scan(),
    ]);
    log::debug(format!("apps: indexed {} programs (merged)", merged.len()));
    merged
}

fn merge_sources(parts: Vec<Vec<AppEntry>>) -> Vec<AppEntry> {
    let mut by_key: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<AppEntry> = Vec::new();

    for part in parts {
        merge_into(&mut out, &mut by_key, part);
    }

    out.sort_by_key(|a| a.name.to_ascii_lowercase());
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
        let Some((raw_target, args)) = resolve_lnk(&lnk) else {
            continue;
        };
        // Shortcuts may store unexpanded `%VAR%` paths; expand so the target
        // is a real path and matches other sources (e.g. App Paths).
        let target = PathBuf::from(expand_env(&raw_target.to_string_lossy()));
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

pub fn entry_id(target: &Path, aumid: Option<&str>, args: &str) -> String {
    let key = aumid.map_or_else(
        || format!("exe:{}|{}", normalize_target(target), args.trim()),
        |a| format!("aumid:{}", a.trim().to_ascii_lowercase()),
    );
    format!("{:x}", fnv1a(&key))
}

/// Canonical form of a launch target for identity comparison.
///
/// Expands `%VAR%` segments, normalizes separators and case so the same
/// executable discovered by different sources (Start Menu shortcut vs.
/// App Paths registry value) maps to the same key.
pub fn normalize_target(target: &Path) -> String {
    let expanded = expand_env(&target.to_string_lossy());
    expanded
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .replace('/', "\\")
        .to_ascii_lowercase()
}

/// Expands `%NAME%` segments using the process environment.
/// Unknown names (or a dangling `%`) are left untouched.
pub fn expand_env(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        if let Ok(value) = std::env::var(name) {
            out.push_str(&value);
        } else {
            out.push('%');
            out.push_str(name);
            out.push('%');
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

pub fn fnv1a(s: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_entry(id: &str, name: &str, target: &str, args: &str, source: AppSource) -> AppEntry {
        AppEntry {
            id: id.into(),
            name: name.into(),
            target: PathBuf::from(target),
            args: args.into(),
            source_lnk: PathBuf::new(),
            search_label: name.into(),
            aumid: None,
            source,
        }
    }

    #[test]
    fn same_exe_same_args_share_id_across_sources() {
        // Start Menu and App Paths both describe chrome.exe with no args.
        let a = entry_id(Path::new(r"C:\Program Files\App\app.exe"), None, "");
        let b = entry_id(Path::new(r"C:\Program Files\App\app.exe"), None, "");
        assert_eq!(a, b);
    }

    #[test]
    fn identity_ignores_case_and_separators() {
        let a = entry_id(Path::new(r"C:\Program Files\App\APP.EXE"), None, "");
        let b = entry_id(Path::new("c:/program files/app/app.exe"), None, "");
        assert_eq!(a, b);
    }

    #[test]
    fn unexpanded_env_target_matches_expanded() {
        let root = std::env::var("SystemRoot").expect("SystemRoot must exist on Windows");
        let a = entry_id(
            Path::new(r"%SystemRoot%\system32\notepad.exe"),
            None,
            "",
        );
        let b = entry_id(
            Path::new(&format!("{root}\\system32\\notepad.exe")),
            None,
            "",
        );
        assert_eq!(a, b);
    }

    #[test]
    fn different_args_stay_distinct() {
        let a = entry_id(Path::new(r"C:\Windows\system32\cmd.exe"), None, "/k one.bat");
        let b = entry_id(Path::new(r"C:\Windows\system32\cmd.exe"), None, "/k other.bat");
        let c = entry_id(Path::new(r"C:\Windows\system32\cmd.exe"), None, "");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(b, c);
    }

    #[test]
    fn aumid_identity_is_case_insensitive() {
        let a = entry_id(Path::new(""), Some("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"), "");
        let b = entry_id(Path::new(""), Some("microsoft.windowsnotepad_8wekyb3d8bbwe!app"), "");
        assert_eq!(a, b);
    }

    #[test]
    fn merge_sources_collapses_same_exe() {
        let chrome_lnk = test_entry(
            &entry_id(Path::new(r"C:\Program Files\App\app.exe"), None, ""),
            "App",
            r"C:\Program Files\App\app.exe",
            "",
            AppSource::StartMenu,
        );
        let mut chrome_reg = chrome_lnk.clone();
        chrome_reg.id = entry_id(Path::new(r"C:\Program Files\App\app.exe"), None, "");
        chrome_reg.source = AppSource::AppPath;

        let merged = merge_sources(vec![vec![chrome_lnk], vec![chrome_reg]]);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_sources_keeps_arg_variants() {
        let one = test_entry(
            &entry_id(Path::new(r"C:\Windows\system32\cmd.exe"), None, "/k one.bat"),
            "One",
            r"C:\Windows\system32\cmd.exe",
            "/k one.bat",
            AppSource::StartMenu,
        );
        let mut other = one.clone();
        other.name = "Other".into();
        other.args = "/k other.bat".into();
        other.id = entry_id(Path::new(r"C:\Windows\system32\cmd.exe"), None, "/k other.bat");

        let merged = merge_sources(vec![vec![one, other]]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn expand_env_leaves_unknown_vars_untouched() {
        assert_eq!(
            expand_env(r"%WINHARPOON_DEFINITELY_MISSING_VAR%\x"),
            r"%WINHARPOON_DEFINITELY_MISSING_VAR%\x"
        );
        assert_eq!(expand_env(r"C:\plain\path.exe"), r"C:\plain\path.exe");
        assert_eq!(expand_env("dangling%"), "dangling%");
    }
}
