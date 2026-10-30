//! Color selection widget with `ColorPicker` surface and `ColorSwatchButton` trigger (§3.12, `UIF-ATOM-012`).
//!
//! A color picker pairs a 28px or 32px trigger button featuring a 16x16px preview well over a
//! 4x4px neutral checkerboard (`surface.base` / `surface.active`) enclosed by a mandatory 1px
//! `border.strong` inner frame with an anchored 240px card flyout (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
//! Features include:
//! - Comprehensive color data models ([`Rgba8`] and [`Hsva`]) with bidirectional conversions, hex string parsing and formatting.
//! - Declarative [`ColorPickerModel`] with snapshot/rollback mechanics on Escape or Cancel (`UIF-PRIN-005`).
//! - Standalone and integrated [`ColorSwatchButton`] trigger displaying hex value in monospace font.
//! - 2D Saturation/Value (`SV`) plane canvas (180x140px) with dual-ring contrast crosshair thumb (`UIF-TOK-006`).
//! - 4px Hue slider track (0..360° rainbow gradient) and optional 4px Alpha slider track.
//! - Hex text input with live validation against `#RRGGBB` and `#RRGGBBAA` formats.
//! - Preset swatches grid of 24x24px swatches over 4x4px checkerboard with 1px `border.strong` inner frame.
//! - Live WCAG 2.1 contrast ratio readout semantic chip against an optional background color reference (`UIF-TOK-006`).
//! - Three-tier interaction callbacks ([`Live`], [`Commit`], [`Cancel`]).
//!
//! Unit tests live in the sibling `color_picker_tests.rs` sidecar.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text as text_core;
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::{Tree, Widget, tree};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::event::Event;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::mouse;
use iced::{Background, Border, Color, Element, Length, Pixels, Point, Rectangle, Shadow, Size};

use crate::binding::snapshot::GestureSlot;
use crate::binding::{Cancel, Commit, Live};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use crate::token::{shape, size, space, typography};
use crate::widget::focus::{self, FocusPlacement};
use crate::widget::focusable::FocusableState;

// ---------------------------------------------------------------------------
// String constants (§3.12, [RUST-021])
// ---------------------------------------------------------------------------

/// Hex prefix symbol (§3.12, `UIF-ATOM-012`).
pub const HEX_PREFIX: &str = "#";

/// Monospace font family string (§3.12, `UIF-ATOM-012`).
pub const FONT_FAMILY_MONO: &str = "monospace";

/// Apply action button label (§3.12, `UIF-ATOM-012`).
pub const LABEL_APPLY: &str = "✓ Apply";

/// Cancel action button label (§3.12, `UIF-ATOM-012`).
pub const LABEL_CANCEL: &str = "Cancel";

/// Hex input placeholder label (§3.12, `UIF-ATOM-012`).
pub const LABEL_HEX: &str = "Hex";

/// Preset swatches header label (§3.12, `UIF-ATOM-012`).
pub const LABEL_PRESETS: &str = "Presets";

/// WCAG AA compliance mark (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
pub const BADGE_AA: &str = "AA";

/// WCAG AAA compliance mark (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
pub const BADGE_AAA: &str = "AAA";

/// WCAG contrast failure mark (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
pub const BADGE_FAIL: &str = "Fail";

/// Pass checkmark icon (§3.12, `UIF-ATOM-012`).
pub const ICON_PASS: &str = "✓";

/// Warning icon (§3.12, `UIF-ATOM-012`).
pub const ICON_WARN: &str = "⚠";

/// Dropdown chevron down glyph (§3.12, `UIF-ATOM-012`).
pub const CHEVRON_DOWN: &str = "▾";

/// Dropdown chevron up glyph (§3.12, `UIF-ATOM-012`).
pub const CHEVRON_UP: &str = "▴";

/// Error message string for invalid hex syntax (§3.12, `UIF-ATOM-012`).
pub const ERR_INVALID_HEX: &str = "Invalid hex color";

// ---------------------------------------------------------------------------
// Layout & geometry constants (§3.12, UIF-ATOM-012, UIF-TOK-002, UIF-PRIN-001 §1.1)
// ---------------------------------------------------------------------------

/// Color picker flyout fixed card width — 240px (§3.12, `UIF-ATOM-012`, `UIF-PRIN-001` §1.1).
pub const COLOR_FLYOUT_WIDTH: f32 = 240.0;

/// Saturation/Value plane well width — 180px (§3.12, `UIF-ATOM-012`, `UIF-PRIN-001` §1.1).
pub const COLOR_WELL_WIDTH: f32 = 180.0;

/// Saturation/Value plane well height — 140px (§3.12, `UIF-ATOM-012`, `UIF-PRIN-001` §1.1).
pub const COLOR_WELL_HEIGHT: f32 = 140.0;

/// Standard trigger control height (compact) — `size::CONTROL_SM` (28px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-002`).
pub const TRIGGER_HEIGHT_SM: f32 = size::CONTROL_SM;

/// Standard trigger control height (medium) — `size::CONTROL_MD` (32px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-002`).
pub const TRIGGER_HEIGHT_MD: f32 = size::CONTROL_MD;

/// Preview well hit and render size — `size::SLIDER_THUMB` (16px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-002`).
pub const PREVIEW_WELL_SIZE: f32 = size::SLIDER_THUMB;

/// Neutral checkerboard square cell size — `space::XS` (4px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
pub const CHECKERBOARD_CELL_SIZE: f32 = space::XS;

/// Preset swatch button hit size — `size::HIT_TARGET_MIN` (24px) (§3.12, `UIF-ATOM-012`, `UIF-ATOM-002b`).
pub const SWATCH_SIZE: f32 = size::HIT_TARGET_MIN;

/// Slider rail track height — `size::SLIDER_TRACK_HEIGHT` (4px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-002`).
pub const SLIDER_TRACK_HEIGHT: f32 = size::SLIDER_TRACK_HEIGHT;

/// Slider and crosshair thumb visual size — `size::SLIDER_THUMB` (16px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-002`).
pub const THUMB_SIZE: f32 = size::SLIDER_THUMB;

/// Card internal padding — `space::MD` (12px) (§3.12, `UIF-ATOM-012`).
pub const CARD_PADDING: f32 = space::MD;

/// Card corner radius — `shape::RADIUS_CARD` (6px) (§3.12, `UIF-ATOM-012`).
pub const CARD_RADIUS: f32 = shape::RADIUS_CARD;

/// Mandatory inner bounding border stroke width — `shape::BORDER_HAIRLINE` (1px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
pub const INNER_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Dual-ring contrast crosshair outer stroke width — `shape::BORDER_FOCUS` (2px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
pub const DUAL_RING_OUTER_WIDTH: f32 = shape::BORDER_FOCUS;

/// Dual-ring contrast crosshair inner stroke width — `shape::BORDER_HAIRLINE` (1px) (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
pub const DUAL_RING_INNER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Spacing gap between swatches — `space::XS` (4px) (§3.12, `UIF-ATOM-012`).
pub const SWATCH_GAP: f32 = space::XS;

/// Vertical spacing between sections in flyout card — `space::SM` (8px) (§3.12, `UIF-ATOM-012`).
pub const SECTION_GAP: f32 = space::SM;

/// Contrast ratio semantic chip height — `size::BADGE_CHIP_HEIGHT` (24px) (§3.12, `UIF-ATOM-012`).
pub const CONTRAST_CHIP_HEIGHT: f32 = size::BADGE_CHIP_HEIGHT;

// ---------------------------------------------------------------------------
// Color Data Models & Conversions (§3.12, UIF-ATOM-012)
// ---------------------------------------------------------------------------

/// 8-bit RGBA color representation with channels in `0..=255` (§3.12, `UIF-ATOM-012`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Rgba8 {
    /// Red color channel (`0..=255`).
    pub r: u8,
    /// Green color channel (`0..=255`).
    pub g: u8,
    /// Blue color channel (`0..=255`).
    pub b: u8,
    /// Alpha opacity channel (`0..=255`).
    pub a: u8,
}

impl Rgba8 {
    /// Construct a new [`Rgba8`] value from explicit channel components (§3.12).
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Construct a fully opaque [`Rgba8`] color with `a = 255` (§3.12).
    #[must_use]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Construct an [`Rgba8`] color with explicit alpha (§3.12).
    #[must_use]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse an [`Rgba8`] from a hex string format (`#RRGGBB`, `#RRGGBBAA`, `RRGGBB`, `RRGGBBAA`) (§3.12).
    ///
    /// Anything that is not a run of ASCII hex digits is rejected with `None`,
    /// never panicked on. Two reasons, both decided before any slicing:
    ///
    /// * **Non-ASCII.** The shape is chosen by `str::len()` — a **byte** count —
    ///   and the channels are then sliced by **byte index**. A six-byte string
    ///   holding a multi-byte character passes the length test and `&hex[0..2]`
    ///   cuts through the middle of that character, so `str` slicing panics
    ///   rather than reporting a bad colour.
    /// * **Signs.** `u8::from_str_radix` accepts a leading `+`, so `"+A2B3C"`
    ///   would otherwise parse as a colour even though `+` is not a hex digit.
    ///
    /// Surrounding whitespace is trimmed, and one leading `#` is allowed.
    #[must_use]
    pub fn from_hex(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
        if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::from_rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::from_rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Format color as an uppercase 6-digit hex string (`#RRGGBB`) (§3.12).
    #[must_use]
    pub fn to_hex_rgb(&self) -> String {
        format!("#{r:02X}{g:02X}{b:02X}", r = self.r, g = self.g, b = self.b)
    }

    /// Format color as an uppercase 8-digit hex string (`#RRGGBBAA`) (§3.12).
    #[must_use]
    pub fn to_hex_rgba(&self) -> String {
        format!(
            "#{r:02X}{g:02X}{b:02X}{a:02X}",
            r = self.r,
            g = self.g,
            b = self.b,
            a = self.a
        )
    }
}

impl std::fmt::Display for Rgba8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.a == 255 {
            write!(f, "{}", self.to_hex_rgb())
        } else {
            write!(f, "{}", self.to_hex_rgba())
        }
    }
}

