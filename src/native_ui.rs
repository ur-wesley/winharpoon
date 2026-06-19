use eframe::egui;

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(99, 140, 255);
pub const GLASS_PANEL: egui::Color32 = egui::Color32::from_rgba_premultiplied(20, 22, 29, 70);
pub const GLASS_RAISED: egui::Color32 = egui::Color32::from_rgba_premultiplied(15, 15, 15, 15);
pub const GLASS_HOVER: egui::Color32 = egui::Color32::from_rgba_premultiplied(26, 26, 26, 26);
pub const GLASS_BORDER: egui::Color32 = egui::Color32::from_rgba_premultiplied(24, 24, 24, 24);
pub const GLASS_INSET: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 0, 0, 64);
pub const POPUP_PANEL: egui::Color32 = egui::Color32::from_rgba_premultiplied(20, 22, 29, 80);
pub const POPUP_SURFACE: egui::Color32 = egui::Color32::from_rgba_premultiplied(4, 4, 6, 56);
pub const POPUP_CARD: egui::Color32 = egui::Color32::from_rgba_premultiplied(4, 4, 4, 44);
pub const POPUP_CARD_HOVER: egui::Color32 = egui::Color32::from_rgba_premultiplied(10, 10, 14, 80);
pub const POPUP_INSET: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 0, 0, 40);
pub const GLASS_KEYCAP: egui::Color32 = egui::Color32::from_rgba_premultiplied(16, 16, 16, 16);
pub const GLASS_KEYCAP_BORDER: egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 30, 30, 30);
pub const BORDER: egui::Color32 = GLASS_BORDER;
pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(148, 156, 178);
pub const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(108, 116, 138);
pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(82, 196, 138);
pub const WARNING: egui::Color32 = egui::Color32::from_rgb(240, 180, 72);
pub const DANGER: egui::Color32 = egui::Color32::from_rgb(240, 98, 108);

pub const OVERLAY_INNER_MARGIN: f32 = 0.0;
#[cfg(windows)]
pub const OVERLAY_SHADOW_BLEED: f32 = 0.0;
#[cfg(not(windows))]
pub const OVERLAY_SHADOW_BLEED: f32 = 8.0;

pub const MARKS_CARD_WIDTH: f32 = 108.0;
pub const MARKS_CARD_GAP: f32 = 6.0;
pub const MARKS_CARD_INNER_WIDTH: f32 = MARKS_CARD_WIDTH - 12.0;
pub const MARKS_HEADER_HEIGHT: f32 = 18.0;
pub const MARKS_CARD_HEIGHT: f32 = 56.0;

const PANEL_CORNER_RADIUS: u8 = 14;
const CARD_CORNER_RADIUS: u8 = 8;

pub fn overlay_viewport_size(content: egui::Vec2) -> egui::Vec2 {
    let bleed = OVERLAY_SHADOW_BLEED * 2.0;
    content + egui::vec2(OVERLAY_INNER_MARGIN + bleed, OVERLAY_INNER_MARGIN + bleed)
}

