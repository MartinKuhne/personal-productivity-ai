//! Shared headless **frame capture** for the demo's render-level ("ink") tests.
//!
//! A layout probe answers "where did the widget say it is". It cannot
//! answer "was anything drawn there". Three bugs in this demo's history
//! sailed through every state and geometry assertion that existed at the
//! time: a checkbox mark painted 8px outside its own box, a navbar menu
//! that latched open while painting **byte-identical ink** to the closed
//! frame, and a row that declared 56px and laid out 32px. None of those
//! are visible to `active_menu_id().is_some()`, to a widget count, or to
//! a bounds check.
//!
//! This module is the one place the demo rasterises a frame. It is the
//! extraction of the capture that [`crate::menu_surface`] tests originally
//! carried inline, so every render test in the crate reads pixels through
//! one implementation rather than inventing a second or third one.
//!
//! `hoarfrost::test_helpers::screenshot` is `#[cfg(test)]` inside
//! hoarfrost and therefore unreachable from this crate, so the capture is
//! built from the same `iced_test` primitives the layout probe uses:
//! [`UserInterface::build`] → [`UserInterface::draw`] →
//! [`renderer::Headless::screenshot`].
//!
//! # What "ink" means here
//!
//! Several readings, because several different defects are being hunted:
//!
//! * [`Frame::ink_count`] counts pixels whose **brightest channel**
//!   clears [`INK_THRESHOLD`] — text, glyphs and accent fills, the thing
//!   that must be legible. Zero means nothing a user could read.
//! * [`Frame::painted_count`] counts pixels that **differ from a
//!   reference surface** by more than a tolerance, which catches a widget
//!   whose whole paint is dim (a disabled fill, a hairline) that the
//!   brightness reading alone would wave through.
//! * [`Frame::ink_bounds`] returns *where* the ink landed, so a test can
//!   say the mark is inside the box that owns it rather than merely that
//!   some ink exists nearby.
//! * [`Frame::differing_pixels`] counts pixels a state change actually
//!   moved. If a control repaints nothing, the two frames are identical
//!   and the test fails — which is exactly the menu bug, and makes this
//!   the hardest of the readings to satisfy by accident.
//!
//! Every reading is absolute. Nothing here compares against a stored
//! golden, so a frame asserts a property rather than reproducing an
//! artefact of the machine that drew a reference file.
//!
//! # Cost
//!
//! Building the headless renderer costs roughly half a second — more
//! than a layout pass and a raster together — so [`Studio`] owns one
//! renderer and takes many snapshots through it. A test that minted a
//! renderer per assertion would spend most of the suite in
//! [`crate::layout_probe::headless_renderer`].

use iced::advanced::renderer::{self, Headless as _};
use iced::mouse::{self, Button};
use iced::{Color, Element, Event, Point, Rectangle, Size};

use iced_test::core::clipboard::Null;
use iced_test::renderer::Renderer as HeadlessRenderer;
use iced_test::runtime::user_interface::{self, UserInterface};

use crate::app::Message;
use crate::layout_probe::{Probe, headless_renderer};

/// Physical pixels per logical pixel, matching the framework's pixel leg.
pub const SCALE: f32 = 2.0;

/// A channel value above which a pixel counts as ink.
///
/// The dark palette's brightest surfaces top out at `surface_overlay`
/// `RGB(24, 24, 27)` and its `border.strong` at `RGB(120, 120, 120)`,
/// while text runs to pure white. Ninety therefore sits above every
/// surface a card can paint and below the strokes and glyphs that must
/// be legible, so a zero count really does mean "nothing was drawn".
pub const INK_THRESHOLD: u8 = 90;

/// How far a channel may drift from a reference surface before the pixel
/// counts as painted over it.
///
/// Four is above the rounding noise a `f32 → u8` colour conversion and
/// the renderer's blending introduce, and far below the smallest
/// deliberate step in the palette (`surface_base` 9 → `surface_raised`
/// 18 is a nine-step jump), so it separates "something was painted"
/// from "the surface underneath was merely re-emitted".
pub const SURFACE_TOLERANCE: u8 = 4;

