use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::hotkeys::post_quit;
use crate::icons::IconCache;
use crate::launcher::TrayClickInfo;
use crate::log;
use crate::paths;
use crate::tray::controller::{TrayAction, TrayMenuController};
use crate::tray::view::render_tray_menu;
use crate::ui::overlay::{
    dismiss_if_focus_lost, handle_escape, handle_viewport_close, hide_overlay_viewport,
    position_overlay_at, prepare_acrylic_overlay, OverlayLifecycle,
};

pub struct TrayMenuPanel {
    controller: TrayMenuController,
    lifecycle: OverlayLifecycle,
    measured_size: egui::Vec2,
}

impl TrayMenuPanel {
    pub fn new() -> Self {
        Self {
            controller: TrayMenuController::default(),
            lifecycle: OverlayLifecycle::default(),
            measured_size: egui::Vec2::ZERO,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.controller.visible
    }

    pub fn show(&mut self, ctx: &egui::Context, anchor: TrayClickInfo) {
        log::debug("tray menu show");
        self.controller.on_show(anchor);
        self.lifecycle.reset();
        self.measured_size = egui::Vec2::ZERO;
        prepare_acrylic_overlay(ctx, None);
        self.position_viewport(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    pub fn hide(&mut self, ctx: &egui::Context) {
        if !self.controller.visible {
            return;
        }
        log::debug("tray menu hide");
        self.controller.on_hide();
        self.lifecycle.reset();
        hide_overlay_viewport(ctx);
        ctx.request_repaint();
    }

    pub fn update(
        &mut self,
        ui: &mut egui::Ui,
        state: &Arc<Mutex<AppState>>,
        _icon_cache: &mut IconCache,
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
            self.position_viewport(&ctx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        if dismiss_if_focus_lost(&ctx, self.lifecycle.open_frames) {
            self.hide(&ctx);
            return;
        }

        if handle_escape(&ctx) {
            self.hide(&ctx);
            return;
        }

        let output = render_tray_menu(ui, state);
        if output.content_size != egui::Vec2::ZERO {
            self.measured_size = output.content_size;
            self.position_viewport(&ctx);
        }

        if let Some(act) = output.action {
            match act {
                TrayAction::Settings => crate::launcher::open_settings(),
                TrayAction::ConfigFolder => paths::open_config_folder(),
                TrayAction::Reload => crate::hotkeys::post_reload(),
                TrayAction::Quit => post_quit(),
                TrayAction::JumpSlot(slot) => {
                    let state_guard = state.lock();
                    let marks = state_guard.marks.lock();
                    let _ = marks.store.jump_slot(slot);
                }
            }
            self.hide(&ctx);
        } else {
            ctx.request_repaint();
        }
    }

    fn position_viewport(&self, ctx: &egui::Context) {
        let content = if self.measured_size != egui::Vec2::ZERO {
            self.measured_size
        } else {
            egui::vec2(
                self.controller.panel_width,
                self.controller.menu_height(),
            )
        };
        let menu_rect = self.controller.menu_screen_rect(ctx, content);
        position_overlay_at(ctx, menu_rect.min, menu_rect.size());
    }
}

impl Default for TrayMenuPanel {
    fn default() -> Self {
        Self::new()
    }
}
