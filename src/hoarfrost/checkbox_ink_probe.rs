//! Empirical probe for where the `Checkbox` mark's ink **actually lands**,
//! the sibling radio dot and switch thumb likewise, plus a glyph-coverage
//! gate over every glyph constant the framework emits (`UIF-ATOM-003`, §3.3).
//!
//! # Why this module exists
//!
//! Layout bounds cannot answer where a glyph was painted, and neither can a
//! style resolution: `toggle_widget::Checkbox::draw` asks for the mark to
//! be centred in a 16×16 indicator box, and the text stack is free to lay
//! out at exactly that size while putting the ink somewhere else. This
//! module therefore reads framebuffers and font tables rather than
//! intentions. It measures; it does not redesign.
//!
//! # The three measurements
//!
//! 1. **The indicator box** — self-calibrated from the *unchecked* frame.
//!    An unchecked checkbox paints exactly one thing (the 16×16 indicator
//!    quad: no mark, no label, no focus ring), so the ink bounding box of
//!    that frame *is* the box, in physical pixels, with no reliance on the
//!    layout tree.
//! 2. **The mark's ink** — isolated by rendering the mark layer alone.
//!    Diffing a checked frame against an unchecked one does **not** isolate
//!    the mark, because the polarity flip repaints the whole box fill; the
//!    probe instead replays the exact `fill_text` call
//!    `Checkbox::draw` makes ([`MarkProbe`]) over a plain background, at
//!    the origin measured from the real widget's own box.
//! 3. **Glyph coverage** — a cmap read of the very bytes iced embeds as
//!    "Fira Sans" ([`present_in_render_font`]), cross-checked against what
//!    cosmic-text's fallback chain actually resolves each codepoint to
//!    ([`resolving_face`]), and rolled up per constant by
//!    [`missing_glyphs`].
//! 4. **The sibling marks** — the radio's inner dot and the switch's thumb
//!    ([`measure_radio_dot`], [`measure_switch_thumb`]), measured the same
//!    way. They are here because they are drawn a few lines from the
//!    checkbox mark and a reader is entitled to know whether the same
//!    displacement reaches them. It does not, and these say so with pixels
//!    rather than by inspection: they are `fill_quad` geometry, so no text
//!    anchor is involved.
//! 5. **The symbol-face ledger (`G-6`)** — every symbol codepoint this
//!    crate emits, classified as typographic, icon or test fixture, each
//!    with its guarantee status read live from the cmap, plus the measured
//!    candidate-face matrix. It lives here rather than in a module of its
//!    own because it is the same question the cmap reader already answers
//!    — "does the render font have this glyph?" — asked of the whole
//!    inventory at once. See the
//!    `The symbol-face ledger (G-6)` section near the end of this file for
//!    the findings and what they decide.
//!
//! # Committed-test safety
//!
//! Nothing here touches a filesystem path to find the font. The cmap is
//! read from `iced::advanced::graphics::text::FIRA_SANS_REGULAR`, which
//! is `include_bytes!`-embedded in `iced_graphics` and is the exact blob
//! iced registers into the render `FontSystem`. The source scanner walks
//! `env!("CARGO_MANIFEST_DIR")`, i.e. this crate's own committed sources.
//! Both work on a clean checkout with no cargo registry present.
//!
//! Unit tests live in the sibling `checkbox_ink_probe_tests.rs` sidecar.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::binding::Controlled;
use crate::test_helpers::Capture;
use crate::widget::toggle::{self, ToggleControl};
use crate::widget::toggle_model::{CheckState, RadioGroupModel, RadioOption};
use crate::widget::toggle_widget::{
    CHECK_MARK, Checkbox, INDETERMINATE_MARK, RadioGroup, ToggleSwitch,
};

use iced::advanced::graphics;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight};
use iced::advanced::widget::{Tree, Widget};
use iced::mouse;
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Size, alignment};

// ---------------------------------------------------------------------------
// Probe configuration
// ---------------------------------------------------------------------------

/// A background no hoarfrost token can produce.
///
/// Pure magenta is outside every colour the framework paints, so a pixel
/// that is not this colour is ink, with no threshold guesswork.
pub const PROBE_BACKGROUND: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// Physical pixels per logical pixel.
///
/// Four, so a glyph's landing point resolves to a quarter of a logical
/// pixel rather than being rounded away.
pub const PROBE_SCALE: f32 = 4.0;

/// The probe viewport — big enough that ink thrown well outside the
/// 16×16 indicator still lands inside the frame.
pub const PROBE_VIEWPORT: Size = Size::new(120.0, 80.0);

/// The largest per-channel distance from a reference colour that still
/// counts as *that* colour.
///
/// Small enough that antialiasing halos register as ink and only exact
/// fills register as background.
pub const COLOR_TOLERANCE: i32 = 4;

/// The `Checkbox` indicator box, in logical pixels.
pub const INDICATOR_BOX: f32 = 16.0;

/// The probe's indicator box as a [`Size`].
#[must_use]
pub fn indicator_box_size() -> Size {
    Size::new(INDICATOR_BOX, INDICATOR_BOX)
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// An inclusive-exclusive bounding box in **physical** pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BBox {
    /// Left edge, inclusive.
    pub x0: u32,
    /// Top edge, inclusive.
    pub y0: u32,
    /// Right edge, exclusive.
    pub x1: u32,
    /// Bottom edge, exclusive.
    pub y1: u32,
}

/// The same box in **logical** pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalBox {
    /// Left edge.
    pub x0: f32,
    /// Top edge.
    pub y0: f32,
    /// Right edge.
    pub x1: f32,
    /// Bottom edge.
    pub y1: f32,
}

impl BBox {
    /// Width in physical pixels.
    #[must_use]
    pub fn w(self) -> u32 {
        self.x1.saturating_sub(self.x0)
    }

    /// Height in physical pixels.
    #[must_use]
    pub fn h(self) -> u32 {
        self.y1.saturating_sub(self.y0)
    }

    /// The same box in logical pixels at `scale`.
    #[must_use]
    pub fn logical(self, scale: f32) -> LogicalBox {
        LogicalBox {
            x0: self.x0 as f32 / scale,
            y0: self.y0 as f32 / scale,
            x1: self.x1 as f32 / scale,
            y1: self.y1 as f32 / scale,
        }
    }
}

impl LogicalBox {
    /// Width in logical pixels.
    #[must_use]
    pub fn w(self) -> f32 {
        self.x1 - self.x0
    }

    /// Height in logical pixels.
    #[must_use]
    pub fn h(self) -> f32 {
        self.y1 - self.y0
    }

    /// The box's centre, in logical pixels.
    ///
    /// Centring is asked about the centre rather than the gaps because a
    /// glyph's ink is not symmetric inside its own line box: `✓` sits high
    /// in its line, so a *correctly* centred mark still shows unequal
    /// top and bottom gaps. The centre is the quantity the paint call
    /// actually controls.
    #[must_use]
    pub fn center(self) -> Point {
        Point::new((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }
}

// ---------------------------------------------------------------------------
// Framebuffer helpers
// ---------------------------------------------------------------------------

/// The captured frame every probe reader works from.
///
/// This is [`crate::test_helpers::Frame`] rather than a probe-local type: the
/// shared frame is what [`Capture`] hands back, so the probe reads the same
/// object the harness rasterised instead of a copy wrapped in a second
/// scale convention.
///
/// The shared frame reads in **logical** coordinates; every reader below is
/// written in **physical** ones, because the boxes this probe reports
/// ([`BBox`]) are physical and exact. [`rgb`] is the one place the two
/// conventions meet.
pub use crate::test_helpers::Frame;

/// The eight-bit form of a colour's channels, as integers wide enough to
/// difference without overflow.
fn channels(color: Color) -> [i32; 3] {
    [
        (color.r * 255.0).round() as i32,
        (color.g * 255.0).round() as i32,
        (color.b * 255.0).round() as i32,
    ]
}

/// The largest per-channel distance between two RGB triples.
fn channel_delta(a: [i32; 3], b: [i32; 3]) -> i32 {
    (a[0] - b[0])
        .abs()
        .max((a[1] - b[1]).abs())
        .max((a[2] - b[2]).abs())
}

/// The RGB triple at physical `(x, y)`.
///
/// The shared frame indexes by logical coordinate, so the physical index is
/// divided back out at [`PROBE_SCALE`]. Both sides of that round trip are
/// powers of two, so no pixel is shifted by rounding.
fn rgb(frame: &Frame, x: u32, y: u32) -> [i32; 3] {
    let pixel = frame.pixel(x as f32 / PROBE_SCALE, y as f32 / PROBE_SCALE);

    [pixel[0] as i32, pixel[1] as i32, pixel[2] as i32]
}

/// The physical width and height a probe frame covers.
fn physical(frame: &Frame) -> (u32, u32) {
    let size = frame.physical_size();

    (size.width, size.height)
}

/// Every pixel that is not `background`, as a row-major mask.
#[must_use]
pub fn mask_against(frame: &Frame, background: Color) -> Vec<bool> {
    let bg = channels(background);
    let (width, height) = physical(frame);
    let mut mask = vec![false; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            if channel_delta(rgb(frame, x, y), bg) > COLOR_TOLERANCE {
                mask[(y * width + x) as usize] = true;
            }
        }
    }
    mask
}

/// The bounding box of every pixel that is not `background`.
///
/// `None` when the frame carries no ink at all.
#[must_use]
pub fn ink_bbox(frame: &Frame, background: Color) -> Option<BBox> {
    let (width, _) = physical(frame);
    bbox_of(&mask_against(frame, background), width)
}

/// The number of ink pixels against `background`.
#[must_use]
pub fn ink_count(frame: &Frame, background: Color) -> usize {
    mask_against(frame, background)
        .iter()
        .filter(|p| **p)
        .count()
}

/// The bounding box of the `true` cells in a row-major mask.
fn bbox_of(mask: &[bool], width: u32) -> Option<BBox> {
    let mut x0 = u32::MAX;
    let mut y0 = u32::MAX;
    let mut x1 = 0u32;
    let mut y1 = 0u32;

    for (i, cell) in mask.iter().enumerate() {
        if !*cell {
            continue;
        }
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + 1);
        y1 = y1.max(y + 1);
    }

