//! Unit tests for the capture leg of the shared harness: the borrowing
//! element, the 2-D ink readers, the frame diff and the repaint assertion.
//!
//! Two of these are anti-vacuity tests, and they are the reason the leg
//! exists. A repaint assertion that never fails is worse than none — it
//! reads as coverage while a control latches into a new state and paints
//! byte-identical ink. These tests pin that the assertion **does** fail on
//! two identical frames, so a pass elsewhere means something.
//!
//! The fixtures are deliberately real where a fake would weaken the claim
//! and deliberately synthetic where a real widget would strengthen nothing:
//! [`FlatFill`] is a mark whose position and extent are known to the pixel,
//! which is what the ink readers have to be measured against; the
//! scrollable is iced's own, because the point is that a widget shipped by
//! somebody else now reaches the recorder.
//!
//! Building iced's headless renderer costs roughly 620 ms, so every claim
//! that only needs a picture shares **one** [`Capture`] per group and reads
//! several frames out of it, and every claim that only needs the operation
//! stream shares one [`Harness`] and swaps the tree with
//! [`Harness::set_view`]. Frames do not share widget state and the candidate
//! recorder is a pure read, so nothing a case observes can come from the case
//! before it. The assertions themselves are unchanged in number and in kind;
//! each one names the claim it belongs to.

use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::time::Instant;

use crate::binding::Controlled;
use crate::test_helpers::ALL_KINDS;
use crate::test_helpers::Blank;
use crate::test_helpers::Capture;
use crate::test_helpers::El;
use crate::test_helpers::Frame;
use crate::test_helpers::Harness;
use crate::test_helpers::INK_THRESHOLD;
use crate::test_helpers::KIND_CUSTOM;
use crate::test_helpers::KIND_FOCUSABLE;
use crate::test_helpers::KIND_SCROLLABLE;
use crate::test_helpers::KIND_TEXT;
use crate::test_helpers::MIN_CHANGED_PIXELS;
use crate::test_helpers::PIXEL_SCALE;
use crate::test_helpers::SURFACE_TOLERANCE;
use crate::test_helpers::assert_panics;
use crate::test_helpers::assert_repainted;
use crate::test_helpers::iced_id;
use crate::test_helpers::moved;
use crate::test_helpers::panic_text;
use crate::test_helpers::screenshot;
use crate::test_helpers::to_rgb;
use crate::test_helpers::wrapped;
use crate::widget::button_model::ButtonModel;
use crate::widget::button_widget::Button;
use crate::widget::hyperlink::Hyperlink;
use crate::widget::hyperlink_model::HyperlinkModel;
use crate::widget::segmented::SegmentedButton;
use crate::widget::segmented_model::SegmentItem;
use crate::widget::segmented_model::SegmentedModel;
use crate::widget::split_button::SplitButton;
use crate::widget::split_button_model::SplitButtonModel;
use crate::widget::split_button_model::SplitMenuRow;
use crate::widget::toggle_model::CheckState;
use crate::widget::toggle_model::CheckboxModel;
use crate::widget::toggle_model::RadioGroupModel;
use crate::widget::toggle_model::RadioOption;
use crate::widget::toggle_model::SwitchModel;
use crate::widget::toggle_widget::Checkbox;
use crate::widget::toggle_widget::RadioGroup;
use crate::widget::toggle_widget::ToggleSwitch;

use iced::Color;
use iced::Element;
use iced::Length;
use iced::Padding;
use iced::Point;
use iced::Rectangle;
use iced::Size;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::renderer::Headless as _;
use iced::advanced::widget::Operation;
use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::alignment;
use iced::mouse;
use iced::widget::Id;
use iced_test::renderer::Renderer as HeadlessRenderer;

/// The canvas every frame in this file is captured at.
const CANVAS: Size = Size::new(120.0, 48.0);

/// A mid-grey flat fill — bright enough to be ink, far from a glyph's white.
const FLAT_GREY: Color = Color::from_rgb(0.5, 0.5, 0.5);

/// A fill dimmer than [`INK_THRESHOLD`] on every channel.
///
/// The brightness reading waves this through; the reference-colour reading
/// must not, which is the whole reason the two readings exist.
const DIM_FILL: Color = Color::from_rgb(0.05, 0.05, 0.05);

/// The value a [`HookPublisher`] pushes through [`Operation::custom`].
const CUSTOM_STATE: u32 = 0x00c0_ffee;

/// One headless session, shared by every candidate probe in this file.
type ProbeHarness = Harness<'static, (), HeadlessRenderer>;

/// The rectangle covering a whole frame.
fn whole(size: Size) -> Rectangle {
    Rectangle::with_size(size)
}

/// A rectangle inset far enough from the frame edge that no blend of the
/// fill with the background can reach it.
fn inset(size: Size, by: f32) -> Rectangle {
    Rectangle::new(
        Point::new(by, by),
        Size::new(size.width - 2.0 * by, size.height - 2.0 * by),
    )
}

/// How many physical pixels a logical rectangle covers.
fn physical_pixels(size: Size, scale: f32) -> usize {
    (size.width * scale).round() as usize * (size.height * scale).round() as usize
}