impl From<Rgba8> for Color {
    fn from(rgba: Rgba8) -> Self {
        Color {
            r: rgba.r as f32 / 255.0,
            g: rgba.g as f32 / 255.0,
            b: rgba.b as f32 / 255.0,
            a: rgba.a as f32 / 255.0,
        }
    }
}

impl From<Color> for Rgba8 {
    fn from(c: Color) -> Self {
        Self {
            r: (c.r.clamp(0.0, 1.0) * 255.0).round() as u8,
            g: (c.g.clamp(0.0, 1.0) * 255.0).round() as u8,
            b: (c.b.clamp(0.0, 1.0) * 255.0).round() as u8,
            a: (c.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }
}

/// Hue, Saturation, Value, Alpha color representation (§3.12, `UIF-ATOM-012`).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Hsva {
    /// Hue in degrees (`0.0..=360.0`).
    pub h: f32,
    /// Saturation proportion (`0.0..=1.0`).
    pub s: f32,
    /// Value / brightness proportion (`0.0..=1.0`).
    pub v: f32,
    /// Alpha / opacity proportion (`0.0..=1.0`).
    pub a: f32,
}

impl Hsva {
    /// Construct a new [`Hsva`] value with channel clamping (§3.12).
    #[must_use]
    pub fn new(h: f32, s: f32, v: f32, a: f32) -> Self {
        Self {
            h: h.clamp(0.0, 360.0),
            s: s.clamp(0.0, 1.0),
            v: v.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }

    /// Construct fully opaque [`Hsva`] color with `a = 1.0` (§3.12).
    #[must_use]
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        Self::new(h, s, v, 1.0)
    }

    /// Construct [`Hsva`] color with explicit alpha channel (§3.12).
    #[must_use]
    pub fn from_hsva(h: f32, s: f32, v: f32, a: f32) -> Self {
        Self::new(h, s, v, a)
    }

    /// Convert [`Hsva`] color to [`Rgba8`] (§3.12).
    #[must_use]
    pub fn to_rgba8(&self) -> Rgba8 {
        let h = if self.h >= 360.0 || self.h < 0.0 {
            self.h.rem_euclid(360.0)
        } else {
            self.h
        };
        let s = self.s.clamp(0.0, 1.0);
        let v = self.v.clamp(0.0, 1.0);
        let a = self.a.clamp(0.0, 1.0);

        let c = v * s;
        let h_prime = h / 60.0;
        let x = c * (1.0 - (h_prime.rem_euclid(2.0) - 1.0).abs());

        let (r1, g1, b1) = match h_prime as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        let m = v - c;
        Rgba8 {
            r: ((r1 + m) * 255.0).round() as u8,
            g: ((g1 + m) * 255.0).round() as u8,
            b: ((b1 + m) * 255.0).round() as u8,
            a: (a * 255.0).round() as u8,
        }
    }

    /// Construct [`Hsva`] color from [`Rgba8`] (§3.12).
    #[must_use]
    pub fn from_rgba8(rgba: Rgba8) -> Self {
        let r = rgba.r as f32 / 255.0;
        let g = rgba.g as f32 / 255.0;
        let b = rgba.b as f32 / 255.0;
        let a = rgba.a as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let v = max;
        let s = if max <= 0.0 { 0.0 } else { delta / max };

        let mut h = if delta <= 1e-6 {
            0.0
        } else if (max - r).abs() < 1e-6 {
            60.0 * (((g - b) / delta).rem_euclid(6.0))
        } else if (max - g).abs() < 1e-6 {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };
        if h < 0.0 {
            h += 360.0;
        }

        Self { h, s, v, a }
    }
}

impl From<Hsva> for Rgba8 {
    fn from(hsva: Hsva) -> Self {
        hsva.to_rgba8()
    }
}

impl From<Rgba8> for Hsva {
    fn from(rgba: Rgba8) -> Self {
        Hsva::from_rgba8(rgba)
    }
}

impl From<Hsva> for Color {
    fn from(hsva: Hsva) -> Self {
        Color::from(hsva.to_rgba8())
    }
}

impl From<Color> for Hsva {
    fn from(c: Color) -> Self {
        Hsva::from_rgba8(Rgba8::from(c))
    }
}

// ---------------------------------------------------------------------------
// Default presets (§3.12, UIF-ATOM-012)
// ---------------------------------------------------------------------------

/// Default preset swatches grid colors (§3.12, `UIF-ATOM-012`).
#[must_use]
pub fn default_preset_swatches() -> Vec<Rgba8> {
    vec![
        Rgba8::from_rgb(0, 0, 0),       // Black
        Rgba8::from_rgb(255, 255, 255), // White
        Rgba8::from_rgb(239, 68, 68),   // Red
        Rgba8::from_rgb(249, 115, 22),  // Orange
        Rgba8::from_rgb(234, 179, 8),   // Amber/Yellow
        Rgba8::from_rgb(34, 197, 94),   // Green
        Rgba8::from_rgb(6, 182, 212),   // Cyan
        Rgba8::from_rgb(59, 130, 246),  // Blue
        Rgba8::from_rgb(168, 85, 247),  // Purple
        Rgba8::from_rgb(236, 72, 153),  // Pink
    ]
}

// ---------------------------------------------------------------------------
// ColorPickerModel (§3.12, UIF-ATOM-012)
// ---------------------------------------------------------------------------

/// Declarative state and configuration for `ColorPicker` & `ColorSwatchButton` (§3.12, `UIF-ATOM-012`).
#[derive(Debug, Clone, PartialEq)]
pub struct ColorPickerModel {
    /// Currently selected 8-bit RGBA color.
    pub value: Rgba8,
    /// Currently active HSV color representation.
    pub hsv: Hsva,
    /// Current text content in the hex input field.
    pub hex_input: String,
    /// Whether alpha channel selection is enabled.
    pub alpha_enabled: bool,
    /// Preset color swatches displayed in the picker grid.
    pub preset_swatches: Vec<Rgba8>,
    /// Optional contrast reference color for live WCAG 2.1 readout.
    pub contrast_reference: Option<Rgba8>,
    /// Whether the flyout card is currently open.
    pub open: bool,
    /// Whether the control is disabled.
    pub disabled: bool,
    /// Snapshot of color prior to opening flyout, restored on Escape / Cancel.
    pub pre_open_snapshot: Option<Rgba8>,
    /// Live hex validation error message, if any.
    pub validation_error: Option<String>,
}

impl ColorPickerModel {
    /// Construct a new [`ColorPickerModel`] initialized with a given color (§3.12).
    #[must_use]
    pub fn new(value: Rgba8) -> Self {
        let hsv = Hsva::from_rgba8(value);
        let hex_input = value.to_hex_rgb();
        Self {
            value,
            hsv,
            hex_input,
            alpha_enabled: false,
            preset_swatches: default_preset_swatches(),
            contrast_reference: None,
            open: false,
            disabled: false,
            pre_open_snapshot: None,
            validation_error: None,
        }
    }

    /// Enable or disable the alpha channel slider and 8-digit hex input (§3.12).
    #[must_use]
    pub fn with_alpha(mut self, enabled: bool) -> Self {
        self.alpha_enabled = enabled;
        self.hex_input = if enabled {
            self.value.to_hex_rgba()
        } else {
            self.value.to_hex_rgb()
        };
        self
    }

    /// Supply custom preset swatches for the picker grid (§3.12).
    #[must_use]
    pub fn with_preset_swatches(mut self, swatches: impl IntoIterator<Item = Rgba8>) -> Self {
        self.preset_swatches = swatches.into_iter().collect();
        self
    }

    /// Supply a contrast reference background color for live WCAG 2.1 readout (§3.12, `UIF-TOK-006`).
    #[must_use]
    pub fn with_contrast_reference(mut self, reference: Rgba8) -> Self {
        self.contrast_reference = Some(reference);
        self
    }

    /// Set initial flyout open state (§3.12).
    #[must_use]
    pub fn with_open(mut self, open: bool) -> Self {
        self.open = open;
        if open {
            self.pre_open_snapshot = Some(self.value);
        }
        self
    }

    /// Set disabled state (§3.12).
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Update color via [`Rgba8`], synchronizing HSV and hex input (§3.12).
    pub fn set_rgba(&mut self, rgba: Rgba8) {
        self.value = rgba;
        self.hsv = Hsva::from_rgba8(rgba);
        self.hex_input = if self.alpha_enabled {
            rgba.to_hex_rgba()
        } else {
            rgba.to_hex_rgb()
        };
        self.validation_error = None;
    }

