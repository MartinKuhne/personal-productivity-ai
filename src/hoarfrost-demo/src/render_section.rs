//! Section-level render assertions shared by the demo's ink tests.
//!
//! Every pane is a column of [`Section`]s, and every section has the same
//! three regions: the card, the caption band, and the body the controls
//! sit in. Reading the frame through those regions is what lets one
//! assertion cover forty builders — "the body of this card paints
//! something" is the render-level statement of "this section works", and
//! it is the statement the menu bug failed.
//!
//! The regions come from the layout probe, never from a constant, so a
//! section that lays out wrong moves the region with it instead of
//! hiding behind a stale number.
//!
//! # Why two builds
//!
//! `Section` owns an `iced::Element`, which is not `Clone`. Geometry and
//! pixels therefore come from two calls to the same pure builder rather
//! than one build read twice. The builders are pure functions of the
//! pane state, so the two trees are identical.

use iced::{Rectangle, Size};

use hoarfrost::composite::card;
use hoarfrost::token::color::Palette;

use crate::layout_probe::Probe;
use crate::render_capture::{Frame, SURFACE_TOLERANCE, Studio, to_rgb};
use crate::section::{HEADING_GAP, Section};

/// The fewest ink pixels a section body may show and still count as painted.
///
/// Deliberately tiny. The cheapest real section in the demo — a single
/// row of small text — paints thousands of pixels above the threshold,
/// so twenty-four is far above "nothing" and far below anything real,
/// which is where a presence floor belongs: it must never be the thing
/// that fails, only the thing that catches a blank.
pub const MIN_BODY_INK: usize = 24;

/// The fewest pixels a named control must paint over its card to count as
/// visible.
///
/// A control that lays out at a real rect and paints nothing over the
/// card behind it is the menu bug again. Two pixels is below every real
/// control's own fill and above the card's own uniform surface.
pub const MIN_CONTROL_PAINT: usize = 2;

/// One demo section, read as geometry and as pixels.
pub struct Captured {
    /// The section's stable layout id.
    pub id: String,
    /// The card's painted bounds.
    pub card: Rectangle,
    /// The caption label's bounds.
    pub caption: Rectangle,
    /// The body region: inside the card's padding, below the caption band.
    pub body: Rectangle,
    /// The geometry snapshot.
    pub probe: Probe,
    /// The painted frame.
    pub frame: Frame,
}

impl Captured {
    /// Captures the section `build()` produces at `viewport`.
    ///
    /// # Panics
    /// When the section's card or caption is not addressable in the probe.
    #[must_use]
    pub fn capture<'a>(
        studio: &mut Studio,
        viewport: Size,
        build: impl Fn() -> Section<'a>,
    ) -> Self {
        let probe = studio.probe(viewport, build().element);
        let frame = studio.frame(viewport, build().element);
        let (id, card, caption) = section_geometry(&probe);

        Self {
            id,
            body: body_region(card, caption),
            card,
            caption,
            probe,
            frame,
        }
    }

    /// Bright ink inside the body region.
    #[must_use]
    pub fn body_ink(&self) -> usize {
        self.frame.ink_count(self.body)
    }

    /// Pixels inside the body region painted over the section's own card.
    #[must_use]
    pub fn body_paint(&self) -> usize {
        self.frame
            .painted_count(self.body, card_fill_rgb(), SURFACE_TOLERANCE)
    }

    /// The entry for a named widget inside this section.
    ///
    /// # Panics
    /// When `id` is not present in the capture.
    #[must_use]
    pub fn control(&self, id: &str) -> Rectangle {
        self.probe
            .find(id)
            .unwrap_or_else(|| panic!("section {} recorded no widget `{id}`", self.id))
            .bounds
    }

    /// How much the named control paints over the card behind it.
    #[must_use]
    pub fn control_paint(&self, id: &str) -> usize {
        self.frame
            .painted_count(self.control(id), card_fill_rgb(), SURFACE_TOLERANCE)
    }
}