pub fn marks_switcher_content_size(count: usize) -> egui::Vec2 {
    const PANEL_PADDING: f32 = 20.0;
    let cards_width = if count == 0 {
        0.0
    } else {
        let count = count as f32;
        count * MARKS_CARD_WIDTH + (count - 1.0) * MARKS_CARD_GAP
    };
    let width = cards_width.max(220.0) + PANEL_PADDING;
    let height = if count == 0 {
        MARKS_HEADER_HEIGHT + PANEL_PADDING
    } else {
        MARKS_HEADER_HEIGHT + 6.0 + MARKS_CARD_HEIGHT + PANEL_PADDING
    };
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
    visuals.panel_fill = egui::Color32::from_rgba_unmultiplied(20, 22, 28, 200);
    visuals.window_fill = GLASS_PANEL;
    visuals.extreme_bg_color = egui::Color32::from_rgba_unmultiplied(14, 16, 22, 220);
    visuals.faint_bg_color = GLASS_RAISED;
    visuals.window_corner_radius = egui::CornerRadius::same(16);
    visuals.menu_corner_radius = egui::CornerRadius::same(12);
    visuals.widgets.noninteractive.bg_fill = GLASS_RAISED;
    visuals.widgets.noninteractive.fg_stroke.color = TEXT_MUTED;
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(10);
    visuals.widgets.inactive.bg_fill = GLASS_RAISED;
    visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(210, 214, 226);
    visuals.widgets.inactive.weak_bg_fill = GLASS_PANEL;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, GLASS_BORDER);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(10);
    visuals.widgets.hovered.bg_fill = GLASS_HOVER;
    visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(10);
    visuals.widgets.active.bg_fill = ACCENT.gamma_multiply(0.55);
    visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(10);
    visuals.widgets.open.bg_fill = GLASS_HOVER;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.22);
    visuals.selection.stroke.color = ACCENT;
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = DANGER;
    visuals.window_stroke.color = GLASS_BORDER;
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
    style.visuals.faint_bg_color = egui::Color32::TRANSPARENT;
    style.visuals.widgets.noninteractive.bg_fill = POPUP_CARD;
    style.visuals.widgets.inactive.bg_fill = POPUP_CARD;
    style.visuals.widgets.inactive.weak_bg_fill = POPUP_SURFACE;
    style.visuals.widgets.hovered.bg_fill = POPUP_CARD_HOVER;
    style.visuals.widgets.open.bg_fill = POPUP_CARD_HOVER;
    ctx.set_global_style(style);
}

pub fn section_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(GLASS_RAISED)
        .stroke(egui::Stroke::new(1.0, GLASS_BORDER))
        .corner_radius(14)
        .inner_margin(egui::Margin::same(14))
}

pub fn tray_menu_divider(ui: &mut egui::Ui) {
    let rect = ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::hover()).1.rect;
    let line = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(rect.width() - 6.0, 1.0),
    );
    ui.painter().rect_filled(line, 0.0, GLASS_BORDER.gamma_multiply(0.55));
}

pub fn tray_menu_section_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(1.0);
    ui.label(
        egui::RichText::new(text)
            .size(9.5)
            .strong()
            .color(TEXT_DIM),
    );
    ui.add_space(1.0);
}

pub fn tray_menu_clipped_label(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    text: &str,
    size: f32,
    color: egui::Color32,
) {
    let mut layout_job = egui::text::LayoutJob::simple(
        text.to_owned(),
        egui::FontId::proportional(size),
        color,
        rect.width(),
    );
    layout_job.wrap.max_rows = 1;
    layout_job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(layout_job));
    let pos = egui::pos2(
        rect.min.x,
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter()
        .with_clip_rect(rect)
        .galley(pos, galley, color);
}

pub fn overlay_panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(POPUP_PANEL)
        .stroke(egui::Stroke::new(1.0, GLASS_BORDER))
        .corner_radius(PANEL_CORNER_RADIUS)
        .inner_margin(egui::Margin::same(10))
}

pub fn show_popup_panel<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    overlay_panel_frame().show(ui, add_contents)
}

pub fn render_overlay_shell<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> (R, egui::Rect) {
    let mut result = None;
    let mut panel_rect = egui::Rect::NOTHING;
    let margin = ((OVERLAY_INNER_MARGIN + OVERLAY_SHADOW_BLEED * 2.0) / 2.0) as i8;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin::same(margin)),
        )
        .show_inside(ui, |ui| {
            let inner = show_popup_panel(ui, add_contents);
            panel_rect = inner.response.rect;
            result = Some(inner.inner);
        });
    (result.expect("overlay shell"), panel_rect)
}

pub fn render_popup_layout<R>(
    ui: &mut egui::Ui,
    footer_id: &'static str,
    hints: &[(&str, &str)],
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    popup_hint_footer(footer_id).show_inside(ui, |ui| {
        overlay_keyboard_hint_bar(ui, hints);
    });
    render_overlay_shell(ui, add_contents).0
}

