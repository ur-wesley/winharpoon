use eframe::egui;

pub const OFF_SCREEN: egui::Pos2 = egui::pos2(-20_000.0, -20_000.0);

pub fn prepare_acrylic_overlay(ctx: &egui::Context, frame: Option<&eframe::Frame>) {
    crate::native_ui::apply_overlay_theme(ctx);
    ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
    #[cfg(windows)]
    {
        let focused = ctx.input(|i| i.viewport().focused) != Some(false);
        crate::platform::refresh_popup_glass(frame, focused);
    }
}

pub fn hide_overlay_viewport(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(OFF_SCREEN));
}

pub fn position_overlay_at(ctx: &egui::Context, pos: egui::Pos2, content: egui::Vec2) {
    let size = crate::native_ui::overlay_viewport_size(content);
    let inset = (size - content) / 2.0;
    ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos - inset));
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
}

pub fn position_centered_overlay(ctx: &egui::Context, content: egui::Vec2) {
    let size = crate::native_ui::overlay_viewport_size(content);
    ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    if let Some(cmd) = egui::ViewportCommand::center_on_screen(ctx) {
        ctx.send_viewport_cmd(cmd);
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
}

pub fn dismiss_if_focus_lost(ctx: &egui::Context, open_frames: u32) -> bool {
    open_frames > 1 && ctx.input(|i| i.viewport().focused == Some(false))
}

pub fn handle_viewport_close(ctx: &egui::Context) -> bool {
    if ctx.input(|i| i.viewport().close_requested()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        return true;
    }
    false
}

pub fn handle_escape(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.key_pressed(egui::Key::Escape))
}

#[derive(Debug, Clone, Copy)]
pub struct OverlayLifecycle {
    pub open_frames: u32,
    pub menu_rect: egui::Rect,
}

impl Default for OverlayLifecycle {
    fn default() -> Self {
        Self {
            open_frames: 0,
            menu_rect: egui::Rect::NOTHING,
        }
    }
}

impl OverlayLifecycle {
    pub fn reset(&mut self) {
        self.open_frames = 0;
        self.menu_rect = egui::Rect::NOTHING;
    }

    pub fn tick(&mut self) {
        self.open_frames = self.open_frames.saturating_add(1);
    }
}
