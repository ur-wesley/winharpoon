use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str};
use parking_lot::Mutex;

use crate::app::AppState;
use crate::config::Config;
use crate::launcher::{
    take_pending_launcher, take_pending_settings, take_pending_tray_menu, UiCommand,
};
use crate::log;
use crate::marks_switcher::ui::MarksSwitcherPanel;
use crate::marks_switcher::take_ui_receiver;
use crate::modes::marks::{SharedMarks, ToggleMarkResult};
use crate::native_ui;
use crate::settings::ui::SettingsPanel;
use crate::tray::ui::TrayMenuPanel;
use crate::window::identity::WindowIdentity;
use crate::window::{capture_stack_snapshot, enumerate_windows, focus, get_foreground_window, restore_stack_snapshot, StackSnapshot, WindowInfo};

const OFF_SCREEN: egui::Pos2 = egui::pos2(-20_000.0, -20_000.0);

pub fn run_ui(
    config: Arc<Mutex<Config>>,
    state: Arc<Mutex<AppState>>,
    marks: SharedMarks,
    command_rx: Receiver<UiCommand>,
) {
    log::debug("run_ui starting persistent event loop");
    let cfg = config.lock().launcher.clone();
    let viewport_size = native_ui::overlay_viewport_size(egui::vec2(cfg.width, cfg.height));
    let options = native_ui::options(
        native_ui::overlay_viewport_builder([viewport_size.x, viewport_size.y])
            .with_position(OFF_SCREEN),
    );

    let settings = Arc::new(Mutex::new(SettingsPanel::new(&config.lock())));
    let marks_switcher = take_ui_receiver().map(MarksSwitcherPanel::new);
    if marks_switcher.is_none() {
        log::error("marks switcher ui receiver missing");
    }
    let result = eframe::run_native(
        "WinHarpoon",
        options,
        Box::new(move |cc| {
            log::debug("run_ui creating app");
            crate::launcher::register_context(&cc.egui_ctx);
            apply_launcher_theme(&cc.egui_ctx);
            Ok(Box::new(UiApp {
                config,
                state,
                marks,
                command_rx,
                launcher: LauncherPanel::default(),
                settings,
                marks_switcher,
                tray_menu: TrayMenuPanel::new(),
            }))
        }),
    );
    match &result {
        Ok(()) => log::debug("run_ui event loop exited"),
        Err(err) => log::error(&format!("run_ui failed: {err}")),
    }
}

struct UiApp {
    config: Arc<Mutex<Config>>,
    state: Arc<Mutex<AppState>>,
    marks: SharedMarks,
    command_rx: Receiver<UiCommand>,
    launcher: LauncherPanel,
    settings: Arc<Mutex<SettingsPanel>>,
    marks_switcher: Option<MarksSwitcherPanel>,
    tray_menu: TrayMenuPanel,
}