/// Put `widget` inside a canvas-sized frame with its top-left at `at`.
///
/// A root widget always lays out at the origin, so a mark that has to sit
/// at a known offset needs a padded frame under it — otherwise the test
/// measures the wrong box and proves nothing.
fn placed<'a>(widget: Element<'a, ()>, at: Point) -> Element<'a, ()> {
    iced::widget::container(widget)
        .width(Length::Fixed(CANVAS.width))
        .height(Length::Fixed(CANVAS.height))
        .align_x(alignment::Horizontal::Left)
        .align_y(alignment::Vertical::Top)
        .padding(Padding {
            top: at.y,
            right: 0.0,
            bottom: 0.0,
            left: at.x,
        })
        .into()
}

/// A solid `colour` rectangle of `size`, as an element.
fn fill(size: Size, colour: Color) -> Element<'static, ()> {
    Element::new(FlatFill::new(size, colour))
}

/// A white square mark, used wherever a frame just has to contain something.
fn flat_mark() -> Element<'static, ()> {
    fill(Size::new(24.0, 24.0), Color::WHITE)
}

/// A widget that publishes itself through one named operation hook.
fn hooked(hook: &'static str, label: &'static str) -> Element<'static, ()> {
    Element::new(HookPublisher::new(hook, label))
}

/// A controlled checkbox over a borrowed model.
fn checkbox<'a>(model: &'a CheckboxModel) -> Element<'a, ()> {
    Checkbox::controlled(Controlled::new(model), "capture_leg_checkbox", |_| None).into()
}

/// Open one candidate-recording session on `first`.
///
/// The session is built once per group of claims and each later tree is
/// swapped in through [`kinds_in`], so a file that used to pay for a
/// renderer per hook now pays for one.
fn probe_session(first: impl Fn() -> El<'static, (), HeadlessRenderer> + 'static) -> ProbeHarness {
    Harness::<(), HeadlessRenderer>::headless(first)
}

/// Point the shared session at `build`'s tree and report the kinds it saw.
///
/// [`Harness::candidates`] is a read of the tree that is currently mounted,
/// so re-pointing the session cannot carry a fact from one probe into the
/// next.
fn kinds_in(
    harness: &mut ProbeHarness,
    build: impl Fn() -> El<'static, (), HeadlessRenderer> + 'static,
) -> Vec<&'static str> {
    harness.set_view(build);

    harness
        .candidates()
        .iter()
        .map(|candidate| candidate.kind)
        .collect()
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A widget that fills its own box with one colour and nothing else.
///
/// A mark whose position and extent are known exactly, so a reader that
/// reports the wrong count or the wrong bounds is the reader's fault and
/// not the fixture's.
struct FlatFill {
    box_size: Size,
    colour: Color,
}

impl FlatFill {
    fn new(box_size: Size, colour: Color) -> Self {
        Self { box_size, colour }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for FlatFill
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.box_size.width),
            Length::Fixed(self.box_size.height),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(self.box_size)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                ..Default::default()
            },
            self.colour,
        );
    }
}

/// A widget that publishes itself through one named operation hook.
///
/// No shipped iced widget calls [`Operation::custom`], so the recorder's
/// blindness to that door could only be proven with a widget written for
/// the purpose. The label is borrowed, so this doubles as a non-`'static`
/// element.
struct HookPublisher<'a> {
    id: Id,
    label: &'a str,
    hook: &'static str,
}

impl<'a> HookPublisher<'a> {
    fn new(hook: &'static str, label: &'a str) -> Self {
        Self {
            id: iced_id(label),
            label,
            hook,
        }
    }

    /// The box this widget lays out to. The width tracks the label so the
    /// borrowed field is genuinely read rather than carried for nothing.
    fn box_size(&self) -> Size {
        Size::new(40.0 + self.label.len() as f32, 24.0)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for HookPublisher<'_>
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.box_size().width),
            Length::Fixed(self.box_size().height),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(self.box_size())
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let bounds = layout.bounds();
        let mut state = CUSTOM_STATE;

