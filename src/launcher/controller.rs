use std::sync::Arc;

use parking_lot::Mutex;

use crate::apps::search::FuzzySearch;
use crate::config::Config;
use crate::log;
use crate::modes::marks::{SharedMarks, ToggleMarkResult};
use crate::ui::list::ListSelection;
use crate::window::identity::WindowIdentity;
use crate::window::{
    capture_stack_snapshot, enumerate_windows, focus, get_foreground_window, restore_stack_snapshot,
    StackSnapshot, WindowInfo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherAction {
    SetQuery,
    Navigate(i32),
    Hover(Option<usize>),
    Commit(isize),
    Cancel,
    ToggleMark,
    MoveMarkSlot { up: bool },
}

pub struct LauncherController {
    pub query: String,
    pub windows: Vec<WindowInfo>,
    pub selection: ListSelection,
    pub foreground_hwnd: Option<isize>,
    pub stack_snapshot: Option<StackSnapshot>,
    pub preview_hwnd: Option<isize>,
    pub preview_active: bool,
    pub visible: bool,
    pub panel_width: f32,
    pub panel_height: f32,
    pub last_query: String,
    search: FuzzySearch,
    search_labels: Vec<String>,
    pub last_refresh: Option<std::time::Instant>,
}

impl Default for LauncherController {
    fn default() -> Self {
        Self {
            query: String::new(),
            windows: Vec::new(),
            selection: ListSelection::default(),
            foreground_hwnd: None,
            stack_snapshot: None,
            preview_hwnd: None,
            preview_active: false,
            visible: false,
            panel_width: 480.0,
            panel_height: 420.0,
            last_query: String::new(),
            search: FuzzySearch::default(),
            search_labels: Vec::new(),
            last_refresh: None,
        }
    }
}

impl LauncherController {
    pub fn on_show(&mut self, config: &Arc<Mutex<Config>>) {
        log::debug("launcher show");
        self.windows = enumerate_windows(None);
        self.rebuild_search_labels();
        self.stack_snapshot = Some(capture_stack_snapshot());
        self.foreground_hwnd = self
            .stack_snapshot
            .as_ref()
            .and_then(|s| s.foreground)
            .or_else(|| get_foreground_window().map(|w| w.hwnd));
        self.preview_hwnd = None;
        self.preview_active = false;
        self.selection.hovered = None;
        self.query.clear();
        self.last_query.clear();
        self.selection.selected = 0;
        self.selection.scroll_to_selected = true;
        self.visible = true;
        self.last_refresh = Some(std::time::Instant::now());

        let cfg = config.lock().launcher.clone();
        self.panel_width = cfg.width;
        self.panel_height = cfg.height;
        log::debug(format!(
            "launcher ready: {} windows, foreground={:?}",
            self.windows.len(),
            self.foreground_hwnd
        ));
    }

    pub fn maybe_refresh_windows(&mut self, max_results: usize) {
        let now = std::time::Instant::now();
        if let Some(last) = self.last_refresh {
            if now.duration_since(last) < std::time::Duration::from_millis(500) {
                return;
            }
        }
        self.last_refresh = Some(now);

        let new_windows = enumerate_windows(None);
        self.update_windows_and_selection(new_windows, max_results);
    }

    pub(crate) fn update_windows_and_selection(&mut self, new_windows: Vec<WindowInfo>, max_results: usize) {
        if new_windows != self.windows {
            let old_filtered = self.filtered_indices(max_results);
            let old_active_hwnd = self.active_window(&old_filtered).map(|w| w.hwnd);

            self.windows = new_windows;
            self.rebuild_search_labels();

            let new_filtered = self.filtered_indices(max_results);

            if let Some(hwnd) = old_active_hwnd {
                if let Some(new_pos) = new_filtered.iter().position(|&idx| self.windows[idx].hwnd == hwnd) {
                    self.selection.selected = new_pos;
                } else {
                    if new_filtered.is_empty() {
                        self.selection.selected = 0;
                    } else if self.selection.selected >= new_filtered.len() {
                        self.selection.selected = new_filtered.len() - 1;
                    }
                }
            } else {
                if new_filtered.is_empty() {
                    self.selection.selected = 0;
                } else if self.selection.selected >= new_filtered.len() {
                    self.selection.selected = new_filtered.len() - 1;
                }
            }
            self.selection.hovered = None;
        }
    }

    pub fn panel_size(&self) -> (f32, f32) {
        (self.panel_width, self.panel_height)
    }

    pub fn on_query_changed(&mut self) {
        if self.query != self.last_query {
            self.last_query = self.query.clone();
            self.selection.reset_on_query_change();
            self.preview_active = true;
        }
    }

    pub fn filtered_indices(&mut self, max_results: usize) -> Vec<usize> {
        if self.query.trim().is_empty() {
            return (0..self.windows.len().min(max_results)).collect();
        }
        let labels: Vec<&str> = self.search_labels.iter().map(String::as_str).collect();
        self.search
            .rank(&self.query, &labels, max_results)
            .into_iter()
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn active_window_index(&self, filtered: &[usize]) -> Option<usize> {
        let active_row = self.selection.active_row();
        filtered.get(active_row).copied()
    }

    pub fn active_window<'a>(&'a self, filtered: &[usize]) -> Option<&'a WindowInfo> {
        self.active_window_index(filtered)
            .map(|idx| &self.windows[idx])
    }

    pub fn handle_action(
        &mut self,
        action: LauncherAction,
        marks: &SharedMarks,
        filtered: &[usize],
    ) -> LauncherEffect {
        match action {
            LauncherAction::SetQuery => {
                self.on_query_changed();
                LauncherEffect::None
            }
            LauncherAction::Navigate(delta) => {
                self.selection.navigate(filtered.len(), delta);
                self.preview_active = true;
                LauncherEffect::None
            }
            LauncherAction::Hover(hovered) => {
                self.selection.hovered = hovered;
                if hovered.is_some() {
                    self.preview_active = true;
                }
                LauncherEffect::None
            }
            LauncherAction::Commit(hwnd) => {
                self.stack_snapshot = None;
                self.preview_hwnd = None;
                self.preview_active = false;
                self.selection.hovered = None;
                self.visible = false;
                LauncherEffect::Focus(hwnd)
            }
            LauncherAction::Cancel => {
                let snapshot = self.stack_snapshot.take();
                self.preview_hwnd = None;
                self.preview_active = false;
                self.selection.hovered = None;
                self.visible = false;
                if let Some(snapshot) = snapshot {
                    LauncherEffect::RestoreStack(snapshot)
                } else {
                    LauncherEffect::None
                }
            }
            LauncherAction::ToggleMark => {
                if let Some(win) = self.active_window(filtered) {
                    let result = marks.lock().store.toggle_mark_for(win);
                    notify_toggle_result(result);
                }
                LauncherEffect::None
            }
            LauncherAction::MoveMarkSlot { up } => {
                if let Some(win) = self.active_window(filtered) {
                    let identity = WindowIdentity::from_window(win);
                    let mut marks_guard = marks.lock();
                    if let Some(slot) = marks_guard.store.find_slot(&identity) {
                        marks_guard.store.move_mark_slot(slot, up);
                    }
                }
                LauncherEffect::None
            }
        }
    }

    pub fn post_frame_effects(&mut self, filtered: &[usize]) -> LauncherEffect {
        if !self.preview_active {
            return LauncherEffect::None;
        }
        let active_row = self.selection.active_row();
        let Some(&idx) = filtered.get(active_row) else {
            return LauncherEffect::None;
        };
        let hwnd = self.windows[idx].hwnd;
        if self.preview_hwnd == Some(hwnd) {
            return LauncherEffect::None;
        }
        self.preview_hwnd = Some(hwnd);
        LauncherEffect::Preview(hwnd)
    }

    fn rebuild_search_labels(&mut self) {
        self.search_labels = self
            .windows
            .iter()
            .map(|win| format!("{} {}", win.title, win.process_name))
            .collect();
    }

    #[cfg(test)]
    pub(crate) fn rebuild_search_labels_for_test(&mut self) {
        self.rebuild_search_labels();
    }
}

