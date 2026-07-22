//! Left-panel layout state — width, dirty flag, and reset counter so the panel remembers its size across workspace changes.

pub struct PanelLayout {
    pub left_panel_width: Option<f32>,
    pub left_panel_dirty: bool,
    pub left_panel_reset_count: u32,
}

impl PanelLayout {
    pub fn new() -> Self {
        Self {
            left_panel_width: None,
            left_panel_dirty: false,
            left_panel_reset_count: 0,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.left_panel_dirty = true;
    }

    pub fn reset_panel(&mut self) {
        self.left_panel_reset_count += 1;
        self.left_panel_dirty = false;
    }

    pub fn set_width(&mut self, width: Option<f32>) {
        self.left_panel_width = width;
    }
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let layout = PanelLayout::new();
        assert!(layout.left_panel_width.is_none());
        assert!(!layout.left_panel_dirty);
        assert_eq!(layout.left_panel_reset_count, 0);
    }

    #[test]
    fn test_mark_dirty() {
        let mut layout = PanelLayout::new();
        layout.mark_dirty();
        assert!(layout.left_panel_dirty);
    }

    #[test]
    fn test_reset_panel() {
        let mut layout = PanelLayout::new();
        layout.mark_dirty();
        layout.reset_panel();
        assert!(!layout.left_panel_dirty);
        assert_eq!(layout.left_panel_reset_count, 1);
    }

    #[test]
    fn test_set_width() {
        let mut layout = PanelLayout::new();
        layout.set_width(Some(200.0));
        assert_eq!(layout.left_panel_width, Some(200.0));
    }
}