/// The whole-window reference viewport.
pub const VIEWPORT: Size = Size::new(1400.0, 900.0);

/// A single-pane viewport, tall enough that any one section fits.
///
/// Sections are captured on their own rather than inside the live
/// scrollable so that nothing sits below the fold while its ink is being
/// counted. [`crate::render_surfaces`] puts them back inside a
/// scrollable when the question is about clipping.
pub const PANE_VIEWPORT: Size = Size::new(700.0, 1600.0);

/// A reusable headless capture session.
///
/// Holds one renderer so a suite of snapshots pays the half-second
/// build once instead of once per assertion.
pub struct Studio {
    renderer: HeadlessRenderer,
}

impl Default for Studio {
    fn default() -> Self {
        Self::new()
    }
}

impl Studio {
    /// Builds the shared headless renderer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            renderer: headless_renderer(),
        }
    }

    /// Rasterises `root` at `viewport`.
    #[must_use]
    pub fn frame(&mut self, viewport: Size, root: Element<'_, Message>) -> Frame {
        self.frame_after(viewport, root, &[])
    }

    /// Rasterises `root` at `viewport` after delivering `events`.
    ///
    /// Delivering events before the draw is what makes a *scrolled* or
    /// *clicked-then-repainted* frame observable: the same widget tree,
    /// after the interaction the user would have had.
    #[must_use]
    pub fn frame_after(
        &mut self,
        viewport: Size,
        root: Element<'_, Message>,
        events: &[Event],
    ) -> Frame {
        let mut ui: UserInterface<'_, Message, iced::Theme, HeadlessRenderer> =
            UserInterface::build(
                root,
                viewport,
                user_interface::Cache::default(),
                &mut self.renderer,
            );

        if !events.is_empty() {
            let mut clipboard = Null;
            let mut messages = Vec::new();
            ui.update(
                events,
                mouse::Cursor::Available(Point::new(viewport.width / 2.0, viewport.height / 2.0)),
                &mut self.renderer,
                &mut clipboard,
                &mut messages,
            );
        }

        ui.draw(
            &mut self.renderer,
            &iced::Theme::Dark,
            &renderer::Style {
                text_color: Color::WHITE,
            },
            mouse::Cursor::Unavailable,
        );

        let physical = Size::new(
            (viewport.width * SCALE).round() as u32,
            (viewport.height * SCALE).round() as u32,
        );
        let rgba = self.renderer.screenshot(physical, SCALE, Color::BLACK);

        Frame {
            rgba,
            stride: (viewport.width * SCALE) as usize,
            viewport,
        }
    }

    /// Records the geometry of `root` at `viewport`.
    #[must_use]
    pub fn probe(&mut self, viewport: Size, root: Element<'_, Message>) -> Probe {
        Probe::capture_with(&mut self.renderer, viewport, root)
    }

    /// Delivers a press-and-release at `at` and returns what the tree published.
    #[must_use]
    pub fn click(&mut self, root: Element<'_, Message>, at: Point) -> Vec<Message> {
        let mut ui: UserInterface<'_, Message, iced::Theme, HeadlessRenderer> =
            UserInterface::build(
                root,
                VIEWPORT,
                user_interface::Cache::default(),
                &mut self.renderer,
            );
        let mut clipboard = Null;
        let mut messages = Vec::new();
        let cursor = mouse::Cursor::Available(at);

        let moved = Event::Mouse(mouse::Event::CursorMoved { position: at });
        let pressed = Event::Mouse(mouse::Event::ButtonPressed(Button::Left));
        let released = Event::Mouse(mouse::Event::ButtonReleased(Button::Left));

        ui.update(
            &[moved, pressed],
            cursor,
            &mut self.renderer,
            &mut clipboard,
            &mut messages,
        );
        ui.update(
            &[released],
            cursor,
            &mut self.renderer,
            &mut clipboard,
            &mut messages,
        );
        messages
    }
}

/// The 8-bit channels an iced [`Color`] must produce on screen.
#[must_use]
pub fn to_rgb(colour: Color) -> [u8; 3] {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [channel(colour.r), channel(colour.g), channel(colour.b)]
}