        match self.hook {
            KIND_CUSTOM => operation.custom(Some(&self.id), bounds, &mut state),
            KIND_TEXT => operation.text(Some(&self.id), bounds, self.label),
            _ => operation.container(Some(&self.id), bounds),
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

// ---------------------------------------------------------------------------
// The lifetime widening
// ---------------------------------------------------------------------------

/// `model` is a stack local and `Controlled` borrows it, so the element is
/// `Element<'_>` and cannot be coerced to `'static`. Under the old
/// `Element<'static, M>` signature this file does not compile.
///
/// Both legs are checked from one test because both had to be widened: the
/// one-shot [`screenshot`] and the many-frame [`Capture`].
#[test]
fn test_a_borrowing_element_reaches_both_capture_legs() {
    let model = CheckboxModel::new("").with_state(CheckState::Checked);
    let root: Element<'_, ()> = checkbox(&model);

    let rgba = screenshot(CANVAS, PIXEL_SCALE, Color::BLACK, root);

    assert_eq!(
        rgba.len(),
        physical_pixels(CANVAS, PIXEL_SCALE) * 4,
        "screenshot: the framebuffer must cover the whole canvas"
    );
    assert!(
        Frame::new(rgba, CANVAS, PIXEL_SCALE).has_ink(whole(CANVAS)),
        "screenshot: the borrowed-model checkbox must actually have painted"
    );
    assert_eq!(
        model.state,
        CheckState::Checked,
        "screenshot: the model must still be readable by its owner after the capture"
    );

    // The same claim through the many-frame path.
    let second = CheckboxModel::new("").with_state(CheckState::Unchecked);
    let root: Element<'_, ()> = checkbox(&second);

    let mut capture = Capture::new();
    let frame = capture.frame(CANVAS, root);

    assert_eq!(
        frame.viewport(),
        CANVAS,
        "Capture: the frame must report the canvas it was asked for"
    );
    assert_eq!(
        frame.physical_size(),
        Size::new(240, 96),
        "Capture: the frame must be rasterised at PIXEL_SCALE"
    );
}

// ---------------------------------------------------------------------------
// The frame diff — anti-vacuity, and everything else a frame reads
// ---------------------------------------------------------------------------

/// Every claim a captured frame makes, driven out of **one** session and
/// the two identical flat-mark frames it produces.
///
/// The frames are deliberately the same picture: the diff has to report
/// zero, the repaint assertion has to go red over them, and the pixel,
/// buffer, mask and renderer reads all have a known answer on a mark whose
/// box is known. One renderer therefore serves all of it.
#[test]
fn test_one_session_answers_every_frame_read_and_the_repaint_guard() {
    let mut capture = Capture::new();

    let before = capture.frame(CANVAS, flat_mark());
    let after = capture.frame(CANVAS, flat_mark());

    // Two frames of one element are pixel-identical, and one session
    // renders the same element the same way twice.
    assert_eq!(
        before.differing_pixels(&after, whole(CANVAS)),
        0,
        "the same element rendered twice must be pixel-identical, or the diff \
         is counting noise and cannot prove anything"
    );
    assert_eq!(
        before.differing_pixels(&after, whole(CANVAS)),
        0,
        "one session must render the same element the same way twice"
    );

    // The load-bearing anti-vacuity claim: the repaint assertion must
    // **fail** when nothing moved.
    let message = assert_panics("two identical frames against the repaint assertion", || {
        assert_repainted(&before, &after, whole(CANVAS), "the checkbox mark");
    });

    let required = format!("0 moved, {MIN_CHANGED_PIXELS} required");

    assert!(
        message.contains("the model moved but the frame did not repaint"),
        "the failure must name the failure mode, got: {message}"
    );
    assert!(
        message.contains(&required),
        "the failure must report the moved-pixel count, got: {message}"
    );
    assert!(
        message.contains("the checkbox mark"),
        "the failure must name the change that stayed dead, got: {message}"
    );
    assert!(
        message.contains("Rectangle"),
        "the failure must name the rect, got: {message}"
    );

    // The raw pixel read, at a point that was painted and one that was not.
    assert_eq!(
        before.pixel(110.0, 44.0),
        [0, 0, 0],
        "the pixel read must report the background where nothing is painted"
    );
    assert_eq!(
        before.pixel(12.0, 12.0),
        [255, 255, 255],
        "the pixel read must report the mark where it was painted"
    );

    // The raw buffer lines up with the geometry the frame claims.
    let physical = before.physical_size();
    assert_eq!(
        before.scale(),
        PIXEL_SCALE,
        "the frame must report the scale it was rasterised at"
    );
    assert_eq!(
        before.pixels().len(),
        physical.width as usize * physical.height as usize * 4,
        "the raw buffer must be RGBA over the whole physical frame"
    );

    // The mask is not a new reading that can drift — it agrees with the
    // reader that already existed, over the same reference and tolerance.
    let marked = before
        .mask_against(Color::BLACK, SURFACE_TOLERANCE)
        .iter()
        .filter(|pixel| **pixel)
        .count();

    assert_eq!(
        marked,
        before.painted_count(whole(CANVAS), to_rgb(Color::BLACK), SURFACE_TOLERANCE),
        "the mask and `painted_count` must disagree about nothing, or one of \
         them is measuring a different picture"
    );
    assert!(
        marked > 0,
        "a white mark over a black surface must mark something"
    );

    // The reference is load-bearing, and so is the pixel it is read at.
    // Four readings over one frame pin that `mask_against` actually
    // compares rather than reporting brightness no matter what it was handed.
    let against_surface = before.mask_against(Color::BLACK, SURFACE_TOLERANCE);
    let against_mark = before.mask_against(Color::WHITE, SURFACE_TOLERANCE);

    let in_mark = |mask: &[bool]| mask[4 * physical.width as usize + 4];
    let on_surface = |mask: &[bool]| {
        mask[(physical.height as usize - 4) * physical.width as usize
            + (physical.width as usize - 4)]
    };

    assert!(
        in_mark(&against_surface),
        "the white mark differs from the black surface"
    );
    assert!(
        !in_mark(&against_mark),
        "the white mark does not differ from white — the reference is being read"
    );
    assert!(
        !on_surface(&against_surface),
        "the bare surface IS the black reference, so it must not be marked"
    );
    assert!(
        on_surface(&against_mark),
        "the bare surface differs from white"
    );

    // The session hands out the renderer it owns, read-only and read-write.
    // Last, so an offscreen raster cannot sit between a frame and the
    // picture it was asked to draw.
    let name = capture.renderer().name();
    assert!(
        !name.is_empty(),
        "`renderer()` must hand back a live renderer that identifies itself"
    );

    let raw = capture
        .renderer_mut()
        .screenshot(physical, PIXEL_SCALE, Color::BLACK);

    assert_eq!(
        raw.len(),
        physical.width as usize * physical.height as usize * 4,
        "`renderer_mut()` must drive an offscreen raster of exactly the \
         geometry handed to it"
    );
}

/// Positive detection: a real control, toggled, really moves pixels, and a
/// reader that compares against a reference sees fills the brightness
/// reading has to wave through.
///
/// Five frames, one session: the two checkbox states, the dim fill, a flat
/// grey canvas and a lone glyph.
#[test]
fn test_the_paint_readers_see_what_their_fixture_painted() {
    let mut capture = Capture::new();

    // --- checking a box moves pixels inside its own box ---
    let unchecked = CheckboxModel::new("");
    let checked = CheckboxModel::new("").with_state(CheckState::Checked);

    let box_rest = capture.frame(CANVAS, checkbox(&unchecked));
    let box_checked = capture.frame(CANVAS, checkbox(&checked));

    let indicator = box_checked
        .ink_bounds(whole(CANVAS))
        .expect("a checked box paints an indicator and a mark");

    assert_repainted(
        &box_rest,
        &box_checked,
        indicator,
        "checking the box (indicator)",
    );
    assert_repainted(
        &box_rest,
        &box_checked,
        whole(CANVAS),
        "checking the box (whole frame)",
    );

    let moved = box_checked.differing_pixels(&box_rest, indicator);
    assert!(
        moved >= MIN_CHANGED_PIXELS,
        "a polarity flip must move at least {MIN_CHANGED_PIXELS} pixels of the \
         indicator, moved {moved}"
    );

    // --- a dim fill the brightness reading misses but the reference sees ---
    let region = inset(CANVAS, 4.0);
    let area = physical_pixels(region.size(), PIXEL_SCALE);

    let dim = capture.frame_at(CANVAS, PIXEL_SCALE, Color::BLACK, fill(CANVAS, DIM_FILL));

    assert_eq!(
        dim.ink_count(region),
        0,
        "a fill under the ink threshold must not read as ink by brightness"
    );
    assert_eq!(
        dim.painted_count(region, to_rgb(Color::BLACK), SURFACE_TOLERANCE),
        area,
        "every pixel of the dim fill must differ from the black reference"
    );
    assert_eq!(
        dim.matching_count(region, to_rgb(DIM_FILL), SURFACE_TOLERANCE),
        area,
        "every pixel of the fill must match its own colour"
    );
    assert_eq!(
        dim.matching_count(region, to_rgb(Color::WHITE), SURFACE_TOLERANCE),
        0,
        "a dim fill must not match white"
    );

    // --- uniformity: a flat fill is, a glyph is not ---
    let flat = capture.frame_at(CANVAS, PIXEL_SCALE, FLAT_GREY, fill(CANVAS, FLAT_GREY));
    assert!(
        flat.is_uniform(region, SURFACE_TOLERANCE),
        "a flat fill over the same background must read as uniform"
    );

    let glyph_mark: Element<'static, ()> = iced::widget::text("X")
        .size(32.0)
        .color(Color::WHITE)
        .into();
    let glyph = capture.frame(CANVAS, glyph_mark);
    let glyph_box = glyph
        .ink_bounds(whole(CANVAS))
        .expect("the glyph paints, so its bounds must exist");

    assert!(
        !glyph.is_uniform(glyph_box, SURFACE_TOLERANCE),
        "a box with a glyph in it must not read as uniform"
    );

    let quiet = Rectangle::new(Point::new(90.0, 4.0), Size::new(24.0, 24.0));
    assert!(
        glyph.is_uniform(quiet, SURFACE_TOLERANCE),
        "a region of the glyph frame away from the ink must still be uniform"
    );
}

/// Both ink readers over one frame of a mark whose box is known exactly:
/// the count over the mark and over bare surface, and the bounds that wrap
/// it, read over the whole frame, over an empty region and over a rect that
/// is not on the canvas at all.
#[test]
fn test_the_ink_readers_count_and_wrap_a_known_mark() {
    let mut capture = Capture::new();
    let mark = Rectangle::new(Point::new(10.0, 10.0), Size::new(20.0, 20.0));

    let frame = capture.frame(
        CANVAS,
        placed(fill(mark.size(), Color::WHITE), mark.position()),
    );

    // ink_count / has_ink
    let inside = frame.ink_count(mark);
    let area = physical_pixels(mark.size(), PIXEL_SCALE);

    assert!(
        inside >= area.saturating_sub(4) && inside <= area,
        "a solid white 20x20 mark at scale 2 must fill its {area} physical \
         pixels, counted {inside}"
    );
    assert!(frame.has_ink(mark));

    let empty = Rectangle::new(Point::new(60.0, 10.0), Size::new(20.0, 20.0));
    assert_eq!(
        frame.ink_count(empty),
        0,
        "a region with nothing painted in it must report zero ink"
    );
    assert!(!frame.has_ink(empty));

    // ink_bounds
    let bounds = frame
        .ink_bounds(whole(CANVAS))
        .expect("the mark paints, so bounds must exist");
    let tol = 1.0 / PIXEL_SCALE;

    assert!(
        (bounds.x - mark.x).abs() <= tol,
        "the ink must start at the mark's x, got {} vs {}",
        bounds.x,
        mark.x
    );
    assert!(
        (bounds.y - mark.y).abs() <= tol,
        "the ink must start at the mark's y, got {} vs {}",
        bounds.y,
        mark.y
    );
    assert!(
        (bounds.width - mark.width).abs() <= tol,
        "the ink must span the mark's width, got {} vs {}",
        bounds.width,
        mark.width
    );
    assert!(
        (bounds.height - mark.height).abs() <= tol,
        "the ink must span the mark's height, got {} vs {}",
        bounds.height,
        mark.height
    );

    assert!(
        frame.ink_bounds(empty).is_none(),
        "an empty region must report no ink bounds"
    );

    let offscreen = Rectangle::new(Point::new(500.0, 500.0), Size::new(20.0, 20.0));
    assert!(
        frame.ink_bounds(offscreen).is_none(),
        "a rect wholly outside the frame must read as empty rather than panic"
    );
}

#[test]
#[should_panic(expected = "falls outside the frame")]
fn test_pixel_panics_when_the_sample_is_outside_the_frame() {
    let mut capture = Capture::new();
    let frame = capture.frame(CANVAS, flat_mark());

    let _ = frame.pixel(500.0, 500.0);
}

#[test]
#[should_panic(expected = "cannot be diffed pixel by pixel")]
fn test_differing_pixels_rejects_frames_of_different_sizes() {
    let mut capture = Capture::new();

    let small = capture.frame(Size::new(40.0, 40.0), flat_mark());
    let large = capture.frame(CANVAS, flat_mark());

    let _ = small.differing_pixels(&large, whole(Size::new(20.0, 20.0)));
}

// ---------------------------------------------------------------------------
// RecordCandidates — the two doors that used to be invisible
// ---------------------------------------------------------------------------

/// One row per publishing hook, so the completeness gate can demand all six.
fn all_kinds_rows() -> Vec<El<'static, (), HeadlessRenderer>> {
    vec![
        iced::widget::container(iced::widget::text("container")).into(),
        iced::widget::scrollable(iced::widget::text("scrollable")).into(),
        wrapped("focusable", true),
        iced::widget::text_input("placeholder", "value").into(),
        hooked(KIND_TEXT, "text"),
        hooked(KIND_CUSTOM, "custom"),
    ]
}

/// Every door [`Operation`] declares has to reach the recorder, and the two
/// that were added late must not have disturbed the ones that already worked.
///
/// Four trees, one session: a real iced `scrollable` (which registers
/// through [`Operation::scrollable`] — until that hook was implemented the
/// trait's default no-op swallowed it), a widget publishing through
/// [`Operation::custom`] (a door no shipped iced widget ever used, so it
/// went entirely unmonitored), the full six-row completeness gate, and a
/// focusable/custom pair.
#[test]
fn test_the_recorder_opens_every_hook_the_operation_trait_declares() {
    let mut harness =
        probe_session(|| iced::widget::scrollable(iced::widget::text("scroll me")).into());

    // A scrollable must be visible to the candidate recorder.
    let kinds = kinds_in(&mut harness, || {
        iced::widget::scrollable(iced::widget::text("scroll me")).into()
    });

    assert!(
        kinds.contains(&KIND_SCROLLABLE),
        "a scrollable must be visible to the candidate recorder, saw {kinds:?}"
    );

    // A widget publishing through `custom` must produce exactly one record.
    let kinds = kinds_in(&mut harness, || hooked(KIND_CUSTOM, "custom_probe"));

    assert_eq!(
        kinds,
        vec![KIND_CUSTOM],
        "a widget publishing through `custom` must produce exactly one `custom` record"
    );

    // The completeness gate: every hook [`Operation`] declares has to be
    // implemented, because an omitted one fails silently rather than loudly.
    let kinds = kinds_in(&mut harness, || {
        iced::widget::column(all_kinds_rows()).into()
    });

    for kind in ALL_KINDS {
        assert!(
            kinds.contains(&kind),
            "`{kind}` never reached the recorder: the hook is not implemented. \
             Saw {kinds:?}"
        );
    }

    // The two new doors must not swallow the ones that already worked.
    let kinds = kinds_in(&mut harness, || {
        let rows: Vec<El<'static, (), HeadlessRenderer>> =
            vec![wrapped("alpha", true), hooked(KIND_CUSTOM, "custom")];

        iced::widget::column(rows).into()
    });

    assert!(
        kinds.contains(&KIND_FOCUSABLE),
        "the focusable door must still report, saw {kinds:?}"
    );
    assert!(
        kinds.contains(&KIND_CUSTOM),
        "the custom door must report, saw {kinds:?}"
    );
}

// ---------------------------------------------------------------------------
// One renderer, many frames
// ---------------------------------------------------------------------------

/// A measurement, not an assertion — asserted timings are flaky on a loaded
/// machine, and the number this exists to produce is the one the report
/// needs.
///
/// It answers the only question a shared capture session could have been
/// worth: how much does a test pay for **not** reusing a renderer across
/// frames? Under `cargo nextest` a session cannot cross a test boundary at
/// all, so the honest comparison is one session driven many times against
/// a fresh session per frame, inside one process.
///
/// Run with:
/// `cargo test -p hoarfrost --lib -- --ignored --nocapture measure_capture`
#[test]
#[ignore = "timing probe: prints renderer build and per-frame cost"]
fn measure_capture_amortisation() {
    const FRAMES: usize = 4;

    let started = Instant::now();
    let mut capture = Capture::new();
    let build_ms = started.elapsed().as_secs_f64() * 1000.0;

    let mut shared_frames = Vec::new();
    for _ in 0..FRAMES {
        let tick = Instant::now();
        let _ = capture.frame(CANVAS, flat_mark());
        shared_frames.push(tick.elapsed().as_secs_f64() * 1000.0);
    }
    let shared_total_ms = build_ms + shared_frames.iter().sum::<f64>();

    let started = Instant::now();
    let mut fresh_total_ms = 0.0;
    for _ in 0..FRAMES {
        let tick = Instant::now();
        let mut fresh = Capture::new();
        let _ = fresh.frame(CANVAS, flat_mark());
        fresh_total_ms += tick.elapsed().as_secs_f64() * 1000.0;
    }
    let fresh_wall_ms = started.elapsed().as_secs_f64() * 1000.0;

    // The one cost a leaked session genuinely avoided: handing the renderer
    // back. A process that never drops one never pays it, so it is measured
    // here rather than argued away. Build first, then time only the drop.
    let mut drop_ms = 0.0;
    for _ in 0..FRAMES {
        let session = Capture::new();
        let tick = Instant::now();
        drop(session);
        drop_ms += tick.elapsed().as_secs_f64() * 1000.0;
    }

    println!("renderer build (Capture::new)     : {build_ms:.1} ms");
    for (i, ms) in shared_frames.iter().enumerate() {
        println!("frame {i} through that session    : {ms:.1} ms");
    }
    println!("{FRAMES} frames, one session         : {shared_total_ms:.1} ms");
    println!(
        "{FRAMES} frames, fresh session each  : {fresh_total_ms:.1} ms (wall {fresh_wall_ms:.1} ms)"
    );
    println!(
        "cost of not sharing             : {:.1} ms ({:.1}x a single build)",
        fresh_total_ms - shared_total_ms,
        if shared_total_ms == 0.0 {
            0.0
        } else {
            (fresh_total_ms - shared_total_ms) / build_ms
        }
    );
    println!(
        "session drop, avg over {FRAMES}       : {:.1} ms",
        drop_ms / FRAMES as f64
    );
}

/// Every controlled shape the suite currently leaks, built **borrowed**
/// instead, and all five through one harness.
///
/// Each block is the construction the matching `leaked()` helper performs,
/// with the leak replaced by a stack local and the model handed straight to
/// a [`Harness`]. These are the compile proofs behind the claim that those
/// helpers can go: if any block stopped compiling, the widening had
/// regressed for that widget family and its `leaked()` helper would have to
/// come back.
///
/// Every model is declared **before** the harness, because the harness's
/// `'a` is fixed at construction and each [`Harness::set_view`] has to
/// satisfy it. The models outlive the session, so nothing is leaked and
/// nothing is borrowed past its owner.
///
/// `text_input_tests`' `Host` is deliberately **not** here. Its leak is not
/// the harness's `'static` view bound — it is the need for one model to be
/// shared-read by a live element while the test writes to it, which no
/// amount of lifetime widening on the harness can alias safely.
#[test]
fn test_every_controlled_shape_borrows_into_a_harness() {
    // `hyperlink_tests::leaked`
    let link_model = HyperlinkModel::external("About", "/about");
    // `segmented_tests::leaked`
    let segment_model: SegmentedModel<&'static str> = SegmentedModel::new(vec![
        SegmentItem::new("left", "Left"),
        SegmentItem::new("right", "Right"),
    ]);
    // `split_button_tests::leaked`
    let split_model = SplitButtonModel::primary(
        "Save",
        vec![SplitMenuRow::new("export".to_string(), "Export PDF")],
    );
    // `toggle_widget_tests::leaked` — radio group
    let radio_id = String::from("a");
    let radio_model =
        RadioGroupModel::new(vec![RadioOption::new(radio_id.clone(), "A")]).with_selected(radio_id);
    // `toggle_widget_tests::leaked` — switch
    let switch_model = SwitchModel::new("Switch").with_on(true);

    let mut harness = Harness::<(), HeadlessRenderer>::headless(|| {
        Hyperlink::controlled(Controlled::new(&link_model), BORROW_FOCUS_ID, |_| None).into()
    });
    assert!(
        !harness.recorded().is_empty(),
        "a borrowed hyperlink model must reach the router"
    );

    harness.set_view(|| {
        SegmentedButton::<(), &'static str>::controlled(
            Controlled::new(&segment_model),
            BORROW_FOCUS_ID,
            |_| None,
        )
        .into()
    });
    assert!(
        !harness.recorded().is_empty(),
        "a borrowed segmented model must reach the router"
    );

    harness.set_view(|| {
        SplitButton::<(), String>::controlled(
            Controlled::new(&split_model),
            BORROW_FOCUS_ID,
            |_| None,
        )
        .into()
    });
    assert!(
        !harness.recorded().is_empty(),
        "a borrowed split-button model must reach the router"
    );

    harness.set_view(|| {
        RadioGroup::<(), String>::controlled(Controlled::new(&radio_model), BORROW_FOCUS_ID, |_| {
            None
        })
        .into()
    });
    assert!(
        !harness.recorded().is_empty(),
        "a borrowed radio-group model must reach the router"
    );

    harness.set_view(|| {
        ToggleSwitch::controlled(Controlled::new(&switch_model), BORROW_FOCUS_ID, |_| None).into()
    });
    assert!(
        !harness.recorded().is_empty(),
        "a borrowed switch model must reach the router"
    );
}