/// The section's id, card bounds and caption bounds, read out of a probe.
///
/// # Panics
/// When the capture holds no `demo.section.*` entry, or that entry has no
/// matching `.caption`. Every builder goes through
/// [`section`](crate::section::section), so a miss means the section was
/// not built the way the demo's own contract requires.
fn section_geometry(probe: &Probe) -> (String, Rectangle, Rectangle) {
    let id = probe
        .entries()
        .iter()
        .find(|e| e.id.starts_with(crate::section::SECTION_ID_PREFIX))
        .map(|e| e.id.clone())
        .expect("every section carries a `demo.section.` id");

    let card = probe
        .find(&id)
        .unwrap_or_else(|| panic!("section {id} recorded no card bounds"))
        .bounds;
    let caption = probe
        .find(&format!("{}{}", id, crate::section::CAPTION_ID_SUFFIX))
        .unwrap_or_else(|| panic!("section {id} recorded no caption bounds"))
        .bounds;

    (id, card, caption)
}

/// The card fill a demo section paints, as the channels it must produce.
#[must_use]
pub fn card_fill_rgb() -> [u8; 3] {
    to_rgb(card::resolve_style(card::CardElevation::Rest, &Palette::dark()).background)
}

/// Before-and-after frames of one section, with the region to read them in.
///
/// Produced by [`capture_diff`], which builds the same section from two
/// host states. Reading the two frames against each other is what turns
/// "the model changed" into "the screen changed".
pub struct Diff {
    /// The section's body region, from the *before* frame's layout.
    pub body: Rectangle,
    /// The frame before the change.
    pub before: Frame,
    /// The frame after the change.
    pub after: Frame,
}

/// Builds one section twice — once from `before`, once from `after` —
/// and rasterises both.
///
/// The generic state type lets the atoms pane and the composites pane
/// share it: both hold their controlled-widget models in a struct and
/// both build sections as pure functions of that struct.
#[must_use]
pub fn capture_diff<S>(
    studio: &mut Studio,
    viewport: Size,
    build: impl for<'a> Fn(&'a S) -> Section<'a>,
    before: &S,
    after: &S,
) -> Diff {
    let probe = studio.probe(viewport, build(before).element);
    let (_id, card, caption) = section_geometry(&probe);

    let before_frame = studio.frame(viewport, build(before).element);
    let after_frame = studio.frame(viewport, build(after).element);

    Diff {
        body: body_region(card, caption),
        before: before_frame,
        after: after_frame,
    }
}

/// The body region of a section card: inside its padding, below the
/// caption band.
///
/// Everything a section's controls live in, and nothing the frame or the
/// heading contributes. A section that lays out but paints nothing shows
/// up here as an empty region.
#[must_use]
pub fn body_region(card: Rectangle, caption: Rectangle) -> Rectangle {
    let top = caption.y + caption.height + HEADING_GAP;
    let bottom = (card.y + card.height - card::PADDING).max(top);

    Rectangle {
        x: card.x + card::PADDING,
        y: top,
        width: (card.width - card::PADDING * 2.0).max(0.0),
        height: (bottom - top).max(0.0),
    }
}

/// Asserts a captured section actually paints inside its own body.
///
/// The cheap, broad assertion: it catches a section whose controls lay
/// out at real bounds and draw nothing, which is precisely the class the
/// navbar menu belonged to.
pub fn assert_body_painted(captured: &Captured) {
    assert!(
        captured.body.height > 0.0,
        "section {} has no body region at all: card {:?} caption {:?}",
        captured.id,
        captured.card,
        captured.caption
    );
    assert!(
        captured.body_ink() >= MIN_BODY_INK,
        "section {} painted no readable ink inside its own body {:?} \
         (card {:?}) — the controls laid out but nothing was drawn",
        captured.id,
        captured.body,
        captured.card
    );
    assert!(
        captured.body_paint() >= MIN_BODY_INK,
        "section {} shows nothing over its own card fill inside {:?}",
        captured.id,
        captured.body
    );
    assert!(
        !captured.frame.is_uniform(captured.body, SURFACE_TOLERANCE),
        "section {}'s body region is a flat, featureless rectangle: {:?}",
        captured.id,
        captured.body
    );
}