    if x1 == 0 || y1 == 0 {
        return None;
    }
    Some(BBox { x0, y0, x1, y1 })
}

/// An ASCII rendering of a mask, `block` physical pixels per character.
///
/// With `block == scale` each character is one logical pixel, which is the
/// resolution the containment question is asked at.
#[must_use]
pub fn ascii_art(mask: &[bool], width: u32, crop: BBox, block: u32) -> String {
    const RAMP: [char; 4] = [' ', '.', '+', '#'];

    let mut out = String::new();
    let mut y = crop.y0;
    while y < crop.y1 {
        let mut row = String::new();
        let mut x = crop.x0;
        while x < crop.x1 {
            let mut on = 0usize;
            let mut total = 0usize;
            for dy in 0..block {
                for dx in 0..block {
                    let py = y + dy;
                    let px = x + dx;
                    if py < crop.y1 && px < crop.x1 {
                        total += 1;
                        if mask[(py * width + px) as usize] {
                            on += 1;
                        }
                    }
                }
            }
            let frac = if total == 0 {
                0.0
            } else {
                on as f32 / total as f32
            };
            let idx = if frac <= 0.0 {
                0
            } else if frac < 0.34 {
                1
            } else if frac < 0.67 {
                2
            } else {
                3
            };
            row.push(RAMP[idx]);
            x += block;
        }
        out.push_str(&row);
        out.push('\n');
        y += block;
    }
    out
}

// ---------------------------------------------------------------------------
// Rendering the widget under test
// ---------------------------------------------------------------------------

/// Centre `inner` in the probe viewport so nothing that escapes the
/// indicator can be clipped by a frame edge.
///
/// Borrowing in and borrowing out, so a controlled widget holding a model
/// that lives on the caller's stack can be centred without being pinned to
/// `'static` first.
fn centered<'a>(inner: Element<'a, ()>) -> Element<'a, ()> {
    iced::widget::container(inner)
        .width(Length::Fixed(PROBE_VIEWPORT.width))
        .height(Length::Fixed(PROBE_VIEWPORT.height))
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .into()
}

/// Rasterise `root` at the probe's viewport, scale and background.
///
/// Every probe render goes through here so the viewport, the 4× scale and
/// the magenta background cannot drift between one frame and the next.
fn capture(cap: &mut Capture, root: Element<'_, ()>) -> Frame {
    cap.frame_at(PROBE_VIEWPORT, PROBE_SCALE, PROBE_BACKGROUND, root)
}

/// Render a hoarfrost `Checkbox` alone in `state`, on `cap`.
///
/// The label is empty so the frame carries the indicator quad, the mark
/// and nothing else.
fn checkbox_frame(cap: &mut Capture, state: CheckState) -> Frame {
    let checkbox = Checkbox::standalone("", state, "hoarfrost_checkbox_ink_probe", |_| None);

    capture(cap, centered(checkbox.into()))
}

/// Render a hoarfrost `Checkbox` alone in `state`.
#[must_use]
pub fn render_checkbox(state: CheckState) -> Frame {
    let mut cap = Capture::new();

    checkbox_frame(&mut cap, state)
}

/// Render `glyph` through **iced's own `text` widget**, centred in a
/// `box_size` box whose top-left is pinned at `origin`.
///
/// This is the *reference* placement. iced's `text` widget applies its own
/// alignment offset before painting, so where it puts the ink is where a
/// correctly-centred mark would be. Comparing the checkbox's raw
/// `fill_text` result against this separates "the glyph is odd" from "the
/// call site is off", and it does so for the same glyph, size, line height
/// and colour.
#[must_use]
pub fn render_iced_text(
    glyph: &str,
    origin: Point,
    box_size: Size,
    size: f32,
    line_height: f32,
    color: Color,
) -> Frame {
    let mut cap = Capture::new();

    iced_text_frame(&mut cap, glyph, origin, box_size, size, line_height, color)
}

/// [`render_iced_text`] driven by a caller-owned capture session.
fn iced_text_frame(
    cap: &mut Capture,
    glyph: &str,
    origin: Point,
    box_size: Size,
    size: f32,
    line_height: f32,
    color: Color,
) -> Frame {
    let txt = iced::widget::text(glyph.to_owned())
        .size(Pixels(size))
        .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
            line_height,
        )))
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .width(Length::Fixed(box_size.width))
        .height(Length::Fixed(box_size.height))
        .color(color);

    let root: Element<'_, ()> = iced::widget::container(txt)
        .width(Length::Fixed(PROBE_VIEWPORT.width))
        .height(Length::Fixed(PROBE_VIEWPORT.height))
        .align_x(alignment::Horizontal::Left)
        .align_y(alignment::Vertical::Top)
        .padding(iced::Padding {
            top: origin.y,
            right: 0.0,
            bottom: 0.0,
            left: origin.x,
        })
        .into();

    capture(cap, root)
}

/// Paints **only** the mark layer `Checkbox::draw` paints.
///
/// The `text_core::Text` below is a line-for-line copy of the call in
/// `widget/toggle_widget.rs::Checkbox::draw` — same `bounds`, same `size`,
/// same `LineHeight::Absolute(Pixels(indicator.height))`, same centring,
/// and the same [`toggle::mark_anchor`] position — with the indicator quad
/// removed. Replaying it over a plain background is what isolates the
/// mark's ink: the checked/unchecked frame diff cannot, because the
/// polarity flip repaints the whole box fill.
///
/// The anchor comes from the widget's own helper rather than a hand-written
/// point, so the replay cannot drift from the call it is meant to mirror.
pub struct MarkProbe {
    glyph: String,
    origin: Point,
    box_size: Size,
    mark_size: f32,
    color: Color,
}

impl MarkProbe {
    /// Build a mark-only probe for `glyph`, drawn as if the indicator box
    /// sat at `origin` with size `box_size`.
    #[must_use]
    pub fn new(glyph: &str, origin: Point, box_size: Size, mark_size: f32, color: Color) -> Self {
        Self {
            glyph: glyph.to_owned(),
            origin,
            box_size,
            mark_size,
            color,
        }
    }
}

impl<Message, Renderer> Widget<Message, iced::Theme, Renderer> for MarkProbe
where
    Renderer: renderer::Renderer + text_core::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let indicator = Rectangle::new(self.origin, self.box_size);
        renderer.fill_text(
            text_core::Text {
                content: self.glyph.clone(),
                bounds: self.box_size,
                size: Pixels(self.mark_size),
                line_height: LineHeight::Absolute(Pixels(self.box_size.height)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            toggle::mark_anchor(indicator),
            self.color,
            *viewport,
        );
    }
}

/// Render `glyph` alone, drawn exactly as the checkbox mark layer draws it,
/// with the indicator box's top-left pinned at `origin`.
#[must_use]
pub fn render_mark_alone(glyph: &str, origin: Point, mark_size: f32, color: Color) -> Frame {
    let mut cap = Capture::new();

    mark_alone_frame(&mut cap, glyph, origin, mark_size, color)
}

/// [`render_mark_alone`] driven by a caller-owned capture session.
fn mark_alone_frame(
    cap: &mut Capture,
    glyph: &str,
    origin: Point,
    mark_size: f32,
    color: Color,
) -> Frame {
    let probe = MarkProbe::new(glyph, origin, indicator_box_size(), mark_size, color);

    capture(cap, Element::new(probe))
}

// ---------------------------------------------------------------------------
// The mark-vs-box report
// ---------------------------------------------------------------------------

/// One mark's placement relative to the 16×16 indicator box, in **logical**
/// pixels. Every figure is signed so the direction is in the number.
#[derive(Debug, Clone, Copy)]
pub struct MarkPlacement {
    /// The indicator box, in logical pixels.
    pub box_logical: LogicalBox,
    /// The mark's ink, in logical pixels.
    pub mark_logical: LogicalBox,
    /// How far the mark's left edge sits left of the box's left edge.
    /// Positive = escapes; negative = inset.
    pub over_left: f32,
    /// How far the mark's top edge sits above the box's top edge.
    pub over_top: f32,
    /// How far the mark's right edge sits right of the box's right edge.
    pub over_right: f32,
    /// How far the mark's bottom edge sits below the box's bottom edge.
    pub over_bottom: f32,
    /// Clear space between the box top and the mark's top.
    pub top_gap: f32,
    /// Clear space between the mark's bottom and the box bottom.
    pub bottom_gap: f32,
    /// Clear space between the box left and the mark's left.
    pub left_gap: f32,
    /// Clear space between the mark's right and the box right.
    pub right_gap: f32,
    /// Asymmetry of the vertical gaps (positive = more space on top).
    pub vertical_gap_imbalance: f32,
    /// Whether any of the four overhangs is positive.
    pub escapes: bool,
    /// Number of ink pixels the mark painted.
    pub ink_pixels: usize,
}

