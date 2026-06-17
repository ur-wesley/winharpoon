use crate::log;
use crate::window::{enumerate_windows, focus, get_foreground_window, WindowInfo};

pub fn cycle_same_app(forward: bool) {
    log::debug(&format!("cycle_same_app forward={forward}"));
    let Some(current) = get_foreground_window() else {
        log::debug("cycle_same_app: no foreground window");
        return;
    };

    let mut group: Vec<WindowInfo> = enumerate_windows(None)
        .into_iter()
        .filter(|w| same_group(&current, w))
        .collect();

    if group.len() <= 1 {
        log::debug(&format!(
            "cycle_same_app: only {} window(s) for {}",
            group.len(),
            current.exe_name
        ));
        return;
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
    log::debug(&format!(
        "cycle_same_app: {} -> {} (idx {idx} -> {next_idx} of {})",
        current.title, next.title, group.len()
    ));
    focus::focus_window(next.hwnd);
}

fn same_group(current: &WindowInfo, candidate: &WindowInfo) -> bool {
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

fn title_prefix_match(a: &str, b: &str) -> bool {
    let a = a.split(" - ").next().unwrap_or(a);
    let b = b.split(" - ").next().unwrap_or(b);
    a.eq_ignore_ascii_case(b)
}
