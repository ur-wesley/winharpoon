use eframe::egui;

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(99, 140, 255);
pub const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(72, 108, 210);
pub const SURFACE: egui::Color32 = egui::Color32::from_rgb(26, 28, 36);
pub const SURFACE_RAISED: egui::Color32 = egui::Color32::from_rgb(34, 37, 48);
pub const SURFACE_HOVER: egui::Color32 = egui::Color32::from_rgb(42, 46, 60);
pub const BORDER: egui::Color32 = egui::Color32::from_rgb(52, 58, 74);
pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(148, 156, 178);
pub const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(108, 116, 138);
pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(82, 196, 138);
pub const WARNING: egui::Color32 = egui::Color32::from_rgb(240, 180, 72);
pub const DANGER: egui::Color32 = egui::Color32::from_rgb(240, 98, 108);

pub const OVERLAY_INNER_MARGIN: f32 = 20.0;
pub const OVERLAY_SHADOW_BLEED: f32 = 8.0;

pub const MARKS_CARD_WIDTH: f32 = 168.0;
pub const MARKS_CARD_GAP: f32 = 8.0;
pub const MARKS_HEADER_HEIGHT: f32 = 22.0;
pub const MARKS_CARD_HEIGHT: f32 = 72.0;

pub fn overlay_viewport_size(content: egui::Vec2) -> egui::Vec2 {
    let bleed = OVERLAY_SHADOW_BLEED * 2.0;
    content + egui::vec2(OVERLAY_INNER_MARGIN + bleed, OVERLAY_INNER_MARGIN + bleed)
}

pub fn marks_switcher_content_size(count: usize) -> egui::Vec2 {
    let count = count.max(1) as f32;
    let width = count * MARKS_CARD_WIDTH + (count - 1.0) * MARKS_CARD_GAP;
    let height = MARKS_HEADER_HEIGHT + 6.0 + MARKS_CARD_HEIGHT;
    egui::vec2(width, height)
}

pub fn overlay_viewport_builder(inner_size: [f32; 2]) -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_inner_size(inner_size)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_active(true)
        .with_taskbar(false)
}

pub fn options(viewport: egui::ViewportBuilder) -> eframe::NativeOptions {
    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;

        eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Glow,
            multisampling: 1,
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        }
    }

    #[cfg(not(windows))]
    {
        eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Glow,
            multisampling: 1,
            ..Default::default()
        }
    }
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.indent = 18.0;
    style.spacing.window_margin = egui::Margin::same(16);
    style.spacing.menu_margin = egui::Margin::same(8);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(20, 22, 28);
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = egui::Color32::from_rgb(14, 16, 22);
    visuals.faint_bg_color = SURFACE_RAISED;
    visuals.window_corner_radius = egui::CornerRadius::same(14);
    visuals.menu_corner_radius = egui::CornerRadius::same(10);
    visuals.widgets.noninteractive.bg_fill = SURFACE_RAISED;
    visuals.widgets.noninteractive.fg_stroke.color = TEXT_MUTED;
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.inactive.bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(210, 214, 226);
    visuals.widgets.inactive.weak_bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.open.bg_fill = SURFACE_HOVER;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.28);
    visuals.selection.stroke.color = ACCENT;
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = DANGER;
    visuals.window_stroke.color = BORDER;
    visuals.override_text_color = Some(egui::Color32::from_rgb(228, 232, 240));

    style.visuals = visuals;
    ctx.set_global_style(style);
}

pub fn apply_overlay_theme(ctx: &egui::Context) {
    apply_theme(ctx);
    let mut style = (*ctx.global_style()).clone();
    style.visuals.panel_fill = egui::Color32::TRANSPARENT;
    style.visuals.window_fill = egui::Color32::TRANSPARENT;
    style.visuals.extreme_bg_color = egui::Color32::TRANSPARENT;
    ctx.set_global_style(style);
}

pub fn section_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE_RAISED)
        .stroke(egui::Stroke::new(1.0, BORDER.gamma_multiply(0.55)))
        .corner_radius(12)
        .inner_margin(egui::Margin::same(14))
}

pub fn tray_menu_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(6, 8))
        .shadow(egui::epaint::Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: egui::Color32::from_black_alpha(110),
        })
}

pub fn tray_menu_divider(ui: &mut egui::Ui) {
    let rect = ui.allocate_exact_size(egui::vec2(ui.available_width(), 9.0), egui::Sense::hover()).1.rect;
    let line = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(rect.width() - 8.0, 1.0),
    );
    ui.painter().rect_filled(line, 0.0, BORDER.gamma_multiply(0.55));
}

pub fn tray_menu_section_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(text)
            .size(10.5)
            .strong()
            .color(TEXT_DIM),
    );
    ui.add_space(2.0);
}

pub fn overlay_panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(egui::Stroke::NONE)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
}

pub fn overlay_card_frame(selected: bool, active: bool) -> egui::Frame {
    let (fill, stroke) = if selected {
        (
            ACCENT.gamma_multiply(0.18),
            egui::Stroke::new(1.0, ACCENT.gamma_multiply(0.7)),
        )
    } else if active {
        (
            SUCCESS.gamma_multiply(0.18),
            egui::Stroke::new(1.0, SUCCESS.gamma_multiply(0.55)),
        )
    } else {
        (
            SURFACE_RAISED,
            egui::Stroke::new(1.0, BORDER.gamma_multiply(0.65)),
        )
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(stroke)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 5))
}

pub fn muted_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(12.5)
            .color(TEXT_MUTED),
    );
}

pub fn section_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(title)
                .size(18.0)
                .strong()
                .color(egui::Color32::WHITE),
        );
        ui.add_space(2.0);
        muted_label(ui, subtitle);
    });
}

pub fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let frame = egui::Frame::NONE
        .fill(color.gamma_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.55)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 3));
    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(text)
                .size(11.0)
                .strong()
                .color(color),
        );
    });
}

pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(
        egui::RichText::new(text).strong().color(egui::Color32::WHITE),
    )
    .fill(ACCENT)
    .corner_radius(8)
    .min_size(egui::vec2(96.0, 34.0));
    ui.add(button)
}

pub fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(text).color(TEXT_MUTED))
        .fill(SURFACE_HOVER)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(8)
        .min_size(egui::vec2(96.0, 34.0));
    ui.add(button)
}

pub fn small_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(
        egui::RichText::new(text)
            .size(11.5)
            .color(TEXT_DIM),
    )
    .fill(SURFACE_HOVER)
    .stroke(egui::Stroke::new(1.0, BORDER.gamma_multiply(0.7)))
    .corner_radius(6)
    .min_size(egui::vec2(0.0, 24.0));
    ui.add(button)
}

pub fn overlay_keyboard_hint_bar(ui: &mut egui::Ui, hints: &[(&str, &str)]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);
        for (key, action) in hints {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let key_frame = egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(48, 52, 66))
                    .stroke(egui::Stroke::new(1.0, BORDER.gamma_multiply(0.8)))
                    .corner_radius(3)
                    .inner_margin(egui::Margin::symmetric(3, 1));
                key_frame.show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(*key)
                            .size(8.0)
                            .strong()
                            .monospace(),
                    );
                });
                ui.label(
                    egui::RichText::new(*action)
                        .size(8.5)
                        .color(TEXT_DIM),
                );
            });
        }
    });
}