/// One rasterised frame, read in logical coordinates.
pub struct Frame {
    rgba: Vec<u8>,
    stride: usize,
    viewport: Size,
}

impl Frame {
    /// The RGB channels painted at logical point `(x, y)`.
    ///
    /// # Panics
    /// When `(x, y)` falls outside the captured frame.
    #[must_use]
    pub fn pixel(&self, x: f32, y: f32) -> [u8; 3] {
        let index = self.index(x, y);
        assert!(
            index + 2 < self.rgba.len(),
            "sample ({x}, {y}) falls outside the frame"
        );
        [self.rgba[index], self.rgba[index + 1], self.rgba[index + 2]]
    }

    /// How many pixels inside `rect` clear [`INK_THRESHOLD`] on some channel.
    #[must_use]
    pub fn ink_count(&self, rect: Rectangle) -> usize {
        let mut count = 0usize;
        self.scan(rect, |pixel| {
            if is_bright(&pixel) {
                count += 1;
            }
            false
        });
        count
    }

    /// Whether any ink lands inside `rect`.
    #[must_use]
    pub fn has_ink(&self, rect: Rectangle) -> bool {
        self.ink_count(rect) > 0
    }

    /// How many pixels inside `rect` differ from `reference` by more than
    /// `tolerance` on some channel.
    #[must_use]
    pub fn painted_count(&self, rect: Rectangle, reference: [u8; 3], tolerance: u8) -> usize {
        let mut count = 0usize;
        self.scan(rect, |pixel| {
            if channel_delta(&pixel, &reference) > tolerance {
                count += 1;
            }
            false
        });
        count
    }

    /// How many pixels inside `rect` sit within `tolerance` of `reference`.
    ///
    /// The complement of [`painted_count`](Self::painted_count). Where that
    /// reading asks what a region is *not*, this one asks how much of it
    /// really **is** a given colour — which is how a status-filled node is
    /// told apart from a node that merely happens to be bright.
    #[must_use]
    pub fn matching_count(&self, rect: Rectangle, reference: [u8; 3], tolerance: u8) -> usize {
        let mut count = 0usize;
        self.scan(rect, |pixel| {
            if channel_delta(&pixel, &reference) <= tolerance {
                count += 1;
            }
            false
        });
        count
    }

    /// The tightest rectangle inside `rect` that holds every bright pixel.
    ///
    /// `None` when `rect` holds no ink at all. This is what lets a test
    /// assert not merely *that* a mark painted but *where* it painted —
    /// the checkbox bug was an ink-presence test passing while the mark
    /// sat outside the box that owned it.
    #[must_use]
    pub fn ink_bounds(&self, rect: Rectangle) -> Option<Rectangle> {
        let (x0, y0, x1, y1) = self.pixel_box(rect);
        let mut min_x = usize::MAX;
        let mut min_y = usize::MAX;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut found = false;

        for y in y0..y1 {
            for x in x0..x1 {
                let i = y * self.stride * 4 + x * 4;
                if i + 2 >= self.rgba.len() {
                    continue;
                }
                let pixel = [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]];
                if is_bright(&pixel) {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    found = true;
                }
            }
        }

        if !found {
            return None;
        }