/// The threshold itself, pinned so a change to it cannot silently
/// invalidate every reading above.
#[test]
fn test_the_ink_threshold_separates_a_glyph_from_a_surface() {
    let glyph = u32::from(to_rgb(Color::WHITE)[0]);
    let surface = u32::from(to_rgb(FLAT_GREY)[0]);
    let dim = u32::from(to_rgb(DIM_FILL)[0]);

    assert!(glyph > INK_THRESHOLD, "white must read as ink");
    assert!(surface > INK_THRESHOLD, "the mid grey must read as ink");
    assert!(dim < INK_THRESHOLD, "the dim fill must not read as ink");
}

// ---------------------------------------------------------------------------
// The borrowing view — a model on the test's own stack
// ---------------------------------------------------------------------------

/// Focus id the borrowed-model fixtures register under.
const BORROW_FOCUS_ID: &str = "widened_borrow";

/// A controlled button whose model is **borrowed** from the caller.
///
/// This is the exact shape the `leaked()` helpers across this crate's tests
/// had to build: `Controlled<'a, M>` borrows the model for `'a`, and the
/// element the widget converts into borrows it in turn. Only `Renderer` is
/// left generic, so both harness legs can drive the same borrowed tree.
fn borrowed_button<'a, Renderer>(model: &'a ButtonModel) -> El<'a, (), Renderer>
where
    Renderer: renderer::Renderer + 'a,
{
    Button::controlled(
        Controlled::new(model),
        Element::new(Blank::new(Size::new(48.0, 16.0))),
        BORROW_FOCUS_ID,
        |_| None,
    )
    .into()
}

