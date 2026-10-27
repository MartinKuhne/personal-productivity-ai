//! Tests for the §4.7.1 movable card silhouette
//! (`UIF-MOV-001`, `UIF-MOV-002`, INT-6).
//!
//! These assert the **anatomy the shell paints**, which is all the shell
//! is: four bands whose heights come off the host, a drag zone that stops
//! short of the trailing actions, and a resize grip in the footer corner.
//! Rendering is not exercised — there is no image-snapshot tier in this
//! repo — so the assertions are against the host's own resolved
//! rectangles, which are the same values the shell reads.

// Several assertions here target compile-time constant rungs on purpose:
// pinning §4.7.1's band heights is the whole point, so a retuned rung must
// fail the suite rather than pass silently. `clippy::assertions_on_constants`
// cannot tell a spec pin from a tautology — the same allowance the
// `hoarfrost::token` test modules use.
#![allow(clippy::assertions_on_constants)]

use super::content_viewport_id;
use crate::ui_iced::message::MovableCard;
use hoarfrost::focus::FocusState;
use hoarfrost::surface::movable::{
    CHROME_HEIGHT, CONTENT_VIEWPORT_MIN, FOOTER_HEIGHT, MovableGeometry, MovableHost, MovableSpec,
    NoopGeometryStorage, RESIZE_HANDLE_SIZE, TITLE_BAR_HEIGHT, Zone,
};
use hoarfrost::surface::zorder::SurfaceId;
use hoarfrost::token::size;
use iced::{Point, Rectangle, Size};

/// A host at a comfortable size, opened against throwaway collaborators.
fn host(width: f32, height: f32, viewport: Rectangle) -> MovableHost {
    let spec = MovableSpec::new(SurfaceId::new("test.movable"), "Test Card").with_geometry(
        MovableGeometry::new(Point::new(50.0, 50.0), Size::new(width, height)),
    );
    MovableHost::open(
        spec,
        viewport,
        &mut FocusState::new(),
        &mut hoarfrost::surface::zorder::SurfaceRegistry::new(),
        &mut NoopGeometryStorage,
    )
}

fn viewport() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(1600.0, 1200.0))
}

/// The band rungs the shell paints are the framework's own, and they are
/// the values §4.7.1 names: 32 / 36 / 24 with a 16 px grip.
#[test]
fn the_band_rungs_are_the_frameworks_own() {
    assert_eq!(TITLE_BAR_HEIGHT, size::CONTROL_MD);
    assert_eq!(FOOTER_HEIGHT, size::BAR_STATUS);
    assert_eq!(RESIZE_HANDLE_SIZE, size::ICON_MD);
    assert_eq!(TITLE_BAR_HEIGHT, 32.0);
    assert_eq!(RESIZE_HANDLE_SIZE, 16.0);
}

/// The four bands account for the card exactly: title + filter + footer is
/// the chrome, and the content viewport is whatever is left over. A band
/// that drifted would show up here as a gap or an overlap.
#[test]
fn the_four_bands_account_for_every_pixel_of_the_card() {
    let host = host(680.0, 440.0, viewport());
    let card = host.card();

    let title = host.title_bar();
    let filter = host.filter_bar();
    let content = host.content_viewport();
    let footer = host.footer();

    assert_eq!(title.height, TITLE_BAR_HEIGHT);
    assert_eq!(filter.height, 36.0);
    assert_eq!(footer.height, FOOTER_HEIGHT);

    // Stacked with no gap and no overlap.
    assert_eq!(filter.y, title.y + title.height);
    assert_eq!(content.y, filter.y + filter.height);
    assert_eq!(footer.y, content.y + content.height);

    // The stack bottoms out exactly on the card's bottom edge.
    assert!(
        (footer.y + footer.height - (card.y + card.height)).abs() < 1e-3,
        "the bands must fill the card exactly"
    );
    assert_eq!(content.height, card.height - CHROME_HEIGHT);
}

/// The content viewport floor is derived, not declared — it is the smallest
/// legal card minus its chrome.
#[test]
fn the_content_viewport_floor_is_derived_from_the_chrome() {
    assert!(CONTENT_VIEWPORT_MIN > 0.0);
    assert_eq!(
        CONTENT_VIEWPORT_MIN,
        hoarfrost::surface::movable::CARD_MIN_HEIGHT - CHROME_HEIGHT
    );
}

/// `UIF-MOV-001`: the whole title bar drags **except** the trailing
/// action cluster, whose width is derived from two control squares and a
/// gap rather than being a magic number.
#[test]
fn the_trailing_actions_are_excluded_from_the_drag_zone() {
    let host = host(680.0, 440.0, viewport());
    let bar = host.title_bar();
    let actions = host.trailing_actions();

    assert_eq!(
        actions.width,
        hoarfrost::surface::movable::ACTION_CONTROL_SIZE * 2.0
            + hoarfrost::surface::movable::ACTION_GAP,
        "the exclusion zone is sized from the controls themselves"
    );

    // Mid-title drags.
    assert_eq!(
        host.zone_at(Point::new(bar.x + bar.width / 2.0, bar.y + 4.0)),
        Zone::TitleDrag
    );
    // The action cluster does not.
    assert_eq!(
        host.zone_at(Point::new(actions.x + actions.width / 2.0, actions.y + 4.0)),
        Zone::TitleActions
    );
}

/// `UIF-MOV-002`: the ◢ grip in the footer's bottom-right corner is a
/// resize zone, and the footer away from the corner is not.
#[test]
fn the_footer_corner_is_the_resize_grip() {
    let host = host(680.0, 440.0, viewport());
    let footer = host.footer();
    let grip = host
        .resize_grip(hoarfrost::geometry::clamp::ResizeHandle::BottomRight)
        .expect("bottom-right grip exists");

    assert_eq!(grip.width, RESIZE_HANDLE_SIZE);
    assert_eq!(grip.height, RESIZE_HANDLE_SIZE);
    assert!(grip.y >= footer.y, "the grip sits in the footer");

    assert!(host.is_grip_at(Point::new(
        grip.x + grip.width / 2.0,
        grip.y + grip.height / 2.0
    )));
    // Mid-footer, above the 16 px grip band and clear of the left/right
    // edge grips: the status area, not a resize target.
    assert!(!host.is_grip_at(Point::new(footer.x + 340.0, footer.y + 4.0)));
}

/// The three handles that would sit on the title bar have no grip, because
/// `UIF-MOV-001` gives that whole bar to the drag.
#[test]
fn no_grip_steals_the_title_bars_drag() {
    let host = host(680.0, 440.0, viewport());
    for handle in [
        hoarfrost::geometry::clamp::ResizeHandle::TopLeft,
        hoarfrost::geometry::clamp::ResizeHandle::Top,
        hoarfrost::geometry::clamp::ResizeHandle::TopRight,
    ] {
        assert!(
            host.resize_grip(handle).is_none(),
            "{handle:?} must not have a grip on the title bar"
        );
    }
}

/// AGENTS §5: the viewport id is stable across calls and distinct per card,
/// so the two live cards never share one id and the reducer can address
/// either one.
#[test]
fn viewport_ids_are_stable_and_distinct_per_card() {
    assert_eq!(
        content_viewport_id(MovableCard::Logs),
        content_viewport_id(MovableCard::Logs),
        "the id must not change between frames"
    );
    assert_eq!(
        content_viewport_id(MovableCard::AgentDebug),
        content_viewport_id(MovableCard::AgentDebug)
    );
    assert_ne!(
        content_viewport_id(MovableCard::Logs),
        content_viewport_id(MovableCard::AgentDebug),
        "one id, one live widget"
    );
}
