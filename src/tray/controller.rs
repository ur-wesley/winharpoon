use crate::launcher::TrayClickInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Settings,
    ConfigFolder,
    Reload,
    RescanApps,
    Quit,
    JumpSlot(u8),
}

pub struct TrayMenuController {
    pub visible: bool,
    pub anchor: TrayClickInfo,
    pub panel_width: f32,
}

impl Default for TrayMenuController {
    fn default() -> Self {
        Self {
            visible: false,
            anchor: TrayClickInfo::default(),
            panel_width: 220.0,
        }
    }
}

impl TrayMenuController {
    pub fn on_show(&mut self, anchor: TrayClickInfo) {
        self.anchor = anchor;
        self.visible = true;
    }

    pub fn on_hide(&mut self) {
        self.visible = false;
    }

    pub fn menu_height(&self) -> f32 {
        const HEADER_HEIGHT: f32 = 28.0;
        const DIVIDER_HEIGHT: f32 = 5.0;
        const ROW_HEIGHT: f32 = 26.0;
        const SLOT_ROW_HEIGHT: f32 = 22.0;
        const SECTION_LABEL_HEIGHT: f32 = 14.0;
        const FRAME_PADDING_V: f32 = 12.0;
        const SLOT_ROWS: f32 = 9.0;
        const DIVIDER_COUNT: f32 = 5.0;
        const ROW_COUNT: f32 = 5.0;

        FRAME_PADDING_V
            + HEADER_HEIGHT
            + DIVIDER_COUNT * DIVIDER_HEIGHT
            + ROW_HEIGHT
            + SECTION_LABEL_HEIGHT
            + SLOT_ROWS * SLOT_ROW_HEIGHT
            + ROW_COUNT * ROW_HEIGHT
    }

    pub fn menu_screen_rect(
        &self,
        ctx: &eframe::egui::Context,
        content: eframe::egui::Vec2,
    ) -> eframe::egui::Rect {
        use eframe::egui;

        let ppp = ctx
            .input(|i| i.viewport().native_pixels_per_point)
            .unwrap_or(1.0);

        let tray_x = self.anchor.rect_x as f32 / ppp;
        let tray_y = self.anchor.rect_y as f32 / ppp;
        let tray_w = self.anchor.rect_w as f32 / ppp;
        let tray_h = self.anchor.rect_h as f32 / ppp;

        let work_area =
            crate::platform::monitor_work_area_at_physical_point(ctx, self.anchor.click_x, self.anchor.click_y);

        let tray_screen_y = tray_y;
        let taskbar_at_bottom = tray_screen_y > work_area.center().y;

        let mut menu_x = tray_x + tray_w - content.x;
        let mut menu_y = if taskbar_at_bottom {
            tray_y - content.y - 10.0
        } else {
            tray_y + tray_h + 10.0
        };

        menu_x = menu_x.clamp(
            work_area.min.x + 8.0,
            work_area.max.x - content.x - 8.0,
        );
        menu_y = menu_y.clamp(
            work_area.min.y + 8.0,
            work_area.max.y - content.y - 8.0,
        );

        egui::Rect::from_min_size(egui::pos2(menu_x, menu_y), content)
    }
}
