//! Render-level tests for the demo's menu surface.
//!
//! These assert the **drawn result**, not the model. A state-only test
//! (`active_menu_id == Some("file")`) passes whether or not anything is
//! painted; every test here asks the framebuffer or the laid-out widget
//! tree what the user would actually see.

use std::collections::BTreeSet;

use iced::{Point, Rectangle};

use hoarfrost::test_helpers::to_rgb;

use crate::app::{DemoApp, Message};
use crate::data;
use crate::layout_probe::{Entry, Probe, VIEWPORT, capture_session};
use crate::menu_surface;

/// The menu card's fill as the channels it must produce on screen.
///
/// The mounted menu card is painted from `surface_overlay`, one rung above
/// the [`crate::section`] card's `surface_raised` — that is what makes it
/// read as floating over the panes. The shared `card_fill_rgb` is the
/// *section* card's fill, so this test names its own token.
fn menu_card_fill_rgb() -> [u8; 3] {
    to_rgb(hoarfrost::token::color::Palette::dark().surface_overlay)
}

/// A booted app with `message` already applied.
fn booted_with(message: Message) -> DemoApp {
    let (mut app, _) = DemoApp::boot();
    let _ = app.update(message);
    app
}

/// The card's rendered bounds, read out of the widget tree.
fn card_bounds(app: &DemoApp, trigger: &str) -> Option<Rectangle> {
    Probe::capture(VIEWPORT, app.view())
        .find(&menu_surface::card_id(trigger))
        .map(|entry| entry.bounds)
}

// ---------------------------------------------------------------------------
// Baseline — nothing is mounted while no trigger is latched
// ---------------------------------------------------------------------------

#[test]
fn no_card_surface_is_rendered_while_no_trigger_is_latched() {
    let (app, _) = DemoApp::boot();
    let probe = Probe::capture(VIEWPORT, app.view());

    assert!(app.active_menu_id().is_none());
    assert!(
        probe.with_prefix(menu_surface::CARD_ID_PREFIX).is_empty(),
        "a closed menu must mount no card: {:?}",
        probe.with_prefix(menu_surface::CARD_ID_PREFIX)
    );
    assert!(
        probe.with_prefix(menu_surface::ROW_ID_PREFIX).is_empty(),
        "a closed menu must mount no rows: {:?}",
        probe.with_prefix(menu_surface::ROW_ID_PREFIX)
    );
}

// ---------------------------------------------------------------------------
// The card actually renders
// ---------------------------------------------------------------------------

#[test]
fn opening_a_trigger_mounts_a_card_that_is_rendered_with_non_zero_bounds() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    assert_eq!(app.active_menu_id(), Some("file"));

    let bounds = card_bounds(&app, "file").expect("the open menu must render a card");

    assert!(
        bounds.width > 0.0 && bounds.height > 0.0,
        "the mounted card collapsed to {bounds:?}"
    );
    assert!(
        crate::layout_probe::is_finite_rect(&bounds),
        "the mounted card has non-finite bounds: {bounds:?}"
    );
}

#[test]
fn the_mounted_card_sits_below_the_navbar_and_inside_the_viewport() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let bounds = card_bounds(&app, "file").expect("the open menu must render a card");

    assert!(
        bounds.y >= menu_surface::ANCHOR_BAND_HEIGHT,
        "the card must hang below the navbar band, got y={} (band {})",
        bounds.y,
        menu_surface::ANCHOR_BAND_HEIGHT
    );
    assert!(
        bounds.x >= 0.0 && bounds.x + bounds.width <= VIEWPORT.width,
        "the card overflows the viewport horizontally: {bounds:?}"
    );
    assert!(
        bounds.y + bounds.height <= VIEWPORT.height,
        "the card overflows the viewport vertically: {bounds:?}"
    );
}

#[test]
fn the_card_is_painted_not_just_laid_out() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let bounds = card_bounds(&app, "file").expect("the open menu must render a card");

    let mut capture = capture_session();
    let frame = capture.frame(VIEWPORT, app.view());
    assert!(
        frame.has_ink(bounds),
        "nothing was painted inside the card's own bounds {bounds:?}"
    );
}

// ---------------------------------------------------------------------------
// Rows are present and hit-testable
// ---------------------------------------------------------------------------

