use std::sync::mpsc::Receiver;

use eframe::egui;

use crate::log;
use crate::marks_switcher::SwitcherUiCommand;
use crate::modes::marks::MarkEntry;
use crate::native_ui;

const OFF_SCREEN: egui::Pos2 = egui::pos2(-20_000.0, -20_000.0);

pub struct MarksSwitcherPanel {
    command_rx: Receiver<SwitcherUiCommand>,
    visible: bool,
    entries: Vec<MarkEntry>,
    selected: usize,
    open_frames: u32,
}

impl MarksSwitcherPanel {
    pub fn new(command_rx: Receiver<SwitcherUiCommand>) -> Self {
        Self {
            command_rx,
            visible: false,
            entries: Vec::new(),
            selected: 0,
            open_frames: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
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
                    self.selected = selected.min(self.entries.len().saturating_sub(1));
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

    pub fn update(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.visible {
            return;
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(&ctx);
            return;
        }

        self.open_frames = self.open_frames.saturating_add(1);
        if self.open_frames <= 3 {
            if let Some(cmd) = egui::ViewportCommand::center_on_screen(&ctx) {
                ctx.send_viewport_cmd(cmd);
            }
        }

        self.resize_viewport(&ctx);

        let selected = self.selected;
        let entries = self.entries.clone();

        egui::CentralPanel::default()
            .frame(native_ui::overlay_panel_frame())
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Marked windows")
                            .size(11.5)
                            .strong()
                            .color(native_ui::ACCENT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Release to switch")
                                .size(10.0)
                                .color(native_ui::TEXT_DIM),
                        );
                    });
                });
                ui.add_space(6.0);

                egui::ScrollArea::horizontal()
                    .id_salt("marks_switcher_cards")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = native_ui::MARKS_CARD_GAP;
                            for (idx, entry) in entries.iter().enumerate() {
                                let is_selected = idx == selected;
                                let card = native_ui::overlay_card_frame(is_selected, false);
                                let response = card.show(ui, |ui| {
                                    ui.set_width(native_ui::MARKS_CARD_WIDTH - 16.0);
                                    ui.vertical(|ui| {
                                        native_ui::badge(
                                            ui,
                                            &format!("{}", entry.slot),
                                            native_ui::ACCENT,
                                        );
                                        ui.add_space(4.0);
                                        if let Some(win) = &entry.window {
                                            ui.label(
                                                egui::RichText::new(&win.title)
                                                    .size(11.0)
                                                    .strong()
                                                    .color(if is_selected {
                                                        egui::Color32::WHITE
                                                    } else {
                                                        egui::Color32::from_rgb(220, 224, 234)
                                                    }),
                                            );
                                            ui.label(
                                                egui::RichText::new(&win.process_name)
                                                    .size(9.5)
                                                    .color(native_ui::TEXT_DIM),
                                            );
                                        }
                                    });
                                });
                                if is_selected {
                                    let bar_rect = egui::Rect::from_min_size(
                                        response.response.rect.left_top() + egui::vec2(0.0, 4.0),
                                        egui::vec2(2.0, response.response.rect.height() - 8.0),
                                    );
                                    ui.painter().rect_filled(bar_rect, 1.0, native_ui::ACCENT);
                                    ui.scroll_to_rect(response.response.rect, Some(egui::Align::Center));
                                }
                            }
                        });
                    });
            });

        ctx.request_repaint();
    }

    fn resize_viewport(&self, ctx: &egui::Context) {
        let content = native_ui::marks_switcher_content_size(self.entries.len());
        let max_width = ctx
            .input(|i| i.viewport().monitor_size.map(|s| s.x * 0.9))
            .unwrap_or(1200.0);
        let capped_content = if content.x > max_width {
            egui::vec2(max_width, content.y)
        } else {
            content
        };
        let size = native_ui::overlay_viewport_size(capped_content);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    }

    fn show(&mut self, ctx: &egui::Context, entries: Vec<MarkEntry>, selected: usize) {
        log::debug(&format!(
            "marks switcher show: {} entries, selected={selected}",
            entries.len()
        ));
        self.entries = entries;
        self.selected = selected.min(self.entries.len().saturating_sub(1));
        self.open_frames = 0;
        self.visible = true;

        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.resize_viewport(ctx);
        if let Some(cmd) = egui::ViewportCommand::center_on_screen(ctx) {
            ctx.send_viewport_cmd(cmd);
        }
        ctx.request_repaint();
    }

    fn hide(&mut self, ctx: &egui::Context) {
        if !self.visible {
            return;
        }
        log::debug("marks switcher hide");
        self.visible = false;
        self.entries.clear();
        self.selected = 0;
        self.open_frames = 0;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(OFF_SCREEN));
        ctx.request_repaint();
    }
}
