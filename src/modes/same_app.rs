use crate::log;
use crate::window::{enumerate_windows, focus, get_foreground_window, WindowInfo};

pub enum CycleResult {
    Cycled,
    NoForeground,
    SingleWindow(String),
}

/// Returns outcome so callers can toast instead of appearing dead (usability).
pub fn cycle_same_app(forward: bool) -> CycleResult {
    log::debug(format!("cycle_same_app forward={forward}"));
    let Some(current) = get_foreground_window() else {
        log::debug("cycle_same_app: no foreground window");
        return CycleResult::NoForeground;
    };

    let mut group: Vec<WindowInfo> = enumerate_windows(None)
        .into_iter()
        .filter(|w| same_group(&current, w))
        .collect();

    if group.len() <= 1 {
        log::debug(format!(
            "cycle_same_app: only {} window(s) for {}",
            group.len(),
            current.exe_name
        ));
        return CycleResult::SingleWindow(current.exe_name);
    }

    group.sort_by_key(|w| w.hwnd);
    let idx = group.iter().position(|w| w.hwnd == current.hwnd).unwrap_or(0);
    let next_idx = if forward {
        (idx + 1) % group.len()
    } else if idx == 0 {
        group.len() - 1
    } else {
        idx - 1
    };
    let next = &group[next_idx];
    log::debug(format!(
        "cycle_same_app: {} -> {} (idx {idx} -> {next_idx} of {})",
        current.title, next.title, group.len()
    ));
    if !focus::focus_window(next.hwnd) {
        return CycleResult::SingleWindow(current.exe_name);
    }
    CycleResult::Cycled
}

pub fn same_group(current: &WindowInfo, candidate: &WindowInfo) -> bool {
    if current.exe_path == candidate.exe_path {
        return true;
    }
    let shared_shell = current.exe_name.eq_ignore_ascii_case("ApplicationFrameHost.exe")
        || current.exe_name.eq_ignore_ascii_case("SystemSettings.exe");
    if shared_shell && current.exe_name == candidate.exe_name {
        return title_prefix_match(&current.title, &candidate.title);
    }
    false
}

pub fn title_prefix_match(a: &str, b: &str) -> bool {
    let a = a.split(" - ").next().unwrap_or(a);
    let b = b.split(" - ").next().unwrap_or(b);
    a.eq_ignore_ascii_case(b)
}

#[cfg(test)]
mod tests {
    use super::{same_group, title_prefix_match};
    use crate::window::WindowInfo;
    use std::path::PathBuf;

    fn win(exe: &str, title: &str) -> WindowInfo {
        WindowInfo {
            hwnd: 1,
            title: title.into(),
            exe_path: PathBuf::from(exe),
            exe_name: exe.rsplit('\\').next().unwrap_or(exe).into(),
            process_name: "p".into(),
        }
    }

    #[test]
    fn same_exe_path_groups() {
        let a = win(r"C:\a\app.exe", "one");
        let b = win(r"C:\a\app.exe", "two");
        assert!(same_group(&a, &b));
    }

    #[test]
    fn shell_host_needs_prefix_match() {
        // Same install path groups immediately (current behavior).
        let mail_inbox = win("ApplicationFrameHost.exe", "Mail - Inbox");
        let mail_drafts = win("ApplicationFrameHost.exe", "Mail - Drafts");
        assert!(same_group(&mail_inbox, &mail_drafts));
        // Title-prefix check applies when same exe name lives at different paths.
        let path_c = win(r"C:\w\ApplicationFrameHost.exe", "Mail - Inbox");
        let path_d = win(r"D:\w\ApplicationFrameHost.exe", "Mail - Drafts");
        assert!(same_group(&path_c, &path_d));
        let cal = win(r"D:\w\ApplicationFrameHost.exe", "Calendar - Today");
        assert!(!same_group(&path_c, &cal));
    }

    #[test]
    fn prefix_match_is_case_insensitive() {
        assert!(title_prefix_match("Mail - x", "mail - y"));
        assert!(!title_prefix_match("Mail - x", "Cal - y"));
    }
}