/// The `MENU_ROWS` entries that carry a key — the navigable rows.
///
/// These are the rows a test addresses by id. Separators are excluded
/// because they are not navigable, **not** because they are unnamed: the
/// demo publishes them under their declared `m-sep-1` / `m-sep-2` keys
/// (see [`data::menu_row_key`]), so they take part in the id set even
/// though no Tab stop ever lands on them.
fn keyed_rows() -> Vec<(&'static str, &'static str)> {
    data::MENU_ROWS
        .iter()
        .filter(|(_, _, kind)| *kind != "separator")
        .map(|(key, label, _)| (*key, *label))
        .collect()
}

#[test]
fn every_keyed_menu_row_is_rendered_as_a_non_degenerate_rect() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let probe = Probe::capture(VIEWPORT, app.view());

    // Count DISTINCT ROW IDS, never records. A navigable row publishes
    // two records under the one id it was built with — its `focusable`
    // box ("can Tab stop here?") and its own `container` box ("where is
    // this row?") — so the record count says 12 for a menu that mounts
    // seven rows. The claim under test is about rows, so the count has to
    // be over the ids the records are published under.
    let rendered: BTreeSet<String> = probe
        .with_prefix(menu_surface::ROW_ID_PREFIX)
        .into_iter()
        // A row's published caption carries the row's own id as its prefix
        // (`…row.file.label`), so a prefix query alone would count it as a
        // second row. Ask whether the tail is a role, not whether the head
        // matches.
        .filter(|entry| !hoarfrost::identity::is_sub_part_name(&entry.id))
        .map(|entry| entry.id.clone())
        .collect();

    // Nothing may be published under an id whose final segment is empty:
    // that is the malformed `demo.menu.row.` a keyless separator used to
    // produce, and an id like that addresses no row at all.
    let malformed: Vec<&String> = rendered
        .iter()
        .filter(|id| id.ends_with(hoarfrost::identity::ROLE_SEPARATOR))
        .collect();
    assert!(
        malformed.is_empty(),
        "a menu row published an id with an empty final segment: {malformed:?}"
    );

    let keyed = keyed_rows();
    let keyed_rendered: BTreeSet<String> = keyed
        .iter()
        .map(|(key, _)| menu_surface::row_id(key))
        .filter(|id| rendered.contains(id))
        .collect();
    assert_eq!(
        keyed.len(),
        keyed_rendered.len(),
        "the render must carry one named row widget per keyed menu row; \
         distinct row ids rendered: {rendered:?}"
    );

    for (key, label) in keyed {
        let id = menu_surface::row_id(key);
        // Every record the row publishes — focusable and container both —
        // has to be a real box, not just the first one `find` happens to
        // return.
        let records: Vec<&Entry> = probe
            .entries()
            .iter()
            .filter(|entry| entry.id == id)
            .collect();
        assert!(
            !records.is_empty(),
            "row {key} ({label}) publishes no record under `{id}`"
        );

        for entry in records {
            assert!(
                entry.bounds.width > 0.0 && entry.bounds.height > 0.0,
                "row {key} ({label}) [{}] rendered degenerate: {:?}",
                entry.kind,
                entry.bounds
            );
            assert!(
                crate::layout_probe::is_finite_rect(&entry.bounds),
                "row {key} ({label}) [{}] has non-finite bounds: {:?}",
                entry.kind,
                entry.bounds
            );
            assert!(
                entry.bounds.x >= 0.0 && entry.bounds.x + entry.bounds.width <= VIEWPORT.width,
                "row {key} ({label}) [{}] falls outside the viewport: {:?}",
                entry.kind,
                entry.bounds
            );
        }
    }
}

#[test]
fn the_card_publishes_exactly_one_distinct_row_id_per_declared_menu_row() {
    // The record-level mirror of the keyed-row test, checked both ways.
    // The fixture declares seven rows — five keyed, two separators — and
    // the mounted card must publish seven distinct row ids: no row
    // missing, no id invented, and no two rows collapsed onto one.
    // A keyless separator would collapse both of them into the malformed
    // `demo.menu.row.` and fail this on the spot.
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let probe = Probe::capture(VIEWPORT, app.view());

    let rendered: BTreeSet<String> = probe
        .with_prefix(menu_surface::ROW_ID_PREFIX)
        .into_iter()
        .filter(|entry| !hoarfrost::identity::is_sub_part_name(&entry.id))
        .map(|entry| entry.id.clone())
        .collect();
    let declared: BTreeSet<String> = data::MENU_ROWS
        .iter()
        .map(|(key, _, _)| menu_surface::row_id(key))
        .collect();

    assert_eq!(
        data::MENU_ROWS.len(),
        declared.len(),
        "the fixture's rows must map to distinct ids"
    );
    assert_eq!(
        declared, rendered,
        "the card must publish exactly one distinct row id per declared menu row"
    );
}