/// A controlled checkbox whose model is borrowed, for the real-renderer leg.
///
/// `Checkbox`'s `From<..> for Element<..>` needs a text renderer, so this
/// one is concrete to [`HeadlessRenderer`] rather than generic like
/// [`borrowed_button`].
fn borrowed_checkbox(model: &CheckboxModel) -> El<'_, (), HeadlessRenderer> {
    Checkbox::controlled(Controlled::new(model), BORROW_FOCUS_ID, |_| None).into()
}

/// The widened [`Harness`] takes a view closure whose element borrows a
/// model living on the test's stack.
///
/// This is the shape that used to force `Box::leak`. `Controlled<'a, M>`
/// borrows the model, `Element<'a>` borrows it in turn, and the harness's
/// `Fn() -> El<'static, ..>` view bound left leaking as the only way to
/// satisfy both. `Harness::recording` now takes
/// `impl Fn() -> El<'a, ..> + 'a`, so `model` below is a plain local
/// that is dropped when the test returns — nothing is leaked to keep the
/// closure's types happy.
///
/// Note the closure is **not** `move`: a `move` closure would relocate the
/// model into the closure's own capture, and `&model` would then be a
/// reference into the closure body that cannot escape. Borrowing the
/// outer local is what makes `'a` the model's own lifetime.
///
/// The assertions are the ones that matter for the widening: the harness
/// **rebuilds** the borrowed tree on every step, so the borrow has to
/// survive a rebuild and a paint, not merely be accepted at construction.
#[test]
fn test_a_harness_view_can_borrow_a_stack_local_model() {
    let model = ButtonModel::new("borrowed");
    let mut harness = Harness::recording(|| borrowed_button(&model));

    let painted = harness.paint();

    assert!(
        !painted.is_empty(),
        "a stack-borrowed model must still drive a real paint through the harness"
    );
    assert!(
        harness.recorded().iter().any(|(id, _bounds)| id.is_some()),
        "a borrowed tree must still register its ids with the router on rebuild"
    );
}