pub enum LauncherEffect {
    None,
    Focus(isize),
    RestoreStack(StackSnapshot),
    Preview(isize),
}

impl LauncherEffect {
    pub fn apply(self) {
        match self {
            LauncherEffect::None => {}
            LauncherEffect::Focus(hwnd) => {
                let _ = focus::focus_window(hwnd);
            }
            LauncherEffect::RestoreStack(snapshot) => restore_stack_snapshot(&snapshot),
            LauncherEffect::Preview(hwnd) => {
                focus::preview_window(hwnd);
            }
        }
    }
}

fn notify_toggle_result(result: ToggleMarkResult) {
    match result {
        ToggleMarkResult::Marked { slot, app } => {
            log::notify("WinHarpoon", &format!("{app} — marked slot {slot}"));
        }
        ToggleMarkResult::Unmarked { slot, app } => {
            log::notify("WinHarpoon", &format!("{app} — unmarked slot {slot}"));
        }
        ToggleMarkResult::NoForeground => {}
        ToggleMarkResult::AllSlotsFull { app } => {
            log::notify("WinHarpoon", &format!("{app} — all 9 mark slots are full"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_window(hwnd: isize, title: &str, process: &str) -> WindowInfo {
        WindowInfo {
            hwnd,
            title: title.into(),
            exe_path: PathBuf::from(format!("C:\\{process}.exe")),
            exe_name: format!("{process}.exe"),
            process_name: process.into(),
        }
    }

    fn sample_controller(windows: Vec<WindowInfo>) -> LauncherController {
        let mut controller = LauncherController {
            windows,
            ..Default::default()
        };
        controller.rebuild_search_labels_for_test();
        controller
    }

    #[test]
    fn filtered_indices_empty_query_returns_first_n() {
        let mut controller = sample_controller(vec![
            test_window(1, "One", "one"),
            test_window(2, "Two", "two"),
            test_window(3, "Three", "three"),
            test_window(4, "Four", "four"),
            test_window(5, "Five", "five"),
        ]);
        assert_eq!(controller.filtered_indices(3), vec![0, 1, 2]);
    }

    #[test]
    fn filtered_indices_fuzzy_matches_title() {
        let mut controller = sample_controller(vec![
            test_window(1, "Notepad", "notepad"),
            test_window(2, "Google Chrome", "chrome"),
            test_window(3, "Explorer", "explorer"),
        ]);
        controller.query = "chrome".into();
        let filtered = controller.filtered_indices(16);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], 1);
    }

    #[test]
    fn on_query_changed_resets_selection() {
        let mut controller = sample_controller(vec![test_window(1, "One", "one")]);
        controller.selection.selected = 2;
        controller.query = "chr".into();
        controller.on_query_changed();
        assert_eq!(controller.selection.selected, 0);
        assert!(controller.selection.scroll_to_selected);
        assert_eq!(controller.last_query, "chr");
    }

    #[test]
    fn handle_action_commit_sets_visible_false() {
        let mut controller = LauncherController::default();
        let marks = crate::modes::marks::shared_marks();
        let effect = controller.handle_action(LauncherAction::Commit(42), &marks, &[]);
        assert!(!controller.visible);
        assert!(matches!(effect, LauncherEffect::Focus(42)));
    }

    #[test]
    fn handle_action_cancel_with_snapshot_returns_restore() {
        let mut controller = LauncherController {
            stack_snapshot: Some(StackSnapshot {
                foreground: Some(1),
                z_order: vec![1, 2],
            }),
            ..Default::default()
        };
        let marks = crate::modes::marks::shared_marks();
        let effect = controller.handle_action(LauncherAction::Cancel, &marks, &[]);
        assert!(!controller.visible);
        assert!(matches!(effect, LauncherEffect::RestoreStack(_)));
    }

    #[test]
    fn post_frame_effects_returns_preview_for_active_row() {
        let mut controller = sample_controller(vec![test_window(9, "Nine", "nine")]);
        controller.preview_active = true;
        controller.selection.selected = 0;
        let effect = controller.post_frame_effects(&[0]);
        assert!(matches!(effect, LauncherEffect::Preview(9)));
    }

    #[test]
    fn update_windows_and_selection_preserves_selection() {
        let win1 = test_window(101, "One", "one");
        let win2 = test_window(102, "Two", "two");
        let win3 = test_window(103, "Three", "three");

        let mut controller = sample_controller(vec![win1.clone(), win2.clone()]);
        controller.selection.selected = 1;

        let new_windows = vec![win1.clone(), win3.clone(), win2.clone()];
        controller.update_windows_and_selection(new_windows, 16);

        assert_eq!(controller.selection.selected, 2);
    }

    #[test]
    fn update_windows_and_selection_clamps_selection() {
        let win1 = test_window(101, "One", "one");
        let win2 = test_window(102, "Two", "two");

        let mut controller = sample_controller(vec![win1.clone(), win2.clone()]);
        controller.selection.selected = 1;

        let new_windows = vec![win1.clone()];
        controller.update_windows_and_selection(new_windows, 16);

        assert_eq!(controller.selection.selected, 0);
    }
}