#[test]
fn the_separators_still_take_vertical_space_in_the_card() {
    // Separators are not navigable, but the card's height must still
    // account for them or the rows below would be drawn over them.
    let menu = data::menu_model();
    let separators = menu.rows().iter().filter(|row| row.is_separator()).count();
    assert_eq!(2, separators, "the fixture declares two separators");

    let keyed_height: f32 = menu
        .rows()
        .iter()
        .filter(|row| !row.is_separator())
        .map(hoarfrost::surface::menu::view::row_height)
        .sum::<f32>();

    let card = menu_surface::card_size(&menu);
    assert!(
        card.height > keyed_height + menu_surface::CARD_INSET * 2.0 - 0.01,
        "the card is only tall enough for the keyed rows: {} vs {keyed_height}",
        card.height
    );
}

#[test]
fn clicking_a_rendered_row_publishes_a_message() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let probe = Probe::capture(VIEWPORT, app.view());

    let entry = probe
        .find(&menu_surface::row_id("m-new"))
        .expect("the New document row must be rendered");

    let centre = Point::new(
        entry.bounds.x + entry.bounds.width / 2.0,
        entry.bounds.y + entry.bounds.height / 2.0,
    );

    let mut capture = capture_session();
    // `click_and_frame` rather than `click`: the bare helper clicks against
    // the library's own 400x300 test viewport, which would move the card out
    // from under `centre` before the press ever landed.
    let (_, messages) = capture.click_and_frame(VIEWPORT, app.view(), centre);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::MenuRowActivated(key) if key == "m-new")),
        "a click on the rendered row produced {messages:?}"
    );
}

#[test]
fn clicking_outside_the_card_does_not_activate_a_row() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let bounds = card_bounds(&app, "file").expect("the open menu must render a card");

    // Well clear of the card, in the middle of the pane area.
    let far = Point::new(bounds.x + bounds.width + 400.0, bounds.y + 300.0);
    let mut capture = capture_session();
    let (_, messages) = capture.click_and_frame(VIEWPORT, app.view(), far);

    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, Message::MenuRowActivated(_))),
        "a click outside the card activated a row: {messages:?}"
    );
}

// ---------------------------------------------------------------------------
// Closing removes it from the render
// ---------------------------------------------------------------------------

#[test]
fn toggling_the_trigger_again_removes_the_card_from_the_render() {
    let mut app = booted_with(Message::NavbarToggled("file".to_string()));
    assert!(
        card_bounds(&app, "file").is_some(),
        "precondition: the card is rendered while open"
    );

    let _ = app.update(Message::NavbarToggled("file".to_string()));
    assert_eq!(app.active_menu_id(), None);

    let probe = Probe::capture(VIEWPORT, app.view());
    assert!(
        probe.with_prefix(menu_surface::CARD_ID_PREFIX).is_empty(),
        "the card is still in the render after the latch cleared: {:?}",
        probe.with_prefix(menu_surface::CARD_ID_PREFIX)
    );
    assert!(
        probe.with_prefix(menu_surface::ROW_ID_PREFIX).is_empty(),
        "menu rows are still in the render after the latch cleared: {:?}",
        probe.with_prefix(menu_surface::ROW_ID_PREFIX)
    );
}

#[test]
fn the_closed_render_matches_the_boot_render_exactly() {
    let (boot_app, _) = DemoApp::boot();
    let boot = Probe::capture(VIEWPORT, boot_app.view()).manifest();

    let mut app = booted_with(Message::NavbarToggled("file".to_string()));
    let _ = app.update(Message::NavbarToggled("file".to_string()));
    let reopened = Probe::capture(VIEWPORT, app.view()).manifest();

    assert_eq!(
        boot, reopened,
        "closing the menu must return the render to exactly the boot frame"
    );
}

// ---------------------------------------------------------------------------
// Z-order — the card is on top, not buried
// ---------------------------------------------------------------------------

