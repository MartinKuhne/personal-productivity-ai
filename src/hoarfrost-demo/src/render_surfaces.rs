//! Render-level (ink) coverage for the window chrome and the scrollable fold.
//!
//! Where `render_atoms` and `render_composites` ask "does this section draw?",
//! this module asks the two questions the chrome and the fold actually get wrong:
//!
//! * **Occlusion** — the navbar and the status bar are painted once, on their
//!   own bands, and nothing the panes do may repaint over them. A pane that
//!   grew, moved or painted past its bounds would show up here as changed
//!   pixels in a band that must stay byte-identical.
//! * **Clipping** — content below a `scrollable`'s fold must not bleed into
//!   the space beneath it. This is asserted by leaving real empty space under
//!   the fold and proving that space stays the window ground.
//!
//! Every claim here is about absolute pixels. Nothing is compared against a
//! golden image, so a palette change cannot make a broken paint pass green.

use iced::widget::{Column, scrollable};
use iced::{Length, Point, Rectangle, Size};

use hoarfrost::test_helpers::{SURFACE_TOLERANCE, to_rgb};
use hoarfrost::token::color::Palette;

use crate::app::{DemoApp, Message};
use crate::chrome;
use crate::layout_probe::{Probe, VIEWPORT, capture_session};
use crate::pane_atoms::{self, AtomsState};

/// A rectangle covering the whole viewport.
fn ground(viewport: Size) -> Rectangle {
    Rectangle::new(Point::ORIGIN, viewport)
}

/// A full-width horizontal band.
fn band(viewport: Size, y: f32, height: f32) -> Rectangle {
    Rectangle::new(Point::new(0.0, y), Size::new(viewport.width, height))
}

/// The navbar's own band: from the top of the window to the bottom of the
/// Close button, which sits inside the navbar row.
fn navbar_band(probe: &crate::layout_probe::Probe) -> Rectangle {
    let close = probe
        .find(chrome::CLOSE_FOCUS_ID)
        .map(|entry| entry.bounds)
        .expect("the chrome registers its Close button");
    Rectangle::new(
        Point::ORIGIN,
        Size::new(VIEWPORT.width, close.y + close.height),
    )
}

/// The strip of window between the two panes, derived from where the panes
/// stop and start rather than from a divider id — the divider registers none.
fn divider_strip(probe: &crate::layout_probe::Probe) -> Rectangle {
    let right_of_left_pane = probe
        .all_entries()
        .iter()
        .filter(|e| e.id.starts_with("demo.atoms"))
        .map(|e| e.bounds.x + e.bounds.width)
        .fold(0.0_f32, f32::max);
    let left_of_right_pane = probe
        .all_entries()
        .iter()
        .filter(|e| e.id.starts_with("demo.composites"))
        .map(|e| e.bounds.x)
        .fold(f32::MAX, f32::min);

    assert!(
        right_of_left_pane < left_of_right_pane,
        "the left pane (right edge {right_of_left_pane}) should stop before the \
         right pane (left edge {left_of_right_pane}) begins — there is no strip \
         left for the split divider to paint in"
    );

    Rectangle::new(
        Point::new(right_of_left_pane, 40.0),
        Size::new(left_of_right_pane - right_of_left_pane, 700.0),
    )
}

// ---------------------------------------------------------------------------
// The four bands
// ---------------------------------------------------------------------------

#[test]
fn the_window_paints_four_distinct_bands() {
    let app = DemoApp::boot().0;
    let mut capture = capture_session();
    let frame = capture.frame(VIEWPORT, app.view());
    let probe = Probe::capture_with(capture.renderer_mut(), VIEWPORT, app.view());

    let navbar = navbar_band(&probe);
    let status = probe
        .find(chrome::STATUS_BAR_ID)
        .map(|e| e.bounds)
        .expect("the chrome registers its status bar");
    let divider = divider_strip(&probe);

    let left_pane = Rectangle::new(
        Point::new(0.0, navbar.height),
        Size::new(divider.x, status.y - navbar.height),
    );
    let right_pane = Rectangle::new(
        Point::new(divider.x + divider.width, navbar.height),
        Size::new(
            VIEWPORT.width - divider.x - divider.width,
            status.y - navbar.height,
        ),
    );

    for (name, rect) in [
        ("navbar", navbar),
        ("left pane", left_pane),
        ("split divider", divider),
        ("right pane", right_pane),
        ("status bar", status),
    ] {
        let ink = frame.ink_count(rect);
        assert!(
            ink > 200,
            "{name} at {rect:?} painted only {ink} ink pixels — the band is \
             effectively blank on a window that should be showing it"
        );
    }
}