impl MarkPlacement {
    /// Compare a mark's ink box against the indicator box.
    #[must_use]
    pub fn measure(box_logical: LogicalBox, mark_logical: LogicalBox, ink_pixels: usize) -> Self {
        let over_left = box_logical.x0 - mark_logical.x0;
        let over_top = box_logical.y0 - mark_logical.y0;
        let over_right = mark_logical.x1 - box_logical.x1;
        let over_bottom = mark_logical.y1 - box_logical.y1;

        Self {
            box_logical,
            mark_logical,
            over_left,
            over_top,
            over_right,
            over_bottom,
            top_gap: mark_logical.y0 - box_logical.y0,
            bottom_gap: box_logical.y1 - mark_logical.y1,
            left_gap: mark_logical.x0 - box_logical.x0,
            right_gap: box_logical.x1 - mark_logical.x1,
            vertical_gap_imbalance: (mark_logical.y0 - box_logical.y0)
                - (box_logical.y1 - mark_logical.y1),
            escapes: over_left > 0.0 || over_top > 0.0 || over_right > 0.0 || over_bottom > 0.0,
            ink_pixels,
        }
    }

    /// The largest single-direction overhang.
    #[must_use]
    pub fn max_overhang(self) -> f32 {
        self.over_left
            .max(self.over_top)
            .max(self.over_right)
            .max(self.over_bottom)
    }

    /// The mark's centre minus the box's centre, in logical pixels.
    ///
    /// The centring question stated as one pair of numbers. Unlike the gap
    /// imbalance it is unaffected by where a glyph's ink sits inside its
    /// own line box, so it is the figure to compare against a reference
    /// placement.
    #[must_use]
    pub fn centre_offset(self) -> (f32, f32) {
        let mark = self.mark_logical.center();
        let box_center = self.box_logical.center();
        (mark.x - box_center.x, mark.y - box_center.y)
    }

    /// A one-line, human-readable summary of the placement.
    #[must_use]
    pub fn summary(self, name: &str) -> String {
        format!(
            "{name}: box=[{:.3},{:.3} .. {:.3},{:.3}] ({:.1}×{:.1})  \
             mark=[{:.3},{:.3} .. {:.3},{:.3}] ({:.1}×{:.1})  ink_px={}  \
             over(L,T,R,B)=({:+.3},{:+.3},{:+.3},{:+.3})  \
             gaps(T,B)=({:.3},{:.3}) L,R=({:.3},{:.3})  v_imbalance={:+.3}  escapes={}",
            self.box_logical.x0,
            self.box_logical.y0,
            self.box_logical.x1,
            self.box_logical.y1,
            self.box_logical.w(),
            self.box_logical.h(),
            self.mark_logical.x0,
            self.mark_logical.y0,
            self.mark_logical.x1,
            self.mark_logical.y1,
            self.mark_logical.w(),
            self.mark_logical.h(),
            self.ink_pixels,
            self.over_left,
            self.over_top,
            self.over_right,
            self.over_bottom,
            self.top_gap,
            self.bottom_gap,
            self.left_gap,
            self.right_gap,
            self.vertical_gap_imbalance,
            self.escapes,
        )
    }
}

/// The whole checkbox ink picture: the box as painted, and each mark as
/// painted, all in logical pixels.
#[derive(Debug, Clone)]
pub struct CheckboxInkReport {
    /// The indicator box, measured from the unchecked frame.
    pub box_logical: LogicalBox,
    /// The unchecked frame's ink bbox (should be the box, and nothing more).
    pub unchecked_ink: Option<BBox>,
    /// The checked frame's total ink bbox (box + mark).
    pub checked_ink: Option<BBox>,
    /// The indeterminate frame's total ink bbox (box + dash).
    pub indeterminate_ink: Option<BBox>,
    /// The `✓` mark, classified out of the **real** checked frame.
    pub checked_mark: MarkPlacement,
    /// The `−` mark, classified out of the **real** indeterminate frame
    /// (positive control).
    pub indeterminate_mark: MarkPlacement,
    /// The `✓` mark replayed in isolation, as a cross-check.
    pub checked_mark_replay: MarkPlacement,
    /// The `−` mark replayed in isolation, as a cross-check.
    pub indeterminate_mark_replay: MarkPlacement,
    /// Where **iced's own `text` widget** lands the `✓` in the same box at
    /// the same size and line height — the acceptance reference.
    pub checked_reference: LogicalBox,
    /// Where **iced's own `text` widget** lands the `−` in the same box.
    pub indeterminate_reference: LogicalBox,
    /// Mark ink in the real checked frame that lies **outside** the box.
    pub checked_escape_bbox: Option<BBox>,
    /// Mark ink in the real indeterminate frame that lies **outside** the box.
    pub indeterminate_escape_bbox: Option<BBox>,
    /// ASCII art of the isolated `✓` mark, one character per logical pixel.
    pub checked_art: String,
    /// ASCII art of the isolated `−` mark, one character per logical pixel.
    pub indeterminate_art: String,
    /// ASCII art of the **real** unchecked checkbox frame (the box alone).
    pub unchecked_frame_art: String,
    /// ASCII art of the **real** checked checkbox frame (box + `✓`).
    pub checked_frame_art: String,
    /// ASCII art of the **real** indeterminate checkbox frame (box + `−`).
    pub indeterminate_frame_art: String,
}

/// The physical-pixel antialiasing band a solid quad paints inside and
/// outside its own bounds.
///
/// Two physical pixels (half a logical one) is generous for a `fill_quad`
/// at scale 4; anything beyond that band is not the box's own edge.
pub const BOX_AA_MARGIN_PHYS: u32 = 2;

/// The largest off-segment residual still counted as a two-colour blend.
pub const RESIDUAL_MAX: f32 = 24.0;

/// Where `px` sits along the `from → to` colour segment, if it lies on it.
///
/// Returns `Some(t)` with `t` in `0..=1` when the pixel is a blend of
/// `from` and `to`, and `None` when it is a blend with some third colour —
/// e.g. the box/background antialiasing at a rounded corner, which must
/// not be mistaken for mark ink.
fn blend_fraction(px: [i32; 3], from: [i32; 3], to: [i32; 3]) -> Option<f32> {
    let v = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let p = [px[0] - from[0], px[1] - from[1], px[2] - from[2]];
    let vv = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if vv == 0 {
        return None;
    }
    let t = (p[0] * v[0] + p[1] * v[1] + p[2] * v[2]) as f32 / vv as f32;
    let res = [
        p[0] as f32 - t * v[0] as f32,
        p[1] as f32 - t * v[1] as f32,
        p[2] as f32 - t * v[2] as f32,
    ];
    let residual = (res[0] * res[0] + res[1] * res[1] + res[2] * res[2]).sqrt();
    if residual > RESIDUAL_MAX {
        return None;
    }
    Some(t.clamp(0.0, 1.0))
}

/// Isolate the mark's ink inside a real checkbox frame.
///
/// Inside the box a pixel counts as mark ink only when it lies on the
/// `fill → mark` segment. That excludes the quad's own rounded-corner
/// antialiasing, which blends toward the frame background and is *not* on
/// that segment. Outside the box and its antialiasing band, anything that
/// is not background is mark ink that escaped.
#[must_use]
pub fn mark_ink_in_frame(
    frame: &Frame,
    box_bb: BBox,
    fill: Color,
    mark: Color,
    aa_margin: u32,
) -> Vec<bool> {
    let fill = channels(fill);
    let mark = channels(mark);
    let bg = channels(PROBE_BACKGROUND);
    let (width, height) = physical(frame);

    let inner = BBox {
        x0: box_bb.x0.saturating_add(aa_margin),
        y0: box_bb.y0.saturating_add(aa_margin),
        x1: box_bb.x1.saturating_sub(aa_margin).min(width),
        y1: box_bb.y1.saturating_sub(aa_margin).min(height),
    };
    let outer = BBox {
        x0: box_bb.x0.saturating_sub(aa_margin),
        y0: box_bb.y0.saturating_sub(aa_margin),
        x1: (box_bb.x1 + aa_margin).min(width),
        y1: (box_bb.y1 + aa_margin).min(height),
    };

    let mut mask = vec![false; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let px = rgb(frame, x, y);
            let inside_box = x >= inner.x0 && x < inner.x1 && y >= inner.y0 && y < inner.y1;
            let inside_outer = x >= outer.x0 && x < outer.x1 && y >= outer.y0 && y < outer.y1;

            let is_mark = if inside_box {
                blend_fraction(px, fill, mark).is_some_and(|t| t > 0.02)
            } else if !inside_outer {
                // Outside the box and its AA band: anything not background.
                channel_delta(px, bg) > COLOR_TOLERANCE
            } else {
                // The AA band between box and background is the box's own
                // edge; it says nothing about the mark.
                false
            };

            mask[(y * width + x) as usize] = is_mark;
        }
    }
    mask
}