#[test]
fn the_card_paints_over_the_panes_instead_of_under_them() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let bounds = card_bounds(&app, "file").expect("the open menu must render a card");

    // Points inside the card's own inset band, clear of the 1px perimeter
    // stroke and of the rounded corners: inside the card, but outside every
    // row, so whatever is painted there is the card's fill and nothing
    // else. If the card were drawn under the panes, these would show pane
    // content instead.
    let band = [
        (bounds.x + 2.0, bounds.y + bounds.height / 2.0),
        (
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height - 3.0,
        ),
    ];
    let expected = menu_card_fill_rgb();

    let mut capture = capture_session();
    let frame = capture.frame(VIEWPORT, app.view());
    for (x, y) in band {
        let painted = frame.pixel(x, y);
        assert_eq!(
            painted, expected,
            "the card's fill must be visible at its own inset ({x}, {y}); \
             something else is on top"
        );
    }
}

#[test]
fn opening_the_menu_changes_the_pixels_at_the_card_rect() {
    let (closed_app, _) = DemoApp::boot();
    let mut capture = capture_session();
    let closed = capture.frame(VIEWPORT, closed_app.view());

    let open_app = booted_with(Message::NavbarToggled("file".to_string()));
    let bounds = card_bounds(&open_app, "file").expect("the open menu must render a card");
    let open = capture.frame(VIEWPORT, open_app.view());

    // Same inset-band points as the z-order test: the closed frame shows
    // pane content there, the open frame must show the card's fill.
    let band = [
        (bounds.x + 2.0, bounds.y + bounds.height / 2.0),
        (
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height - 3.0,
        ),
    ];
    let expected = menu_card_fill_rgb();

    for (x, y) in band {
        assert_ne!(
            closed.pixel(x, y),
            open.pixel(x, y),
            "opening the menu left the pixels at ({x}, {y}) untouched — nothing was drawn"
        );
        assert_eq!(open.pixel(x, y), expected);
    }
}

// ---------------------------------------------------------------------------
// Switching triggers
// ---------------------------------------------------------------------------

#[test]
fn switching_triggers_re_mounts_the_card_under_the_new_trigger_id() {
    let mut app = booted_with(Message::NavbarToggled("file".to_string()));
    let _ = app.update(Message::NavbarToggled("view".to_string()));

    let probe = Probe::capture(VIEWPORT, app.view());
    assert!(
        probe.find(&menu_surface::card_id("file")).is_none(),
        "the previous trigger's card is still rendered"
    );
    assert!(
        probe.find(&menu_surface::card_id("view")).is_some(),
        "the newly latched trigger mounted no card"
    );
}

// ---------------------------------------------------------------------------
// The status bar reports the mounted rows
// ---------------------------------------------------------------------------

#[test]
fn the_status_bar_count_grows_by_the_mounted_rows() {
    let (closed_app, _) = DemoApp::boot();
    let closed_count = closed_app.widget_count();

    let open_app = booted_with(Message::NavbarToggled("file".to_string()));
    let shown = format!("{} widgets rendered", open_app.widget_count());
    let rendered = crate::layout_probe::text_of(open_app.view());

    assert!(
        rendered.contains(&shown),
        "the status bar must show the live count ({shown}); rendered text was: {rendered}"
    );
    assert_eq!(
        open_app.widget_count(),
        closed_count + menu_surface::mounted_row_count(&data::menu_model()),
        "opening the menu must add exactly the card's rows to the count"
    );
}

// ---------------------------------------------------------------------------
// Geometry helpers are pure and self-consistent
// ---------------------------------------------------------------------------

#[test]
fn the_solved_card_size_matches_the_rendered_card_bounds() {
    let menu = data::menu_model();
    let solved = menu_surface::card_rect(&menu);
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let rendered = card_bounds(&app, "file").expect("the open menu must render a card");

    assert!(
        (solved.width - rendered.width).abs() <= 0.01,
        "solved width {} != rendered width {}",
        solved.width,
        rendered.width
    );
    assert!(
        (solved.height - rendered.height).abs() <= 0.01,
        "solved height {} != rendered height {}",
        solved.height,
        rendered.height
    );
}

#[test]
fn the_rendered_card_respects_the_framework_minimum_card_width() {
    let app = booted_with(Message::NavbarToggled("file".to_string()));
    let bounds = card_bounds(&app, "file").expect("the open menu must render a card");

    assert!(
        bounds.width >= hoarfrost::surface::menu::MENU_CARD_WIDTH_MIN,
        "the rendered card is {} wide, below the framework's minimum menu-card width {}",
        bounds.width,
        hoarfrost::surface::menu::MENU_CARD_WIDTH_MIN
    );
}