        Some(Rectangle {
            x: min_x as f32 / SCALE,
            y: min_y as f32 / SCALE,
            width: (max_x - min_x + 1) as f32 / SCALE,
            height: (max_y - min_y + 1) as f32 / SCALE,
        })
    }

    /// Whether `rect` is entirely one colour, within `tolerance`.
    #[must_use]
    pub fn is_uniform(&self, rect: Rectangle, tolerance: u8) -> bool {
        let mut reference: Option<[u8; 3]> = None;
        let mut diverged = false;
        self.scan(rect, |pixel| {
            if let Some(base) = reference {
                if channel_delta(&pixel, &base) > tolerance {
                    diverged = true;
                    return true;
                }
            } else {
                reference = Some(pixel);
            }
            false
        });
        !diverged
    }

    /// How many pixels inside `rect` this frame and `other` disagree on.
    ///
    /// The anti-vacuity reading: a control whose repaint draws nothing
    /// leaves the two frames byte-identical here, so a zero is a real
    /// failure rather than a test that never looked.
    #[must_use]
    pub fn differing_pixels(&self, other: &Frame, rect: Rectangle) -> usize {
        let (x0, y0, x1, y1) = self.pixel_box(rect);
        let mut count = 0usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = y * self.stride * 4 + x * 4;
                if i + 2 >= self.rgba.len() || i + 2 >= other.rgba.len() {
                    continue;
                }
                let a = [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]];
                let b = [other.rgba[i], other.rgba[i + 1], other.rgba[i + 2]];
                if channel_delta(&a, &b) > SURFACE_TOLERANCE {
                    count += 1;
                }
            }
        }
        count
    }

    /// The viewport this frame was captured at.
    #[must_use]
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// Walks every pixel of `rect`, stopping as soon as `visit` returns `true`.
    fn scan(&self, rect: Rectangle, mut visit: impl FnMut([u8; 3]) -> bool) {
        let (x0, y0, x1, y1) = self.pixel_box(rect);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = y * self.stride * 4 + x * 4;
                if i + 2 >= self.rgba.len() {
                    continue;
                }
                if visit([self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]) {
                    return;
                }
            }
        }
    }

    /// The pixel range covering `rect`, clipped to the frame.
    fn pixel_box(&self, rect: Rectangle) -> (usize, usize, usize, usize) {
        let frame_x = (self.viewport.width * SCALE) as usize;
        let frame_y = (self.viewport.height * SCALE) as usize;

        let x0 = (rect.x.max(0.0) * SCALE) as usize;
        let y0 = (rect.y.max(0.0) * SCALE) as usize;
        let x1 = ((rect.x + rect.width).max(0.0) * SCALE) as usize;
        let y1 = ((rect.y + rect.height).max(0.0) * SCALE) as usize;

        // Pull the origin inside the frame *before* clamping the far edge:
        // a rect wholly below the fold has `y0 > frame_y`, and `clamp` with
        // `min > max` panics. Clipping tests ask about exactly those rects,
        // so they must come back as an empty range (zero pixels), not a
        // crash.
        let x0 = x0.min(frame_x);
        let y0 = y0.min(frame_y);

        (x0, y0, x1.clamp(x0, frame_x), y1.clamp(y0, frame_y))
    }

    fn index(&self, x: f32, y: f32) -> usize {
        let px = (x * SCALE) as usize;
        let py = (y * SCALE) as usize;
        py * self.stride * 4 + px * 4
    }
}

/// Whether a pixel is bright enough to be ink.
fn is_bright(pixel: &[u8; 3]) -> bool {
    pixel[0].max(pixel[1]).max(pixel[2]) > INK_THRESHOLD
}

/// The fewest pixels a state change must move for the repaint to count
/// as having happened.
///
/// Eight is below the smallest honest repaint in the demo — flipping a
/// 16px checkbox mark moves hundreds — and above the handful of pixels
/// antialiasing might shuffle on an unrelated edge, so a control that
/// repaints nothing cannot clear it by accident.
pub const MIN_CHANGED_PIXELS: usize = 8;

/// Asserts `after` genuinely repaints `rect` differently from `before`.
///
/// The strongest cheap render assertion in the crate: it fails on a
/// widget whose state moved while its pixels did not, which is exactly
/// the navbar-menu failure and is invisible to every state and geometry
/// assertion.
///
/// `what` names the change in the failure message so the report says
/// which control stayed dead.
pub fn assert_repainted(before: &Frame, after: &Frame, rect: Rectangle, what: &str) {
    let moved = before.differing_pixels(after, rect);
    assert!(
        moved >= MIN_CHANGED_PIXELS,
        "{what} changed no pixels inside {rect:?} \
         ({moved} moved, {MIN_CHANGED_PIXELS} required) — the model moved but \
         the frame did not repaint"
    );
}

/// The largest single-channel distance between two colours.
fn channel_delta(a: &[u8; 3], b: &[u8; 3]) -> u8 {
    let d = |x: u8, y: u8| x.abs_diff(y);
    d(a[0], b[0]).max(d(a[1], b[1])).max(d(a[2], b[2]))
}
