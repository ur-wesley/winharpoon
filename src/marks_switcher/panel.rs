use std::sync::mpsc::Receiver;

use eframe::egui;

use crate::icons::IconCache;
use crate::log;
use crate::marks_switcher::SwitcherUiCommand;
use crate::modes::marks::MarkEntry;
use crate::native_ui;
use crate::ui::overlay::{
    handle_viewport_close, hide_overlay_viewport, position_centered_overlay,
    prepare_acrylic_overlay, OverlayLifecycle,
};

pub struct MarksSwitcherController {
    pub visible: bool,
    pub entries: Vec<MarkEntry>,
    pub selected: usize,
}

impl MarksSwitcherController {
    pub fn on_show(&mut self, entries: Vec<MarkEntry>, selected: usize) {
        log::debug(format!(
            "marks switcher show: {} entries, selected={selected}",
            entries.len()
        ));
        self.entries = entries;
        self.selected = selected.min(self.entries.len().saturating_sub(1));
        self.visible = true;
    }

    pub fn on_hide(&mut self) {
        if !self.visible {
            return;
        }
        log::debug("marks switcher hide");
        self.visible = false;
        self.entries.clear();
        self.selected = 0;
    }

    pub fn set_selected(&mut self, selected: usize) {
        self.selected = selected.min(self.entries.len().saturating_sub(1));
    }
}

fn capped_content_size(ctx: &egui::Context, measured: egui::Vec2, entry_count: usize) -> egui::Vec2 {
    let content = if measured != egui::Vec2::ZERO {
        measured
    } else {
        native_ui::marks_switcher_content_size(entry_count)
    };
    let max_width = ctx
        .input(|i| i.viewport().monitor_size.map(|s| s.x * 0.9))
        .unwrap_or(1200.0);
    if content.x > max_width {
        egui::vec2(max_width, content.y)
    } else {
        content
    }
}

pub struct MarksSwitcherPanel {
    command_rx: Receiver<SwitcherUiCommand>,
    controller: MarksSwitcherController,
    measured_size: egui::Vec2,
    last_viewport_content: egui::Vec2,
    lifecycle: OverlayLifecycle,
}

impl MarksSwitcherPanel {
    pub fn new(command_rx: Receiver<SwitcherUiCommand>) -> Self {
        Self {
            command_rx,
            controller: MarksSwitcherController {
                visible: false,
                entries: Vec::new(),
                selected: 0,
            },
            measured_size: egui::Vec2::ZERO,
            last_viewport_content: egui::Vec2::ZERO,
            lifecycle: OverlayLifecycle::default(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.controller.visible
    }

    pub fn force_hide(&mut self, ctx: &egui::Context) {
        self.hide(ctx);
    }

    pub fn poll_commands(&mut self, ctx: &egui::Context) {
        let mut pending_show: Option<(Vec<MarkEntry>, usize)> = None;
        while let Ok(cmd) = self.command_rx.try_recv() {
            match cmd {
                SwitcherUiCommand::Show { entries, selected } => {
                    pending_show = Some((entries, selected));
                }
                SwitcherUiCommand::SetSelected(selected) => {
                    self.controller.set_selected(selected);
                    ctx.request_repaint();
                }
                SwitcherUiCommand::Hide => {
                    self.hide(ctx);
                }
            }
        }
        if let Some((entries, selected)) = pending_show {
            self.show(ctx, entries, selected);
        }
    }

    pub fn update(&mut self, ui: &mut egui::Ui, icon_cache: &mut IconCache) {
        let ctx = ui.ctx().clone();
        if !self.controller.visible {
            return;
        }
        if !crate::marks_switcher::hook::is_switcher_active() {
            self.hide(&ctx);
            return;
        }

        prepare_acrylic_overlay(&ctx, None);

        if handle_viewport_close(&ctx) {
            self.hide(&ctx);
            return;
        }

        self.lifecycle.tick();
        if self.lifecycle.open_frames <= 3 {
            let content = capped_content_size(
                &ctx,
                self.measured_size,
                self.controller.entries.len(),
            );
            position_centered_overlay(&ctx, content);
            self.last_viewport_content = content;
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        let output = crate::marks_switcher::view::render_marks_switcher(
            ui,
            &self.controller,
            icon_cache,
            &ctx,
        );
        if output.content_size != egui::Vec2::ZERO {
            self.measured_size = output.content_size;
        }

        let content = capped_content_size(
            &ctx,
            self.measured_size,
            self.controller.entries.len(),
        );
        if content != self.last_viewport_content {
            self.last_viewport_content = content;
            position_centered_overlay(&ctx, content);
        }

        ctx.request_repaint();
    }

    fn show(&mut self, ctx: &egui::Context, entries: Vec<MarkEntry>, selected: usize) {
        self.controller.on_show(entries, selected);
        self.measured_size = egui::Vec2::ZERO;
        self.last_viewport_content = egui::Vec2::ZERO;
        self.lifecycle.reset();
        prepare_acrylic_overlay(ctx, None);
        let content = capped_content_size(ctx, egui::Vec2::ZERO, self.controller.entries.len());
        position_centered_overlay(ctx, content);
        self.last_viewport_content = content;
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn hide(&mut self, ctx: &egui::Context) {
        self.controller.on_hide();
        self.lifecycle.reset();
        self.measured_size = egui::Vec2::ZERO;
        self.last_viewport_content = egui::Vec2::ZERO;
        hide_overlay_viewport(ctx);
        ctx.request_repaint();
    }
}