    /// Update color via [`Hsva`], synchronizing RGBA and hex input (§3.12).
    pub fn set_hsva(&mut self, hsva: Hsva) {
        self.hsv = hsva;
        self.value = hsva.to_rgba8();
        self.hex_input = if self.alpha_enabled {
            self.value.to_hex_rgba()
        } else {
            self.value.to_hex_rgb()
        };
        self.validation_error = None;
    }

    /// Update hex input string with live validation (§3.12).
    ///
    /// Returns `true` if parsed successfully, or `false` if invalid.
    pub fn set_hex_input(&mut self, hex: &str) -> bool {
        self.hex_input = hex.to_string();
        if let Some(mut parsed) = Rgba8::from_hex(hex) {
            if !self.alpha_enabled {
                parsed.a = 255;
            }
            self.value = parsed;
            self.hsv = Hsva::from_rgba8(parsed);
            self.validation_error = None;
            true
        } else {
            self.validation_error = Some(ERR_INVALID_HEX.to_string());
            false
        }
    }

    /// Open flyout card and record pre-open snapshot for rollback (§3.12, `UIF-PRIN-005`).
    pub fn open_flyout(&mut self) {
        if !self.disabled && !self.open {
            self.pre_open_snapshot = Some(self.value);
            self.open = true;
        }
    }

    /// Close flyout card discarding pre-open snapshot (§3.12).
    pub fn close_flyout(&mut self) {
        self.open = false;
        self.pre_open_snapshot = None;
    }

    /// Cancel color selection and revert to pre-open snapshot (§3.12, `UIF-PRIN-005`).
    pub fn cancel(&mut self) -> Rgba8 {
        if let Some(snap) = self.pre_open_snapshot {
            self.set_rgba(snap);
        }
        self.open = false;
        self.pre_open_snapshot = None;
        self.value
    }

    /// Commit currently selected color and close flyout (§3.12, `UIF-PRIN-005`).
    pub fn commit(&mut self) -> Rgba8 {
        self.open = false;
        self.pre_open_snapshot = None;
        self.value
    }

    /// Format display hex string for trigger button (§3.12).
    #[must_use]
    pub fn format_display(&self) -> String {
        if self.alpha_enabled {
            self.value.to_hex_rgba()
        } else {
            self.value.to_hex_rgb()
        }
    }
}

impl Default for ColorPickerModel {
    fn default() -> Self {
        Self::new(Rgba8::from_rgb(0, 0, 0))
    }
}

// ---------------------------------------------------------------------------
// Flyout Layout Calculator (§3.12, UIF-ATOM-012)
// ---------------------------------------------------------------------------

/// Calculated geometry bounds for color flyout card components (§3.12, `UIF-ATOM-012`).
#[derive(Debug, Clone)]
pub struct ColorFlyoutLayout {
    /// Overall flyout card bounding box.
    pub card_bounds: Rectangle,
    /// 2D Saturation/Value plane well bounds (180x140px).
    pub sv_well_bounds: Rectangle,
    /// 4px Hue slider track bounding box.
    pub hue_track_bounds: Rectangle,
    /// Optional 4px Alpha slider track bounding box.
    pub alpha_track_bounds: Option<Rectangle>,
    /// Hex input and preview row bounding box.
    pub hex_row_bounds: Rectangle,
    /// Preview swatch well bounds inside the hex row.
    pub hex_swatch_bounds: Rectangle,
    /// Hex text input editor bounds.
    pub hex_input_bounds: Rectangle,
    /// Preset swatch bounding boxes in grid order.
    pub swatches: Vec<Rectangle>,
    /// Live contrast ratio chip bounding box.
    pub contrast_bounds: Option<Rectangle>,
    /// Cancel action button bounding box.
    pub cancel_button_bounds: Rectangle,
    /// Apply action button bounding box.
    pub apply_button_bounds: Rectangle,
}

impl ColorFlyoutLayout {
    /// Compute layout bounds based on top-left flyout anchor point and model options (§3.12).
    #[must_use]
    pub fn compute(top_left: Point, model: &ColorPickerModel) -> Self {
        let card_w = COLOR_FLYOUT_WIDTH;
        let well_x = top_left.x + (card_w - COLOR_WELL_WIDTH) / 2.0;
        let mut cur_y = top_left.y + CARD_PADDING;

        // 1. SV well
        let sv_well_bounds = Rectangle {
            x: well_x,
            y: cur_y,
            width: COLOR_WELL_WIDTH,
            height: COLOR_WELL_HEIGHT,
        };
        cur_y += COLOR_WELL_HEIGHT + SECTION_GAP;

        // 2. Hue slider
        let hue_track_y = cur_y + (THUMB_SIZE - SLIDER_TRACK_HEIGHT) / 2.0;
        let hue_track_bounds = Rectangle {
            x: well_x,
            y: hue_track_y,
            width: COLOR_WELL_WIDTH,
            height: SLIDER_TRACK_HEIGHT,
        };
        cur_y += THUMB_SIZE + SECTION_GAP;

        // 3. Alpha slider (if enabled)
        let alpha_track_bounds = if model.alpha_enabled {
            let alpha_track_y = cur_y + (THUMB_SIZE - SLIDER_TRACK_HEIGHT) / 2.0;
            let bounds = Rectangle {
                x: well_x,
                y: alpha_track_y,
                width: COLOR_WELL_WIDTH,
                height: SLIDER_TRACK_HEIGHT,
            };
            cur_y += THUMB_SIZE + SECTION_GAP;
            Some(bounds)
        } else {
            None
        };

        // 4. Hex row
        let hex_row_bounds = Rectangle {
            x: well_x,
            y: cur_y,
            width: COLOR_WELL_WIDTH,
            height: TRIGGER_HEIGHT_SM,
        };
        let hex_swatch_bounds = Rectangle {
            x: well_x,
            y: cur_y + (TRIGGER_HEIGHT_SM - SWATCH_SIZE) / 2.0,
            width: SWATCH_SIZE,
            height: SWATCH_SIZE,
        };
        let input_x = hex_swatch_bounds.x + hex_swatch_bounds.width + space::SM;
        let input_w = well_x + COLOR_WELL_WIDTH - input_x;
        let hex_input_bounds = Rectangle {
            x: input_x,
            y: cur_y,
            width: input_w,
            height: TRIGGER_HEIGHT_SM,
        };
        cur_y += TRIGGER_HEIGHT_SM + SECTION_GAP;

        // 5. Preset swatches
        let mut swatches = Vec::new();
        if !model.preset_swatches.is_empty() {
            let cols = 6;
            for (idx, _preset) in model.preset_swatches.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let s_x = well_x + col as f32 * (SWATCH_SIZE + SWATCH_GAP);
                let s_y = cur_y + row as f32 * (SWATCH_SIZE + SWATCH_GAP);
                swatches.push(Rectangle {
                    x: s_x,
                    y: s_y,
                    width: SWATCH_SIZE,
                    height: SWATCH_SIZE,
                });
            }
            let rows = model.preset_swatches.len().div_ceil(cols);
            let swatches_h =
                rows as f32 * SWATCH_SIZE + (rows.saturating_sub(1) as f32 * SWATCH_GAP);
            cur_y += swatches_h + SECTION_GAP;
        }

        // 6. Contrast readout chip
        let contrast_bounds = if model.contrast_reference.is_some() {
            let bounds = Rectangle {
                x: well_x,
                y: cur_y,
                width: COLOR_WELL_WIDTH,
                height: CONTRAST_CHIP_HEIGHT,
            };
            cur_y += CONTRAST_CHIP_HEIGHT + SECTION_GAP;
            Some(bounds)
        } else {
            None
        };

        // 7. Footer buttons
        let btn_w = (COLOR_WELL_WIDTH - space::SM) / 2.0;
        let cancel_button_bounds = Rectangle {
            x: well_x,
            y: cur_y,
            width: btn_w,
            height: TRIGGER_HEIGHT_SM,
        };
        let apply_button_bounds = Rectangle {
            x: well_x + btn_w + space::SM,
            y: cur_y,
            width: btn_w,
            height: TRIGGER_HEIGHT_SM,
        };
        cur_y += TRIGGER_HEIGHT_SM + CARD_PADDING;

        let card_bounds = Rectangle {
            x: top_left.x,
            y: top_left.y,
            width: card_w,
            height: cur_y - top_left.y,
        };