pub fn popup_hint_footer(id: &'static str) -> egui::Panel {
    egui::Panel::bottom(id)
        .frame(
            egui::Frame::NONE
                .fill(POPUP_PANEL)
                .stroke(egui::Stroke::new(1.0, GLASS_BORDER))
                .inner_margin(egui::Margin::symmetric(10, 8)),
        )
}

pub fn overlay_card_frame(selected: bool, active: bool) -> egui::Frame {
    let (fill, stroke) = if selected {
        (
            ACCENT.gamma_multiply(0.16),
            egui::Stroke::new(1.0, ACCENT.gamma_multiply(0.55)),
        )
    } else if active {
        (
            SUCCESS.gamma_multiply(0.16),
            egui::Stroke::new(1.0, SUCCESS.gamma_multiply(0.45)),
        )
    } else {
        (
            POPUP_CARD,
            egui::Stroke::new(1.0, GLASS_BORDER),
        )
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CARD_CORNER_RADIUS)
        .inner_margin(egui::Margin::symmetric(6, 4))
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
        .fill(color.gamma_multiply(0.14))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.45)))
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
    .fill(ACCENT.gamma_multiply(0.85))
    .stroke(egui::Stroke::new(1.0, ACCENT.gamma_multiply(0.6)))
    .corner_radius(10)
    .min_size(egui::vec2(96.0, 34.0));
    ui.add(button)
}

pub fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(text).color(TEXT_MUTED))
        .fill(GLASS_HOVER)
        .stroke(egui::Stroke::new(1.0, GLASS_BORDER))
        .corner_radius(10)
        .min_size(egui::vec2(96.0, 34.0));
    ui.add(button)
}

pub fn small_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(
        egui::RichText::new(text)
            .size(11.5)
            .color(TEXT_DIM),
    )
    .fill(GLASS_HOVER)
    .stroke(egui::Stroke::new(1.0, GLASS_BORDER))
    .corner_radius(8)
    .min_size(egui::vec2(0.0, 24.0));
    ui.add(button)
}

pub fn icon_slot(ui: &mut egui::Ui, size: egui::Vec2, content: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), content);
}

const SEARCH_BAR_ICON_SIZE: f32 = 14.0;
const SEARCH_BAR_ROW_HEIGHT: f32 = 20.0;

fn search_bar_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(POPUP_INSET)
        .stroke(egui::Stroke::new(1.0, GLASS_BORDER))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(8, 6))
}

pub fn overlay_search_bar(ui: &mut egui::Ui, query: &mut String, hint: &str) -> egui::Response {
    let mut text_response = None;
    search_bar_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            icon_slot(
                ui,
                egui::vec2(SEARCH_BAR_ICON_SIZE, SEARCH_BAR_ROW_HEIGHT),
                |ui| {
                    crate::icons::ui_icons::search_icon(ui, SEARCH_BAR_ICON_SIZE, TEXT_DIM);
                },
            );
            ui.add_space(2.0);
            text_response = Some(ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY)
                    .frame(egui::Frame::NONE)
                    .font(egui::FontId::proportional(13.0)),
            ));
        });
    });
    text_response.expect("search bar text field")
}

pub fn list_icon(ui: &mut egui::Ui, texture: &egui::TextureHandle, size: f32) {
    ui.add(
        egui::Image::new(texture)
            .fit_to_exact_size(egui::vec2(size, size))
            .corner_radius(4),
    );
}

pub fn overlay_keyboard_hint_bar(ui: &mut egui::Ui, hints: &[(&str, &str)]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);
        for (key, action) in hints {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let key_frame = egui::Frame::NONE
                    .fill(GLASS_KEYCAP)
                    .stroke(egui::Stroke::new(1.0, GLASS_KEYCAP_BORDER))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(3, 1));
                key_frame.show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(*key)
                            .size(8.0)
                            .strong()
                            .monospace()
                            .color(egui::Color32::from_rgb(240, 244, 255)),
                    );
                });
                ui.label(
                    egui::RichText::new(*action)
                        .size(8.5)
                        .color(TEXT_MUTED),
                );
            });
        }
    });
}
