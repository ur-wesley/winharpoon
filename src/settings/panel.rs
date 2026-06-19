use std::sync::Arc;

use eframe::egui;
use parking_lot::Mutex;

use crate::app::AppState;
use crate::config::Config;
use crate::native_ui;
use crate::settings::controller::{SettingsAction, SettingsController};
use crate::settings::view::render_settings;
use crate::ui::chord::{poll_chord_capture, ChordCaptureResult};

pub struct SettingsPanel {
    pub controller: SettingsController,
}

impl SettingsPanel {
    pub fn new(config: &Config) -> Self {
        Self {
            controller: SettingsController::new(config),
        }
    }

    pub fn visible(&self) -> bool {
        self.controller.visible
    }

    pub fn show(&mut self, config: &Config) {
        self.controller.on_show(config);
    }

    pub fn hide(&mut self, ctx: &egui::Context) {
        if !self.controller.visible {
            return;
        }
        self.controller.on_hide();
        native_ui::apply_overlay_theme(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub fn update(
        &mut self,
        ui: &mut egui::Ui,
        state: &Arc<Mutex<AppState>>,
        config: &Arc<Mutex<Config>>,
    ) {
        let ctx = ui.ctx().clone();

        #[cfg(windows)]
        crate::platform::apply_dwm_glass_for_title(
            crate::platform::SETTINGS_VIEWPORT_TITLE,
            crate::platform::GlassBackdrop::Mica,
        );

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.controller.visible {
                self.controller.on_hide();
                native_ui::apply_overlay_theme(&ctx);
            }
            return;
        }

        let output = render_settings(ui, &self.controller, state, config);

        for action in output.actions {
            self.controller.handle_action(action, config);
        }

        if let Some(label) = self.controller.capture_label.clone() {
            match poll_chord_capture(&ctx, true) {
                ChordCaptureResult::Cancelled => {
                    self.controller.handle_action(SettingsAction::CancelCapture, config);
                }
                ChordCaptureResult::Captured(chord) => {
                    self.controller.handle_action(
                        SettingsAction::FinishCapture {
                            label: label.clone(),
                            chord,
                        },
                        config,
                    );
                }
                ChordCaptureResult::Pending => {}
            }

            if ctx.input(|i| i.pointer.any_click()) && !output.capture_ui_used {
                self.controller.handle_action(SettingsAction::CancelCapture, config);
            }
        }
    }
}