        Self {
            card_bounds,
            sv_well_bounds,
            hue_track_bounds,
            alpha_track_bounds,
            hex_row_bounds,
            hex_swatch_bounds,
            hex_input_bounds,
            swatches,
            contrast_bounds,
            cancel_button_bounds,
            apply_button_bounds,
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering Primitives & Helpers (§3.12, UIF-ATOM-012, UIF-TOK-006)
// ---------------------------------------------------------------------------

/// Draw a neutral 4x4px checkerboard pattern (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
fn draw_checkerboard<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    cell_size: f32,
    palette: &Palette,
) where
    Renderer: renderer::Renderer,
{
    let cols = (bounds.width / cell_size).ceil() as usize;
    let rows = (bounds.height / cell_size).ceil() as usize;
    for r in 0..rows {
        for c in 0..cols {
            let x = bounds.x + c as f32 * cell_size;
            let y = bounds.y + r as f32 * cell_size;
            let w = (bounds.x + bounds.width - x).min(cell_size);
            let h = (bounds.y + bounds.height - y).min(cell_size);
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            let fill = if (c + r) % 2 == 0 {
                palette.surface_base
            } else {
                palette.surface_active
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x,
                        y,
                        width: w,
                        height: h,
                    },
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(fill),
            );
        }
    }
}

/// Draw a color preview well over neutral checkerboard enclosed by 1px `border.strong` inner frame (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`).
fn draw_preview_well<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    color: Color,
    palette: &Palette,
) where
    Renderer: renderer::Renderer,
{
    draw_checkerboard(renderer, bounds, CHECKERBOARD_CELL_SIZE, palette);
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border::default(),
            shadow: Shadow::default(),
            snap: true,
        },
        Background::Color(color),
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border::default()
                .color(palette.border_strong)
                .width(INNER_BORDER_WIDTH)
                .rounded(shape::RADIUS_WIDGET),
            shadow: Shadow::default(),
            snap: true,
        },
        Background::Color(Color::TRANSPARENT),
    );
}

// ---------------------------------------------------------------------------
// Standalone ColorSwatchButton Widget (§3.12, UIF-ATOM-012)
// ---------------------------------------------------------------------------

/// The focus-registration name a [`ColorSwatchButton`] claims when its host
/// does not name it.
///
/// Stable by construction, so the router keeps addressing the same control
/// across rebuilds. A host with more than one swatch button on screen must
/// give each one its own name through [`ColorSwatchButton::with_id`].
const DEFAULT_COLOR_SWATCH_BUTTON_ID: &str = "hoarfrost.color_swatch_button";

/// Standalone trigger button displaying a color swatch preview with hex label (§3.12, `UIF-ATOM-012`).
pub struct ColorSwatchButton<'a, Message> {
    widget_id: iced::advanced::widget::Id,
    model: ColorPickerModel,
    palette: Palette,
    height: f32,
    on_press: Option<Box<dyn Fn() -> Message + 'a>>,
}

impl<'a, Message> ColorSwatchButton<'a, Message> {
    /// Construct a new [`ColorSwatchButton`] for a given model (§3.12).
    ///
    /// The button registers with the focus router under
    /// [`DEFAULT_COLOR_SWATCH_BUTTON_ID`]; call [`with_id`](Self::with_id) to
    /// name it when more than one swatch shares a screen.
    #[must_use]
    pub fn new(model: ColorPickerModel) -> Self {
        Self {
            widget_id: iced::advanced::widget::Id::new(DEFAULT_COLOR_SWATCH_BUTTON_ID),
            model,
            palette: Palette::dark(),
            height: TRIGGER_HEIGHT_MD,
            on_press: None,
        }
    }

    /// Name the control the focus router addresses (§3.12).
    ///
    /// The id is fixed for the lifetime of the built widget, so a host that
    /// passes the same name on every rebuild keeps addressing the same button
    /// — which is what makes `focus(id)` and `is_focused(id)` mean anything
    /// once the tree has been rebuilt underneath them.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<iced::advanced::widget::Id>) -> Self {
        self.widget_id = id.into();
        self
    }

    /// The id this button registers with the focus router.
    #[must_use]
    pub fn widget_id(&self) -> &iced::advanced::widget::Id {
        &self.widget_id
    }

    /// Set control trigger height (e.g. 28px `size::CONTROL_SM` or 32px `size::CONTROL_MD`) (§3.12).
    #[must_use]
    pub const fn with_height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Set click callback (§3.12).
    #[must_use]
    pub fn on_press(mut self, on_press: impl Fn() -> Message + 'a) -> Self {
        self.on_press = Some(Box::new(on_press));
        self
    }
}

/// Helper function to construct a [`ColorSwatchButton`] trigger widget (§3.12, `UIF-ATOM-012`).
#[must_use]
pub fn color_swatch_button<'a, Message>(
    model: &ColorPickerModel,
) -> ColorSwatchButton<'a, Message> {
    ColorSwatchButton::new(model.clone())
}

