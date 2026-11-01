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

use super::{close_action, content_viewport_id};
use crate::ui::strings;
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

/// A content box the size of the title bar's `×` glyph: narrower and shorter
/// than §3.1's icon-only floor of [`size::CONTROL_SM`] (28 px) on both axes.
fn glyph_content() -> Size {
    Size::new(8.0, 16.0)
}

/// Defect #18 — "Selecting Windows -> Agent Debug from the menu crashes the
/// application".
///
/// The title bar's Close action is a single narrow glyph, so its measured
/// content is icon-only: `Button::outer_size` then hits the `debug_assert!`
/// behind [`Button::tooltip_violation`] and the app panics the moment a
/// movable card — the Agent Debug window — is laid out.
///
/// Asserted through the non-panicking predicate rather than the assert itself,
/// so the defect is pinned without a window and without a crash handler.
#[test]
fn close_action_carries_a_tooltip_so_icon_only_layout_is_legal() {
    let button = close_action(MovableCard::AgentDebug);
    let glyph = glyph_content();

    // The rule must actually be engaged: below the 28 px floor on both axes
    // the box is the icon-only minimum, not a box around a readable label.
    assert!(
        glyph.width <= hoarfrost::widget::button::ICON_ONLY_MIN_WIDTH
            && glyph.height <= hoarfrost::widget::button::ICON_ONLY_MIN_HEIGHT,
        "the glyph content box must sit at or below the icon-only floor, otherwise \
         this test pins nothing"
    );
    assert!(
        button.is_icon_only(glyph),
        "defect #18: the Close button's `×` content must measure icon-only, otherwise \
         the tooltip rule is not exercised"
    );

    // …and the button must satisfy it.
    assert_eq!(
        button.tooltip_violation(glyph),
        None,
        "defect #18: the movable shell's Close button was built icon-only with no \
         tooltip, which trips `Button::outer_size`'s debug_assert and crashes the app \
         when the Agent Debug window opens. Attach `strings::MOVABLE_CLOSE_TOOLTIP` \
         via `Button::with_tooltip`."
    );
    assert_eq!(
        button.tooltip_text(),
        Some(strings::MOVABLE_CLOSE_TOOLTIP),
        "the tooltip must come from the shared string table, not an inline literal"
    );
}

/// The shell under test, painted at a known size with a trivial status line,
/// so a test can measure the bands it lays out instead of restating them.
fn shell_for(host: &MovableHost) -> iced::Element<'static, crate::ui_iced::message::Message> {
    use crate::ui_iced::movable_shell::movable_shell;
    use iced::widget::{Row, container, text};

    movable_shell(
        host,
        MovableCard::AgentDebug,
        Row::new().push(text("controls")),
        container(text("content band")).into(),
        text("3 entries").into(),
    )
}

/// The laid-out bounds of the footer's painted ◢ glyph.
fn painted_grip_bounds(
    sim: &mut iced_test::Simulator<
        '_,
        crate::ui_iced::message::Message,
        iced::Theme,
        iced::Renderer,
    >,
) -> Rectangle {
    sim.find(
        |candidate: iced_test::selector::Candidate<'_>| match candidate {
            iced_test::selector::Candidate::Text {
                content, bounds, ..
            } if content == strings::ICON_RESIZE_GRIP => Some(bounds),
            _ => None,
        },
    )
    .expect("the footer paints the resize grip glyph")
}

/// Defect: the footer painted ◢ at the end of the padded status row, 12 px
/// inside the card's corner, while the host hit-tests the corner itself
/// (`resize_grip(BottomRight)`). Every press on the *visible* grip landed in
/// `Zone::Footer` and was absorbed, so the card never resized.
///
/// The affordance and the hit box must be one rectangle. This measures the
/// glyph out of a real layout and compares it with the host's own grip
/// rectangle, both expressed relative to the card's top-left — the shell
/// paints the card at its own origin and [`movable_layer`] places it.
#[test]
fn the_painted_grip_sits_on_the_hosts_resize_rectangle() {
    let host = host(680.0, 440.0, viewport());
    let mut sim = iced_test::simulator(shell_for(&host));

    let card = sim
        .find(content_viewport_id(MovableCard::AgentDebug))
        .expect("the shell's own container carries the card's viewport id")
        .bounds();
    let grip = host
        .resize_grip(hoarfrost::geometry::clamp::ResizeHandle::BottomRight)
        .expect("bottom-right grip exists");
    let glyph = painted_grip_bounds(&mut sim);

    // The host reports root-space rectangles and the shell is laid out here at
    // the layout origin, so the host's rectangle is shifted by however far the
    // card actually landed — `movable_layer` performs the same shift in the app.
    let origin = host.card().position();
    let shift = Point::new(card.x - origin.x, card.y - origin.y);
    let grip = Rectangle::new(Point::new(grip.x + shift.x, grip.y + shift.y), grip.size());

    let centre = glyph.center();
    assert!(
        grip.contains(centre),
        "the grip glyph's centre {centre:?} must land inside the host's hit box \
         {grip:?}: a press on what the user can see has to be a press on what the \
         host resizes from"
    );
    assert!(
        glyph.x >= grip.x - 0.5
            && glyph.y >= grip.y - 0.5
            && glyph.x + glyph.width <= grip.x + grip.width + 0.5
            && glyph.y + glyph.height <= grip.y + grip.height + 0.5,
        "the whole glyph {glyph:?} must sit inside the hit box {grip:?}, otherwise part \
         of the affordance is a press the host absorbs"
    );
}