/// The part of a mark mask that lies outside the indicator box's AA band.
fn outside_box(mask: &[bool], frame: &Frame, box_bb: BBox, aa_margin: u32) -> Option<BBox> {
    let (width, height) = physical(frame);
    let outer = BBox {
        x0: box_bb.x0.saturating_sub(aa_margin),
        y0: box_bb.y0.saturating_sub(aa_margin),
        x1: box_bb.x1 + aa_margin,
        y1: box_bb.y1 + aa_margin,
    };
    let mut clipped = mask.to_vec();
    for y in 0..height {
        for x in 0..width {
            if x >= outer.x0 && x < outer.x1 && y >= outer.y0 && y < outer.y1 {
                clipped[(y * width + x) as usize] = false;
            }
        }
    }
    bbox_of(&clipped, width)
}

/// Measure every state of the checkbox and produce the report.
///
/// `mark_size` is the logical font size the widget asks for the mark
/// (`CHECKBOX_MARK_SIZE`); `mark_color` and `box_fill` are the colours
/// `toggle::resolve_style` hands the paint path.
///
/// The whole report is rasterised on **one** capture session. It needs
/// seven frames — three checkbox states, two isolated mark replays and two
/// iced `text` references — and each frame costs under a millisecond once
/// the renderer exists but the full build cost when it does not.
#[must_use]
pub fn measure_checkbox_ink(
    mark_size: f32,
    mark_color: Color,
    box_fill: Color,
) -> CheckboxInkReport {
    let mut cap = Capture::new();

    measure_checkbox_ink_on(&mut cap, mark_size, mark_color, box_fill)
}

/// [`measure_checkbox_ink`] driven by a caller-owned capture session.
fn measure_checkbox_ink_on(
    cap: &mut Capture,
    mark_size: f32,
    mark_color: Color,
    box_fill: Color,
) -> CheckboxInkReport {
    let (token_w, token_h) = toggle::indicator_size(ToggleControl::Checkbox);
    assert!(
        (token_w - INDICATOR_BOX).abs() <= f32::EPSILON
            && (token_h - INDICATOR_BOX).abs() <= f32::EPSILON,
        "probe constant INDICATOR_BOX ({INDICATOR_BOX}) no longer matches the widget's own \
         size::CHECKBOX_BOX ({token_w}x{token_h}); the probe would be grading a box the \
         widget does not paint"
    );

    let unchecked = checkbox_frame(cap, CheckState::Unchecked);
    let checked = checkbox_frame(cap, CheckState::Checked);
    let indeterminate = checkbox_frame(cap, CheckState::Indeterminate);

    let unchecked_ink = ink_bbox(&unchecked, PROBE_BACKGROUND)
        .expect("an unchecked checkbox must still paint its indicator box");

    // The unchecked frame paints exactly the indicator quad, so its ink
    // bbox is the box — no layout tree involved.
    let box_logical = unchecked_ink.logical(PROBE_SCALE);
    let frame_width = physical(&unchecked).0;

    // --- the mark, classified out of the real frames -------------------
    let checked_mask = mark_ink_in_frame(
        &checked,
        unchecked_ink,
        box_fill,
        mark_color,
        BOX_AA_MARGIN_PHYS,
    );
    let indeterminate_mask = mark_ink_in_frame(
        &indeterminate,
        unchecked_ink,
        box_fill,
        mark_color,
        BOX_AA_MARGIN_PHYS,
    );

    let checked_mark_bbox = bbox_of(&checked_mask, frame_width).unwrap_or(unchecked_ink);
    let indeterminate_mark_bbox =
        bbox_of(&indeterminate_mask, frame_width).unwrap_or(unchecked_ink);

    // --- the same mark replayed in isolation, as a cross-check --------
    let origin = Point::new(box_logical.x0, box_logical.y0);
    let checked_alone = mark_alone_frame(cap, CHECK_MARK, origin, mark_size, mark_color);
    let indeterminate_alone =
        mark_alone_frame(cap, INDETERMINATE_MARK, origin, mark_size, mark_color);
    let checked_replay_bbox = ink_bbox(&checked_alone, PROBE_BACKGROUND).unwrap_or(unchecked_ink);
    let indeterminate_replay_bbox =
        ink_bbox(&indeterminate_alone, PROBE_BACKGROUND).unwrap_or(unchecked_ink);

    // --- iced's own `text` widget, as the acceptance reference --------
    // Same glyph, size, line height, colour and box. Where this lands is
    // where the widget's mark must land; it is the only placement that is
    // correct by construction rather than by assertion.
    let mut reference = |glyph: &str| -> LogicalBox {
        let frame = iced_text_frame(
            cap,
            glyph,
            origin,
            indicator_box_size(),
            mark_size,
            INDICATOR_BOX,
            mark_color,
        );

        ink_bbox(&frame, PROBE_BACKGROUND)
            .unwrap_or_else(|| panic!("iced's own `text` widget must paint {glyph}"))
            .logical(PROBE_SCALE)
    };
    let checked_reference = reference(CHECK_MARK);
    let indeterminate_reference = reference(INDETERMINATE_MARK);

    let art_box = BBox {
        x0: ((box_logical.x0 - 12.0) * PROBE_SCALE).max(0.0) as u32,
        y0: ((box_logical.y0 - 12.0) * PROBE_SCALE).max(0.0) as u32,
        x1: ((box_logical.x1 + 12.0) * PROBE_SCALE).min(PROBE_VIEWPORT.width * PROBE_SCALE) as u32,
        y1: ((box_logical.y1 + 12.0) * PROBE_SCALE).min(PROBE_VIEWPORT.height * PROBE_SCALE) as u32,
    };

    CheckboxInkReport {
        box_logical,
        unchecked_ink: Some(unchecked_ink),
        checked_ink: ink_bbox(&checked, PROBE_BACKGROUND),
        indeterminate_ink: ink_bbox(&indeterminate, PROBE_BACKGROUND),
        checked_mark: MarkPlacement::measure(
            box_logical,
            checked_mark_bbox.logical(PROBE_SCALE),
            checked_mask.iter().filter(|p| **p).count(),
        ),
        indeterminate_mark: MarkPlacement::measure(
            box_logical,
            indeterminate_mark_bbox.logical(PROBE_SCALE),
            indeterminate_mask.iter().filter(|p| **p).count(),
        ),
        checked_mark_replay: MarkPlacement::measure(
            box_logical,
            checked_replay_bbox.logical(PROBE_SCALE),
            ink_count(&checked_alone, PROBE_BACKGROUND),
        ),
        indeterminate_mark_replay: MarkPlacement::measure(
            box_logical,
            indeterminate_replay_bbox.logical(PROBE_SCALE),
            ink_count(&indeterminate_alone, PROBE_BACKGROUND),
        ),
        checked_reference,
        indeterminate_reference,
        checked_escape_bbox: outside_box(
            &checked_mask,
            &checked,
            unchecked_ink,
            BOX_AA_MARGIN_PHYS,
        ),
        indeterminate_escape_bbox: outside_box(
            &indeterminate_mask,
            &indeterminate,
            unchecked_ink,
            BOX_AA_MARGIN_PHYS,
        ),
        checked_art: ascii_art(
            &mask_against(&checked_alone, PROBE_BACKGROUND),
            frame_width,
            art_box,
            PROBE_SCALE as u32,
        ),
        indeterminate_art: ascii_art(
            &mask_against(&indeterminate_alone, PROBE_BACKGROUND),
            frame_width,
            art_box,
            PROBE_SCALE as u32,
        ),
        unchecked_frame_art: ascii_art(
            &mask_against(&unchecked, PROBE_BACKGROUND),
            frame_width,
            art_box,
            PROBE_SCALE as u32,
        ),
        checked_frame_art: ascii_art(
            &mask_against(&checked, PROBE_BACKGROUND),
            frame_width,
            art_box,
            PROBE_SCALE as u32,
        ),
        indeterminate_frame_art: ascii_art(
            &mask_against(&indeterminate, PROBE_BACKGROUND),
            frame_width,
            art_box,
            PROBE_SCALE as u32,
        ),
    }
}

// ---------------------------------------------------------------------------
// The sibling marks — the radio dot and the switch thumb
// ---------------------------------------------------------------------------

/// Render a single-option `RadioGroup` alone, that option selected or not.
///
/// One option with an empty label, so the frame carries the ring and —
/// when selected — the inner dot, and nothing else.
#[must_use]
pub fn render_radio(selected: bool) -> Frame {
    let mut cap = Capture::new();

    radio_frame(&mut cap, selected)
}

/// [`render_radio`] driven by a caller-owned capture session.
///
/// The model is a local the element borrows. It used to be parked in a
/// `static` because the one-frame screenshot helper demanded a `'static`
/// element; the capture session takes a borrowing one, so the model lives
/// and dies inside this call.
fn radio_frame(cap: &mut Capture, selected: bool) -> Frame {
    let mut model = RadioGroupModel::new(vec![RadioOption::new(0usize, "")]);
    if selected {
        model = model.with_selected(0);
    }

    let group =
        RadioGroup::controlled(Controlled::new(&model), "hoarfrost_radio_ink_probe", |_| {
            None
        });

    capture(cap, centered(group.into()))
}

/// Render a `ToggleSwitch` alone, in `on`.
///
/// No label, so the frame carries the track and the thumb only.
#[must_use]
pub fn render_switch(on: bool) -> Frame {
    let mut cap = Capture::new();

    switch_frame(&mut cap, on)
}

/// [`render_switch`] driven by a caller-owned capture session.
fn switch_frame(cap: &mut Capture, on: bool) -> Frame {
    let switch = ToggleSwitch::standalone("", on, "hoarfrost_switch_ink_probe", |_| None);

    capture(cap, centered(switch.into()))
}

