use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::hotkeys::post_quit;
use crate::launcher::TrayClickInfo;
use crate::log;
use crate::native_ui;
use crate::paths;

const MENU_WIDTH: f32 = 272.0;
const ROW_HEIGHT: f32 = 34.0;
const HEADER_HEIGHT: f32 = 44.0;
const SECTION_GAP: f32 = 6.0;
const SLOT_ROWS: f32 = 9.0;

pub struct TrayMenuPanel {
    visible: bool,
    anchor: TrayClickInfo,
    open_frames: u32,
    menu_rect: egui::Rect,
}

impl TrayMenuPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            anchor: TrayClickInfo::default(),
            open_frames: 0,
            menu_rect: egui::Rect::NOTHING,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, ctx: &egui::Context, anchor: TrayClickInfo) {
        log::debug("tray menu show");
        self.anchor = anchor;
        self.open_frames = 0;
        self.visible = true;
        native_ui::apply_overlay_theme(ctx);
        crate::platform::cover_monitor_at_physical_point(ctx, anchor.click_x, anchor.click_y);
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    pub fn hide(&mut self, ctx: &egui::Context) {
        if !self.visible {
            return;
        }
        log::debug("tray menu hide");
        self.visible = false;
        self.open_frames = 0;
        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(-20_000.0, -20_000.0)));
        ctx.request_repaint();
    }

    pub fn update(&mut self, ctx: &egui::Context, state: &Arc<Mutex<AppState>>) {
        if !self.visible {
            return;
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(ctx);
            return;
        }

        self.open_frames = self.open_frames.saturating_add(1);
        let menu_height = self.menu_height();
        self.menu_rect = self.menu_screen_rect(ctx, menu_height);

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.hide(ctx);
            return;
        }

        if self.open_frames > 1 && ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !self.menu_rect.contains(pos) {
                    self.hide(ctx);
                    return;
                }
            }
        }

        let conflicts = state.lock().hotkey_conflicts;
        let slot_labels: Vec<(u8, String, bool)> = {
            let state_guard = state.lock();
            let marks = state_guard.marks.lock();
            (1..=9)
                .map(|slot| {
                    let label = marks.store.slot_label(slot);
                    let filled = label != "empty";
                    (slot, label, filled)
                })
                .collect()
        };

        let mut close = false;
        let mut action: Option<TrayAction> = None;

        let frame = native_ui::tray_menu_frame();
        egui::Area::new(egui::Id::new("tray_menu_panel"))
            .fixed_pos(self.menu_rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(MENU_WIDTH - 12.0);
                    ui.vertical(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), HEADER_HEIGHT - 8.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("WinHarpoon")
                                            .size(15.0)
                                            .strong()
                                            .color(egui::Color32::WHITE),
                                    );
                                    ui.label(
                                        egui::RichText::new("Window switcher")
                                            .size(11.0)
                                            .color(native_ui::TEXT_DIM),
                                    );
                                });
                                if conflicts > 0 {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            native_ui::badge(
                                                ui,
                                                &format!("{conflicts} conflicts"),
                                                native_ui::WARNING,
                                            );
                                        },
                                    );
                                }
                            },
                        );

                        native_ui::tray_menu_divider(ui);

                        if tray_menu_item(ui, "Settings", "⚙", None).clicked() {
                            action = Some(TrayAction::Settings);
                        }

                        native_ui::tray_menu_divider(ui);
                        native_ui::tray_menu_section_label(ui, "Marked slots");

                        for (slot, label, filled) in &slot_labels {
                            let subtitle = if *filled {
                                label.as_str()
                            } else {
                                "Empty"
                            };
                            let response = tray_slot_item(ui, *slot, subtitle, *filled);
                            if *filled && response.clicked() {
                                action = Some(TrayAction::JumpSlot(*slot));
                            }
                        }

                        native_ui::tray_menu_divider(ui);

                        if tray_menu_item(ui, "Open config folder", "📁", None).clicked() {
                            action = Some(TrayAction::ConfigFolder);
                        }
                        if tray_menu_item(ui, "Reload config", "↻", None).clicked() {
                            action = Some(TrayAction::Reload);
                        }

                        native_ui::tray_menu_divider(ui);

                        if tray_menu_item(ui, "Quit", "⏻", Some(native_ui::DANGER)).clicked() {
                            action = Some(TrayAction::Quit);
                        }
                    });
                });
            });

        if let Some(act) = action {
            close = true;
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
        }

        if close {
            self.hide(ctx);
        } else {
            ctx.request_repaint();
        }
    }

    fn menu_height(&self) -> f32 {
        HEADER_HEIGHT
            + ROW_HEIGHT
            + SECTION_GAP
            + 18.0
            + SLOT_ROWS * 30.0
            + SECTION_GAP
            + ROW_HEIGHT * 2.0
            + SECTION_GAP
            + ROW_HEIGHT
            + 16.0
    }

    fn menu_screen_rect(&self, ctx: &egui::Context, height: f32) -> egui::Rect {
        let ppp = ctx
            .input(|i| i.viewport().native_pixels_per_point)
            .unwrap_or(1.0);

        let tray_x = self.anchor.rect_x as f32 / ppp;
        let tray_y = self.anchor.rect_y as f32 / ppp;
        let tray_w = self.anchor.rect_w as f32 / ppp;
        let tray_h = self.anchor.rect_h as f32 / ppp;

        let viewport_origin = ctx
            .input(|i| i.viewport().outer_rect)
            .map(|r| r.min)
            .unwrap_or(egui::Pos2::ZERO);

        let monitor_size = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(egui::vec2(1920.0, 1080.0));

        let tray_screen_y = tray_y;
        let taskbar_at_bottom = tray_screen_y > monitor_size.y * 0.5;

        let mut menu_x = tray_x + tray_w - MENU_WIDTH;
        let mut menu_y = if taskbar_at_bottom {
            tray_y - height - 10.0
        } else {
            tray_y + tray_h + 10.0
        };

        menu_x = menu_x.clamp(viewport_origin.x + 8.0, viewport_origin.x + monitor_size.x - MENU_WIDTH - 8.0);
        menu_y = menu_y.clamp(viewport_origin.y + 8.0, viewport_origin.y + monitor_size.y - height - 8.0);

        egui::Rect::from_min_size(egui::pos2(menu_x, menu_y), egui::vec2(MENU_WIDTH, height))
    }
}

