#[derive(Debug, Clone, Copy, Default)]
pub struct ListSelection {
    pub selected: usize,
    pub hovered: Option<usize>,
    pub scroll_to_selected: bool,
}

impl ListSelection {
    pub fn active_row(&self) -> usize {
        self.hovered.unwrap_or(self.selected)
    }

    pub fn navigate(&mut self, len: usize, delta: i32) {
        if len == 0 {
            return;
        }
        self.selected = if delta > 0 {
            (self.selected + 1) % len
        } else if self.selected == 0 {
            len - 1
        } else {
            self.selected - 1
        };
        self.hovered = None;
        self.scroll_to_selected = true;
    }

    pub fn reset_on_query_change(&mut self) {
        self.selected = 0;
        self.scroll_to_selected = true;
        self.hovered = None;
    }

    pub fn take_scroll(&mut self) -> bool {
        if self.scroll_to_selected {
            self.scroll_to_selected = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigate_wraps_forward() {
        let mut sel = ListSelection {
            selected: 2,
            ..Default::default()
        };
        sel.navigate(3, 1);
        assert_eq!(sel.selected, 0);
    }

    #[test]
    fn navigate_wraps_backward() {
        let mut sel = ListSelection::default();
        sel.navigate(3, -1);
        assert_eq!(sel.selected, 2);
    }

    #[test]
    fn active_row_prefers_hovered() {
        let sel = ListSelection {
            selected: 1,
            hovered: Some(3),
            scroll_to_selected: false,
        };
        assert_eq!(sel.active_row(), 3);
    }

    #[test]
    fn reset_on_query_change_clears_hovered() {
        let mut sel = ListSelection {
            selected: 4,
            hovered: Some(2),
            scroll_to_selected: false,
        };
        sel.reset_on_query_change();
        assert_eq!(sel.selected, 0);
        assert!(sel.scroll_to_selected);
        assert_eq!(sel.hovered, None);
    }
}