/// The same widening on the real-renderer leg, driven twice.
///
/// Two rebuilds over one borrowed model is the regression this pins: the
/// first build takes the element out of the closure, the second does it
/// again after the cache has come back around, and neither may require the
/// model to be `'static`.
#[test]
fn test_a_borrowing_harness_rebuilds_the_same_model_twice() {
    let model = CheckboxModel::new("borrowed twice").with_state(CheckState::Unchecked);
    let mut harness = Harness::headless(|| borrowed_checkbox(&model));

    let first = harness.recorded();
    harness.render();
    let second = harness.recorded();

    assert_eq!(
        first, second,
        "rebuilding a borrowed tree must be deterministic across steps"
    );
    assert!(
        !first.is_empty(),
        "the borrowed model must reach the real renderer at all"
    );
}

// ---------------------------------------------------------------------------
// Gap-closer 4 — one delivery, both halves of the claim
// ---------------------------------------------------------------------------

/// Focus id the click-and-frame fixtures register under.
const CLICK_FOCUS_ID: &str = "capture_click_frame";

/// A point the checkbox's own hit target certainly covers.
fn click_target() -> Point {
    Point::new(12.0, 12.0)
}

/// A checkbox that reports its toggle instead of swallowing it.
fn clicky_checkbox(model: &CheckboxModel) -> Element<'_, String> {
    Checkbox::controlled(Controlled::new(model), CLICK_FOCUS_ID, |_| {
        Some(String::from("toggled"))
    })
    .into()
}

