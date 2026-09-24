use std::path::{Path, PathBuf};

use crate::log;
use crate::window::WindowInfo;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WindowIdentity {
    pub exe: PathBuf,
    pub title: String,
}

impl WindowIdentity {
    pub fn from_window(info: &WindowInfo) -> Self {
        Self {
            exe: info.exe_path.clone(),
            title: info.title.clone(),
        }
    }

    pub fn display_label(&self) -> String {
        let exe = self.exe.file_name().map_or_else(
            || self.exe.display().to_string(),
            |s| s.to_string_lossy().into_owned(),
        );
        format!("{} — {exe}", self.title)
    }
}

// Allow: `WindowIdentity.exe` vs `WindowInfo.exe_path` are different field names
// for the same concept (stored exe path), not a copy-paste bug.
#[allow(clippy::suspicious_operation_groupings)]
pub fn resolve_identity<'a>(
    identity: &WindowIdentity,
    windows: &'a [WindowInfo],
) -> Option<&'a WindowInfo> {
    log::debug(format!(
        "resolve_identity: {} in {} windows",
        identity.display_label(),
        windows.len()
    ));
    let exact: Vec<_> = windows
        .iter()
        .filter(|w| w.exe_path == identity.exe && w.title == identity.title)
        .collect();
    if exact.len() == 1 {
        log::debug("resolve_identity: exact match");
        return Some(exact[0]);
    }
    if !exact.is_empty() {
        log::debug(format!("resolve_identity: {} exact matches, using first", exact.len()));
        return Some(exact[0]);
    }

    let result = windows
        .iter()
        .filter(|w| paths_match(&w.exe_path, &identity.exe))
        .max_by_key(|w| title_score(&w.title, &identity.title))
        .filter(|w| title_score(&w.title, &identity.title) > 0);
    if result.is_some() {
        log::debug("resolve_identity: fuzzy match");
    } else {
        log::warn(format!("resolve_identity: no match for {}", identity.display_label()));
    }
    result
}

pub fn identities_match(stored: &WindowIdentity, live: &WindowIdentity) -> bool {
    stored == live
        || (paths_match(&stored.exe, &live.exe)
            && title_score(&live.title, &stored.title) > 0)
}

fn paths_match(a: &Path, b: &Path) -> bool {
    a == b
        || a.file_name()
            .is_some_and(|fa| b.file_name().is_some_and(|fb| fa.eq_ignore_ascii_case(fb)))
}

fn title_score(candidate: &str, target: &str) -> i32 {
    if candidate == target {
        return 100;
    }
    if candidate.starts_with(target) || target.starts_with(candidate) {
        return 50;
    }
    let c = candidate.to_lowercase();
    let t = target.to_lowercase();
    if c.contains(&t) || t.contains(&c) {
        return 25;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{identities_match, resolve_identity, title_score, WindowIdentity};
    use crate::window::WindowInfo;
    use std::path::PathBuf;

    fn win(hwnd: isize, exe: &str, title: &str) -> WindowInfo {
        WindowInfo {
            hwnd,
            title: title.into(),
            exe_path: PathBuf::from(exe),
            exe_name: exe.rsplit('\\').next().unwrap_or(exe).into(),
            process_name: "p".into(),
        }
    }

    fn id(exe: &str, title: &str) -> WindowIdentity {
        WindowIdentity {
            exe: PathBuf::from(exe),
            title: title.into(),
        }
    }

    #[test]
    fn exact_match_wins() {
        let wins = vec![win(1, r"C:\a.exe", "Doc"), win(2, r"C:\b.exe", "Doc")];
        assert_eq!(resolve_identity(&id(r"C:\a.exe", "Doc"), &wins).map(|w| w.hwnd), Some(1));
    }

    #[test]
    fn fuzzy_filename_match_needs_title_score() {
        let wins = vec![win(1, r"C:\x\app.exe", "Quarterly Report - Word")];
        assert!(resolve_identity(&id(r"D:\y\APP.exe", "Quarterly Report"), &wins).is_some());
        assert!(resolve_identity(&id(r"D:\y\APP.exe", "Unrelated"), &wins).is_none());
    }

    #[test]
    fn title_score_tiers() {
        assert_eq!(title_score("abc", "abc"), 100);
        assert_eq!(title_score("abcdef", "abc"), 50);
        assert_eq!(title_score("XABCx", "abc"), 25);
        assert_eq!(title_score("xyz", "abc"), 0);
    }

    #[test]
    fn identities_match_fuzzy_title_drift() {
        let stored = id(
            r"C:\Arbeit\project-vault\src-tauri\target\debug\project-vault.exe",
            "Project Vault",
        );
        let live = id(
            r"C:\Arbeit\project-vault\src-tauri\target\debug\project-vault.exe",
            "project-vault - Project Vault",
        );
        assert!(identities_match(&stored, &live));
    }

    #[test]
    fn identities_match_rejects_unrelated_title() {
        let stored = id(r"D:\y\APP.exe", "Quarterly Report");
        let live = id(r"D:\y\APP.exe", "Unrelated");
        assert!(!identities_match(&stored, &live));
    }
}