/// Local state for [`ColorSwatchButton`] (§3.12).
#[derive(Debug, Clone, Default)]
pub struct ColorSwatchButtonState {
    /// Keyboard focus tracking.
    pub focusable: FocusableState,
    /// Hovered state for trigger.
    pub hovered: bool,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for ColorSwatchButton<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ColorSwatchButtonState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ColorSwatchButtonState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(COLOR_FLYOUT_WIDTH),
            Length::Fixed(self.height),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let w = COLOR_FLYOUT_WIDTH;
        let h = self.height;
        let size = limits.resolve(Length::Fixed(w), Length::Fixed(h), Size::new(w, h));
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<ColorSwatchButtonState>();
        if !self.model.disabled {
            operation.focusable(Some(&self.widget_id), layout.bounds(), &mut state.focusable);
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if self.model.disabled {
            return;
        }

        let state = tree.state.downcast_mut::<ColorSwatchButtonState>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let hovered = bounds.contains(*position);
                if hovered != state.hovered {
                    state.hovered = hovered;
                    shell.request_redraw();
                }
            }

            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position()
                    && bounds.contains(pos)
                {
                    if let Some(on_press) = &self.on_press {
                        shell.publish(on_press());
                    }
                    shell.request_redraw();
                }
            }

            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focusable.is_focused() =>
            {
                if matches!(key, Key::Named(Named::Enter | Named::Space)) {
                    if let Some(on_press) = &self.on_press {
                        shell.publish(on_press());
                    }
                    shell.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<ColorSwatchButtonState>();
        let bounds = layout.bounds();

        let border_color = if self.model.disabled {
            self.palette.border_subtle
        } else if state.focusable.is_focused() {
            self.palette.accent_border
        } else if state.hovered {
            self.palette.border_strong
        } else {
            self.palette.border_subtle
        };

        let bg_fill = if self.model.disabled {
            self.palette.surface_base
        } else if state.hovered {
            self.palette.surface_hover
        } else {
            self.palette.surface_active
        };

        // Surface background
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border::default()
                    .color(border_color)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(bg_fill),
        );

        // Focus indicator ring (§3.0.2, UIF-TOK-006)
        if state.focusable.is_focused() {
            let indicator = focus::resolve_focus_indicator_for_background(
                self.palette.surface_base,
                &self.palette,
                FocusPlacement::Outer,
            );
            renderer.fill_quad(
                renderer::Quad {
                    bounds: indicator.bounds(bounds),
                    border: Border::default()
                        .color(indicator.color)
                        .width(indicator.width)
                        .rounded(indicator.radius(shape::RADIUS_WIDGET)),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::TRANSPARENT),
            );
        }

        // Preview well (16x16px over 4x4 checkerboard with 1px border.strong) (§3.12, UIF-TOK-006)
        let well_bounds = Rectangle {
            x: bounds.x + space::MD,
            y: bounds.y + (bounds.height - PREVIEW_WELL_SIZE) / 2.0,
            width: PREVIEW_WELL_SIZE,
            height: PREVIEW_WELL_SIZE,
        };
        draw_preview_well(
            renderer,
            well_bounds,
            Color::from(self.model.value),
            &self.palette,
        );

        // Monospace hex label (§3.12)
        let display_hex = self.model.format_display();
        let text_color = if self.model.disabled {
            self.palette.text_disabled
        } else {
            self.palette.text_strong
        };

        let text_x = well_bounds.x + well_bounds.width + space::MD;
        let text_area = Rectangle {
            x: text_x,
            y: bounds.y,
            width: bounds.width - text_x - size::ICON_SM - space::MD,
            height: bounds.height,
        };

        renderer.fill_text(
            text_core::Text {
                content: display_hex,
                bounds: text_area.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(text_area.height)),
                font: iced::Font::MONOSPACE,
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(text_area.x, text_area.center_y()),
            text_color,
            *viewport,
        );

        // Trailing dropdown chevron glyph (§3.12)
        let chevron_bounds = Rectangle {
            x: bounds.x + bounds.width - size::ICON_SM - space::MD,
            y: bounds.y,
            width: size::ICON_SM,
            height: bounds.height,
        };
        let chevron_glyph = if self.model.open {
            CHEVRON_UP
        } else {
            CHEVRON_DOWN
        };
        let chevron_color = if self.model.disabled {
            self.palette.text_disabled
        } else {
            self.palette.text_muted
        };

        renderer.fill_text(
            text_core::Text {
                content: chevron_glyph.to_owned(),
                bounds: chevron_bounds.size(),
                size: Pixels(size::ICON_SM),
                line_height: text_core::LineHeight::Absolute(Pixels(chevron_bounds.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            chevron_bounds.center(),
            chevron_color,
            *viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<ColorSwatchButton<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(button: ColorSwatchButton<'a, Message>) -> Self {
        Element::new(button)
    }
}

// ---------------------------------------------------------------------------
// ColorPicker Interaction State & Target Enum (§3.12, UIF-ATOM-012)
// ---------------------------------------------------------------------------

/// Currently captured drag target in the color picker surface (§3.12, `UIF-ATOM-012`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    /// 2D Saturation/Value plane drag.
    SvCanvas,
    /// 1D Rainbow Hue slider track drag.
    HueSlider,
    /// 1D Alpha opacity slider track drag.
    AlphaSlider,
}

/// Local state for the [`ColorPicker`] widget (§3.12, `UIF-ATOM-012`).
#[derive(Debug, Clone, Default)]
pub struct ColorPickerState {
    /// Keyboard focus tracking.
    pub focusable: FocusableState,
    /// Currently active mouse drag target.
    pub dragging: Option<DragTarget>,
    /// Hovered state for trigger.
    pub hovered_trigger: bool,
    /// Hovered state for Cancel button.
    pub hovered_cancel: bool,
    /// Hovered state for Apply button.
    pub hovered_apply: bool,
    /// Hovered preset swatch index.
    pub hovered_swatch: Option<usize>,
    /// Pre-open snapshot for Escape rollback cancellation.
    pub pre_open_snapshot: Option<Rgba8>,
    /// Gesture slot for Live/Commit/Cancel transitions.
    pub gesture: GestureSlot<Rgba8>,
    /// Whether the hex text input field currently has keyboard focus.
    pub focused_hex: bool,
    /// In-progress hex input string buffer while editing.
    pub hex_buffer: Option<String>,
}

// ---------------------------------------------------------------------------
// ColorPicker Widget (§3.12, UIF-ATOM-012)
// ---------------------------------------------------------------------------

/// The focus-registration name a [`ColorPicker`] claims when its host does not
/// name it.
///
/// Stable by construction, so the router keeps addressing the same control
/// across rebuilds. A host with more than one picker on screen must give each
/// one its own name through [`ColorPicker::with_id`].
const DEFAULT_COLOR_PICKER_ID: &str = "hoarfrost.color_picker";

/// Interactive color picker widget combining trigger button and modal flyout (§3.12, `UIF-ATOM-012`).
pub struct ColorPicker<'a, Message> {
    widget_id: iced::advanced::widget::Id,
    model: ColorPickerModel,
    palette: Palette,
    trigger_height: f32,
    on_change: Option<Box<dyn Fn(Live<Rgba8>) -> Message + 'a>>,
    on_commit: Option<Box<dyn Fn(Commit<Rgba8>) -> Message + 'a>>,
    on_cancel: Option<Box<dyn Fn(Cancel<Rgba8>) -> Message + 'a>>,
    on_open_change: Option<Box<dyn Fn(Live<bool>) -> Message + 'a>>,
}

impl<'a, Message> ColorPicker<'a, Message> {
    /// Construct a new [`ColorPicker`] widget (§3.12).
    ///
    /// The trigger registers with the focus router under
    /// [`DEFAULT_COLOR_PICKER_ID`]; call [`with_id`](Self::with_id) to name it
    /// when more than one picker shares a screen.
    #[must_use]
    pub fn new(model: ColorPickerModel) -> Self {
        Self {
            widget_id: iced::advanced::widget::Id::new(DEFAULT_COLOR_PICKER_ID),
            model,
            palette: Palette::dark(),
            trigger_height: TRIGGER_HEIGHT_MD,
            on_change: None,
            on_commit: None,
            on_cancel: None,
            on_open_change: None,
        }
    }

    /// Name the control the focus router addresses (§3.12).
    ///
    /// The id is fixed for the lifetime of the built widget, so a host that
    /// passes the same name on every rebuild keeps addressing the same trigger
    /// — which is what makes `focus(id)` and `is_focused(id)` mean anything
    /// once the tree has been rebuilt underneath them.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<iced::advanced::widget::Id>) -> Self {
        self.widget_id = id.into();
        self
    }

    /// The id this picker registers with the focus router.
    #[must_use]
    pub fn widget_id(&self) -> &iced::advanced::widget::Id {
        &self.widget_id
    }

    /// Set control trigger height (e.g. 28px `size::CONTROL_SM` or 32px `size::CONTROL_MD`) (§3.12).
    #[must_use]
    pub const fn with_trigger_height(mut self, height: f32) -> Self {
        self.trigger_height = height;
        self
    }

    /// Register live color scrub change callback (`Live<Rgba8>`) (§3.12, `UIF-ATOM-012`).
    #[must_use]
    pub fn on_change(mut self, on_change: impl Fn(Live<Rgba8>) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }

    /// Register interaction commit callback (`Commit<Rgba8>`) (§3.12, `UIF-ATOM-012`).
    #[must_use]
    pub fn on_commit(mut self, on_commit: impl Fn(Commit<Rgba8>) -> Message + 'a) -> Self {
        self.on_commit = Some(Box::new(on_commit));
        self
    }

    /// Register cancellation / rollback callback (`Cancel<Rgba8>`) (§3.12, `UIF-ATOM-012`).
    #[must_use]
    pub fn on_cancel(mut self, on_cancel: impl Fn(Cancel<Rgba8>) -> Message + 'a) -> Self {
        self.on_cancel = Some(Box::new(on_cancel));
        self
    }

    /// Register flyout open state change callback (`Live<bool>`) (§3.12).
    #[must_use]
    pub fn on_open_change(mut self, on_open_change: impl Fn(Live<bool>) -> Message + 'a) -> Self {
        self.on_open_change = Some(Box::new(on_open_change));
        self
    }

    /// Access the underlying model (§3.12).
    #[must_use]
    pub const fn model(&self) -> &ColorPickerModel {
        &self.model
    }

    /// Access the underlying model mutably (§3.12).
    pub fn model_mut(&mut self) -> &mut ColorPickerModel {
        &mut self.model
    }
}

/// Helper function to construct a [`ColorPicker`] widget (§3.12, `UIF-ATOM-012`).
#[must_use]
pub fn color_picker<'a, Message>(model: ColorPickerModel) -> ColorPicker<'a, Message> {
    ColorPicker::new(model)
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for ColorPicker<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ColorPickerState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ColorPickerState::default())
    }

    fn size(&self) -> Size<Length> {
        let flyout_h = if self.model.open {
            let layout = ColorFlyoutLayout::compute(Point::ORIGIN, &self.model);
            space::XS + layout.card_bounds.height
        } else {
            0.0
        };
        Size::new(
            Length::Fixed(COLOR_FLYOUT_WIDTH),
            Length::Fixed(self.trigger_height + flyout_h),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let w = COLOR_FLYOUT_WIDTH;
        let flyout_h = if self.model.open {
            let layout = ColorFlyoutLayout::compute(Point::ORIGIN, &self.model);
            space::XS + layout.card_bounds.height
        } else {
            0.0
        };
        let h = self.trigger_height + flyout_h;
        let size = limits.resolve(Length::Fixed(w), Length::Fixed(h), Size::new(w, h));
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<ColorPickerState>();
        if !self.model.disabled {
            let trigger_bounds = Rectangle {
                x: layout.bounds().x,
                y: layout.bounds().y,
                width: layout.bounds().width,
                height: self.trigger_height,
            };
            operation.focusable(Some(&self.widget_id), trigger_bounds, &mut state.focusable);
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if self.model.disabled {
            return;
        }

        let state = tree.state.downcast_mut::<ColorPickerState>();
        let bounds = layout.bounds();

        let trigger_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: self.trigger_height,
        };

        let flyout_origin = Point {
            x: bounds.x,
            y: bounds.y + self.trigger_height + space::XS,
        };
        let flyout_layout = ColorFlyoutLayout::compute(flyout_origin, &self.model);

        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let position = *position;
                let hovered_trig = trigger_bounds.contains(position);
                if hovered_trig != state.hovered_trigger {
                    state.hovered_trigger = hovered_trig;
                    shell.request_redraw();
                }

                if self.model.open {
                    let hovered_c = flyout_layout.cancel_button_bounds.contains(position);
                    let hovered_a = flyout_layout.apply_button_bounds.contains(position);
                    let mut hovered_s = None;
                    for (i, sb) in flyout_layout.swatches.iter().enumerate() {
                        if sb.contains(position) {
                            hovered_s = Some(i);
                            break;
                        }
                    }

                    if hovered_c != state.hovered_cancel
                        || hovered_a != state.hovered_apply
                        || hovered_s != state.hovered_swatch
                    {
                        state.hovered_cancel = hovered_c;
                        state.hovered_apply = hovered_a;
                        state.hovered_swatch = hovered_s;
                        shell.request_redraw();
                    }

                    if let Some(dragging) = state.dragging {
                        match dragging {
                            DragTarget::SvCanvas => {
                                let s = ((position.x - flyout_layout.sv_well_bounds.x)
                                    / COLOR_WELL_WIDTH)
                                    .clamp(0.0, 1.0);
                                let v = (1.0
                                    - (position.y - flyout_layout.sv_well_bounds.y)
                                        / COLOR_WELL_HEIGHT)
                                    .clamp(0.0, 1.0);
                                self.model.set_hsva(Hsva::new(
                                    self.model.hsv.h,
                                    s,
                                    v,
                                    self.model.hsv.a,
                                ));
                                if let Some(cb) = &self.on_change {
                                    shell.publish(cb(Live::new(self.model.value)));
                                }
                                shell.request_redraw();
                            }
                            DragTarget::HueSlider => {
                                let h = (((position.x - flyout_layout.hue_track_bounds.x)
                                    / flyout_layout.hue_track_bounds.width)
                                    * 360.0)
                                    .clamp(0.0, 360.0);
                                self.model.set_hsva(Hsva::new(
                                    h,
                                    self.model.hsv.s,
                                    self.model.hsv.v,
                                    self.model.hsv.a,
                                ));
                                if let Some(cb) = &self.on_change {
                                    shell.publish(cb(Live::new(self.model.value)));
                                }
                                shell.request_redraw();
                            }
                            DragTarget::AlphaSlider => {
                                if let Some(alpha_bounds) = flyout_layout.alpha_track_bounds {
                                    let a = ((position.x - alpha_bounds.x) / alpha_bounds.width)
                                        .clamp(0.0, 1.0);
                                    self.model.set_hsva(Hsva::new(
                                        self.model.hsv.h,
                                        self.model.hsv.s,
                                        self.model.hsv.v,
                                        a,
                                    ));
                                    if let Some(cb) = &self.on_change {
                                        shell.publish(cb(Live::new(self.model.value)));
                                    }
                                    shell.request_redraw();
                                }
                            }
                        }
                    }
                }
            }

            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    if trigger_bounds.contains(pos) {
                        state.focused_hex = false;
                        state.hex_buffer = None;
                        if self.model.open {
                            self.model.close_flyout();
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(false)));
                            }
                        } else {
                            self.model.open_flyout();
                            state.pre_open_snapshot = self.model.pre_open_snapshot;
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(true)));
                            }
                        }
                        shell.request_redraw();
                    } else if self.model.open && flyout_layout.card_bounds.contains(pos) {
                        if flyout_layout.hex_input_bounds.contains(pos) {
                            state.focused_hex = true;
                            state.hex_buffer = Some(self.model.hex_input.clone());
                            shell.request_redraw();
                        } else {
                            state.focused_hex = false;
                            state.hex_buffer = None;
                            if flyout_layout.cancel_button_bounds.contains(pos) {
                                let restored = self.model.cancel();
                                if let Some(cb) = &self.on_cancel {
                                    shell.publish(cb(Cancel::new(restored)));
                                }
                                if let Some(cb) = &self.on_open_change {
                                    shell.publish(cb(Live::new(false)));
                                }
                                shell.request_redraw();
                            } else if flyout_layout.apply_button_bounds.contains(pos) {
                                let committed = self.model.commit();
                                if let Some(cb) = &self.on_commit {
                                    shell.publish(cb(Commit::new(committed)));
                                }
                                if let Some(cb) = &self.on_open_change {
                                    shell.publish(cb(Live::new(false)));
                                }
                                shell.request_redraw();
                            } else if flyout_layout.sv_well_bounds.contains(pos) {
                                state.dragging = Some(DragTarget::SvCanvas);
                                let s = ((pos.x - flyout_layout.sv_well_bounds.x)
                                    / COLOR_WELL_WIDTH)
                                    .clamp(0.0, 1.0);
                                let v = (1.0
                                    - (pos.y - flyout_layout.sv_well_bounds.y) / COLOR_WELL_HEIGHT)
                                    .clamp(0.0, 1.0);
                                self.model.set_hsva(Hsva::new(
                                    self.model.hsv.h,
                                    s,
                                    v,
                                    self.model.hsv.a,
                                ));
                                if let Some(cb) = &self.on_change {
                                    shell.publish(cb(Live::new(self.model.value)));
                                }
                                shell.request_redraw();
                            } else if flyout_layout.hue_track_bounds.contains(pos) {
                                state.dragging = Some(DragTarget::HueSlider);
                                let h = (((pos.x - flyout_layout.hue_track_bounds.x)
                                    / flyout_layout.hue_track_bounds.width)
                                    * 360.0)
                                    .clamp(0.0, 360.0);
                                self.model.set_hsva(Hsva::new(
                                    h,
                                    self.model.hsv.s,
                                    self.model.hsv.v,
                                    self.model.hsv.a,
                                ));
                                if let Some(cb) = &self.on_change {
                                    shell.publish(cb(Live::new(self.model.value)));
                                }
                                shell.request_redraw();
                            } else if let Some(alpha_bounds) = flyout_layout.alpha_track_bounds
                                && alpha_bounds.contains(pos)
                            {
                                state.dragging = Some(DragTarget::AlphaSlider);
                                let a =
                                    ((pos.x - alpha_bounds.x) / alpha_bounds.width).clamp(0.0, 1.0);
                                self.model.set_hsva(Hsva::new(
                                    self.model.hsv.h,
                                    self.model.hsv.s,
                                    self.model.hsv.v,
                                    a,
                                ));
                                if let Some(cb) = &self.on_change {
                                    shell.publish(cb(Live::new(self.model.value)));
                                }
                                shell.request_redraw();
                            } else {
                                // Check preset swatches
                                for (idx, sb) in flyout_layout.swatches.iter().enumerate() {
                                    if sb.contains(pos) && idx < self.model.preset_swatches.len() {
                                        let swatch = self.model.preset_swatches[idx];
                                        self.model.set_rgba(swatch);
                                        if let Some(cb) = &self.on_change {
                                            shell.publish(cb(Live::new(swatch)));
                                        }
                                        if let Some(cb) = &self.on_commit {
                                            shell.publish(cb(Commit::new(swatch)));
                                        }
                                        shell.request_redraw();
                                        break;
                                    }
                                }
                            }
                        }
                    } else if self.model.open {
                        state.focused_hex = false;
                        state.hex_buffer = None;
                        // Click outside card closes and cancels
                        let restored = self.model.cancel();
                        if let Some(cb) = &self.on_cancel {
                            shell.publish(cb(Cancel::new(restored)));
                        }
                        if let Some(cb) = &self.on_open_change {
                            shell.publish(cb(Live::new(false)));
                        }
                        shell.request_redraw();
                    }
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.dragging.is_some() {
                    state.dragging = None;
                    if let Some(cb) = &self.on_commit {
                        shell.publish(cb(Commit::new(self.model.value)));
                    }
                    shell.request_redraw();
                }
            }

            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focusable.is_focused() || self.model.open =>
            {
                if state.focused_hex {
                    match key {
                        Key::Named(Named::Escape) => {
                            state.focused_hex = false;
                            state.hex_buffer = None;
                            if self.model.open {
                                let restored = self.model.cancel();
                                if let Some(cb) = &self.on_cancel {
                                    shell.publish(cb(Cancel::new(restored)));
                                }
                                if let Some(cb) = &self.on_open_change {
                                    shell.publish(cb(Live::new(false)));
                                }
                                shell.request_redraw();
                            }
                        }
                        Key::Named(Named::Enter) => {
                            state.focused_hex = false;
                            state.hex_buffer = None;
                            if self.model.open {
                                let committed = self.model.commit();
                                if let Some(cb) = &self.on_commit {
                                    shell.publish(cb(Commit::new(committed)));
                                }
                                if let Some(cb) = &self.on_open_change {
                                    shell.publish(cb(Live::new(false)));
                                }
                                shell.request_redraw();
                            }
                        }
                        Key::Named(Named::Tab) => {
                            state.focused_hex = false;
                            state.hex_buffer = None;
                            shell.request_redraw();
                        }
                        Key::Named(Named::Backspace) => {
                            let mut buf = state
                                .hex_buffer
                                .take()
                                .unwrap_or_else(|| self.model.hex_input.clone());
                            if !buf.is_empty() {
                                buf.pop();
                                if buf.is_empty() {
                                    buf = "#".to_string();
                                }
                            }
                            let ok = self.model.set_hex_input(&buf);
                            state.hex_buffer = Some(buf);
                            if ok && let Some(cb) = &self.on_change {
                                shell.publish(cb(Live::new(self.model.value)));
                            }
                            shell.request_redraw();
                        }
                        Key::Character(ch_str) => {
                            let max_len = if self.model.alpha_enabled { 9 } else { 7 };
                            let mut buf = state
                                .hex_buffer
                                .take()
                                .unwrap_or_else(|| self.model.hex_input.clone());
                            let mut changed = false;
                            for ch in ch_str.chars() {
                                if (ch.is_ascii_hexdigit() || ch == '#') && buf.len() < max_len {
                                    if ch == '#' && buf.contains('#') {
                                        continue;
                                    }
                                    buf.push(ch);
                                    changed = true;
                                }
                            }
                            if changed {
                                let ok = self.model.set_hex_input(&buf);
                                if ok && let Some(cb) = &self.on_change {
                                    shell.publish(cb(Live::new(self.model.value)));
                                }
                                shell.request_redraw();
                            }
                            state.hex_buffer = Some(buf);
                        }
                        _ => {}
                    }
                    return;
                }

                match key {
                    Key::Named(Named::Escape) => {
                        if self.model.open {
                            let restored = self.model.cancel();
                            if let Some(cb) = &self.on_cancel {
                                shell.publish(cb(Cancel::new(restored)));
                            }
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(false)));
                            }
                            shell.request_redraw();
                        }
                    }
                    Key::Named(Named::Enter) => {
                        if self.model.open {
                            let committed = self.model.commit();
                            if let Some(cb) = &self.on_commit {
                                shell.publish(cb(Commit::new(committed)));
                            }
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(false)));
                            }
                            shell.request_redraw();
                        } else {
                            self.model.open_flyout();
                            state.pre_open_snapshot = self.model.pre_open_snapshot;
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(true)));
                            }
                            shell.request_redraw();
                        }
                    }
                    Key::Named(Named::Space) => {
                        if !self.model.open {
                            self.model.open_flyout();
                            state.pre_open_snapshot = self.model.pre_open_snapshot;
                            if let Some(cb) = &self.on_open_change {
                                shell.publish(cb(Live::new(true)));
                            }
                            shell.request_redraw();
                        }
                    }
                    Key::Named(Named::ArrowLeft) if self.model.open => {
                        let new_s = (self.model.hsv.s - 0.05).clamp(0.0, 1.0);
                        self.model.set_hsva(Hsva::new(
                            self.model.hsv.h,
                            new_s,
                            self.model.hsv.v,
                            self.model.hsv.a,
                        ));
                        if let Some(cb) = &self.on_change {
                            shell.publish(cb(Live::new(self.model.value)));
                        }
                        shell.request_redraw();
                    }
                    Key::Named(Named::ArrowRight) if self.model.open => {
                        let new_s = (self.model.hsv.s + 0.05).clamp(0.0, 1.0);
                        self.model.set_hsva(Hsva::new(
                            self.model.hsv.h,
                            new_s,
                            self.model.hsv.v,
                            self.model.hsv.a,
                        ));
                        if let Some(cb) = &self.on_change {
                            shell.publish(cb(Live::new(self.model.value)));
                        }
                        shell.request_redraw();
                    }
                    Key::Named(Named::ArrowUp) if self.model.open => {
                        let new_v = (self.model.hsv.v + 0.05).clamp(0.0, 1.0);
                        self.model.set_hsva(Hsva::new(
                            self.model.hsv.h,
                            self.model.hsv.s,
                            new_v,
                            self.model.hsv.a,
                        ));
                        if let Some(cb) = &self.on_change {
                            shell.publish(cb(Live::new(self.model.value)));
                        }
                        shell.request_redraw();
                    }
                    Key::Named(Named::ArrowDown) if self.model.open => {
                        let new_v = (self.model.hsv.v - 0.05).clamp(0.0, 1.0);
                        self.model.set_hsva(Hsva::new(
                            self.model.hsv.h,
                            self.model.hsv.s,
                            new_v,
                            self.model.hsv.a,
                        ));
                        if let Some(cb) = &self.on_change {
                            shell.publish(cb(Live::new(self.model.value)));
                        }
                        shell.request_redraw();
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<ColorPickerState>();
        let bounds = layout.bounds();

        let trigger_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: self.trigger_height,
        };

        // -------------------------------------------------------------------
        // 1. Draw Trigger Surface & Preview Well (§3.12, UIF-ATOM-012, UIF-TOK-006)
        // -------------------------------------------------------------------
        let is_invalid = self.model.validation_error.is_some();
        let border_color = if self.model.disabled {
            self.palette.border_subtle
        } else if is_invalid {
            self.palette.status_danger_text
        } else if state.focusable.is_focused() {
            self.palette.accent_border
        } else if state.hovered_trigger {
            self.palette.border_strong
        } else {
            self.palette.border_subtle
        };

        let bg_fill = if self.model.disabled {
            self.palette.surface_base
        } else if state.hovered_trigger {
            self.palette.surface_hover
        } else {
            self.palette.surface_active
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: trigger_bounds,
                border: Border::default()
                    .color(border_color)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(bg_fill),
        );

        // Focus indicator ring (§3.0.2, UIF-TOK-006)
        if state.focusable.is_focused() {
            let indicator = focus::resolve_focus_indicator_for_background(
                self.palette.surface_base,
                &self.palette,
                FocusPlacement::Outer,
            );
            renderer.fill_quad(
                renderer::Quad {
                    bounds: indicator.bounds(trigger_bounds),
                    border: Border::default()
                        .color(indicator.color)
                        .width(indicator.width)
                        .rounded(indicator.radius(shape::RADIUS_WIDGET)),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::TRANSPARENT),
            );
        }

        // Preview well (16x16px over 4x4 checkerboard with 1px border.strong) (§3.12, UIF-TOK-006)
        let well_bounds = Rectangle {
            x: trigger_bounds.x + space::MD,
            y: trigger_bounds.y + (self.trigger_height - PREVIEW_WELL_SIZE) / 2.0,
            width: PREVIEW_WELL_SIZE,
            height: PREVIEW_WELL_SIZE,
        };
        draw_preview_well(
            renderer,
            well_bounds,
            Color::from(self.model.value),
            &self.palette,
        );

        // Monospace hex label (§3.12)
        let display_hex = self.model.format_display();
        let text_color = if self.model.disabled {
            self.palette.text_disabled
        } else {
            self.palette.text_strong
        };

        let text_x = well_bounds.x + well_bounds.width + space::MD;
        let text_area = Rectangle {
            x: text_x,
            y: trigger_bounds.y,
            width: trigger_bounds.width - text_x - size::ICON_SM - space::MD,
            height: trigger_bounds.height,
        };

        renderer.fill_text(
            text_core::Text {
                content: display_hex,
                bounds: text_area.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(text_area.height)),
                font: iced::Font::MONOSPACE,
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(text_area.x, text_area.center_y()),
            text_color,
            *viewport,
        );

        // Trailing dropdown chevron glyph (§3.12)
        let chevron_bounds = Rectangle {
            x: trigger_bounds.x + trigger_bounds.width - size::ICON_SM - space::MD,
            y: trigger_bounds.y,
            width: size::ICON_SM,
            height: trigger_bounds.height,
        };
        let chevron_glyph = if self.model.open {
            CHEVRON_UP
        } else {
            CHEVRON_DOWN
        };
        let chevron_color = if self.model.disabled {
            self.palette.text_disabled
        } else {
            self.palette.text_muted
        };

        renderer.fill_text(
            text_core::Text {
                content: chevron_glyph.to_owned(),
                bounds: chevron_bounds.size(),
                size: Pixels(size::ICON_SM),
                line_height: text_core::LineHeight::Absolute(Pixels(chevron_bounds.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            chevron_bounds.center(),
            chevron_color,
            *viewport,
        );

        // -------------------------------------------------------------------
        // 2. Draw ColorPicker Flyout Surface when Open (§3.12, Z-Band 2, Escape Rank 3)
        // -------------------------------------------------------------------
        if !self.model.open {
            return;
        }

        let flyout_origin = Point {
            x: bounds.x,
            y: bounds.y + self.trigger_height + space::XS,
        };
        let flyout = ColorFlyoutLayout::compute(flyout_origin, &self.model);

        // Card Container background & border (§3.12)
        renderer.fill_quad(
            renderer::Quad {
                bounds: flyout.card_bounds,
                border: Border::default()
                    .color(self.palette.border_strong)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(CARD_RADIUS),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_overlay),
        );

        // -------------------------------------------------------------------
        // 2A. 2D Saturation/Value Canvas (180x140px) (§3.12, UIF-ATOM-012)
        // -------------------------------------------------------------------
        let sv_cols = 18;
        let sv_rows = 14;
        let tile_w = COLOR_WELL_WIDTH / sv_cols as f32;
        let tile_h = COLOR_WELL_HEIGHT / sv_rows as f32;

        for r in 0..sv_rows {
            for c in 0..sv_cols {
                let sat = (c as f32 + 0.5) / sv_cols as f32;
                let val = 1.0 - (r as f32 + 0.5) / sv_rows as f32;
                let col = Hsva::new(self.model.hsv.h, sat, val, 1.0).to_rgba8();

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: flyout.sv_well_bounds.x + c as f32 * tile_w,
                            y: flyout.sv_well_bounds.y + r as f32 * tile_h,
                            width: tile_w,
                            height: tile_h,
                        },
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(Color::from(col)),
                );
            }
        }

        // SV well bounding frame
        renderer.fill_quad(
            renderer::Quad {
                bounds: flyout.sv_well_bounds,
                border: Border::default()
                    .color(self.palette.border_strong)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(Color::TRANSPARENT),
        );

        // Crosshair thumb with dual-ring contrast (§3.12, UIF-TOK-006)
        let thumb_cx = flyout.sv_well_bounds.x + self.model.hsv.s * COLOR_WELL_WIDTH;
        let thumb_cy = flyout.sv_well_bounds.y + (1.0 - self.model.hsv.v) * COLOR_WELL_HEIGHT;

        // Outer contrast ring (2px text.strong)
        let outer_bounds = Rectangle {
            x: thumb_cx - THUMB_SIZE / 2.0,
            y: thumb_cy - THUMB_SIZE / 2.0,
            width: THUMB_SIZE,
            height: THUMB_SIZE,
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: outer_bounds,
                border: Border::default()
                    .color(self.palette.text_strong)
                    .width(DUAL_RING_OUTER_WIDTH)
                    .rounded(THUMB_SIZE / 2.0),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(Color::TRANSPARENT),
        );

        // Inner contrast ring (1px surface.base)
        let inner_size = THUMB_SIZE - DUAL_RING_OUTER_WIDTH * 2.0;
        let inner_bounds = Rectangle {
            x: thumb_cx - inner_size / 2.0,
            y: thumb_cy - inner_size / 2.0,
            width: inner_size,
            height: inner_size,
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: inner_bounds,
                border: Border::default()
                    .color(self.palette.surface_base)
                    .width(DUAL_RING_INNER_WIDTH)
                    .rounded(inner_size / 2.0),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(Color::TRANSPARENT),
        );

        // -------------------------------------------------------------------
        // 2B. Hue Slider (4px track, 0..360° gradient, 16px thumb) (§3.12)
        // -------------------------------------------------------------------
        let hue_segments = 18;
        let seg_w = flyout.hue_track_bounds.width / hue_segments as f32;
        for i in 0..hue_segments {
            let hue = (i as f32 + 0.5) / hue_segments as f32 * 360.0;
            let col = Hsva::new(hue, 1.0, 1.0, 1.0).to_rgba8();
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: flyout.hue_track_bounds.x + i as f32 * seg_w,
                        y: flyout.hue_track_bounds.y,
                        width: seg_w,
                        height: SLIDER_TRACK_HEIGHT,
                    },
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::from(col)),
            );
        }

        // Hue slider thumb (16px)
        let hue_thumb_x = flyout.hue_track_bounds.x
            + (self.model.hsv.h / 360.0) * flyout.hue_track_bounds.width
            - THUMB_SIZE / 2.0;
        let hue_thumb_y = flyout.hue_track_bounds.y + (SLIDER_TRACK_HEIGHT - THUMB_SIZE) / 2.0;
        let hue_thumb_bounds = Rectangle {
            x: hue_thumb_x,
            y: hue_thumb_y,
            width: THUMB_SIZE,
            height: THUMB_SIZE,
        };
        let hue_color = Hsva::new(self.model.hsv.h, 1.0, 1.0, 1.0).to_rgba8();
        renderer.fill_quad(
            renderer::Quad {
                bounds: hue_thumb_bounds,
                border: Border::default()
                    .color(self.palette.text_strong)
                    .width(shape::BORDER_FOCUS)
                    .rounded(THUMB_SIZE / 2.0),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(Color::from(hue_color)),
        );

        // -------------------------------------------------------------------
        // 2C. Optional Alpha Slider (4px track over 4x4 checkerboard) (§3.12)
        // -------------------------------------------------------------------
        if let Some(alpha_bounds) = flyout.alpha_track_bounds {
            draw_checkerboard(
                renderer,
                alpha_bounds,
                CHECKERBOARD_CELL_SIZE,
                &self.palette,
            );
            let alpha_segs = 18;
            let alpha_seg_w = alpha_bounds.width / alpha_segs as f32;
            for i in 0..alpha_segs {
                let a = (i as f32 + 0.5) / alpha_segs as f32;
                let col =
                    Hsva::new(self.model.hsv.h, self.model.hsv.s, self.model.hsv.v, a).to_rgba8();
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: alpha_bounds.x + i as f32 * alpha_seg_w,
                            y: alpha_bounds.y,
                            width: alpha_seg_w,
                            height: SLIDER_TRACK_HEIGHT,
                        },
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(Color::from(col)),
                );
            }

            // Alpha thumb (16px)
            let alpha_thumb_x =
                alpha_bounds.x + self.model.hsv.a * alpha_bounds.width - THUMB_SIZE / 2.0;
            let alpha_thumb_y = alpha_bounds.y + (SLIDER_TRACK_HEIGHT - THUMB_SIZE) / 2.0;
            let alpha_thumb_bounds = Rectangle {
                x: alpha_thumb_x,
                y: alpha_thumb_y,
                width: THUMB_SIZE,
                height: THUMB_SIZE,
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: alpha_thumb_bounds,
                    border: Border::default()
                        .color(self.palette.text_strong)
                        .width(shape::BORDER_FOCUS)
                        .rounded(THUMB_SIZE / 2.0),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(Color::from(self.model.value)),
            );
        }

        // -------------------------------------------------------------------
        // 2D. Validated Hex Input & Preview Well Row (§3.12)
        // -------------------------------------------------------------------
        draw_preview_well(
            renderer,
            flyout.hex_swatch_bounds,
            Color::from(self.model.value),
            &self.palette,
        );

        let hex_border_color = if self.model.validation_error.is_some() {
            self.palette.status_danger_text
        } else if state.focused_hex {
            self.palette.accent_fill
        } else {
            self.palette.border_subtle
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: flyout.hex_input_bounds,
                border: Border::default()
                    .color(hex_border_color)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(self.palette.surface_active),
        );

        let hex_display = if state.focused_hex
            && let Some(ref buf) = state.hex_buffer
        {
            buf.clone()
        } else {
            self.model.hex_input.clone()
        };

        renderer.fill_text(
            text_core::Text {
                content: hex_display,
                bounds: flyout.hex_input_bounds.size(),
                size: Pixels(typography::BODY.size),
                line_height: text_core::LineHeight::Absolute(Pixels(
                    flyout.hex_input_bounds.height,
                )),
                font: iced::Font::MONOSPACE,
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            flyout.hex_input_bounds.center(),
            self.palette.text_strong,
            *viewport,
        );

        // -------------------------------------------------------------------
        // 2E. Preset Theme Swatches Grid (24x24px swatches) (§3.12, UIF-ATOM-012)
        // -------------------------------------------------------------------
        for (i, &swatch_rect) in flyout.swatches.iter().enumerate() {
            if i < self.model.preset_swatches.len() {
                let swatch = self.model.preset_swatches[i];
                draw_preview_well(renderer, swatch_rect, Color::from(swatch), &self.palette);

                // Highlight selection or hover ring
                if swatch == self.model.value {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: swatch_rect,
                            border: Border::default()
                                .color(self.palette.accent_border)
                                .width(shape::BORDER_FOCUS)
                                .rounded(shape::RADIUS_WIDGET),
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        Background::Color(Color::TRANSPARENT),
                    );
                } else if state.hovered_swatch == Some(i) {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: swatch_rect,
                            border: Border::default()
                                .color(self.palette.text_muted)
                                .width(shape::BORDER_HAIRLINE)
                                .rounded(shape::RADIUS_WIDGET),
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        Background::Color(Color::TRANSPARENT),
                    );
                }
            }
        }

        // -------------------------------------------------------------------
        // 2F. Live WCAG 2.1 Contrast Readout Semantic Chip (§3.12, UIF-TOK-006)
        // -------------------------------------------------------------------
        if let (Some(contrast_bounds), Some(reference)) =
            (flyout.contrast_bounds, self.model.contrast_reference)
        {
            let ratio = contrast_ratio(Color::from(self.model.value), Color::from(reference));
            let (icon, badge_text, status_color) = if ratio >= 7.0 {
                (ICON_PASS, BADGE_AAA, self.palette.status_success)
            } else if ratio >= 4.5 {
                (ICON_PASS, BADGE_AA, self.palette.status_success)
            } else {
                (ICON_WARN, BADGE_FAIL, self.palette.status_danger_text)
            };

            let chip_label = format!("{icon} {ratio:.2}:1 {badge_text}");

            renderer.fill_quad(
                renderer::Quad {
                    bounds: contrast_bounds,
                    border: Border::default()
                        .color(status_color)
                        .width(shape::BORDER_HAIRLINE)
                        .rounded(shape::RADIUS_WIDGET),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(self.palette.surface_raised),
            );

            renderer.fill_text(
                text_core::Text {
                    content: chip_label,
                    bounds: contrast_bounds.size(),
                    size: Pixels(typography::CAPTION.size),
                    line_height: text_core::LineHeight::Absolute(Pixels(contrast_bounds.height)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                contrast_bounds.center(),
                status_color,
                *viewport,
            );
        }

        // -------------------------------------------------------------------
        // 2G. Footer Action Buttons ([Cancel], [✓ Apply]) (§3.12, UIF-PRIN-005)
        // -------------------------------------------------------------------
        // Cancel Button
        let cancel_fill = if state.hovered_cancel {
            self.palette.surface_hover
        } else {
            self.palette.surface_active
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: flyout.cancel_button_bounds,
                border: Border::default()
                    .color(self.palette.border_subtle)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(cancel_fill),
        );
        renderer.fill_text(
            text_core::Text {
                content: LABEL_CANCEL.to_owned(),
                bounds: flyout.cancel_button_bounds.size(),
                size: Pixels(typography::CAPTION.size),
                line_height: text_core::LineHeight::Absolute(Pixels(
                    flyout.cancel_button_bounds.height,
                )),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            flyout.cancel_button_bounds.center(),
            self.palette.text_muted,
            *viewport,
        );

        // Apply Button
        let apply_fill = if state.hovered_apply {
            self.palette.accent_fill_active
        } else {
            self.palette.accent_fill
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: flyout.apply_button_bounds,
                border: Border::default()
                    .color(self.palette.accent_border)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(apply_fill),
        );
        renderer.fill_text(
            text_core::Text {
                content: LABEL_APPLY.to_owned(),
                bounds: flyout.apply_button_bounds.size(),
                size: Pixels(typography::CAPTION.size),
                line_height: text_core::LineHeight::Absolute(Pixels(
                    flyout.apply_button_bounds.height,
                )),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            flyout.apply_button_bounds.center(),
            self.palette.text_strong,
            *viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<ColorPicker<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(picker: ColorPicker<'a, Message>) -> Self {
        Element::new(picker)
    }
}

#[cfg(test)]
#[path = "color_picker_tests.rs"]
mod color_picker_tests;