/// Both event-delivery paths, from one session: the click that reports the
/// message it produced **and** the frame it left, and the general event path
/// that reports the same pair.
///
/// The half-claim this replaces: `click` gave the messages and never drew,
/// `frame_after` drew and threw the messages away, so proving "the click
/// fired and the widget repainted" meant running the gesture twice and
/// hoping both halves described the same run. Here they cannot diverge.
///
/// The general path's message list is the tree's own — cross-checked against
/// the harness over identical events rather than asserted against a number
/// that would pass on an empty vector either way. Each half diffs against a
/// rest frame taken from **its own** model, so a repaint can only be credited
/// to the gesture and never to a caption that differs between the two.
#[test]
fn test_one_delivery_reports_its_message_and_its_frame() {
    let at = click_target();
    let mut capture = Capture::new();

    // --- the click: message and frame from one delivery ---
    let model = CheckboxModel::new("Click and frame");
    let rest = capture.frame(CANVAS, clicky_checkbox(&model));
    let (clicked, messages) = capture.click_and_frame(CANVAS, clicky_checkbox(&model), at);

    assert_eq!(
        messages,
        vec![String::from("toggled")],
        "the click must produce exactly the widget's toggle message"
    );
    assert_repainted(&rest, &clicked, whole(CANVAS), "clicking the checkbox");

    // --- the general event path: the same pair ---
    let injected = CheckboxModel::new("Injected hover");
    let injected_rest = capture.frame(CANVAS, clicky_checkbox(&injected));
    let (hovered, messages) = capture.frame_and_messages_after(
        CANVAS,
        clicky_checkbox(&injected),
        &[moved(at)],
        mouse::Cursor::Available(at),
    );

    let mut harness = Harness::<String, HeadlessRenderer>::headless(|| clicky_checkbox(&injected));
    let via_harness = harness.hover(at);

    assert_eq!(
        messages, via_harness,
        "the capture path and the harness must report the same reply to the \
         same event, or one of them is not delivering it"
    );
    assert_repainted(
        &injected_rest,
        &hovered,
        whole(CANVAS),
        "hovering the checkbox",
    );
}