/// The track box plus the thumb as painted in each position.
#[derive(Debug, Clone, Copy)]
pub struct SwitchMarks {
    /// The track, measured from the off frame's ink.
    pub track: LogicalBox,
    /// The thumb in the off position.
    pub off: MarkPlacement,
    /// The thumb in the on position.
    pub on: MarkPlacement,
}

/// A mask of pixels within `tolerance` per channel of `color`.
///
/// The right tool for a **solid quad**: the radio dot and the switch thumb
/// are painted as one flat colour, so asking "is this that colour" pins
/// them exactly. The `fill → mark` segment test the checkbox needs is the
/// wrong question here — a track border or any third colour that happens to
/// lie near that segment registers as ink, which is how a 14×14 thumb came
/// back measuring the whole track.
#[must_use]
pub fn solid_ink(frame: &Frame, color: Color, tolerance: i32) -> Vec<bool> {
    let target = channels(color);
    let (width, height) = physical(frame);
    let mut mask = vec![false; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            if channel_delta(rgb(frame, x, y), target) <= tolerance {
                mask[(y * width + x) as usize] = true;
            }
        }
    }
    mask
}

/// Per-channel distance that still counts a pixel as the mark's solid fill.
///
/// Wider than [`COLOR_TOLERANCE`] so the antialiased rim of a rounded thumb
/// is not shaved off, but far inside the distance between any two hoarfrost
/// tokens, so no neighbouring colour can be mistaken for it.
pub const SOLID_MARK_TOLERANCE: i32 = 8;

/// The bbox and pixel count of `color`'s solid ink in `frame`.
fn solid_span(frame: &Frame, color: Color) -> (LogicalBox, usize) {
    let mask = solid_ink(frame, color, SOLID_MARK_TOLERANCE);
    let (width, _) = physical(frame);
    let box_phys = bbox_of(&mask, width)
        .unwrap_or_else(|| panic!("no solid ink of {color:?} found in the frame"));
    (
        box_phys.logical(PROBE_SCALE),
        mask.iter().filter(|p| **p).count(),
    )
}

/// Where the radio's inner dot actually lands inside its ring.
///
/// `mark` is the resolved `On`-polarity colour the widget paints the dot
/// with; the ring is calibrated from the unselected frame's own ink. Both
/// frames come off one capture session.
#[must_use]
pub fn measure_radio_dot(mark: Color) -> MarkPlacement {
    let mut cap = Capture::new();

    let ring_bb = ink_bbox(&radio_frame(&mut cap, false), PROBE_BACKGROUND)
        .expect("an unselected radio must paint its ring");
    let (dot, ink) = solid_span(&radio_frame(&mut cap, true), mark);

    MarkPlacement::measure(ring_bb.logical(PROBE_SCALE), dot, ink)
}

/// Where the switch thumb actually lands in each position.
///
/// Both polarities are passed because the thumb is painted in both and is
/// **not** the same colour in both: the off thumb is `text.muted` on a
/// `surface.code` track, the on thumb is `text.strong` on accent. Isolating
/// one state's colour against the other state's frame would find nothing.
/// Both frames come off one capture session.
#[must_use]
pub fn measure_switch_thumb(on_mark: Color, off_mark: Color) -> SwitchMarks {
    let mut cap = Capture::new();

    let off = switch_frame(&mut cap, false);
    let track_bb = ink_bbox(&off, PROBE_BACKGROUND).expect("a switch must paint its track");
    let track = track_bb.logical(PROBE_SCALE);

    let (off_box, off_ink) = solid_span(&off, off_mark);
    let (on_box, on_ink) = solid_span(&switch_frame(&mut cap, true), on_mark);

    SwitchMarks {
        track,
        off: MarkPlacement::measure(track, off_box, off_ink),
        on: MarkPlacement::measure(track, on_box, on_ink),
    }
}

// ---------------------------------------------------------------------------
// The font cmap reader
// ---------------------------------------------------------------------------

/// Read a big-endian `u16` out of a font blob.
fn be16(b: &[u8], off: usize) -> Option<usize> {
    let v = b.get(off..off + 2)?;
    Some(u16::from_be_bytes([v[0], v[1]]) as usize)
}

/// Read a big-endian `u32` out of a font blob.
fn be32(b: &[u8], off: usize) -> Option<usize> {
    let v = b.get(off..off + 4)?;
    Some(u32::from_be_bytes([v[0], v[1], v[2], v[3]]) as usize)
}

/// The glyph id `cp` maps to in `font`'s cmap, or `None` when the codepoint
/// is absent (or maps to `.notdef`).
///
/// Handles format 4 (BMP) and format 12 (full Unicode), preferring the
/// wider subtable when both are present.
#[must_use]
pub fn glyph_id(font: &[u8], cp: char) -> Option<u16> {
    let num_tables = be16(font, 4)?;
    let mut cmap_off = None;
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if font.get(rec..rec + 4)? == b"cmap" {
            cmap_off = be32(font, rec + 8);
        }
    }
    let cmap = cmap_off?;
    let subtables = be16(font, cmap + 2)?;

    // (rank, subtable offset, format) — lower rank wins.
    let mut chosen: Option<(usize, usize, usize)> = None;
    for i in 0..subtables {
        let rec = cmap + 4 + i * 8;
        let platform = be16(font, rec)?;
        let encoding = be16(font, rec + 2)?;
        let off = cmap + be32(font, rec + 4)?;
        let format = be16(font, off)?;
        let rank = match (platform, encoding, format) {
            (_, _, 12) => 0,
            (3, 1, 4) | (0, 3, 4) | (0, 4, 4) => 1,
            (_, _, 4) => 2,
            _ => 9,
        };
        if chosen.is_none_or(|(best, _, _)| rank < best) {
            chosen = Some((rank, off, format));
        }
    }

    let (_, off, format) = chosen?;
    match format {
        4 => glyph_id_format4(font, off, cp),
        12 => glyph_id_format12(font, off, cp),
        _ => None,
    }
    .filter(|gid| *gid != 0)
}

/// cmap format 4 — the 16-bit BMP subtable.
fn glyph_id_format4(font: &[u8], off: usize, cp: char) -> Option<u16> {
    let cp = cp as usize;
    if cp > 0xFFFF {
        return None;
    }
    let seg_count = be16(font, off + 6)? / 2;
    let ends = off + 14;
    let starts = ends + seg_count * 2 + 2;
    let deltas = starts + seg_count * 2;
    let ranges = deltas + seg_count * 2;

    for i in 0..seg_count {
        let end = be16(font, ends + i * 2)?;
        if cp > end {
            continue;
        }
        let start = be16(font, starts + i * 2)?;
        if cp < start {
            continue;
        }
        let delta = be16(font, deltas + i * 2)? as u16;
        let range = be16(font, ranges + i * 2)?;
        if range == 0 {
            return Some((cp as u16).wrapping_add(delta));
        }
        let idx_at = ranges + i * 2 + range + (cp - start) * 2;
        let raw = be16(font, idx_at)? as u16;
        if raw == 0 {
            return None;
        }
        return Some(raw.wrapping_add(delta));
    }
    None
}

/// cmap format 12 — the full-Unicode segmented subtable.
fn glyph_id_format12(font: &[u8], off: usize, cp: char) -> Option<u16> {
    let groups = be32(font, off + 6)?;
    let base = off + 12;
    let c = cp as u32;
    for i in 0..groups {
        let g = base + i * 12;
        let start = be32(font, g)? as u32;
        let end = be32(font, g + 4)? as u32;
        let start_glyph = be32(font, g + 8)? as u32;
        if c >= start && c <= end {
            return Some((start_glyph + (c - start)) as u16);
        }
    }
    None
}

/// The exact bytes iced embeds and registers as "Fira Sans".
#[must_use]
pub fn render_font_bytes() -> &'static [u8] {
    graphics::text::FIRA_SANS_REGULAR
}

/// Whether `cp` is present in the render font's own cmap.
#[must_use]
pub fn present_in_render_font(font: &[u8], cp: char) -> bool {
    glyph_id(font, cp).is_some()
}

/// Which face the render pipeline actually resolves `cp` to, given the
/// configured family.
///
/// This is the fallback question the cmap cannot answer: a codepoint absent
/// from Fira Sans may still paint, because cosmic-text walks the platform
/// fallback chain. Returns `None` when the probe cannot resolve.
#[must_use]
pub fn resolving_face(cp: char) -> Option<String> {
    use iced::advanced::graphics::text::cosmic_text;

    let lock = graphics::text::font_system();
    let mut guard = lock.write().ok()?;
    let fs = guard.raw();

    let metrics = cosmic_text::Metrics::new(14.0, 16.0);
    let mut buffer = cosmic_text::Buffer::new(fs, metrics);
    buffer.set_size(fs, None, None);
    let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Name("Fira Sans"));
    buffer.set_text(
        fs,
        cp.to_string().as_str(),
        &attrs,
        cosmic_text::Shaping::Advanced,
        None,
    );

    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let name = fs
                .db_mut()
                .face(glyph.font_id)
                .map(|f| f.post_script_name.clone());
            if name.is_some() {
                return name;
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The glyph-constant scanner
// ---------------------------------------------------------------------------

/// The `match` arm separator, named because the scanner looks for it in
/// three places and a bare `"=>"` in each of them says nothing about why.
const MATCH_FAT_ARROW: &str = "=>";

/// A non-ASCII glyph constant discovered in this crate's sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphConst {
    /// The constant's name, e.g. `CHECK_MARK`.
    pub name: String,
    /// `U+2713`-style codepoint label.
    pub codepoint: String,
    /// The character itself.
    pub ch: char,
    /// `relative/path.rs:line`.
    pub location: String,
    /// Whether the render font's cmap carries it.
    pub in_render_font: bool,
    /// Which face the render pipeline resolves it to.
    pub resolved_face: Option<String>,
}