impl eframe::App for UiApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_commands(&ctx);

        let switcher_visible = self
            .marks_switcher
            .as_ref()
            .is_some_and(|s| s.is_visible());

        let tray_menu_visible = self.tray_menu.is_visible();

        if switcher_visible || self.launcher.visible || tray_menu_visible {
            crate::platform::apply_dwm_transparency(frame);
        }

        if let Some(switcher) = &mut self.marks_switcher {
            switcher.poll_commands(&ctx);
            if switcher.is_visible() {
                switcher.update(ui);
                ctx.request_repaint();
            }
        }

        if !switcher_visible && self.launcher.visible {
            self.launcher
                .update(&ctx, &self.config, &self.marks);
        }

        if tray_menu_visible {
            self.tray_menu.update(&ctx, &self.state);
        }

        if self.settings.lock().visible {
            let settings = self.settings.clone();
            let state = self.state.clone();
            let config = self.config.clone();
            ctx.show_viewport_deferred(
                egui::ViewportId::from_hash_of("winharpoon_settings"),
                egui::ViewportBuilder::default()
                    .with_inner_size([760.0, 720.0])
                    .with_title("WinHarpoon Settings")
                    .with_always_on_top(),
                move |ui, _class| {
                    settings.lock().update(ui, &state, &config);
                },
            );
        }

        let settings_visible = self.settings.lock().visible;
        if !self.launcher.visible && !settings_visible && !switcher_visible && !tray_menu_visible {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

impl UiApp {
    fn poll_commands(&mut self, ctx: &egui::Context) {
        if take_pending_launcher() {
            self.hide_switcher(ctx);
            self.launcher.show(ctx, &self.config);
        }
        if take_pending_settings() {
            self.open_settings_panel(ctx);
        }
        if let Some(info) = take_pending_tray_menu() {
            self.open_tray_menu(ctx, info);
        }
        while let Ok(command) = self.command_rx.try_recv() {
            match command {
                UiCommand::Launcher => {
                    self.hide_switcher(ctx);
                    self.launcher.show(ctx, &self.config);
                }
                UiCommand::Settings => self.open_settings_panel(ctx),
                UiCommand::TrayMenu => {
                    if let Some(info) = take_pending_tray_menu() {
                        self.open_tray_menu(ctx, info);
                    }
                }
            }
        }
    }

    fn open_tray_menu(&mut self, ctx: &egui::Context, info: crate::launcher::TrayClickInfo) {
        self.hide_switcher(ctx);
        if self.launcher.visible {
            self.launcher.hide(ctx);
        }
        if self.settings.lock().visible {
            self.settings.lock().hide(ctx);
        }
        self.tray_menu.show(ctx, info);
    }

    fn hide_switcher(&mut self, ctx: &egui::Context) {
        crate::marks_switcher::cancel_if_active();
        if let Some(switcher) = &mut self.marks_switcher {
            switcher.force_hide(ctx);
        }
    }

    fn open_settings_panel(&mut self, ctx: &egui::Context) {
        self.hide_switcher(ctx);
        if self.launcher.visible {
            self.launcher.hide(ctx);
        }
        self.settings.lock().show(&self.config.lock());
        ctx.request_repaint();
    }
}

struct LauncherPanel {
    query: String,
    windows: Vec<WindowInfo>,
    selected: usize,
    foreground_hwnd: Option<isize>,
    stack_snapshot: Option<StackSnapshot>,
    preview_hwnd: Option<isize>,
    preview_active: bool,
    hovered_row: Option<usize>,
    matcher: Matcher,
    utf32_buf: Vec<char>,
    visible: bool,
    panel_size: egui::Vec2,
    open_frames: u32,
    scroll_to_selected: bool,
    last_query: String,
}

impl LauncherPanel {
    fn show(&mut self, ctx: &egui::Context, config: &Arc<Mutex<Config>>) {
        log::debug("launcher show");
        self.windows = enumerate_windows(None);
        self.stack_snapshot = Some(capture_stack_snapshot());
        self.foreground_hwnd = self
            .stack_snapshot
            .as_ref()
            .and_then(|s| s.foreground)
            .or_else(|| get_foreground_window().map(|w| w.hwnd));
        self.preview_hwnd = None;
        self.preview_active = false;
        self.hovered_row = None;
        self.query.clear();
        self.last_query.clear();
        self.selected = 0;
        self.open_frames = 0;
        self.scroll_to_selected = true;
        self.visible = true;

        let cfg = config.lock().launcher.clone();
        self.panel_size = egui::vec2(cfg.width, cfg.height);
        crate::platform::cover_active_monitor(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        log::debug(&format!(
            "launcher ready: {} windows, foreground={:?}",
            self.windows.len(),
            self.foreground_hwnd
        ));
    }

    fn cancel(&mut self, ctx: &egui::Context) {
        if let Some(snapshot) = self.stack_snapshot.take() {
            restore_stack_snapshot(&snapshot);
        }
        self.preview_hwnd = None;
        self.preview_active = false;
        self.hovered_row = None;
        self.hide(ctx);
    }

    fn commit(&mut self, ctx: &egui::Context, hwnd: isize) {
        self.stack_snapshot = None;
        self.preview_hwnd = None;
        self.preview_active = false;
        self.hovered_row = None;
        self.hide(ctx);
        focus::focus_window(hwnd);
    }

    fn hide(&mut self, ctx: &egui::Context) {
        if !self.visible {
            return;
        }
        log::debug("launcher hide");
        self.visible = false;
        self.open_frames = 0;
        self.preview_hwnd = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(OFF_SCREEN));
    }

    fn filtered(&mut self, config: &Arc<Mutex<Config>>) -> Vec<(usize, u32)> {
        let max_results = config.lock().launcher.max_results;
        if self.query.trim().is_empty() {
            return self
                .windows
                .iter()
                .enumerate()
                .take(max_results)
                .map(|(i, _)| (i, 0))
                .collect();
        }

        let pattern = Pattern::parse(
            self.query.as_str(),
            CaseMatching::Ignore,
            Normalization::Smart,
        );
        let mut scored = Vec::new();
        for (idx, win) in self.windows.iter().enumerate() {
            let label = format!("{} {}", win.title, win.process_name);
            let haystack = Utf32Str::new(&label, &mut self.utf32_buf);
            if let Some(score) = pattern.score(haystack, &mut self.matcher) {
                if score > 0 {
                    scored.push((idx, score));
                }
            }
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(max_results);
        scored
    }

    fn navigate(&mut self, filtered: &[(usize, u32)], delta: i32) {
        if filtered.is_empty() {
            return;
        }
        let len = filtered.len();
        self.selected = if delta > 0 {
            (self.selected + 1) % len
        } else if self.selected == 0 {
            len - 1
        } else {
            self.selected - 1
        };
        self.scroll_to_selected = true;
        self.hovered_row = None;
        self.preview_active = true;
    }

    fn set_preview(&mut self, ctx: &egui::Context, hwnd: isize) {
        if self.preview_hwnd == Some(hwnd) {
            return;
        }
        self.preview_hwnd = Some(hwnd);
        focus::preview_window(hwnd);
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn sync_preview(&mut self, ctx: &egui::Context, filtered: &[(usize, u32)]) {
        if !self.preview_active {
            return;
        }
        let active_row = self.hovered_row.unwrap_or(self.selected);
        let Some((idx, _)) = filtered.get(active_row) else {
            return;
        };
        self.set_preview(ctx, self.windows[*idx].hwnd);
    }

    fn active_window(&self, filtered: &[(usize, u32)]) -> Option<&WindowInfo> {
        let active_row = self.hovered_row.unwrap_or(self.selected);
        filtered
            .get(active_row)
            .map(|(idx, _)| &self.windows[*idx])
    }

    fn update(
        &mut self,
        ctx: &egui::Context,
        config: &Arc<Mutex<Config>>,
        marks: &SharedMarks,
    ) {
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cancel(ctx);
            return;
        }

        self.open_frames = self.open_frames.saturating_add(1);
        if self.open_frames <= 3 {
            crate::platform::cover_active_monitor(ctx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel(ctx);
            return;
        }

        let filtered = self.filtered(config);

        if ctx.input(|i| {
            i.key_pressed(egui::Key::M)
                && i.modifiers.ctrl
                && !i.modifiers.shift
                && !i.modifiers.alt
        }) {
            ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL, egui::Key::M);
            });
            if let Some(win) = self.active_window(&filtered) {
                let result = marks.lock().store.toggle_mark_for(win);
                notify_toggle_result(result);
            }
        }

        let mark_slot = self.active_window(&filtered).and_then(|win| {
            marks
                .lock()
                .store
                .find_slot(&WindowIdentity::from_window(win))
        });

        if let Some(slot) = mark_slot {
            if ctx.input(|i| {
                i.key_pressed(egui::Key::ArrowUp)
                    && i.modifiers.ctrl
                    && i.modifiers.shift
            }) {
                ctx.input_mut(|i| {
                    i.consume_key(
                        egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                        egui::Key::ArrowUp,
                    );
                });
                marks.lock().store.move_mark_slot(slot, true);
            } else if ctx.input(|i| {
                i.key_pressed(egui::Key::ArrowDown)
                    && i.modifiers.ctrl
                    && i.modifiers.shift
            }) {
                ctx.input_mut(|i| {
                    i.consume_key(
                        egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                        egui::Key::ArrowDown,
                    );
                });
                marks.lock().store.move_mark_slot(slot, false);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) && !i.modifiers.ctrl) && !filtered.is_empty()
        {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
            self.navigate(&filtered, 1);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) && !i.modifiers.ctrl) && !filtered.is_empty()
        {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
            self.navigate(&filtered, -1);
        }

        if self.query != self.last_query {
            self.last_query = self.query.clone();
            self.selected = 0;
            self.scroll_to_selected = true;
            self.hovered_row = None;
            self.preview_active = true;
        }

        self.hovered_row = None;

        let mark_slots: Vec<(WindowIdentity, u8)> = {
            let store = &marks.lock().store;
            (1..=9)
                .filter_map(|slot| {
                    store
                        .slots
                        .get(&slot.to_string())
                        .map(|id| (id.clone(), slot))
                })
                .collect()
        };

        let viewport_rect = ctx.input(|i| {
            i.viewport()
                .inner_rect
                .unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, ctx.content_rect().size()))
        });

        let mut backdrop_click = None;
        egui::Area::new(egui::Id::new("launcher_backdrop"))
            .order(egui::Order::Background)
            .fixed_pos(viewport_rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (_, response) =
                    ui.allocate_exact_size(viewport_rect.size(), egui::Sense::click());
                backdrop_click = Some(response);
            });

        egui::Area::new(egui::Id::new("launcher_panel"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_min_size(self.panel_size);
                ui.set_max_size(self.panel_size);
                native_ui::overlay_panel_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("WinHarpoon")
                                .size(12.0)
                                .strong()
                                .color(native_ui::ACCENT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{} windows", self.windows.len()))
                                    .size(10.5)
                                    .color(native_ui::TEXT_MUTED),
                            );
                        });
                    });
                    ui.add_space(2.0);

                    search_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("⌕")
                                    .size(14.0)
                                    .color(native_ui::TEXT_DIM),
                            );
                            ui.add_space(2.0);
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut self.query)
                                    .hint_text("Search windows…")
                                    .desired_width(f32::INFINITY)
                                    .frame(egui::Frame::NONE)
                                    .font(egui::FontId::proportional(13.0)),
                            );
                            if self.open_frames <= 3 || response.gained_focus() || self.query.is_empty()
                            {
                                response.request_focus();
                            }
                        });
                    });

                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp))
                    {
                        ui.ctx().input_mut(|i| {
                            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
                            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
                        });
                    }

                    ui.add_space(6.0);

                    let hint_reserve = 28.0;
                    let list_width = ui.available_width();
                    let scroll_height = (ui.available_height() - hint_reserve).max(80.0);
                    let selected_row = self.selected;
                    let scroll_to = self.scroll_to_selected;
                    if scroll_to {
                        self.scroll_to_selected = false;
                    }

                    egui::ScrollArea::vertical()
                        .id_salt("launcher_list")
                        .max_height(scroll_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(list_width);
                            ui.with_layout(
                                egui::Layout::top_down(egui::Align::Min).with_cross_justify(true),
                                |ui| {
                                    ui.set_width(list_width);
                                    if filtered.is_empty() {
                                        ui.vertical_centered(|ui| {
                                            ui.add_space(12.0);
                                            ui.label(
                                                egui::RichText::new("No matching windows")
                                                    .size(11.5)
                                                    .color(native_ui::TEXT_MUTED),
                                            );
                                        });
                                        return;
                                    }

                                    for (row, (idx, _score)) in filtered.iter().enumerate() {
                                        let win = &self.windows[*idx];
                                        let is_fg = self.foreground_hwnd == Some(win.hwnd);
                                        let win_identity = WindowIdentity::from_window(win);
                                        let mark_slot = mark_slots
                                            .iter()
                                            .find(|(id, _)| id == &win_identity)
                                            .map(|(_, slot)| *slot);
                                        let keyboard_highlight =
                                            self.hovered_row.is_none() && row == selected_row;

                                        let card =
                                            native_ui::overlay_card_frame(keyboard_highlight, is_fg);
                                        let card_response = card.show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.set_width(ui.available_width());
                                                ui.vertical(|ui| {
                                                    ui.set_width((ui.available_width() - 72.0).max(0.0));
                                                    ui.label(
                                                        egui::RichText::new(&win.title)
                                                            .size(12.0)
                                                            .strong()
                                                            .color(if keyboard_highlight {
                                                                egui::Color32::WHITE
                                                            } else {
                                                                egui::Color32::from_rgb(220, 224, 234)
                                                            }),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&win.process_name)
                                                            .size(10.0)
                                                            .color(native_ui::TEXT_DIM),
                                                    );
                                                });

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        if let Some(slot) = mark_slot {
                                                            native_ui::badge(
                                                                ui,
                                                                &format!("Mark {slot}"),
                                                                native_ui::ACCENT,
                                                            );
                                                        }
                                                    },
                                                );
                                            });
                                        });

                                let hovered = card_response.response.hovered();
                                if hovered {
                                    self.hovered_row = Some(row);
                                    self.preview_active = true;
                                }
                                let highlighted = hovered || keyboard_highlight;
                                if hovered && !keyboard_highlight {
                                    let rect = card_response.response.rect;
                                    ui.painter().rect_filled(
                                        rect,
                                        6.0,
                                        native_ui::ACCENT.gamma_multiply(0.18),
                                    );
                                    ui.painter().rect_stroke(
                                        rect,
                                        6,
                                        egui::Stroke::new(1.0, native_ui::ACCENT.gamma_multiply(0.7)),
                                        egui::StrokeKind::Inside,
                                    );
                                }

                                if highlighted {
                                    let bar_rect = egui::Rect::from_min_size(
                                        card_response.response.rect.left_top()
                                            + egui::vec2(0.0, 5.0),
                                        egui::vec2(2.0, card_response.response.rect.height() - 10.0),
                                    );
                                    ui.painter().rect_filled(bar_rect, 1.0, native_ui::ACCENT);
                                    if scroll_to {
                                        ui.scroll_to_rect(
                                            card_response.response.rect,
                                            Some(egui::Align::Center),
                                        );
                                    }
                                }

                                if card_response.response.clicked() {
                                    let hwnd = win.hwnd;
                                    log::debug(&format!(
                                        "launcher click: {} ({})",
                                        win.title, win.process_name
                                    ));
                                    self.commit(ui.ctx(), hwnd);
                                }
                                ui.add_space(2.0);
                            }
                                },
                            );
                        });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);
                    native_ui::overlay_keyboard_hint_bar(
                        ui,
                        &[
                            ("↑↓", "Navigate"),
                            ("Enter", "Switch"),
                            ("Ctrl+M", "Toggle mark"),
                            ("Ctrl+Shift+↑↓", "Reorder mark"),
                            ("Esc", "Close"),
                        ],
                    );
                });
            });

        if backdrop_click.is_some_and(|r| r.clicked()) {
            self.cancel(ctx);
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            let active_row = self.hovered_row.unwrap_or(self.selected);
            if let Some((idx, _)) = filtered.get(active_row) {
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                let hwnd = self.windows[*idx].hwnd;
                let title = self.windows[*idx].title.clone();
                let exe = self.windows[*idx].process_name.clone();
                log::debug(&format!("launcher select: {title} ({exe})"));
                self.commit(ctx, hwnd);
                return;
            }
        }

        self.sync_preview(ctx, &filtered);
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

fn apply_launcher_theme(ctx: &egui::Context) {
    native_ui::apply_overlay_theme(ctx);
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 3.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(12.0),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(13.0),
    );
    ctx.set_global_style(style);
}

fn search_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(18, 20, 26))
        .stroke(egui::Stroke::new(1.0, native_ui::BORDER))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(8, 6))
}

impl Default for LauncherPanel {
    fn default() -> Self {
        Self {
            query: String::new(),
            windows: Vec::new(),
            selected: 0,
            foreground_hwnd: None,
            stack_snapshot: None,
            preview_hwnd: None,
            preview_active: false,
            hovered_row: None,
            matcher: Matcher::new(MatcherConfig::DEFAULT),
            utf32_buf: Vec::new(),
            visible: false,
            panel_size: egui::vec2(480.0, 420.0),
            open_frames: 0,
            scroll_to_selected: false,
            last_query: String::new(),
        }
    }
}
