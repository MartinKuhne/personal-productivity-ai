//! Window geometry core types, UI scale quantization, and layout solvers (`UIF-WIN-001`).
//!
//! Provides the canonical window dimensions schema, `UiScale` stepping math,
//! a pure window layout solver guaranteeing the 240px central workspace floor,
//! and the `WindowStorage` persistence seam.
//!
//! Unit tests live in the sibling `window_geometry_tests.rs` sidecar.

use std::sync::RwLock;

/// Minimum main window logical dimensions (640 x 480 px) per §2.2 / §4.1.2 (`UIF-WIN-001`).
pub const WINDOW_MIN_WIDTH: f32 = 640.0;
/// Minimum main window logical height (480px) per §2.2 / §4.1.2 (`UIF-WIN-001`).
pub const WINDOW_MIN_HEIGHT: f32 = 480.0;

/// Default main window logical width (1000px) per §2.2 / §4.1.2 (`UIF-WIN-001`).
pub const WINDOW_DEFAULT_WIDTH: f32 = 1000.0;
/// Default main window logical height (700px) per §2.2 / §4.1.2 (`UIF-WIN-001`).
pub const WINDOW_DEFAULT_HEIGHT: f32 = 700.0;

/// Central workspace minimum width floor (240px) per §2.2 / §4.1.2 (`UIF-WIN-001`).
pub const CENTER_PANE_MIN_W: f32 = 240.0;

/// UI Scale multiplier (`0.75x` to `2.00x` in `0.10x` steps) (`UIF-WIN-001`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScale(f32);

impl UiScale {
    /// Minimum allowed UI scale factor (`0.75x`).
    pub const MIN: f32 = 0.75;
    /// Maximum allowed UI scale factor (`2.00x`).
    pub const MAX: f32 = 2.00;
    /// Quantization step size (`0.10x`).
    pub const STEP: f32 = 0.10;
    /// Default UI scale factor (`1.00x`).
    pub const DEFAULT: Self = Self(1.00);

    /// Create a clamped and step-quantized UiScale (`UIF-WIN-001`).
    ///
    /// Clamps within `[MIN, MAX]` (0.75..=2.00) and quantizes to the nearest 0.10 step,
    /// preserving the 0.75 floor and 1.00 default. Non-finite values resolve to `DEFAULT`.
    #[must_use]
    pub fn new(val: f32) -> Self {
        if !val.is_finite() {
            return Self::DEFAULT;
        }
        if val <= Self::MIN + 1e-4 {
            return Self(Self::MIN);
        }
        if val >= Self::MAX - 1e-4 {
            return Self(Self::MAX);
        }
        let rounded = ((val * 10.0).round() / 10.0).clamp(0.80, Self::MAX);
        Self(rounded)
    }

    /// Step up the scale factor by one step, clamped at `MAX` (`UIF-WIN-001`).
    #[must_use]
    pub fn step_up(self) -> Self {
        if self.0 < 0.80 - 1e-4 {
            Self(0.80)
        } else {
            let next = ((self.0 * 10.0).round() + 1.0) / 10.0;
            Self(next.min(Self::MAX))
        }
    }

    /// Step down the scale factor by one step, clamped at `MIN` (`UIF-WIN-001`).
    #[must_use]
    pub fn step_down(self) -> Self {
        if self.0 <= 0.80 + 1e-4 {
            Self(Self::MIN)
        } else {
            let prev = ((self.0 * 10.0).round() - 1.0) / 10.0;
            Self(prev.max(Self::MIN))
        }
    }

    /// Retrieve the resolved scale multiplier as an `f32` factor (`UIF-WIN-001`).
    #[must_use]
    pub fn factor(self) -> f32 {
        self.0
    }

    /// Compose with OS DPI scale factor at the render boundary (`UIF-WIN-001`).
    #[must_use]
    pub fn compose_with_dpi(self, os_dpi_scale: f32) -> f32 {
        let dpi = if os_dpi_scale.is_finite() && os_dpi_scale > 0.0 {
            os_dpi_scale
        } else {
            1.0
        };
        self.0 * dpi
    }
}

impl Default for UiScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Window dimensions, screen position, and split pane ratios (`UIF-WIN-001`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowGeometry {
    /// Optional logical X screen coordinate.
    pub x: Option<f32>,
    /// Optional logical Y screen coordinate.
    pub y: Option<f32>,
    /// Window logical width in pixels.
    pub width: f32,
    /// Window logical height in pixels.
    pub height: f32,
    /// Split ratio for the left pane (typically `0.0..=1.0`).
    pub left_split_ratio: f32,
    /// Split ratio for the right pane (typically `0.0..=1.0`).
    pub right_split_ratio: f32,
    /// User UI scale multiplier.
    pub ui_scale: UiScale,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: WINDOW_DEFAULT_WIDTH,
            height: WINDOW_DEFAULT_HEIGHT,
            left_split_ratio: 0.20,
            right_split_ratio: 0.20,
            ui_scale: UiScale::DEFAULT,
        }
    }
}

/// Solved layout partitioning widths ensuring center pane floor (`UIF-WIN-001`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedWindowLayout {
    /// Clamped logical window width.
    pub window_width: f32,
    /// Clamped logical window height.
    pub window_height: f32,
    /// Left pane resolved width.
    pub left_pane_width: f32,
    /// Central workspace resolved width (guaranteed >= [`CENTER_PANE_MIN_W`]).
    pub center_pane_width: f32,
    /// Right pane resolved width (0.0 when unmounted).
    pub right_pane_width: f32,
}

