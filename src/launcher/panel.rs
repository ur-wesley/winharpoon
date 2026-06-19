use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::config::Config;
use crate::icons::IconCache;
use crate::launcher::controller::{LauncherAction, LauncherController};
use crate::launcher::view::render_launcher;
use crate::log;
use crate::modes::marks::SharedMarks;
use crate::ui::overlay::{
    dismiss_if_focus_lost, handle_escape, handle_viewport_close, hide_overlay_viewport,
    position_centered_overlay, prepare_acrylic_overlay, OverlayLifecycle,
};

#[derive(Default)]
pub struct LauncherPanel {
    controller: LauncherController,
    lifecycle: OverlayLifecycle,
}


impl LauncherPanel {
    pub fn visible(&self) -> bool {
        self.controller.visible
    }

    pub fn show(&mut self, ctx: &egui::Context, config: &Arc<Mutex<Config>>) {
        self.controller.on_show(config);
        self.lifecycle.reset();
        prepare_acrylic_overlay(ctx, None);
        let (w, h) = self.controller.panel_size();
        position_centered_overlay(ctx, egui::vec2(w, h));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    pub fn hide(&mut self, ctx: &egui::Context) {
        if !self.controller.visible {
            return;
        }
        log::debug("launcher hide");
        self.controller.visible = false;
        self.lifecycle.reset();
        self.controller.preview_hwnd = None;
        hide_overlay_viewport(ctx);
    }

    pub fn dismiss(&mut self, ctx: &egui::Context, marks: &SharedMarks) {
        if !self.controller.visible {
            return;
        }
        log::debug("launcher dismiss");
        let effect = self
            .controller
            .handle_action(LauncherAction::Cancel, marks, &[]);
        effect.apply();
        self.lifecycle.reset();
        hide_overlay_viewport(ctx);
    }

    pub fn update(
        &mut self,
        ui: &mut egui::Ui,
        config: &Arc<Mutex<Config>>,
        marks: &SharedMarks,
        icon_cache: &mut IconCache,
    ) {
        let ctx = ui.ctx().clone();
        if handle_viewport_close(&ctx) {
            self.dismiss(&ctx, marks);
            return;
        }

        self.lifecycle.tick();
        if self.lifecycle.open_frames <= 3 {
            let (w, h) = self.controller.panel_size();
            position_centered_overlay(&ctx, egui::vec2(w, h));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        if dismiss_if_focus_lost(&ctx, self.lifecycle.open_frames) {
            self.dismiss(&ctx, marks);
            return;
        }

        if handle_escape(&ctx) {
            self.dismiss(&ctx, marks);
            return;
        }

        let max_results = config.lock().launcher.max_results;
        let filtered = self.controller.filtered_indices(max_results);

        let scroll_to = self.controller.selection.take_scroll();
        let output = render_launcher(
            ui,
            &mut self.controller,
            &filtered,
            marks,
            icon_cache,
            self.lifecycle.open_frames,
            scroll_to,
        );

        for action in output.actions {
            self.controller
                .handle_action(action, marks, &filtered)
                .apply();
        }

        let preview_effect = self.controller.post_frame_effects(&filtered);
        preview_effect.apply();

        if !self.controller.visible {
            hide_overlay_viewport(&ctx);
        }
    }
}