#[test]
fn the_close_button_paints_inside_its_recorded_rect() {
    let app = DemoApp::boot().0;
    let mut capture = capture_session();
    let frame = capture.frame(VIEWPORT, app.view());
    let probe = Probe::capture_with(capture.renderer_mut(), VIEWPORT, app.view());

    let close = probe
        .find(chrome::CLOSE_FOCUS_ID)
        .map(|e| e.bounds)
        .expect("the chrome registers its Close button");

    assert!(
        frame.ink_count(close) > 50,
        "the Close button at {close:?} paints {ink} ink pixels — its label and \
         its Danger fill should both be visible",
        ink = frame.ink_count(close)
    );
    assert!(
        !frame.is_uniform(close, SURFACE_TOLERANCE),
        "the Close button's rect is one flat colour: nothing was drawn on it"
    );
}

#[test]
fn the_split_divider_paints_a_grip_not_a_blank_strip() {
    let app = DemoApp::boot().0;
    let mut capture = capture_session();
    let frame = capture.frame(VIEWPORT, app.view());
    let probe = Probe::capture_with(capture.renderer_mut(), VIEWPORT, app.view());

    let strip = divider_strip(&probe);
    let palette = Palette::dark();

    assert!(
        !frame.is_uniform(strip, SURFACE_TOLERANCE),
        "the strip between the panes {strip:?} is uniform — the divider drew \
         nothing into the gap it owns"
    );
    assert!(
        frame.painted_count(strip, to_rgb(palette.surface_raised), SURFACE_TOLERANCE) > 20,
        "the divider strip shows nothing brighter than the card fill: the grip \
         stroke is missing"
    );
}

// ---------------------------------------------------------------------------
// The status bar read-out is painted, not merely stored
// ---------------------------------------------------------------------------

#[test]
fn the_status_bar_band_is_its_own_surface() {
    let app = DemoApp::boot().0;
    let mut capture = capture_session();
    let frame = capture.frame(VIEWPORT, app.view());
    let probe = Probe::capture_with(capture.renderer_mut(), VIEWPORT, app.view());

    let status = probe
        .find(chrome::STATUS_BAR_ID)
        .map(|e| e.bounds)
        .expect("the chrome registers its status bar");
    let palette = Palette::dark();

    // A point on the band that is fill rather than glyph: the vertical centre,
    // far from the left-aligned event text and the right-aligned count.
    let fill = frame.pixel(
        status.x + status.width / 2.0,
        status.y + status.height / 2.0,
    );
    let expected = to_rgb(palette.surface_raised);
    let off = fill
        .iter()
        .zip(expected.iter())
        .map(|(a, b)| (*a as i16 - *b as i16).abs())
        .max()
        .unwrap_or(255);

    assert!(
        off <= i16::from(SURFACE_TOLERANCE),
        "the status bar band should be painted from the raised-surface token \
         {expected:?}, but its centre pixel is {fill:?} (off by {off})"
    );
    assert!(
        !frame.is_uniform(status, SURFACE_TOLERANCE),
        "the status bar band is its fill colour everywhere: neither the event \
         read-out nor the widget count was painted"
    );
}

#[test]
fn a_changed_event_repaints_the_status_bar_readout() {
    let mut app = DemoApp::boot().0;
    let mut capture = capture_session();

    let before = capture.frame(VIEWPORT, app.view());
    let status = Probe::capture_with(capture.renderer_mut(), VIEWPORT, app.view())
        .find(chrome::STATUS_BAR_ID)
        .map(|e| e.bounds)
        .expect("the chrome registers its status bar");

    let _task = app.update(Message::SplitCommitted(0.42));
    let after = capture.frame(VIEWPORT, app.view());

    let changed = after.differing_pixels(&before, status);
    assert!(
        changed > 8,
        "the event read-out changed in the model but only {changed} pixels in \
         the status bar band moved — the band is showing a stale painting"
    );
}

// ---------------------------------------------------------------------------
// Occlusion — the panes never paint over the chrome
// ---------------------------------------------------------------------------

#[test]
fn moving_the_panes_leaves_the_navbar_band_byte_identical() {
    let mut app = DemoApp::boot().0;
    let mut capture = capture_session();

    let before = capture.frame(VIEWPORT, app.view());
    let navbar = navbar_band(&Probe::capture_with(
        capture.renderer_mut(),
        VIEWPORT,
        app.view(),
    ));

    // A committed split moves both panes' widths, so the pane region must
    // change; the navbar occupies a band above that region and must not.
    let _task = app.update(Message::SplitCommitted(0.72));
    let after = capture.frame(VIEWPORT, app.view());

    let over_navbar = after.differing_pixels(&before, navbar);
    assert_eq!(
        over_navbar, 0,
        "{over_navbar} pixels changed inside the navbar band {navbar:?} while \
         only the panes were asked to move — a pane is painting over the chrome"
    );
}

