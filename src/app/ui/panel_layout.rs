//! Left-panel layout state — width and dirty flag so the panel remembers its size across workspace changes.

#[derive(Clone, Debug, PartialEq)]
pub struct PanelLayout {
    pub left_panel_width: Option<f32>,
    pub right_panel_width: Option<f32>,
    pub left_panel_dirty: bool,
}

impl PanelLayout {
    pub fn new() -> Self {
        Self {
            left_panel_width: None,
            right_panel_width: None,
            left_panel_dirty: false,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.left_panel_dirty = true;
    }

    pub fn set_width(&mut self, width: Option<f32>) {
        self.left_panel_width = width;
    }

    pub fn set_right_width(&mut self, width: Option<f32>) {
        self.right_panel_width = width;
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
        assert!(layout.right_panel_width.is_none());
        assert!(!layout.left_panel_dirty);
    }

    #[test]
    fn test_mark_dirty() {
        let mut layout = PanelLayout::new();
        layout.mark_dirty();
        assert!(layout.left_panel_dirty);
    }

    #[test]
    fn test_set_width() {
        let mut layout = PanelLayout::new();
        layout.set_width(Some(200.0));
        assert_eq!(layout.left_panel_width, Some(200.0));
    }

    #[test]
    fn test_set_right_width() {
        let mut layout = PanelLayout::new();
        layout.set_right_width(Some(300.0));
        assert_eq!(layout.right_panel_width, Some(300.0));
    }

    #[test]
    fn test_set_width_none_resets() {
        let mut layout = PanelLayout::new();
        layout.set_width(Some(200.0));
        layout.set_width(None);
        assert_eq!(layout.left_panel_width, None);
    }

    #[test]
    fn test_set_right_width_none_resets() {
        let mut layout = PanelLayout::new();
        layout.set_right_width(Some(300.0));
        layout.set_right_width(None);
        assert_eq!(layout.right_panel_width, None);
    }

    #[test]
    fn test_set_width_negative_is_stored_verbatim() {
        // Negative widths are stored as-is (the caller is responsible
        // for sanitising); this pins that no clamping is applied here.
        let mut layout = PanelLayout::new();
        layout.set_width(Some(-10.0));
        layout.set_right_width(Some(-5.0));
        assert_eq!(layout.left_panel_width, Some(-10.0));
        assert_eq!(layout.right_panel_width, Some(-5.0));
    }

    #[test]
    fn test_mark_dirty_does_not_clear_on_set_width() {
        let mut layout = PanelLayout::new();
        layout.mark_dirty();
        layout.set_width(Some(150.0));
        assert!(layout.left_panel_dirty);
    }
}
