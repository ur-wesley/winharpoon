use std::sync::mpsc::Receiver;

use std::sync::Arc;

use std::time::Duration;



use eframe::egui;

use parking_lot::Mutex;



use crate::app::AppState;

use crate::apps::controller::AppMenuAnchor;

use crate::apps::panel::AppMenuPanel;

use crate::config::Config;

use crate::icons::IconCache;

use crate::launcher::panel::LauncherPanel;

use crate::launcher::{

    take_pending_app_menu, take_pending_tray_menu, TrayClickInfo, UiCommand,

};

use crate::log;

use crate::marks_switcher::panel::MarksSwitcherPanel;

use crate::marks_switcher::take_ui_receiver;

use crate::modes::marks::SharedMarks;

use crate::native_ui;

use crate::settings::SettingsPanel;

use crate::tray::panel::TrayMenuPanel;

use crate::ui::overlay::{prepare_acrylic_overlay, OFF_SCREEN};



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

            crate::icons::ui_icons::init(&cc.egui_ctx);

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

                app_menu: AppMenuPanel::new(),

                icon_cache: IconCache::new(),

            }))

        }),

    );

    match &result {

        Ok(()) => log::debug("run_ui event loop exited"),

        Err(err) => log::error(format!("run_ui failed: {err}")),

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

    app_menu: AppMenuPanel,

    icon_cache: IconCache,

}



impl eframe::App for UiApp {

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {

        egui::Rgba::TRANSPARENT.to_array()

    }



    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {

        let ctx = ui.ctx().clone();

        self.poll_commands(&ctx);



        if let Some(switcher) = &mut self.marks_switcher {

            switcher.poll_commands(&ctx);

        }



        let switcher_visible = self

            .marks_switcher

            .as_ref()

            .is_some_and(|s| s.is_visible());



        let tray_menu_visible = self.tray_menu.is_visible();

        let app_menu_visible = self.app_menu.is_visible();

        let overlay_visible = switcher_visible

            || self.launcher.visible()

            || tray_menu_visible

            || app_menu_visible;



        if overlay_visible {

            prepare_acrylic_overlay(&ctx, Some(frame));

        } else {

            crate::platform::reset_popup_glass();

        }



        if let Some(switcher) = &mut self.marks_switcher {

            if switcher.is_visible() {

                switcher.update(ui, &mut self.icon_cache);

                ctx.request_repaint();

            }

        }



        if !switcher_visible && self.launcher.visible() {

            self.launcher

                .update(ui, &self.config, &self.marks, &mut self.icon_cache);

        }



        if tray_menu_visible {

            self.tray_menu

                .update(ui, &self.state, &mut self.icon_cache);

        }



        if app_menu_visible {

            self.app_menu.update(

                ui,

                &self.config,

                &self.state.lock().favorites,

                &self.state,

                &mut self.icon_cache,

            );

        }



        if self.settings.lock().visible() {

            let settings = self.settings.clone();

            let state = self.state.clone();

            let config = self.config.clone();

            ctx.show_viewport_deferred(

                egui::ViewportId::from_hash_of("winharpoon_settings"),

                egui::ViewportBuilder::default()

                    .with_inner_size([760.0, 720.0])

                    .with_title("WinHarpoon Settings")

                    .with_transparent(true)

                    .with_always_on_top(),

                move |ui, _class| {

                    settings.lock().update(ui, &state, &config);

                },

            );

        }



        let settings_visible = self.settings.lock().visible();

        if !self.launcher.visible()

            && !settings_visible

            && !switcher_visible

            && !tray_menu_visible

            && !app_menu_visible

        {

            ctx.request_repaint_after(Duration::from_millis(200));

        }

    }

}



impl UiApp {

    fn poll_commands(&mut self, ctx: &egui::Context) {

        if let Some(info) = take_pending_tray_menu() {

            self.open_tray_menu(ctx, info);

        }

        if let Some(anchor) = take_pending_app_menu() {

            self.open_app_menu(ctx, anchor);

        }

        while let Ok(command) = self.command_rx.try_recv() {

            match command {

                UiCommand::Launcher => self.toggle_launcher(ctx),

                UiCommand::Settings => self.open_settings_panel(ctx),

                UiCommand::TrayMenu => {

                    if let Some(info) = take_pending_tray_menu() {

                        self.open_tray_menu(ctx, info);

                    }

                }

                UiCommand::AppMenu => {

                    if let Some(anchor) = take_pending_app_menu() {

                        self.open_app_menu(ctx, anchor);

                    }

                }

            }

        }

    }



    fn toggle_launcher(&mut self, ctx: &egui::Context) {

        if self.launcher.visible() {

            self.launcher.dismiss(ctx, &self.marks);

            return;

        }

        self.hide_switcher(ctx);

        self.hide_other_panels(ctx, PanelId::Launcher);

        self.launcher.show(ctx, &self.config);

    }



    fn hide_other_panels(&mut self, ctx: &egui::Context, except: PanelId) {

        if !matches!(except, PanelId::Launcher) && self.launcher.visible() {

            self.launcher.hide(ctx);

        }

        if !matches!(except, PanelId::AppMenu) && self.app_menu.is_visible() {

            self.app_menu.hide(ctx);

        }

        if !matches!(except, PanelId::TrayMenu) && self.tray_menu.is_visible() {

            self.tray_menu.hide(ctx);

        }

        if !matches!(except, PanelId::Settings) && self.settings.lock().visible() {

            self.settings.lock().hide(ctx);

        }

    }



    fn open_app_menu(&mut self, ctx: &egui::Context, anchor: AppMenuAnchor) {

        self.hide_switcher(ctx);

        self.hide_other_panels(ctx, PanelId::AppMenu);

        self.app_menu.show(

            ctx,

            anchor,

            &self.config.lock(),

            &self.state.lock().favorites,

        );

    }



    fn open_tray_menu(&mut self, ctx: &egui::Context, info: TrayClickInfo) {

        self.hide_switcher(ctx);

        self.hide_other_panels(ctx, PanelId::TrayMenu);

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

        self.hide_other_panels(ctx, PanelId::Settings);

        self.settings.lock().show(&self.config.lock());

        ctx.request_repaint();

    }

}



#[derive(Debug, Clone, Copy, PartialEq, Eq)]

enum PanelId {

    Launcher,

    AppMenu,

    TrayMenu,

    Settings,

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