/// Pure solver clamping window minimums and preserving center pane floor >= 240px (`UIF-WIN-001`).
///
/// Clamps window dimensions to at least `WINDOW_MIN_WIDTH = 640px` and `WINDOW_MIN_HEIGHT = 480px`.
/// Left pane width is solved within `[LEFT_PANE_MIN_W, max(LEFT_PANE_MIN_W, 0.20 * window_width)]`.
/// Right pane width (when mounted) is solved within `[RIGHT_PANE_MIN_W, max(RIGHT_PANE_MIN_W, 0.25 * window_width)]`.
/// If the right pane is unmounted, its width is `0.0` and its allocation returns to the center workspace.
/// The center pane absorbs the remaining width and is strictly preserved to be >= `CENTER_PANE_MIN_W = 240px`.
#[must_use]
pub fn solve_window_layout(
    requested_w: f32,
    requested_h: f32,
    left_ratio: f32,
    right_ratio: f32,
    right_pane_mounted: bool,
) -> SolvedWindowLayout {
    use crate::geometry::pane_solver::{LEFT_PANE_MIN_W, RIGHT_PANE_MIN_W};

    let w = if requested_w.is_finite() && requested_w >= WINDOW_MIN_WIDTH {
        requested_w
    } else {
        WINDOW_MIN_WIDTH
    };
    let h = if requested_h.is_finite() && requested_h >= WINDOW_MIN_HEIGHT {
        requested_h
    } else {
        WINDOW_MIN_HEIGHT
    };

    // Left pane requested width from ratio (defaulting to 0.20 if non-finite or <= 0.0)
    let safe_left_ratio = if left_ratio.is_finite() && left_ratio > 0.0 {
        left_ratio
    } else {
        0.20
    };
    let left_max = (0.20 * w).max(LEFT_PANE_MIN_W);
    let mut left_w = (safe_left_ratio * w).clamp(LEFT_PANE_MIN_W, left_max);

    // Right pane requested width from ratio
    let mut right_w = if right_pane_mounted {
        let safe_right_ratio = if right_ratio.is_finite() && right_ratio > 0.0 {
            right_ratio
        } else {
            0.20
        };
        let right_max = (0.25 * w).max(RIGHT_PANE_MIN_W);
        (safe_right_ratio * w).clamp(RIGHT_PANE_MIN_W, right_max)
    } else {
        0.0
    };

    // Ensure center pane width does not collapse below CENTER_PANE_MIN_W
    let remaining = w - left_w - right_w;
    if remaining < CENTER_PANE_MIN_W {
        let deficit = CENTER_PANE_MIN_W - remaining;
        // First squeeze right pane down to its minimum (or 0)
        let min_right = if right_pane_mounted {
            RIGHT_PANE_MIN_W
        } else {
            0.0
        };
        let right_reduction = (right_w - min_right).clamp(0.0, deficit);
        right_w -= right_reduction;

        // Any remaining deficit squeezes left pane down to LEFT_PANE_MIN_W
        let deficit_left = deficit - right_reduction;
        if deficit_left > 0.0 {
            let left_reduction = (left_w - LEFT_PANE_MIN_W).clamp(0.0, deficit_left);
            left_w -= left_reduction;
        }
    }

    let center_w = w - left_w - right_w;

    SolvedWindowLayout {
        window_width: w,
        window_height: h,
        left_pane_width: left_w,
        center_pane_width: center_w,
        right_pane_width: right_w,
    }
}

/// Injected storage seam for window geometry persistence (`[RUST-024]`, `UIF-WIN-001`).
pub trait WindowStorage: Send + Sync {
    /// Load the persisted window geometry, if available.
    fn load_geometry(&self) -> Option<WindowGeometry>;
    /// Persist the window geometry.
    fn save_geometry(&self, geometry: &WindowGeometry);
}

/// No-op window storage sink that ignores saves and returns `None` on load (`[RUST-006]`, `UIF-WIN-001`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoopWindowStorage;

impl WindowStorage for NoopWindowStorage {
    fn load_geometry(&self) -> Option<WindowGeometry> {
        None
    }

    fn save_geometry(&self, _geometry: &WindowGeometry) {}
}

/// In-memory window storage sink for unit testing and ephemeral sessions (`[RUST-006]`, `UIF-WIN-001`).
#[derive(Debug, Default)]
pub struct MemoryWindowStorage {
    slot: RwLock<Option<WindowGeometry>>,
}

impl MemoryWindowStorage {
    /// Create a new empty in-memory window storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slot: RwLock::new(None),
        }
    }

    /// Create an in-memory window storage pre-populated with `geometry`.
    #[must_use]
    pub fn with_geometry(geometry: WindowGeometry) -> Self {
        Self {
            slot: RwLock::new(Some(geometry)),
        }
    }
}

impl WindowStorage for MemoryWindowStorage {
    fn load_geometry(&self) -> Option<WindowGeometry> {
        *self.slot.read().unwrap_or_else(|e| e.into_inner())
    }

    fn save_geometry(&self, geometry: &WindowGeometry) {
        let mut slot = self.slot.write().unwrap_or_else(|e| e.into_inner());
        *slot = Some(*geometry);
    }
}

#[cfg(test)]
#[path = "window_geometry_tests.rs"]
mod window_geometry_tests;