enum TrayAction {
    Settings,
    ConfigFolder,
    Reload,
    Quit,
    JumpSlot(u8),
}

fn tray_menu_item(
    ui: &mut egui::Ui,
    label: &str,
    icon: &str,
    accent: Option<egui::Color32>,
) -> egui::Response {
    let text_color = accent.unwrap_or(egui::Color32::from_rgb(220, 224, 234));
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(
                rect.expand2(egui::vec2(-4.0, 1.0)),
                8,
                native_ui::SURFACE_HOVER,
            );
        }
        let icon_pos = rect.left_center() + egui::vec2(14.0, 0.0);
        ui.painter().text(
            icon_pos,
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(14.0),
            native_ui::ACCENT,
        );
        ui.painter().text(
            rect.left_center() + egui::vec2(38.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            text_color,
        );
    }
    response
}

fn tray_slot_item(ui: &mut egui::Ui, slot: u8, label: &str, filled: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 30.0),
        if filled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    if ui.is_rect_visible(rect) {
        if filled && response.hovered() {
            ui.painter().rect_filled(
                rect.expand2(egui::vec2(-4.0, 1.0)),
                7,
                native_ui::SURFACE_HOVER,
            );
        }
        let badge_rect = egui::Rect::from_center_size(
            rect.left_center() + egui::vec2(18.0, 0.0),
            egui::vec2(20.0, 20.0),
        );
        let badge_fill = if filled {
            native_ui::ACCENT.gamma_multiply(0.22)
        } else {
            native_ui::SURFACE_RAISED
        };
        let badge_stroke = if filled {
            native_ui::ACCENT.gamma_multiply(0.5)
        } else {
            native_ui::BORDER.gamma_multiply(0.6)
        };
        ui.painter().rect(
            badge_rect,
            5.0,
            badge_fill,
            egui::Stroke::new(1.0, badge_stroke),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            slot.to_string(),
            egui::FontId::proportional(11.0),
            if filled {
                native_ui::ACCENT
            } else {
                native_ui::TEXT_DIM
            },
        );
        ui.painter().text(
            rect.left_center() + egui::vec2(38.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.5),
            if filled {
                egui::Color32::from_rgb(210, 214, 226)
            } else {
                native_ui::TEXT_DIM
            },
        );
    }
    response
}