impl GlyphConst {
    /// A printable row for reports.
    #[must_use]
    pub fn row(&self) -> String {
        format!(
            "{:<28} {:<8} {:<6} {:<44} face={}",
            self.name,
            self.codepoint,
            if self.in_render_font { "yes" } else { "NO" },
            self.location,
            self.resolved_face
                .clone()
                .unwrap_or_else(|| "?".to_string()),
        )
    }
}

/// `U+2713`-style label for a codepoint.
#[must_use]
pub fn codepoint_label(ch: char) -> String {
    format!("U+{:04X}", ch as u32)
}

/// Whether a codepoint is in the range the gate covers: the General
/// Punctuation block and above, plus the multiplication sign.
#[must_use]
pub fn is_symbol_codepoint(ch: char) -> bool {
    (ch as u32) >= 0x2000 || ch == '\u{00D7}'
}

/// Every non-ASCII glyph constant declared in this crate's own sources.
///
/// Walks `env!("CARGO_MANIFEST_DIR")` for `.rs` files and reads both
/// shapes a glyph is emitted from — `const` declarations and `match`
/// arms returning a literal (see [`glyph_literals_on`]). Test sidecars and
/// the gallery binary are included — a gallery glyph is still a glyph the
/// crate emits.
#[must_use]
pub fn scan_glyph_constants() -> Vec<GlyphConst> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);
    files.sort();

    let mut out = Vec::new();
    for file in files {
        let Ok(src) = fs::read_to_string(&file) else {
            continue;
        };
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(file.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        out.extend(scan_glyph_source(&rel, &src));
    }
    out
}

/// Every glyph constant declared in one file's source text.
///
/// The pure half of [`scan_glyph_constants`]. `rel` is carried only to
/// label findings, which is what lets a test run the real pipeline over
/// synthetic source and prove a glyph that does not exist yet would be
/// caught rather than waved through.
#[must_use]
pub fn scan_glyph_source(rel: &str, src: &str) -> Vec<GlyphConst> {
    let font = render_font_bytes();
    let mut out = Vec::new();

    for (idx, line) in src.lines().enumerate() {
        for (name, literal) in glyph_literals_on(line) {
            for ch in literal.chars() {
                if !is_symbol_codepoint(ch) {
                    continue;
                }
                out.push(GlyphConst {
                    name: name.clone(),
                    codepoint: codepoint_label(ch),
                    ch,
                    location: format!("{rel}:{}", idx + 1),
                    in_render_font: present_in_render_font(font, ch),
                    resolved_face: resolving_face(ch),
                });
            }
        }
    }
    out
}

/// Every `(name, literal)` pair one source line declares.
///
/// Two shapes are read, and they are mutually exclusive per line:
///
/// * a `const` declaration — `const CHECK_MARK: &str = "✓";`;
/// * a `match` arm whose body is a literal — `Self::Danger => "✕",`.
///
/// The second shape is not a nicety. Not every icon the framework emits
/// is a `const`: the notification kinds and badge kinds pick their glyph
/// in an arm (`Self::Info => "ⓘ"`, `Self::Info => '●'`), and so does the
/// table header's sort arrow. A scanner reading only `const`s reports
/// those codepoints as never emitted — which is worse than silence,
/// because it tells the reader the ledger is clean when it never looked
/// at the line the icon lives on.
///
/// A line already parsed as a `const` is never re-read for arms, so the
/// `=>` inside `const S: &str = "a=>b";` cannot invent a finding.
///
/// Two limits, both deliberate and both worth knowing before trusting a
/// clean result: the literal must **begin** the arm's body, so
/// `Some("✓")` is not seen; and the scanner is line-based, so an arm
/// whose literal sits on the following line is not seen either.
fn glyph_literals_on(line: &str) -> Vec<(String, String)> {
    if let Some(pair) = parse_glyph_const(line) {
        return vec![pair];
    }
    parse_match_arm_glyphs(line)
}

/// The `match` arms on one line that return a string or char literal.
///
/// Every `=>` on the line is tried, so several arms on one line are all
/// seen. Comment lines are skipped, because a doc comment quoting an arm
/// is prose about code, not code that emits anything.
fn parse_match_arm_glyphs(line: &str) -> Vec<(String, String)> {
    let t = line.trim_start();
    if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') || t.starts_with('#') {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut at = 0usize;
    while let Some(rel) = line[at..].find(MATCH_FAT_ARROW) {
        let arrow = at + rel;
        let body = line[arrow + MATCH_FAT_ARROW.len()..].trim_start();
        if let Some(literal) = parse_literal(body) {
            let pattern = arm_pattern(&line[..arrow]);
            if !pattern.is_empty() {
                out.push((pattern, literal));
            }
        }
        at = arrow + MATCH_FAT_ARROW.len();
    }
    out
}

/// The arm a literal belongs to, as the text immediately left of its
/// `=>`, cut back at the nearest thing that starts a new arm.
fn arm_pattern(before: &str) -> String {
    let cut = [MATCH_FAT_ARROW, "{", ";", ","]
        .iter()
        .filter_map(|sep| before.rfind(sep).map(|i| i + sep.len()))
        .max()
        .unwrap_or(0);
    before[cut..].trim().to_string()
}

/// The gate: every glyph constant the render font cannot supply.
///
/// `(const name, codepoint, file:line)`.
#[must_use]
pub fn missing_glyphs() -> Vec<(String, String, String)> {
    scan_glyph_constants()
        .into_iter()
        .filter(|g| !g.in_render_font)
        .map(|g| (g.name, g.codepoint, g.location))
        .collect()
}

/// Recursively collect `.rs` files, skipping build output.
fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if path.is_dir() {
            if name == "target" || name.starts_with('.') {
                continue;
            }
            collect_rust_files(&path, out);
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// Parse one line into `(const_name, decoded_literal)`.
fn parse_glyph_const(line: &str) -> Option<(String, String)> {
    let t = line.trim_start();
    let rest = t
        .strip_prefix("pub const ")
        .or_else(|| t.strip_prefix("pub(crate) const "))
        .or_else(|| t.strip_prefix("const "))?;

    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_uppercase() || *c == '_' || c.is_ascii_digit())
        .collect();
    if name.is_empty() {
        return None;
    }

    let after = rest[name.len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let after = after
        .strip_prefix("&str")
        .or_else(|| after.strip_prefix("str"))
        .or_else(|| after.strip_prefix("char"))?;
    let after = after.trim_start().strip_prefix('=')?.trim_start();

    let literal = parse_literal(after)?;
    Some((name, literal))
}

/// Decode a Rust string or char literal at the start of `s`.
fn parse_literal(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let first = *bytes.first()?;

    if first == b'r' {
        let mut hashes = 0usize;
        let mut i = 1usize;
        while bytes.get(i) == Some(&b'#') {
            hashes += 1;
            i += 1;
        }
        if bytes.get(i) != Some(&b'"') {
            return None;
        }
        let close = format!("\"{}", "#".repeat(hashes));
        let body = &s[i + 1..];
        let end = body.find(&close)?;
        return Some(body[..end].to_string());
    }

    if first == b'"' || first == b'\'' {
        return parse_quoted(s, first);
    }
    None
}

/// Decode a quoted literal with the usual Rust escapes.
fn parse_quoted(s: &str, quote: u8) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let quote_char = char::from(quote);
    if chars.first() != Some(&quote_char) {
        return None;
    }
    let mut out = String::new();
    let mut i = 1usize;
    while i < chars.len() {
        let c = chars[i];
        if c == quote_char {
            return Some(out);
        }
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        let esc = *chars.get(i)?;
        match esc {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            'u' => {
                if chars.get(i + 1) != Some(&'{') {
                    return None;
                }
                let mut hex = String::new();
                let mut j = i + 2;
                while j < chars.len() && chars[j] != '}' {
                    hex.push(chars[j]);
                    j += 1;
                }
                let cp = u32::from_str_radix(&hex, 16).ok()?;
                out.push(char::from_u32(cp)?);
                i = j;
            }
            _ => return None,
        }
        i += 1;
    }
    None
}

