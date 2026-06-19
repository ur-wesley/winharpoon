use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::apps::controller::{AppMenuAction, AppMenuAnchor, AppMenuController};
use crate::apps::favorites::SharedFavorites;
use crate::apps::view::render_app_menu;
use crate::config::Config;
use crate::icons::IconCache;
use crate::ui::chord::{poll_chord_capture, ChordCaptureResult};
use crate::ui::overlay::{
    dismiss_if_focus_lost, handle_escape, handle_viewport_close, hide_overlay_viewport,
    position_centered_overlay, prepare_acrylic_overlay, OverlayLifecycle,
};

pub struct AppMenuPanel {
    controller: AppMenuController,
    lifecycle: OverlayLifecycle,
}

impl AppMenuPanel {
    pub fn new() -> Self {
        Self {
            controller: AppMenuController::default(),
            lifecycle: OverlayLifecycle::default(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.controller.visible
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        anchor: AppMenuAnchor,
        config: &Config,
        favorites: &SharedFavorites,
    ) {
        self.controller.on_show(anchor, config, favorites);
        self.lifecycle.reset();
        prepare_acrylic_overlay(ctx, None);
        let (w, h) = self.controller.panel_size();
        position_centered_overlay(ctx, egui::vec2(w, h.min(520.0)));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    pub fn hide(&mut self, ctx: &egui::Context) {
        if !self.controller.visible {
            return;
        }
        self.controller.visible = false;
        self.lifecycle.reset();
        self.controller.capture_hotkey_id = None;
        hide_overlay_viewport(ctx);
    }

    pub fn update(
        &mut self,
        ui: &mut egui::Ui,
        config: &Arc<Mutex<Config>>,
        favorites: &SharedFavorites,
        state: &Arc<Mutex<AppState>>,
        icon_cache: &mut IconCache,
    ) {
        let ctx = ui.ctx().clone();
        if !self.controller.visible {
            return;
        }

        if handle_viewport_close(&ctx) {
            self.hide(&ctx);
            return;
        }

        self.lifecycle.tick();
        if self.lifecycle.open_frames <= 3 {
            let (panel_w, panel_h) = self.controller.panel_size();
            position_centered_overlay(&ctx, egui::vec2(panel_w, panel_h.min(520.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        if dismiss_if_focus_lost(&ctx, self.lifecycle.open_frames) {
            self.hide(&ctx);
            return;
        }

        let cfg = config.lock().apps.clone();
        self.controller.panel_width = cfg.width;
        self.controller.panel_height = cfg.height;
        self.controller.max_results = cfg.max_results;

        if let Some(id) = self.controller.capture_hotkey_id.clone() {
            match poll_chord_capture(&ctx, false) {
                ChordCaptureResult::Cancelled => {
                    self.controller
                        .handle_action(AppMenuAction::CancelHotkeyCapture, favorites, state);
                }
                ChordCaptureResult::Captured(chord) => {
                    self.controller.handle_action(
                        AppMenuAction::SetFavoriteHotkey {
                            id: id.clone(),
                            chord,
                        },
                        favorites,
                        state,
                    );
                }
                ChordCaptureResult::Pending => {}
            }
        }

        if handle_escape(&ctx) && self.controller.capture_hotkey_id.is_none() {
            self.controller.handle_action(AppMenuAction::Close, favorites, state);
            hide_overlay_viewport(&ctx);
            return;
        }

        let scroll_to = self.controller.selection.take_scroll();
        let output = render_app_menu(
            ui,
            &mut self.controller,
            favorites,
            state,
            icon_cache,
            self.lifecycle.open_frames,
            scroll_to,
        );

        let mut should_hide = false;
        for action in &output.actions {
            if let AppMenuAction::Launch(_) = action {
                should_hide = true;
            }
        }

        if should_hide {
            hide_overlay_viewport(&ctx);
            ctx.request_repaint();
        }

        for action in output.actions {
            self.controller.handle_action(action, favorites, state);
        }
    }
}

impl Default for AppMenuPanel {
    fn default() -> Self {
        Self::new()
    }
}