// ---------------------------------------------------------------------------
// Gap-closer 3 — the shared panic reader and its guard
// ---------------------------------------------------------------------------

/// The panic reader and the guard that uses it, both directions.
///
/// A panic payload is `Box<dyn Any + Send>` and arrives in whichever shape
/// the panicking side liked; all four shapes read back the same way. And the
/// guard is load-bearing in both directions: it returns the message when the
/// probe fires, and refuses to pass a probe that stayed quiet.
#[test]
fn test_the_panic_reader_and_its_guard_hold_in_both_directions() {
    let string_payload: Result<(), Box<dyn Any + Send>> =
        Err(Box::new(String::from("a String payload")));
    let str_payload: Result<(), Box<dyn Any + Send>> = Err(Box::new("a &str payload"));
    let opaque_payload: Result<(), Box<dyn Any + Send>> = Err(Box::new(0xdead_beef_u32));
    let no_panic: Result<(), Box<dyn Any + Send>> = Ok(());

    assert_eq!(panic_text(&string_payload), "a String payload");
    assert_eq!(panic_text(&str_payload), "a &str payload");
    assert_eq!(panic_text(&opaque_payload), "<non-string panic payload>");
    assert_eq!(panic_text(&no_panic), "<no panic>");

    let message = assert_panics("a deliberate panic", || panic!("the message it carries"));

    assert!(
        message.contains("the message it carries"),
        "the guard must hand the caller the wording to pin, got {message}"
    );

    let guard = catch_unwind(AssertUnwindSafe(|| {
        let _ = assert_panics("a guard over a quiet closure", || {});
    }));

    assert!(
        guard.is_err(),
        "a probe that never panicked must not satisfy the guard, or every \
         load-bearing claim built on it is vacuous"
    );
    assert!(
        panic_text(&guard).contains("a guard over a quiet closure"),
        "the guard's own failure must name what it was guarding"
    );
}