/// Whether the laid-out tree paints `needle` as text anywhere.
fn paints_text(
    sim: &mut iced_test::Simulator<
        '_,
        crate::ui_iced::message::Message,
        iced::Theme,
        iced::Renderer,
    >,
    needle: &str,
) -> bool {
    let needle = needle.to_owned();
    sim.find(
        move |candidate: iced_test::selector::Candidate<'_>| match candidate {
            iced_test::selector::Candidate::Text { content, .. } if content == needle => Some(()),
            _ => None,
        },
    )
    .is_ok()
}

/// The title bar's trailing cluster carries `Close ×` and nothing else: the
/// `Dock` control was removed because no dock behaviour exists, and a button
/// that does nothing is worse than no button.
#[test]
fn the_title_bar_surfaces_close_only_and_no_dock_control() {
    let host = host(680.0, 440.0, viewport());
    let mut sim = iced_test::simulator(shell_for(&host));

    assert!(
        !paints_text(&mut sim, "Dock"),
        "the inert Dock control must not be painted"
    );
    assert!(
        paints_text(&mut sim, super::CLOSE_GLYPH),
        "Close must still be surfaced"
    );
}

/// The shell's contract is that it paints **the card only**, sized to the host's
/// own geometry, so [`movable_layer`](crate::ui_iced::overlay::movable_layer)
/// can place it without a second source of size. Laid out headlessly, the card
/// container must therefore come out at exactly the host's size, and the content
/// band the caller handed in must be mounted inside it rather than dropped.
#[test]
fn the_shell_is_sized_to_the_host_geometry_and_mounts_its_content() {
    use crate::ui_iced::message::Message;
    use crate::ui_iced::movable_shell::movable_shell;
    use iced::widget::{Row, container, text};

    /// Id of the content element handed to the shell, so the layout can be
    /// measured through it.
    const CONTENT_MARKER_ID: &str = "test.movable.content_marker";
    const CARD_WIDTH: f32 = 680.0;
    const CARD_HEIGHT: f32 = 440.0;

    let host = host(CARD_WIDTH, CARD_HEIGHT, viewport());
    let shell = movable_shell(
        &host,
        MovableCard::AgentDebug,
        Row::new().push(text("controls")),
        container(text("content band"))
            .id(iced::widget::Id::from(CONTENT_MARKER_ID))
            .into(),
        text("3 entries").into(),
    );

    let mut sim: iced_test::Simulator<'_, Message, iced::Theme, iced::Renderer> =
        iced_test::simulator(shell);

    let card = sim
        .find(content_viewport_id(MovableCard::AgentDebug))
        .expect("the shell's own container carries the card's viewport id")
        .bounds();
    assert!(
        (card.width - CARD_WIDTH).abs() < 0.5,
        "the card must be as wide as the host's geometry ({CARD_WIDTH}), got {}",
        card.width
    );
    assert!(
        (card.height - CARD_HEIGHT).abs() < 0.5,
        "the card must be as tall as the host's geometry ({CARD_HEIGHT}), got {}",
        card.height
    );

    let content = sim
        .find(iced::widget::Id::from(CONTENT_MARKER_ID))
        .expect("the content element the caller handed in must be mounted")
        .bounds();
    assert!(
        content.height > 0.0,
        "the content band takes the leftover height and must not collapse, got {content:?}"
    );
    assert!(
        content.x >= card.x
            && content.y >= card.y
            && content.x + content.width <= card.x + card.width + 1e-3
            && content.y + content.height <= card.y + card.height + 1e-3,
        "the content band must sit inside the card it was handed to, got content \
         {content:?} inside card {card:?}"
    );
}
