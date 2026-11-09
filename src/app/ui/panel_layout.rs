//! Left-panel layout state — width and dirty flag so the panel remembers its size across workspace changes.
//!
//! Unit tests live in the sibling `panel_layout_tests.rs` sidecar.

/// Maximum fraction of the viewport width the left (workspace) panel may
/// occupy (RUST-021 — the panel-width magic numbers live here, not inline).
pub const LEFT_PANEL_MAX_WIDTH_FRACTION: f32 = 0.2;

/// Default width in logical pixels of the left panel when no width has been
/// remembered yet.
pub const LEFT_PANEL_DEFAULT_WIDTH: f32 = 280.0;

/// Minimum width in logical pixels of the left panel.
pub const LEFT_PANEL_MIN_WIDTH: f32 = 180.0;

/// Maximum fraction of the viewport width the right (table-of-contents) panel
/// may occupy.
pub const RIGHT_PANEL_MAX_WIDTH_FRACTION: f32 = 0.3;

/// Default width in logical pixels of the right panel when no width has been
/// remembered yet.
pub const RIGHT_PANEL_DEFAULT_WIDTH: f32 = 200.0;

/// Minimum width in logical pixels of the right panel.
pub const RIGHT_PANEL_MIN_WIDTH: f32 = 150.0;

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
#[path = "panel_layout_tests.rs"]
mod tests;