/// The environment variable the manifest dir came from, exposed so a test can
/// report where the scanner looked.
#[must_use]
pub fn manifest_dir() -> String {
    env::var("CARGO_MANIFEST_DIR").unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The symbol-face ledger (`G-6`)
// ---------------------------------------------------------------------------
//
// The probe already answers "does the render font have this glyph?". The
// ledger is what you build once you stop asking it one glyph at a time:
// every symbol codepoint hoarfrost emits, classified by the role it plays,
// each with its guarantee status read live from the cmap rather than from a
// README.
//
// A generic family is not a face. `Family::Sans` resolves to iced's generic
// `Family::SansSerif` and `Family::Mono` to `Family::Monospace`, and
// cosmic-text answers a generic from whatever the host has installed. For
// prose that is fine. For a `✓` or a `⚠` it means the framework cannot say
// which face will paint, and therefore cannot say *whether* it will paint —
// the same widget is a checkmark on one machine and a tofu box on another.
//
// # Measured findings — why bundling a face cannot close `G-6`
//
// Every figure below is a cmap read, not an estimate. The inventory is the
// twenty-six codepoints in [`ICON_SYMBOLS`]; the render font is
// [`render_font_bytes`], the exact blob iced registers as "Fira Sans".
//
// 1. **The bundled face covers 0 of the 26 icon codepoints.** Not one of
//    the checkmarks, warnings, chevrons, triangles, stars or emoji that
//    *are* the affordance is in `FiraSans-Regular.ttf`'s own cmap. Every
//    icon hoarfrost emits today reaches the screen through host fallback.
// 2. **The only face covering all 26 is `seguisym.ttf`** (Segoe UI
//    Symbol), and its own `name` table forbids shipping it: *"You may use
//    this font to create, display and print content as permitted by the
//    license terms … of the Microsoft product"*. It is not redistributable.
// 3. **The best OFL candidate cannot carry the labels its own icons live
//    in.** Noto Sans Symbols 2 reaches 19 of 26 — the best any permissive
//    face manages — but it has **no Latin letters at all**: 0 of the 52
//    `A–Z`/`a–z` codepoints are in its cmap. hoarfrost's icons are not
//    standalone runs, they sit inside strings — `"✓ Apply"`, `"Finish ✓"`,
//    `"↑↓ Navigate · ↵ Run · Esc Close"`. Naming a letterless face for
//    those runs strands the *prose* on host fallback, which is worse than
//    the status quo, not better.
// 4. **The remaining gaps have no permissive replacement worth naming.**
//    `ⓘ` (U+24D8) survives in exactly one redistributable face, Noto Sans
//    Symbols, which covers 1 of 26 overall. `📍` (U+1F4CD) survives only
//    in Noto Emoji, which covers none of the geometric icons at all. There
//    is nothing to substitute them with inside a face this crate is allowed
//    to ship.
//
// Therefore **bundling a face cannot close `G-6`**. The correct fix is
// **drawn geometry**: an icon becomes a stroked path inside the widget's
// own `draw`, sized and coloured from the same tokens that size the text
// beside it. That removes the font dependency for the whole icon class
// rather than narrowing it — identical on every host because there is no
// fallback to fall through, tintable from the palette (`status.danger.text`
// on a warning, `text.muted` on a chevron) where a colour-emoji face makes
// tinting impossible, exact metrics per rung instead of per-face advance
// widths, and no licence surface at all.
//
// The typographic codepoints — `– — … ← ↑ → ↓ −` — stay exactly as they
// are. They are punctuation, they are already inside Fira Sans, and they
// are therefore already guaranteed. Only the icon class moves.
//
// That migration is a separate feature, not this module. What this module
// owns is the machine-visible version of the gap: it fails when a new raw
// icon glyph is added without a decision about how the framework will
// guarantee it.

/// The role a symbol plays, which decides whether it needs a face at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolClass {
    /// Punctuation that reads as part of a sentence or label — dashes,
    /// ellipses, prose arrows, the minus sign. Owned by the text face.
    Typographic,
    /// A pictogram that *is* the affordance: a checkmark, a warning, a
    /// chevron, a star, an emoji. The class the framework must guarantee,
    /// and the class drawn geometry replaces.
    Icon,
    /// A codepoint that exists only in a test fixture — the text a test
    /// truncates or measures, never something the framework paints.
    ///
    /// Classified rather than filtered out of the scan, because the scan
    /// deliberately covers test sidecars too and a reader of the ledger is
    /// entitled to see *why* an entry is not debt.
    Fixture,
}

/// The codepoints iced's own bundled `Fira Sans` face carries.
///
/// These need no guarantee: they never reach fallback, because the blob
/// iced ships draws them. Verified live rather than asserted from
/// documentation.
pub const RENDER_FONT_COVERS: [char; 10] = [
    '\u{00D7}', // ×  clear / close affordance
    '\u{2013}', // –  en dash, "Showing 1–25 of 300"
    '\u{2014}', // —  em dash, report prose and the empty-field placeholder
    '\u{2022}', // •  dot indicator
    '\u{2026}', // …  ellipsis, truncation and elision
    '\u{2190}', // ←  "← Previous"
    '\u{2191}', // ↑  keyboard legend
    '\u{2192}', // →  "Next →"
    '\u{2193}', // ↓  keyboard legend
    '\u{2212}', // −  minus sign
];

/// The icon codepoints the render font does **not** carry — the full
/// measured inventory: 26 codepoints, 0 of them bundled.
///
/// This is what drawn geometry replaces, one glyph at a time.
pub const ICON_SYMBOLS: [char; 26] = [
    '\u{21B5}',  // ↵  return, command-palette legend
    '\u{22EF}',  // ⋯  overflow trigger
    '\u{23ED}',  // ⏭  pagination last
    '\u{23EE}',  // ⏮  pagination first
    '\u{24D8}',  // ⓘ  notification / badge info  (declared in a match arm)
    '\u{25B2}',  // ▲  sort ascending
    '\u{25B4}',  // ▴  picker chevron up
    '\u{25B6}',  // ▶  pagination next
    '\u{25BC}',  // ▼  sort descending
    '\u{25BE}',  // ▾  combobox / split-button chevron
    '\u{25C0}',  // ◀  pagination previous
    '\u{25CF}',  // ●  status dot                 (declared in a match arm)
    '\u{2605}',  // ★  rating filled
    '\u{2606}',  // ☆  rating empty
    '\u{2630}',  // ☰  navigation-rail toggle
    '\u{26A0}',  // ⚠  warning
    '\u{2713}',  // ✓  check mark
    '\u{2715}',  // ✕  multiplication X          (declared in a match arm)
    '\u{1F441}', // 👁  password reveal
    '\u{1F464}', // 👤 avatar person
    '\u{1F4C5}', // 📅 calendar
    '\u{1F4CB}', // 📋 copy
    '\u{1F4CC}', // 📌 pinned
    '\u{1F4CD}', // 📍 unpinned
    '\u{1F50D}', // 🔍 search
    '\u{1F552}', // 🕒 clock
];

/// The typographic codepoints the render font does **not** carry.
///
/// One entry, and it is not a control: `⇒` appears only inside a
/// coverage-report sentence. Recorded rather than waved away, because a
/// report that renders as tofu is still a report nobody can read.
pub const TYPOGRAPHIC_SYMBOLS_UNCOVERED: [char; 1] = [
    '\u{21D2}', // ⇒  implication, spec_coverage prose
];

/// The codepoints that live only in a test fixture.
///
/// The seven of `microcopy/truncate_tests.rs`'s `"テスト日本語"` — three
/// kanji and four katakana, a string chosen precisely because it is wide
/// and has no spaces to break on. It is what the truncation test
/// *measures*, never something hoarfrost paints, and it is CJK script
/// rather than a symbol block, so no icon decision applies to it.
///
/// Classified rather than filtered out of the scan, so the ledger says out
/// loud why the entry is not debt instead of leaving it to look like a glyph
/// nobody dealt with.
pub const FIXTURE_SYMBOLS: [char; 7] = [
    '\u{65E5}', // 日  CJK ideograph, truncate fixture
    '\u{672C}', // 本  CJK ideograph, truncate fixture
    '\u{8A9E}', // 語  CJK ideograph, truncate fixture
    '\u{30C6}', // テ  katakana, truncate fixture
    '\u{30AD}', // キ  katakana, truncate fixture
    '\u{30B9}', // ス  katakana, truncate fixture
    '\u{30C8}', // ト  katakana, truncate fixture
];

/// Classify `ch` against the declared inventory.
///
/// `None` means the codepoint is not in the ledger, which is the case the
/// guard test treats as a finding rather than as silence.
#[must_use]
pub fn class_of(ch: char) -> Option<SymbolClass> {
    if ICON_SYMBOLS.contains(&ch) {
        return Some(SymbolClass::Icon);
    }
    if FIXTURE_SYMBOLS.contains(&ch) {
        return Some(SymbolClass::Fixture);
    }
    if RENDER_FONT_COVERS.contains(&ch) || TYPOGRAPHIC_SYMBOLS_UNCOVERED.contains(&ch) {
        return Some(SymbolClass::Typographic);
    }
    None
}

/// Whether `ch` is one of the icon pictograms.
#[must_use]
pub fn is_icon(ch: char) -> bool {
    ICON_SYMBOLS.contains(&ch)
}

/// Whether the face iced bundles can draw `ch` without falling back.
#[must_use]
pub fn render_font_has(ch: char) -> bool {
    present_in_render_font(render_font_bytes(), ch)
}

/// One candidate face measured against [`ICON_SYMBOLS`].
///
/// The numbers are measurements, not estimates: each row is the face's
/// own `cmap` read against the same twenty-six-codepoint inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceMeasurement {
    /// Human-readable face name.
    pub face: &'static str,
    /// The licence the asset actually ships under.
    pub licence: &'static str,
    /// Whether the crate would be permitted to redistribute it.
    pub redistributable: bool,
    /// How many of [`ICON_SYMBOLS`] the face's cmap carries.
    pub icons_covered: usize,
    /// Whether the face carries Latin letters at all — the property that
    /// decides whether it can serve a mixed label like `"✓ Apply"`.
    pub has_latin: bool,
    /// The icon codepoints this face lacks.
    pub icon_gaps: &'static [char],
}

