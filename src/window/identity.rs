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
        let exe = self
            .exe
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.exe.display().to_string());
        format!("{} — {exe}", self.title)
    }
}

pub fn resolve_identity<'a>(
    identity: &WindowIdentity,
    windows: &'a [WindowInfo],
) -> Option<&'a WindowInfo> {
    log::debug(&format!(
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
        log::debug(&format!("resolve_identity: {} exact matches, using first", exact.len()));
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
        log::warn(&format!("resolve_identity: no match for {}", identity.display_label()));
    }
    result
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