#[test]
fn moving_the_panes_actually_moves_them() {
    let mut app = DemoApp::boot().0;
    let mut capture = capture_session();

    let before = capture.frame(VIEWPORT, app.view());
    let divider = divider_strip(&Probe::capture_with(
        capture.renderer_mut(),
        VIEWPORT,
        app.view(),
    ));

    let _task = app.update(Message::SplitCommitted(0.72));
    let after = capture.frame(VIEWPORT, app.view());

    let moved = after.differing_pixels(&before, divider);
    assert!(
        moved > 8,
        "the split ratio changed but only {moved} pixels moved in the divider \
         strip {divider:?} — the control is inert, so the occlusion test above \
         would pass for the wrong reason"
    );
}

// ---------------------------------------------------------------------------
// Clipping at the fold
// ---------------------------------------------------------------------------

/// The atoms pane inside a scrollable whose viewport stops well short of the
/// content, with real empty window space left beneath it.
///
/// The empty space is the point: if the scrollable failed to clip, the
/// sections below the fold would paint straight through it.
fn folded_pane(state: &AtomsState, fold: f32) -> iced::Element<'_, Message> {
    Column::new()
        .width(Length::Fill)
        .height(Length::Fill)
        .push(
            scrollable(pane_atoms::atoms_pane(state).sections)
                .width(Length::Fill)
                .height(Length::Fixed(fold)),
        )
        .push(
            iced::widget::Space::new()
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into()
}

#[test]
fn nothing_paints_below_the_scrollable_fold() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let viewport = Size::new(700.0, 1600.0);
    let fold = 420.0;

    let frame = capture.frame(viewport, folded_pane(&state, fold));
    let below = band(viewport, fold + 1.0, viewport.height - fold - 1.0);

    assert_eq!(
        frame.viewport(),
        viewport,
        "the capture must come back at the viewport it was asked for — every \
         ink measurement below is indexed against it"
    );
    assert_eq!(
        frame.ink_count(below),
        0,
        "{} ink pixels were painted below the fold at y={fold} — the scrollable \
         is leaking its off-screen content into the window",
        frame.ink_count(below)
    );
    assert!(
        frame.is_uniform(below, SURFACE_TOLERANCE),
        "the space below the fold {below:?} is not even a flat ground colour"
    );
}

#[test]
fn the_fold_hides_content_that_really_was_there() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let viewport = Size::new(700.0, 1600.0);
    let fold = 420.0;

    // Same content, same width, but a viewport tall enough to hold all of it:
    // the band that was empty above must fill with ink. Without this control
    // the zero below the fold would be indistinguishable from an empty pane.
    let folded = capture.frame(viewport, folded_pane(&state, fold));
    let unfolded = capture.frame(viewport, folded_pane(&state, viewport.height));

    let below = band(viewport, fold + 1.0, viewport.height - fold - 1.0);
    assert!(
        folded.ink_count(below) == 0,
        "with the fold at {fold} the band below it should hold no ink"
    );
    assert!(
        unfolded.ink_count(below) > 1000,
        "with the fold removed the same band should fill with content, but it \
         holds only {} — the content below the fold was never there to hide",
        unfolded.ink_count(below)
    );
}

#[test]
fn the_fold_clips_at_the_viewport_edge_not_earlier() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let viewport = Size::new(700.0, 1600.0);
    let fold = 420.0;

    let frame = capture.frame(viewport, folded_pane(&state, fold));
    let above = band(viewport, 0.0, fold);

    assert!(
        frame.ink_count(above) > 10_000,
        "above the fold the pane painted only {} ink pixels — clipping should \
         hide what is below the edge, not thin out what is above it",
        frame.ink_count(above)
    );
    assert!(
        frame.ink_count(ground(viewport)) > frame.ink_count(below_fold(viewport, fold)),
        "the whole viewport should hold more ink than the empty band beneath \
         the fold"
    );
}

fn below_fold(viewport: Size, fold: f32) -> Rectangle {
    band(viewport, fold + 1.0, viewport.height - fold - 1.0)
}

#[test]
fn a_shorter_fold_hides_more_than_a_taller_one() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let viewport = Size::new(700.0, 1600.0);

    let short = capture.frame(viewport, folded_pane(&state, 200.0));
    let tall = capture.frame(viewport, folded_pane(&state, 600.0));

    let short_only = band(viewport, 201.0, 399.0);
    assert_eq!(
        short.ink_count(short_only),
        0,
        "the band between the two folds should be empty under the short fold"
    );
    assert!(
        tall.ink_count(short_only) > 1000,
        "under the taller fold the same band should be showing content, got {}",
        tall.ink_count(short_only)
    );
}