impl FaceMeasurement {
    /// Whether this row could close the gap on its own: permissive, and
    /// no icon left uncovered.
    #[must_use]
    pub fn closes_the_gap(self) -> bool {
        self.redistributable && self.icon_gaps.is_empty()
    }
}

/// The measured candidate matrix. See the ledger section doc for what it
/// decides.
pub const COVERAGE_MATRIX: [FaceMeasurement; 10] = [
    FaceMeasurement {
        face: "Segoe UI Symbol",
        licence: "Microsoft supplied font",
        redistributable: false,
        icons_covered: 26,
        has_latin: true,
        icon_gaps: &[],
    },
    FaceMeasurement {
        face: "Noto Sans Symbols 2",
        licence: "SIL OFL 1.1",
        redistributable: true,
        icons_covered: 19,
        has_latin: false,
        icon_gaps: &[
            '\u{21B5}',
            '\u{22EF}',
            '\u{24D8}',
            '\u{1F464}',
            '\u{1F4C5}',
            '\u{1F4CC}',
            '\u{1F4CD}',
        ],
    },
    FaceMeasurement {
        face: "DejaVu Sans",
        licence: "Bitstream Vera + public domain",
        redistributable: true,
        icons_covered: 15,
        has_latin: true,
        icon_gaps: &[
            '\u{23ED}',
            '\u{23EE}',
            '\u{24D8}',
            '\u{1F441}',
            '\u{1F464}',
            '\u{1F4C5}',
            '\u{1F4CB}',
            '\u{1F4CC}',
            '\u{1F4CD}',
            '\u{1F50D}',
            '\u{1F552}',
        ],
    },
    FaceMeasurement {
        face: "Noto Emoji",
        licence: "SIL OFL 1.1",
        redistributable: true,
        icons_covered: 10,
        has_latin: false,
        icon_gaps: &[
            '\u{21B5}',
            '\u{22EF}',
            '\u{23ED}',
            '\u{23EE}',
            '\u{24D8}',
            '\u{25B2}',
            '\u{25B4}',
            '\u{25BC}',
            '\u{25BE}',
            '\u{25CF}',
            '\u{2605}',
            '\u{2606}',
            '\u{2630}',
            '\u{2713}',
            '\u{2715}',
            '\u{1F441}',
        ],
    },
    FaceMeasurement {
        face: "Inter",
        licence: "SIL OFL 1.1",
        redistributable: true,
        icons_covered: 10,
        has_latin: true,
        icon_gaps: &[
            '\u{22EF}',
            '\u{23ED}',
            '\u{23EE}',
            '\u{24D8}',
            '\u{25B4}',
            '\u{25BE}',
            '\u{2630}',
            '\u{2715}',
            '\u{1F441}',
            '\u{1F464}',
            '\u{1F4C5}',
            '\u{1F4CB}',
            '\u{1F4CC}',
            '\u{1F4CD}',
            '\u{1F50D}',
            '\u{1F552}',
        ],
    },
    FaceMeasurement {
        face: "Hack",
        licence: "MIT",
        redistributable: true,
        icons_covered: 9,
        has_latin: true,
        icon_gaps: &[
            '\u{23ED}',
            '\u{23EE}',
            '\u{24D8}',
            '\u{2605}',
            '\u{2606}',
            '\u{2630}',
            '\u{26A0}',
            '\u{2713}',
            '\u{2715}',
            '\u{1F441}',
            '\u{1F464}',
            '\u{1F4C5}',
            '\u{1F4CB}',
            '\u{1F4CC}',
            '\u{1F4CD}',
            '\u{1F50D}',
            '\u{1F552}',
        ],
    },
    FaceMeasurement {
        face: "Noto Sans Symbols",
        licence: "SIL OFL 1.1",
        redistributable: true,
        icons_covered: 1,
        has_latin: true,
        icon_gaps: &[
            '\u{21B5}',
            '\u{22EF}',
            '\u{23ED}',
            '\u{23EE}',
            '\u{25B2}',
            '\u{25B4}',
            '\u{25B6}',
            '\u{25BC}',
            '\u{25BE}',
            '\u{25C0}',
            '\u{25CF}',
            '\u{2605}',
            '\u{2606}',
            '\u{2630}',
            '\u{26A0}',
            '\u{2713}',
            '\u{2715}',
            '\u{1F441}',
            '\u{1F464}',
            '\u{1F4C5}',
            '\u{1F4CB}',
            '\u{1F4CC}',
            '\u{1F4CD}',
            '\u{1F50D}',
            '\u{1F552}',
        ],
    },
    FaceMeasurement {
        face: "Symbols Nerd Font Mono",
        licence: "MIT",
        redistributable: true,
        icons_covered: 1,
        has_latin: false,
        icon_gaps: &[
            '\u{21B5}',
            '\u{22EF}',
            '\u{23ED}',
            '\u{23EE}',
            '\u{24D8}',
            '\u{25B2}',
            '\u{25B4}',
            '\u{25B6}',
            '\u{25BC}',
            '\u{25BE}',
            '\u{25C0}',
            '\u{25CF}',
            '\u{2605}',
            '\u{2606}',
            '\u{26A0}',
            '\u{2713}',
            '\u{2715}',
            '\u{1F441}',
            '\u{1F464}',
            '\u{1F4C5}',
            '\u{1F4CB}',
            '\u{1F4CC}',
            '\u{1F4CD}',
            '\u{1F50D}',
            '\u{1F552}',
        ],
    },
    FaceMeasurement {
        face: "Fira Sans (bundled today)",
        licence: "SIL OFL 1.1",
        redistributable: true,
        icons_covered: 0,
        has_latin: true,
        icon_gaps: &ICON_SYMBOLS,
    },
    FaceMeasurement {
        face: "Iced Icons",
        licence: "SIL OFL 1.1",
        redistributable: true,
        icons_covered: 0,
        has_latin: false,
        icon_gaps: &ICON_SYMBOLS,
    },
];

/// One emitted symbol and its guarantee status, read live from the cmap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerEntry {
    /// `U+2713`-style label.
    pub codepoint: String,
    /// The character itself.
    pub ch: char,
    /// The role the ledger assigns it.
    pub class: Option<SymbolClass>,
    /// The `const` name or `match` arm pattern that declares it, e.g.
    /// `CHECK_MARK` or `Self::Danger`.
    pub constant: String,
    /// `file:line` of the declaration.
    pub location: String,
    /// Whether the bundled render font draws it without falling back.
    pub render_font_has: bool,
}

impl LedgerEntry {
    /// A printable row for reports.
    #[must_use]
    pub fn row(&self) -> String {
        format!(
            "{:<8} {:<12} {:<22} {:<34} face={}",
            self.codepoint,
            class_label(self.class),
            self.constant,
            self.location,
            if self.render_font_has {
                "bundled"
            } else {
                "HOST FALLBACK"
            },
        )
    }
}

/// The class column text for a possibly-unclassified codepoint.
#[must_use]
pub fn class_label(class: Option<SymbolClass>) -> &'static str {
    match class {
        Some(SymbolClass::Typographic) => "typographic",
        Some(SymbolClass::Icon) => "icon",
        Some(SymbolClass::Fixture) => "fixture",
        None => "UNCLASSIFIED",
    }
}

/// Every symbol hoarfrost declares, with its live guarantee status.
///
/// Built from [`scan_glyph_constants`], so it tracks the sources rather
/// than a transcription of them.
#[must_use]
pub fn ledger() -> Vec<LedgerEntry> {
    ledger_from(scan_glyph_constants())
}

/// The ledger over an already-scanned set of glyph constants.
///
/// Split out so a test can feed it synthetic source — that is how "adding
/// an icon turns the suite red" is proven without adding an icon to the
/// real crate.
#[must_use]
pub fn ledger_from(glyphs: Vec<GlyphConst>) -> Vec<LedgerEntry> {
    glyphs
        .into_iter()
        .map(|g| LedgerEntry {
            codepoint: g.codepoint.clone(),
            ch: g.ch,
            class: class_of(g.ch),
            constant: g.name,
            location: g.location,
            render_font_has: g.in_render_font,
        })
        .collect()
}

/// The entries in `entries` the ledger has no class for.
///
/// A non-empty result means a raw symbol glyph was added without a
/// decision about how the framework will guarantee it.
#[must_use]
pub fn unclassified_among(entries: &[LedgerEntry]) -> Vec<char> {
    let mut out: Vec<char> = entries
        .iter()
        .filter(|e| e.class.is_none())
        .map(|e| e.ch)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Symbols found in the sources that the ledger has no class for.
#[must_use]
pub fn unclassified_symbols() -> Vec<char> {
    unclassified_among(&ledger())
}

/// The icon symbols the framework cannot guarantee today.
#[must_use]
pub fn unguaranteed_icons() -> Vec<LedgerEntry> {
    ledger()
        .into_iter()
        .filter(|e| e.class == Some(SymbolClass::Icon) && !e.render_font_has)
        .collect()
}

/// Every redistributable row in [`COVERAGE_MATRIX`] that still leaves an
/// icon uncovered — i.e. every face that fails to close the gap.
#[must_use]
pub fn permissive_faces_that_fail() -> Vec<&'static FaceMeasurement> {
    COVERAGE_MATRIX
        .iter()
        .filter(|m| m.redistributable && !m.closes_the_gap())
        .collect()
}

#[cfg(test)]
#[path = "checkbox_ink_probe_tests.rs"]
mod checkbox_ink_probe_tests;
