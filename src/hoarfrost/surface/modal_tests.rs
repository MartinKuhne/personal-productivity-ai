//! Tests for [`modal`] — the modal host's scrim, focus trap, card-level click
//! absorption, dirty-dismiss guard and error banner (`UIF-MOD-001`,
//! `UIF-MOD-002`, `UIF-MOD-003`, `UIF-VER-003`).
//!
//! Nine groups, in order:
//!
//! 1. **Sizing** — every row of §4.6.2 solved against real viewports, down
//!    to the 640×480 floor of `UIF-VER-002`, plus the gutter rule and the
//!    degenerate cases.
//! 2. **Opening and registration** — what a modal puts into the focus
//!    model and the z-order registry, and where focus lands.
//! 3. **Tab containment** — `UIF-MOD-001`, asserted over long cycles and
//!    across a focus-group rebuild, and through the real bridge onto iced's
//!    router with the shared harness.
//! 4. **Escape and the dirty guard** — `UIF-MOD-002` alone, and then
//!    through the `UIF-WIN-003` chain with a real menu tree open above the
//!    modal. Group 4b, immediately after it, adds the pending-action
//!    refusal: a running command refuses the unwind on a card that is
//!    *not* dirty, so the two refusals stay provably independent
//!    (§3.0.4.3, `INC-36`).
//! 5. **The scrim** — clean dismisses, dirty does not, and the workspace
//!    never gets the click either way.
//! 6. **Card-level absorption** — the `UIF-VER-003` verbatim cases, plus
//!    the grid sweeps that show no interior point can dismiss.
//! 7. **The error banner** — persistence across every interaction that
//!    leaves the modal open, and the recovery affordances.
//! 8. **Nested modals** — the confirm-over-modal case.
//! 9. **Teardown, focus restoration, the frame and determinism.**
//!
//! # Why the containment tests assert membership rather than a sequence
//!
//! The tab order inside a card is the caller's business; the *set* it can
//! never leave is the invariant. Every containment test therefore collects
//! what was visited and asserts it belongs to the card's own roster, which
//! stays true when the roster changes and would still fail if a trap leaked.

use super::*;

use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::surface::menu::cascade::MenuCascade;
use crate::surface::menu::cascade::MenuTreeKind;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::MenuId;
use crate::surface::menu::model::Row;
use crate::surface::zorder::Declination;
use crate::surface::zorder::EscapeRefusal;
use crate::surface::zorder::PointerTarget;
use crate::surface::zorder::SCRIM_ALPHA;
use crate::test_helpers::Harness;
use crate::test_helpers::assert_agree;
use crate::test_helpers::iced_id;
use crate::test_helpers::mixed_focusable_tree;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The background focus group a real window would already have registered.
const BACKGROUND: [&str; 3] = ["bg.navbar", "bg.tree", "bg.tabs"];

/// The card's own controls, in tab order. The primary is disabled so the
/// `UIF-VER-003` disabled-button case is part of the standard fixture
/// rather than a special one.
const CARD: [&str; 4] = ["card.field", "card.option", "card.cancel", "card.primary"];

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(pt(x, y), Size::new(w, h))
}

fn sid(name: &str) -> SurfaceId {
    SurfaceId::new(name)
}

fn fid(name: &str) -> FocusId {
    FocusId::new(name)
}

fn gid(name: &str) -> GroupId {
    GroupId::new(name)
}

/// The unmeasured content extent — what a host passes before the card has
/// been laid out.
const UNMEASURED: Size = Size::new(0.0, 0.0);

/// A modal with the standard card roster.
fn standard_spec() -> ModalSpec {
    ModalSpec::new("editor", "Inline Document Editor", ModalSizing::editor())
        .with_controls(vec![
            FocusMember::new("card.field"),
            FocusMember::new("card.option"),
            FocusMember::new("card.cancel"),
            FocusMember::disabled("card.primary"),
        ])
        .with_primary("card.field")
}

/// A standard modal that is dirty.
fn dirty_spec() -> ModalSpec {
    standard_spec().with_dirty(true)
}

/// A standard modal already showing an error.
fn errored_spec() -> ModalSpec {
    standard_spec().with_error(
        ModalError::new("MOD-5001", "Save failed")
            .at(ErrorLocation::Position {
                line: 12,
                column: 7,
            })
            .with_recovery(Recovery::Retry),
    )
}

/// A host opened over a window that already has background focus groups.
struct Scenario {
    host: ModalHost,
    focus: FocusState,
    registry: SurfaceRegistry,
}

impl Scenario {
    fn open(spec: ModalSpec) -> Self {
        Self::at(spec, rect(0.0, 0.0, 1920.0, 1080.0), None)
    }

    fn at(spec: ModalSpec, viewport: Rectangle, prefocus: Option<&str>) -> Self {
        let mut focus = FocusState::new();
        let mut registry = SurfaceRegistry::new();

        focus.register_group(
            gid("bg"),
            BACKGROUND
                .iter()
                .map(|label| FocusMember::new(*label))
                .collect(),
        );
        if let Some(label) = prefocus {
            assert!(
                focus.focus(&fid(label)),
                "the pre-modal control must be focusable"
            );
        }

        let host = ModalHost::open(spec, viewport, UNMEASURED, &mut focus, &mut registry);
        Self {
            host,
            focus,
            registry,
        }
    }

    /// A dirty modal over a focused background.
    fn dirty() -> Self {
        Self::at(
            dirty_spec(),
            rect(0.0, 0.0, 1920.0, 1080.0),
            Some("bg.tree"),
        )
    }

    /// A clean modal over a focused background.
    fn clean_focused() -> Self {
        Self::at(
            standard_spec(),
            rect(0.0, 0.0, 1920.0, 1080.0),
            Some("bg.tree"),
        )
    }

    /// A point inside the card — its centre.
    fn card_centre(&self) -> Point {
        let card = self.host.card();
        pt(card.x + card.width / 2.0, card.y + card.height / 2.0)
    }

    /// A point on the scrim — the viewport's top-left corner, which no
    /// centred card of these sizes covers.
    fn scrim_point(&self) -> Point {
        pt(1.0, 1.0)
    }
}

/// Assert a rectangle to a tolerance, so the tests read as the numbers the
/// spec prints rather than as bit patterns.
fn assert_rect(actual: Rectangle, x: f32, y: f32, w: f32, h: f32) {
    let tolerance = 1e-4;
    assert!(
        (actual.x - x).abs() < tolerance
            && (actual.y - y).abs() < tolerance
            && (actual.width - w).abs() < tolerance
            && (actual.height - h).abs() < tolerance,
        "expected rect({x}, {y}, {w}, {h}), got {actual:?}"
    );
}

/// Assert a value is finite, which every geometry answer must be.
fn assert_finite(value: f32, what: &str) {
    assert!(value.is_finite(), "{what} must be finite, got {value}");
}

/// A context menu tree for the escape-chain scenarios.
fn context_menu() -> Menu {
    Menu::new(
        MenuId::new("menu.root"),
        vec![
            Row::action("root.a", "Root action A"),
            Row::action("root.b", "Root action B"),
        ],
    )
}

/// A scenario with a Band 2 menu tree already open, and the modal on top.
///
/// The menu is opened *before* the modal so it mounts fully — the modal's
/// trap would otherwise reject its attempt to focus its own first row.
fn with_menu_above(spec: ModalSpec) -> (Scenario, MenuCascade) {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();

    focus.register_group(
        gid("bg"),
        BACKGROUND
            .iter()
            .map(|label| FocusMember::new(*label))
            .collect(),
    );
    focus.focus(&fid("bg.tree"));

    let mut cascade = MenuCascade::new();
    cascade.open(&context_menu(), MenuTreeKind::Context, &mut focus);
    assert!(
        cascade.is_open(),
        "the menu tree must be open for this scenario"
    );

    for card in cascade.open_cards() {
        registry.register(
            sid(card.id().as_str()),
            SurfaceKind::MenuCard,
            vec![rect(100.0, 100.0, 200.0, 120.0)],
        );
    }

    let host = ModalHost::open(
        spec,
        rect(0.0, 0.0, 1920.0, 1080.0),
        UNMEASURED,
        &mut focus,
        &mut registry,
    );

    (
        Scenario {
            host,
            focus,
            registry,
        },
        cascade,
    )
}

/// Ask the escape chain, routing menu cards through the cascade and the
/// modal through its own answer — the shape a host drives.
fn drive_escape(scenario: &mut Scenario, cascade: &mut MenuCascade) -> EscapeOutcome {
    scenario.registry.escape(|surface| {
        if surface.kind() == SurfaceKind::MenuCard {
            let closed = cascade.close_deepest(&mut scenario.focus);
            return UnwindAnswer::from_cascade_close(&closed);
        }
        if surface.id() == scenario.host.id() {
            return scenario.host.unwind_answer(&mut scenario.focus);
        }
        UnwindAnswer::Declined(DeclineReason::NotOpen)
    })
}

// ---------------------------------------------------------------------------
// 1. Sizing — §4.6.2
// ---------------------------------------------------------------------------

#[test]
fn test_viewport_gutter_is_the_thirty_two_pixel_token() {
    assert_eq!(VIEWPORT_GUTTER, space::XXL);
    assert_eq!(VIEWPORT_GUTTER, 2.0 * space::LG);
}

#[test]
fn test_editor_sizing_at_full_desktop_hits_both_ceilings() {
    let card = ModalSizing::editor().solve(rect(0.0, 0.0, 1920.0, 1080.0), UNMEASURED);
    // 80% of 1920 is 1536 and of 1080 is 864; both exceed the caps.
    assert_rect(card, 480.0, 160.0, 960.0, 760.0);
}

#[test]
fn test_editor_sizing_at_1280x800_caps_width_and_fits_height() {
    let card = ModalSizing::editor().solve(rect(0.0, 0.0, 1280.0, 800.0), UNMEASURED);
    // width 1024 -> capped at 960; height 640 sits inside 380..=760.
    assert_rect(card, 160.0, 80.0, 960.0, 640.0);
}

#[test]
fn test_editor_sizing_at_the_640x480_minimum_viewport() {
    let card = ModalSizing::editor().solve(rect(0.0, 0.0, 640.0, 480.0), UNMEASURED);
    // width 512 -> raised to the 520 floor, which still fits the 608 gutter;
    // height 384 sits inside 380..=760.
    assert_rect(card, 60.0, 48.0, 520.0, 384.0);
}

#[test]
fn test_editor_floors_lose_to_the_gutter_in_a_tiny_viewport() {
    let card = ModalSizing::editor().solve(rect(0.0, 0.0, 400.0, 300.0), UNMEASURED);
    // 520 > 400 - 32 and 380 > 300 - 32: the gutter wins both ways.
    assert_rect(card, 16.0, 16.0, 368.0, 268.0);
}

#[test]
fn test_tools_width_is_fixed_until_the_gutter_bites() {
    let wide = ModalSizing::tools_and_integrations()
        .width
        .resolve(1280.0, 600.0);
    let narrow = ModalSizing::tools_and_integrations()
        .width
        .resolve(640.0, 600.0);

    assert_eq!(wide, TOOLS_WIDTH);
    assert_eq!(narrow, 640.0 - VIEWPORT_GUTTER);
}

#[test]
fn test_tools_height_floors_at_320_and_caps_at_eighty_percent() {
    let policy = ModalSizing::tools_and_integrations().height;

    assert_eq!(policy.resolve(800.0, 100.0), TOOLS_HEIGHT_MIN);
    assert_eq!(policy.resolve(800.0, 600.0), 600.0);
    assert_eq!(
        policy.resolve(800.0, 900.0),
        800.0 * TOOLS_HEIGHT_MAX_FRACTION
    );
}

#[test]
fn test_batch_processing_caps_its_content_height() {
    let policy = ModalSizing::batch_processing();

    let small = policy.solve(rect(0.0, 0.0, 1280.0, 800.0), Size::new(0.0, 300.0));
    assert_rect(small, 360.0, 250.0, BATCH_WIDTH, 300.0);

    let large = policy.solve(rect(0.0, 0.0, 1280.0, 800.0), Size::new(0.0, 900.0));
    assert_rect(large, 360.0, 160.0, BATCH_WIDTH, CONTENT_HEIGHT_MAX);
}

#[test]
fn test_batch_width_clamps_to_the_gutter() {
    let card = ModalSizing::batch_processing().solve(rect(0.0, 0.0, 500.0, 400.0), UNMEASURED);
    assert_eq!(card.width, 500.0 - VIEWPORT_GUTTER);
}

#[test]
fn test_about_and_attributions_sizes() {
    let about = ModalSizing::about().solve(rect(0.0, 0.0, 1280.0, 800.0), Size::new(0.0, 200.0));
    assert_rect(about, 420.0, 300.0, ABOUT_WIDTH, 200.0);

    let attributions =
        ModalSizing::attributions().solve(rect(0.0, 0.0, 1280.0, 800.0), Size::new(0.0, 200.0));
    assert_rect(
        attributions,
        320.0,
        160.0,
        ATTRIBUTIONS_WIDTH,
        ATTRIBUTIONS_HEIGHT,
    );
}

#[test]
fn test_the_card_is_centred_in_an_offset_viewport() {
    let card = ModalSizing::about().solve(rect(100.0, 50.0, 1000.0, 700.0), Size::new(0.0, 200.0));
    assert_rect(card, 380.0, 300.0, ABOUT_WIDTH, 200.0);
}

#[test]
fn test_no_policy_ever_crosses_the_viewport_gutter() {
    let policies = [
        ModalSizing::editor(),
        ModalSizing::tools_and_integrations(),
        ModalSizing::batch_processing(),
        ModalSizing::about(),
        ModalSizing::attributions(),
    ];
    let extents = [640.0f32, 700.0, 800.0, 1024.0, 1440.0, 1920.0, 2560.0];

    for policy in policies {
        for width in extents {
            for height in extents {
                let viewport = rect(0.0, 0.0, width, height);
                let card = policy.solve(viewport, Size::new(width, height));
                let room = width - VIEWPORT_GUTTER;
                let vertical_room = height - VIEWPORT_GUTTER;

                assert!(
                    card.width <= room + 1e-4,
                    "policy {policy:?} at {width}x{height} is {} wide, over the gutter",
                    card.width
                );
                assert!(
                    card.height <= vertical_room + 1e-4,
                    "policy {policy:?} at {width}x{height} is {} tall, over the gutter",
                    card.height
                );
                assert!(
                    card.x >= -1e-4 && card.x + card.width <= width + 1e-4,
                    "policy {policy:?} at {width}x{height} escapes the viewport horizontally"
                );
                assert!(
                    card.y >= -1e-4 && card.y + card.height <= height + 1e-4,
                    "policy {policy:?} at {width}x{height} escapes the viewport vertically"
                );
            }
        }
    }
}

#[test]
fn test_degenerate_viewports_stay_finite_and_non_negative() {
    let viewports = [
        rect(0.0, 0.0, 0.0, 0.0),
        rect(0.0, 0.0, -100.0, -100.0),
        rect(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
        rect(f32::INFINITY, f32::INFINITY, f32::INFINITY, f32::INFINITY),
        rect(0.0, 0.0, VIEWPORT_GUTTER, VIEWPORT_GUTTER),
    ];
    let policies = [
        ModalSizing::editor(),
        ModalSizing::tools_and_integrations(),
        ModalSizing::batch_processing(),
    ];

    for policy in policies {
        for viewport in viewports {
            let card = policy.solve(viewport, Size::new(f32::NAN, f32::NAN));
            assert_finite(card.x, "card.x");
            assert_finite(card.y, "card.y");
            assert_finite(card.width, "card.width");
            assert_finite(card.height, "card.height");
            assert!(card.width >= 0.0 && card.height >= 0.0);
        }
    }
}

#[test]
fn test_axis_policy_shapes_resolve_as_documented() {
    assert_eq!(AxisPolicy::fixed(300.0).resolve(1000.0, 999.0), 300.0);
    // A fixed 300 in a 200px window is clamped by the gutter, not the floor.
    assert_eq!(AxisPolicy::fixed(300.0).resolve(200.0, 0.0), 168.0);
    assert_eq!(
        AxisPolicy::viewport(0.5, 100.0, 400.0).resolve(1000.0, 0.0),
        400.0
    );
    // The 100px floor survives, the gutter caps it at 68.
    assert_eq!(
        AxisPolicy::viewport(0.5, 100.0, 400.0).resolve(100.0, 0.0),
        68.0
    );
    assert_eq!(
        AxisPolicy::content(50.0, Cap::Pixels(200.0)).resolve(1000.0, 500.0),
        200.0
    );
    assert_eq!(
        AxisPolicy::content(50.0, Cap::Pixels(200.0)).resolve(1000.0, 10.0),
        50.0
    );
}

#[test]
fn test_cap_resolve_shapes() {
    assert_eq!(Cap::Pixels(120.0).resolve(800.0), 120.0);
    assert_eq!(Cap::Viewport(0.25).resolve(800.0), 200.0);
    assert_eq!(Cap::None.resolve(800.0), f32::INFINITY);
    assert_eq!(Cap::Viewport(f32::NAN).resolve(800.0), 0.0);
}

#[test]
fn test_an_inverted_range_does_not_panic_and_falls_back_to_the_floor() {
    // A `Cap::Viewport` shrinks with the window and can fall under a fixed
    // `min`; `f32::clamp` would panic on that, so the floor is raised to
    // meet it rather than inverting.
    let policy = AxisPolicy::content(500.0, Cap::Viewport(0.1));
    let resolved = policy.resolve(1000.0, 10.0);

    assert!(resolved.is_finite());
    assert!(resolved <= 500.0 + 1e-4);
    assert!(resolved >= 0.0);
}

#[test]
fn test_sizing_is_deterministic() {
    let viewport = rect(0.0, 0.0, 1337.0, 733.0);
    let content = Size::new(611.0, 411.0);

    let first = ModalSizing::tools_and_integrations().solve(viewport, content);
    let second = ModalSizing::tools_and_integrations().solve(viewport, content);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// 1b. Scrollable sub-region solving (UIF-MOD-004)
// ---------------------------------------------------------------------------

/// The band composition the Attributions card actually uses, spelled out
/// here so the framework test can exercise the solver against a real set
/// of terms rather than against numbers invented for the test.
fn attributions_bands() -> SubRegionBands {
    use crate::token::{shape, size, space, typography};
    let header = typography::TITLE_SM.line_height
        + typography::BODY.line_height
        + shape::BORDER_HAIRLINE
        + 3.0 * space::SM;
    let footer = shape::BORDER_HAIRLINE + size::CONTROL_MD + 2.0 * space::SM;
    SubRegionBands::from_frame(&ModalFrame::resolve(&Palette::dark()))
        .with_header(header)
        .with_footer(footer)
}

#[test]
fn subregion_max_height_is_the_formula_the_requirement_states() {
    // §4.6.2: subregion_max_h = modal_height - (header + 2 × body_padding + footer)
    let bands = SubRegionBands::new(70.0, 50.0, 16.0);
    assert_eq!(bands.total(), 70.0 + 2.0 * 16.0 + 50.0);
    assert_eq!(subregion_max_height(480.0, bands), 480.0 - 152.0);
    assert_eq!(subregion_max_height(480.0, bands), 328.0);
}

#[test]
fn the_body_padding_is_charged_twice_and_only_inside_total() {
    // Doubling the padding must move the total by exactly 2 × the delta,
    // which is only true if the doubling happens once, in `total`.
    let one = SubRegionBands::new(10.0, 10.0, 16.0);
    let wider = SubRegionBands::new(10.0, 10.0, 24.0);
    assert_eq!(wider.total() - one.total(), 2.0 * (24.0 - 16.0));
}

#[test]
fn from_frame_seeds_the_body_padding_and_leaves_the_content_bands_empty() {
    let frame = ModalFrame::resolve(&Palette::dark());
    let bands = SubRegionBands::from_frame(&frame);
    assert_eq!(bands.body_padding, frame.content_padding);
    assert_eq!(bands.header, 0.0);
    assert_eq!(bands.footer, 0.0);
    // A bare full-bleed list: the card minus the two padding sides.
    assert_eq!(
        subregion_max_height(480.0, bands),
        480.0 - 2.0 * frame.content_padding
    );
}

#[test]
fn the_builders_set_one_band_without_touching_the_others() {
    let frame = ModalFrame::resolve(&Palette::dark());
    let bands = SubRegionBands::from_frame(&frame)
        .with_header(69.0)
        .with_footer(49.0);
    assert_eq!(bands.header, 69.0);
    assert_eq!(bands.footer, 49.0);
    assert_eq!(bands.body_padding, frame.content_padding);
}

#[test]
fn the_attributions_band_composition_is_every_token_that_is_on_it() {
    use crate::token::{shape, size, space, typography};
    let bands = attributions_bands();

    // header: title rung + summary rung + the divider under them + the
    // three space::SM gaps they sit on.
    assert_eq!(
        bands.header,
        typography::TITLE_SM.line_height
            + typography::BODY.line_height
            + shape::BORDER_HAIRLINE
            + 3.0 * space::SM
    );
    // footer: the divider above the footer + the footer control row + the
    // two space::SM gaps they sit on.
    assert_eq!(
        bands.footer,
        shape::BORDER_HAIRLINE + size::CONTROL_MD + 2.0 * space::SM
    );
    // padding: the frame's own, which is space::LG.
    assert_eq!(bands.body_padding, space::LG);

    // No term is a bare literal: each equals a token path, asserted above.
    // Pinned here as well so the total is traceable to the rungs.
    assert_eq!(
        bands.total(),
        24.0 + 20.0 + 1.0 + 24.0 + 32.0 + 1.0 + 32.0 + 16.0
    );
}

#[test]
fn the_attributions_subregion_is_the_measured_remainder_not_the_card_constant() {
    let bands = attributions_bands();
    let solved = subregion_max_height(ATTRIBUTIONS_HEIGHT, bands);

    // 480 - 150 = 330, which is what the rendered card measures for the
    // credits list. Asserted against the card's own constant to make the
    // point the requirement makes: the sub-region is NOT the card.
    assert_eq!(solved, 330.0);
    assert_ne!(
        solved, ATTRIBUTIONS_HEIGHT,
        "a sub-region solved at the card's outer height is the exact overflow \
         UIF-MOD-004 forbids"
    );
    assert!(solved < ATTRIBUTIONS_HEIGHT);
}

#[test]
fn the_cap_is_applied_after_the_remainder_not_instead_of_it() {
    let bands = attributions_bands();

    // Roomy card: the cap binds.
    assert_eq!(
        subregion_height(ATTRIBUTIONS_HEIGHT, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT),
        ATTRIBUTIONS_LIST_MAX_HEIGHT
    );
    // Short card: the remainder binds, so the cap never invents space.
    let short = bands.total() + 80.0;
    assert_eq!(
        subregion_height(short, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT),
        80.0,
        "a card that cannot reach the cap must still give the sub-region \
         everything it has"
    );
    // Exactly at the cap.
    let exact = bands.total() + ATTRIBUTIONS_LIST_MAX_HEIGHT;
    assert_eq!(
        subregion_height(exact, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT),
        ATTRIBUTIONS_LIST_MAX_HEIGHT
    );
}

#[test]
fn the_solved_height_never_exceeds_either_the_remainder_or_the_cap() {
    let bands = attributions_bands();
    for card in [0.0f32, 50.0, 150.0, 151.0, 300.0, 480.0, 1200.0] {
        let solved = subregion_height(card, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT);
        let remainder = subregion_max_height(card, bands);
        assert!(
            solved <= remainder,
            "card {card}: solved {solved} > remainder {remainder}"
        );
        assert!(
            solved <= ATTRIBUTIONS_LIST_MAX_HEIGHT,
            "card {card}: solved {solved} breached the cap"
        );
        assert!(solved >= 0.0, "card {card}: solved {solved} is negative");
    }
}

#[test]
fn a_card_smaller_than_its_own_furniture_clamps_to_zero_rather_than_going_negative() {
    let bands = attributions_bands();
    // The Attributions card clamps to viewport - 32, so a viewport under
    // 182px tall solves to a card smaller than the 150px it carries.
    assert_eq!(subregion_max_height(150.0, bands), 0.0);
    assert_eq!(subregion_max_height(100.0, bands), 0.0);
    assert_eq!(
        subregion_height(100.0, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT),
        0.0,
        "a negative sub-region height would hand iced an inverted scrollable \
         viewport; the clamp is load-bearing"
    );
    assert_eq!(subregion_max_height(151.0, bands), 1.0);
}

#[test]
fn non_finite_inputs_cannot_poison_the_solved_height() {
    let bands = attributions_bands();
    assert_eq!(subregion_max_height(f32::NAN, bands), 0.0);
    assert_eq!(
        subregion_height(f32::NAN, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT),
        0.0
    );
    assert!(subregion_max_height(f32::INFINITY, bands).is_finite());
}

#[test]
fn the_solver_is_pure_and_deterministic() {
    let bands = attributions_bands();
    for _ in 0..3 {
        assert_eq!(
            subregion_height(ATTRIBUTIONS_HEIGHT, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT),
            ATTRIBUTIONS_LIST_MAX_HEIGHT
        );
    }
}

/// `assertions_on_constants` is allowed on this one test: pinning a value
/// the §2.2 closed table fixes *is* a constant-vs-constant comparison, and
/// that is the whole point — an accidental edit to the constant has to fail
/// something. The app's own gate file takes the same allowance for the same
/// reason; here it is scoped to the function rather than the file so no
/// other test inherits it.
#[test]
#[allow(clippy::assertions_on_constants)]
fn attributions_list_max_height_is_the_number_the_closed_table_names() {
    assert_eq!(ATTRIBUTIONS_LIST_MAX_HEIGHT, 200.0);
    assert!(
        ATTRIBUTIONS_LIST_MAX_HEIGHT < ATTRIBUTIONS_HEIGHT,
        "the cap must be a cap on a 480px card, not a restatement of it"
    );
}

#[test]
fn attributions_list_max_height_is_registered_as_a_surface_geometry_constant() {
    let hit = crate::token_lint::SURFACE_GEOMETRY_CONSTANTS
        .iter()
        .find(|e| e.name == "ATTRIBUTIONS_LIST_MAX_HEIGHT");
    let entry = hit.expect("the new cap must be registered or Rule B reports it unbound");
    assert_eq!(entry.file, "surface/modal.rs");
    assert!(entry.spec.contains("UIF-MOD-004"));
}

// ---------------------------------------------------------------------------
// 2. Opening and registration
// ---------------------------------------------------------------------------

#[test]
fn test_open_registers_a_band_3_modal_surface() {
    let scenario = Scenario::open(standard_spec());

    let surface = scenario
        .registry
        .get(&sid("editor"))
        .expect("the modal must register its surface");
    assert_eq!(surface.kind(), SurfaceKind::Modal);
    assert_eq!(surface.band(), ZBand::Modal);
    assert_eq!(surface.band().index(), 3);
    assert!(surface.band().blocks_lower_bands());
}

#[test]
fn test_open_registers_the_viewport_rectangle_as_its_region() {
    let viewport = rect(10.0, 20.0, 1600.0, 900.0);
    let scenario = Scenario::at(standard_spec(), viewport, None);
    let surface = scenario.registry.get(&sid("editor")).unwrap();

    assert_eq!(surface.regions(), &[viewport]);
    assert!(surface.has_regions());
}

#[test]
fn test_open_pushes_a_trap_on_the_modal_group() {
    let scenario = Scenario::open(standard_spec());

    assert!(scenario.focus.is_trapped());
    assert_eq!(scenario.focus.trap_depth(), 1);
    assert_eq!(scenario.focus.active_trap(), Some(scenario.host.group_id()));
    assert_eq!(scenario.host.group_id(), &group_for(&sid("editor")));
}

#[test]
fn test_open_focuses_the_declared_primary_control() {
    let scenario = Scenario::open(standard_spec());

    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
}

#[test]
fn test_open_without_a_primary_focuses_the_first_enabled_control() {
    let spec = ModalSpec::new("about", "About", ModalSizing::about()).with_controls(vec![
        FocusMember::new("card.close"),
        FocusMember::new("card.more"),
    ]);
    let scenario = Scenario::open(spec);

    assert_eq!(scenario.focus.current(), Some(&fid("card.close")));
}

#[test]
fn test_a_disabled_primary_falls_back_to_the_first_enabled_control() {
    let spec = ModalSpec::new("about", "About", ModalSizing::about())
        .with_controls(vec![
            FocusMember::disabled("card.nope"),
            FocusMember::new("card.close"),
        ])
        .with_primary("card.nope");
    let scenario = Scenario::open(spec);

    assert_eq!(scenario.focus.current(), Some(&fid("card.close")));
}

#[test]
fn test_a_primary_outside_the_card_is_rejected_by_the_trap() {
    let spec = standard_spec().with_primary("bg.tree");
    let scenario = Scenario::at(spec, rect(0.0, 0.0, 1920.0, 1080.0), Some("bg.navbar"));

    assert_eq!(
        scenario.focus.current(),
        Some(&fid("card.field")),
        "a primary that is not in the card must not be focusable through the trap"
    );
}

#[test]
fn test_a_modal_with_nothing_focusable_clears_focus_rather_than_leaving_the_background_lit() {
    let spec = ModalSpec::new("empty", "Empty", ModalSizing::about())
        .with_controls(vec![
            FocusMember::disabled("card.a"),
            FocusMember::disabled("card.b"),
        ])
        .with_primary("card.a");
    let scenario = Scenario::at(spec, rect(0.0, 0.0, 1920.0, 1080.0), Some("bg.tree"));

    assert_eq!(scenario.focus.current(), None);
    assert!(
        scenario.focus.is_trapped(),
        "the trap still governs an empty card: focus must not fall back out of it"
    );
}

#[test]
fn test_open_makes_the_modal_the_topmost_surface() {
    let mut scenario = Scenario::open(standard_spec());
    scenario.registry.register(
        sid("bg"),
        SurfaceKind::Workspace,
        vec![rect(0.0, 0.0, 1920.0, 1080.0)],
    );

    assert_eq!(
        scenario.registry.topmost().map(|s| s.id().as_str()),
        Some("editor")
    );
    assert_eq!(
        scenario.registry.highest_open_band(),
        Some(ZBand::Modal),
        "the modal must own routing while it is open"
    );
}

#[test]
fn test_an_open_modal_makes_the_workspace_unreachable_to_the_pointer() {
    let scenario = Scenario::open(standard_spec());

    for x in (0..40).map(|i| i as f32 * 64.0 - 128.0) {
        for y in (0..30).map(|i| i as f32 * 64.0 - 128.0) {
            let route = scenario.registry.route_pointer(pt(x, y));
            assert!(
                !route.reaches_workspace(),
                "a modal is open and the workspace still got the click at ({x}, {y})"
            );
            assert!(route.passed_through().is_empty());
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Tab containment — UIF-MOD-001
// ---------------------------------------------------------------------------

#[test]
fn test_tab_never_leaves_the_modal_across_many_cycles() {
    let mut scenario = Scenario::clean_focused();

    for _ in 0..64 {
        let outcome =
            scenario
                .host
                .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        match outcome {
            ModalOutcome::FocusMoved { focused } => {
                assert!(
                    CARD.contains(&focused.as_str()),
                    "Tab left the card and landed on {focused}"
                );
            }
            other => panic!("expected a focus move inside the card, got {other:?}"),
        }
    }
}

#[test]
fn test_shift_tab_never_leaves_the_modal_across_many_cycles() {
    let mut scenario = Scenario::clean_focused();

    for _ in 0..64 {
        let outcome = scenario.host.on_key(
            KeyClass::ShiftTab,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        let ModalOutcome::FocusMoved { focused } = outcome else {
            panic!("expected a focus move inside the card, got {outcome:?}");
        };
        assert!(
            CARD.contains(&focused.as_str()),
            "Shift+Tab left the card and landed on {focused}"
        );
    }
}

#[test]
fn test_alternating_tab_cycles_never_reach_a_background_control() {
    let mut scenario = Scenario::clean_focused();

    for i in 0..200 {
        let class = if i % 2 == 0 {
            KeyClass::Tab
        } else {
            KeyClass::ShiftTab
        };
        scenario
            .host
            .on_key(class, &mut scenario.focus, &mut scenario.registry);

        let current = scenario
            .focus
            .current()
            .unwrap_or_else(|| panic!("focus vanished at step {i}"));
        assert!(
            !BACKGROUND.contains(&current.as_str()),
            "step {i} ({class:?}) put focus on {current}"
        );
    }
}

#[test]
fn test_tab_containment_survives_a_focus_group_rebuild() {
    let mut scenario = Scenario::clean_focused();
    // The ids the card is allowed to hold focus on. A rebuild replaces this
    // set entirely; the trap is what keeps the walk inside it.
    let mut allowed: Vec<String> = CARD.iter().map(|label| (*label).to_string()).collect();

    for round in 0..6 {
        for _ in 0..5 {
            scenario
                .host
                .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
            let current = scenario.focus.current().unwrap();
            assert!(
                allowed.iter().any(|label| label == current.as_str()),
                "round {round}: focus escaped the card and landed on {current}"
            );
        }

        let roster = if round % 2 == 0 {
            vec![
                FocusMember::new("card.only"),
                FocusMember::disabled("card.dead"),
                FocusMember::new("card.other"),
            ]
        } else {
            vec![
                FocusMember::new("card.field"),
                FocusMember::new("card.cancel"),
            ]
        };
        scenario
            .host
            .sync_members(roster.clone(), &mut scenario.focus);
        allowed = roster
            .iter()
            .filter(|member| member.enabled)
            .map(|member| member.id.to_string())
            .collect();

        for _ in 0..5 {
            scenario
                .host
                .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
            let current = scenario.focus.current().unwrap();
            assert!(
                !BACKGROUND.contains(&current.as_str()),
                "round {round}: a rebuilt card let focus reach {current}"
            );
        }
    }
}

#[test]
fn test_disabled_members_are_stepped_over_but_never_visited() {
    let mut scenario = Scenario::clean_focused();

    for _ in 0..12 {
        scenario
            .host
            .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        assert_ne!(
            scenario.focus.current(),
            Some(&fid("card.primary")),
            "a disabled member must never hold focus"
        );
    }
}

#[test]
fn test_the_model_refuses_focus_on_a_background_control_while_trapped() {
    let mut scenario = Scenario::clean_focused();

    for label in BACKGROUND {
        assert!(
            !scenario.focus.focus(&fid(label)),
            "focus on the background control {label} must be refused"
        );
        assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
    }
}

#[test]
fn test_rebuilding_the_background_group_cannot_widen_the_trap() {
    let mut scenario = Scenario::clean_focused();

    scenario.focus.register_group(
        gid("bg"),
        vec![FocusMember::new("bg.new"), FocusMember::new("bg.newer")],
    );

    for _ in 0..8 {
        scenario
            .host
            .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        assert!(
            CARD.contains(&scenario.focus.current().unwrap().as_str()),
            "the rebuilt background group pulled focus out of the card"
        );
    }
}

#[test]
fn test_a_card_with_one_control_stays_on_it() {
    let spec = ModalSpec::new("one", "One", ModalSizing::about())
        .with_controls(vec![FocusMember::new("card.only")]);
    let mut scenario = Scenario::open(spec);

    for _ in 0..4 {
        let outcome =
            scenario
                .host
                .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        assert_eq!(
            outcome,
            ModalOutcome::FocusMoved {
                focused: fid("card.only")
            }
        );
    }
}

#[test]
fn test_tabbing_a_card_with_nothing_enabled_reports_nothing_focusable() {
    let spec = ModalSpec::new("dead", "Dead", ModalSizing::about())
        .with_controls(vec![FocusMember::disabled("card.a")]);
    let mut scenario = Scenario::open(spec);

    let outcome = scenario
        .host
        .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        ModalOutcome::NoOp {
            reason: ModalNoOp::NothingFocusable
        }
    );
}

#[test]
fn test_the_router_agrees_with_the_modal_after_every_tab() {
    const ROWS: &[(&str, bool)] = &[
        ("bg.navbar", true),
        ("bg.tree", true),
        ("card.field", true),
        ("card.cancel", true),
        ("card.primary", false),
    ];

    let mut harness = Harness::headless(|| mixed_focusable_tree(ROWS));
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();

    focus.register_group(
        gid("bg"),
        BACKGROUND
            .iter()
            .map(|label| FocusMember::new(*label))
            .collect(),
    );
    focus.focus(&fid("bg.tree"));

    let spec = ModalSpec::new("editor", "Editor", ModalSizing::editor())
        .with_controls(vec![
            FocusMember::new("card.field"),
            FocusMember::new("card.cancel"),
            FocusMember::disabled("card.primary"),
        ])
        .with_primary("card.field");
    let mut host = ModalHost::open(
        spec,
        rect(0.0, 0.0, 1920.0, 1080.0),
        UNMEASURED,
        &mut focus,
        &mut registry,
    );

    harness.sync(&focus);
    assert_agree(&mut harness, &focus, "open");

    for step in 0..12 {
        let class = if step % 2 == 0 {
            KeyClass::Tab
        } else {
            KeyClass::ShiftTab
        };
        host.on_key(class, &mut focus, &mut registry);
        harness.sync(&focus);
        assert_agree(&mut harness, &focus, "a Tab step");

        let focused = harness.focused().expect("the router must name a control");
        assert!(
            focused == iced_id("card.field") || focused == iced_id("card.cancel"),
            "after {step} steps the router named {focused:?}, outside the card"
        );
    }
}

#[test]
fn test_a_router_side_focus_on_the_background_is_pulled_back_by_the_model() {
    const ROWS: &[(&str, bool)] = &[
        ("bg.tree", true),
        ("card.field", true),
        ("card.cancel", true),
    ];

    let mut harness = Harness::headless(|| mixed_focusable_tree(ROWS));
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();

    focus.register_group(gid("bg"), vec![FocusMember::new("bg.tree")]);
    focus.focus(&fid("bg.tree"));

    let spec = ModalSpec::new("editor", "Editor", ModalSizing::editor())
        .with_controls(vec![
            FocusMember::new("card.field"),
            FocusMember::new("card.cancel"),
        ])
        .with_primary("card.field");
    let host = ModalHost::open(
        spec,
        rect(0.0, 0.0, 1920.0, 1080.0),
        UNMEASURED,
        &mut focus,
        &mut registry,
    );
    assert!(host.is_open());
    harness.sync(&focus);

    // The router has no concept of a trap and can be written directly...
    harness.focus(&iced_id("bg.tree"));
    assert_eq!(harness.focused(), Some(iced_id("bg.tree")));

    // ...but the model is authoritative and never accepted that move, so the
    // next projection puts focus back inside the card.
    assert!(!focus.focus(&fid("bg.tree")));
    harness.sync(&focus);
    assert_eq!(harness.focused(), Some(iced_id("card.field")));
}

// ---------------------------------------------------------------------------
// 4. Escape and the dirty guard — UIF-MOD-002
// ---------------------------------------------------------------------------

#[test]
fn test_escape_on_a_clean_modal_dismisses() {
    let mut scenario = Scenario::clean_focused();

    let outcome = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::Dismissed {
            cause: DismissCause::Escape
        }
    );
    assert!(!scenario.host.is_open());
}

#[test]
fn test_escape_on_a_dirty_modal_refuses() {
    let mut scenario = Scenario::dirty();

    let outcome = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::DismissRefused {
            cause: DismissCause::Escape
        }
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_refused_escape_changes_nothing_at_all() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before_error = scenario.host.error().cloned();
    let before_card = scenario.host.card();

    scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
    assert_eq!(scenario.focus.trap_depth(), 1);
    assert_eq!(scenario.host.error().cloned(), before_error);
    assert_eq!(scenario.host.card(), before_card);
    assert!(scenario.registry.contains(&sid("editor")));
    assert!(scenario.focus.is_trapped());
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_dirty_modal_refuses_escape_every_time() {
    let mut scenario = Scenario::dirty();

    for _ in 0..10 {
        assert!(
            scenario
                .host
                .on_key(
                    KeyClass::Escape,
                    &mut scenario.focus,
                    &mut scenario.registry
                )
                .is_refused()
        );
    }
    assert!(scenario.host.is_open());
}

#[test]
fn test_clearing_dirty_lets_escape_through() {
    let mut scenario = Scenario::dirty();

    assert!(scenario.host.set_dirty(false));
    assert!(
        scenario
            .host
            .on_key(
                KeyClass::Escape,
                &mut scenario.focus,
                &mut scenario.registry
            )
            .is_dismissed()
    );
}

#[test]
fn test_the_escape_chain_gets_dismissed_from_a_clean_modal() {
    let mut scenario = Scenario::clean_focused();

    let answer = scenario.host.unwind_answer(&mut scenario.focus);
    assert_eq!(answer, UnwindAnswer::Dismissed);
}

#[test]
fn test_the_escape_chain_gets_cannot_unwind_from_a_dirty_modal() {
    let mut scenario = Scenario::dirty();

    let answer = scenario.host.unwind_answer(&mut scenario.focus);
    assert_eq!(answer, UnwindAnswer::Declined(DeclineReason::CannotUnwind));
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_closed_modal_declines_the_unwind_as_not_open() {
    let mut scenario = Scenario::clean_focused();
    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    let answer = scenario.host.unwind_answer(&mut scenario.focus);
    assert_eq!(answer, UnwindAnswer::Declined(DeclineReason::NotOpen));
}

#[test]
fn test_escape_via_the_registry_removes_the_surface_when_clean() {
    let mut scenario = Scenario::clean_focused();

    let outcome = scenario
        .host
        .escape_via(&mut scenario.registry, &mut scenario.focus);
    assert!(outcome.dismissed());
    assert_eq!(outcome.consumed_by(), Some(&sid("editor")));
    assert!(!scenario.registry.contains(&sid("editor")));
}

#[test]
fn test_escape_via_the_registry_keeps_the_surface_when_dirty() {
    let mut scenario = Scenario::dirty();

    let outcome = scenario
        .host
        .escape_via(&mut scenario.registry, &mut scenario.focus);
    assert!(!outcome.is_consumed());
    assert!(scenario.registry.contains(&sid("editor")));
    assert_eq!(
        outcome.refusal(),
        Some(&EscapeRefusal::AllDeclined {
            declined: vec![Declination {
                surface: sid("editor"),
                reason: DeclineReason::CannotUnwind,
            }]
        })
    );
}

#[test]
fn test_a_menu_above_the_modal_unwinds_first_and_the_modal_is_untouched() {
    let (mut scenario, mut cascade) = with_menu_above(standard_spec());
    assert!(cascade.depth() >= 1);

    let outcome = drive_escape(&mut scenario, &mut cascade);

    assert!(
        outcome.is_consumed(),
        "the menu band must consume the key before the modal is reached"
    );
    assert_ne!(
        outcome.consumed_by(),
        Some(&sid("editor")),
        "the modal consumed a key that belonged to the menu above it"
    );
    assert!(scenario.host.is_open());
    assert_eq!(scenario.focus.trap_depth(), 1);
    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
}

#[test]
fn test_a_menu_above_a_dirty_modal_unwinds_first_and_the_modal_stays_dirty() {
    let (mut scenario, mut cascade) = with_menu_above(dirty_spec());

    let mut consumed = 0;
    for _ in 0..8 {
        let outcome = drive_escape(&mut scenario, &mut cascade);
        if !outcome.is_consumed() {
            break;
        }
        consumed += 1;
    }

    assert!(consumed >= 1, "the menu should have unwound a level");
    assert!(
        scenario.host.is_open() && scenario.host.is_dirty(),
        "draining the menu band must not close a dirty modal"
    );
    assert!(scenario.registry.contains(&sid("editor")));
}

#[test]
fn test_after_the_menu_drains_the_modal_is_reached() {
    let (mut scenario, mut cascade) = with_menu_above(standard_spec());

    let mut last = None;
    for _ in 0..8 {
        let outcome = drive_escape(&mut scenario, &mut cascade);
        let stop = !outcome.is_consumed() || !scenario.host.is_open();
        last = Some(outcome);
        if stop {
            break;
        }
    }

    let last = last.expect("the chain must run");
    assert!(last.is_consumed());
    assert_eq!(last.consumed_by(), Some(&sid("editor")));
    assert!(!scenario.host.is_open());
}

// ---------------------------------------------------------------------------
// 4b. The pending action (§3.0.4.3, `INC-36`)
// ---------------------------------------------------------------------------
//
// `UIF-MOD-002`'s dirty guard protects the user's unsaved *edits*. The
// pending-action guard protects the application from being left holding a
// half-finished operation with no card to report it in. Both refuse
// `Escape`, and this group keeps them visibly separate: every test here
// runs against a **clean** card, so nothing but the action can be the
// thing that refuses.

#[test]
fn test_a_modal_opens_with_nothing_under_way() {
    let mut scenario = Scenario::clean_focused();

    assert_eq!(scenario.host.pending(), ActionPending::idle());
    assert!(!scenario.host.is_busy());
    assert_eq!(
        scenario.host.unwind_answer(&mut scenario.focus),
        UnwindAnswer::Dismissed,
        "with no action open the ordinary clean-card answer stands"
    );
}

#[test]
fn test_a_running_command_refuses_the_unwind() {
    let mut scenario = Scenario::clean_focused();
    scenario.host.set_pending(ActionPending::indeterminate());

    assert!(scenario.host.is_busy());
    assert_eq!(
        scenario.host.unwind_answer(&mut scenario.focus),
        UnwindAnswer::Declined(DeclineReason::CannotUnwind),
        "walking away mid-operation would strand the result"
    );
    assert!(
        scenario.host.is_open(),
        "a refusal leaves the card where it is: the user still has somewhere to watch the job"
    );
}

#[test]
fn test_the_action_guard_is_independent_of_the_dirty_guard() {
    // Clean card, busy action: only the action can be refusing.
    let mut busy_clean = Scenario::clean_focused();
    busy_clean.host.set_pending(ActionPending::indeterminate());
    assert!(!busy_clean.host.is_dirty());
    assert_eq!(
        busy_clean.host.unwind_answer(&mut busy_clean.focus),
        UnwindAnswer::Declined(DeclineReason::CannotUnwind)
    );

    // The action settles; the same clean card closes normally.
    busy_clean.host.set_pending(ActionPending::idle());
    assert_eq!(
        busy_clean.host.unwind_answer(&mut busy_clean.focus),
        UnwindAnswer::Dismissed,
        "the refusal was the action's, and it is gone with the action"
    );
}

#[test]
fn test_a_second_primary_press_is_suppressed_inside_the_window() {
    let mut scenario = Scenario::clean_focused();
    scenario
        .host
        .set_pending(ActionPending::idle().with_cancellable(true));

    assert_eq!(
        scenario.host.try_activate_primary(),
        Activation::Fired,
        "the first press goes through"
    );
    assert!(
        scenario.host.try_activate_primary().is_suppressed(),
        "§3.0.4.3: a second press inside the round-trip window is withheld even before the projection catches up"
    );
}

#[test]
fn test_a_stop_request_is_surfaced_once_and_only_for_a_cancellable_action() {
    let mut cancellable = Scenario::clean_focused();
    cancellable
        .host
        .set_pending(ActionPending::indeterminate().with_cancellable(true));
    assert!(cancellable.host.request_cancel().is_some());
    assert!(
        cancellable.host.request_cancel().is_none(),
        "one request per busy period: the stop is already in flight"
    );

    let mut unstoppable = Scenario::clean_focused();
    unstoppable.host.set_pending(ActionPending::indeterminate());
    assert!(
        unstoppable.host.request_cancel().is_none(),
        "an action with no stop path shows no dead stop affordance"
    );
}

#[test]
fn test_teardown_drops_the_latches_but_not_the_app_projection() {
    let mut scenario = Scenario::clean_focused();
    scenario.host.set_pending(ActionPending::indeterminate());
    assert!(scenario.host.is_busy());

    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    assert!(!scenario.host.is_open());
    // The latches are what teardown owns. A host that kept an in-flight
    // gate could never fire again, and one that kept a stop request would
    // hold a stop affordance for a card that no longer exists.
    assert!(
        !scenario.host.ephemeral().get().is_pending(),
        "the gate is released, so nothing can be wedged by a torn-down card"
    );
    assert!(
        !scenario.host.ephemeral().get().cancel_requested(),
        "no stop request survives the card it was asked of"
    );
    // The projection itself belongs to the application. The host does not
    // write a new one on the way out, because whatever value it picked
    // would be a guess about whether the caller's job actually finished.
    assert_eq!(scenario.host.pending(), ActionPending::indeterminate());
}

#[test]
fn test_the_controlled_and_ephemeral_views_split_the_frame() {
    let mut scenario = Scenario::clean_focused();
    scenario.host.set_pending(ActionPending::indeterminate());

    // The application's half: readable through `Controlled`, and there is
    // deliberately no mutable path through it.
    let controlled = scenario.host.controlled();
    assert!(controlled.get().is_open());
    assert!(!controlled.get().is_dirty());

    // The framework's half: what the frame knows about the job.
    let ephemeral = scenario.host.ephemeral();
    assert!(ephemeral.get().is_busy());
}

// ---------------------------------------------------------------------------
// 5. The scrim
// ---------------------------------------------------------------------------

#[test]
fn test_a_scrim_click_on_a_clean_modal_dismisses() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.scrim_point();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::Dismissed {
            cause: DismissCause::Scrim
        }
    );
    assert!(!scenario.host.is_open());
}

#[test]
fn test_a_scrim_click_on_a_dirty_modal_does_not_dismiss() {
    let mut scenario = Scenario::dirty();
    let at = scenario.scrim_point();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::DismissRefused {
            cause: DismissCause::Scrim
        }
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_refused_scrim_click_changes_nothing() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before_error = scenario.host.error().cloned();
    let at = scenario.scrim_point();

    scenario.host.on_pointer(
        at,
        CardTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
    assert_eq!(scenario.focus.trap_depth(), 1);
    assert_eq!(scenario.host.error().cloned(), before_error);
    assert!(scenario.registry.contains(&sid("editor")));
}

#[test]
fn test_a_scrim_click_never_reaches_the_workspace() {
    let scenario = Scenario::dirty();
    let at = scenario.scrim_point();

    let route = scenario.registry.route_pointer(at);
    assert!(!route.reaches_workspace());
    assert!(
        matches!(route.target(), PointerTarget::Surface(id) if id.as_str() == "editor"),
        "the full-viewport region should own the point, got {:?}",
        route.target()
    );
}

#[test]
fn test_the_right_and_bottom_edges_are_scrim() {
    let scenario = Scenario::clean_focused();
    let card = scenario.host.card();

    assert_eq!(
        scenario
            .host
            .hit_test(pt(card.x + card.width, card.y + card.height / 2.0)),
        ModalHit::Scrim
    );
    assert_eq!(
        scenario
            .host
            .hit_test(pt(card.x + card.width / 2.0, card.y + card.height)),
        ModalHit::Scrim
    );
    assert_eq!(scenario.host.hit_test(pt(card.x, card.y)), ModalHit::Card);
}

#[test]
fn test_a_point_just_inside_the_card_is_not_the_scrim() {
    let scenario = Scenario::clean_focused();
    let card = scenario.host.card();

    assert!(scenario.host.is_inside_card(pt(card.x + 0.5, card.y + 0.5)));
    assert!(
        scenario
            .host
            .is_inside_card(pt(card.x + card.width - 0.5, card.y + card.height - 0.5))
    );
}

// ---------------------------------------------------------------------------
// 6. Card-level absorption — UIF-VER-003
// ---------------------------------------------------------------------------

#[test]
fn test_clicking_a_disabled_primary_button_does_not_dismiss() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Control(fid("card.primary")),
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(
        outcome,
        ModalOutcome::Absorbed {
            target: CardTarget::Disabled
        },
        "a disabled primary must be absorbed as a disabled control"
    );
    assert!(scenario.host.is_open());
    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
}

#[test]
fn test_clicking_empty_card_padding_does_not_dismiss() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(
        outcome,
        ModalOutcome::Absorbed {
            target: CardTarget::Padding
        }
    );
    assert!(scenario.host.is_open());
    assert!(scenario.registry.contains(&sid("editor")));
}

#[test]
fn test_a_disabled_control_reported_as_disabled_is_absorbed() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Disabled,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert!(outcome.is_absorbed());
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_control_that_is_not_one_of_ours_is_absorbed_as_disabled() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Control(fid("bg.tree")),
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(
        outcome,
        ModalOutcome::Absorbed {
            target: CardTarget::Disabled
        }
    );
    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
}

#[test]
fn test_an_unregistered_control_is_absorbed_as_disabled() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Control(fid("card.never.mounted")),
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::Absorbed {
            target: CardTarget::Disabled
        }
    );
}

#[test]
fn test_clicking_an_enabled_control_moves_focus_and_keeps_the_dialog_open() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Control(fid("card.cancel")),
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(
        outcome,
        ModalOutcome::FocusMoved {
            focused: fid("card.cancel")
        }
    );
    assert!(scenario.host.is_open());
    assert_eq!(scenario.focus.current(), Some(&fid("card.cancel")));
}

#[test]
fn test_geometry_outranks_the_callers_report_outside_the_card() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.scrim_point();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::CloseButton,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::Dismissed {
            cause: DismissCause::Scrim
        },
        "a Close claim outside the card is a scrim click, not a close"
    );
}

#[test]
fn test_a_padding_report_outside_the_card_takes_the_scrim_path() {
    let mut scenario = Scenario::dirty();
    let at = scenario.scrim_point();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::DismissRefused {
            cause: DismissCause::Scrim
        }
    );
}

#[test]
fn test_no_point_inside_the_card_can_dismiss_the_modal() {
    let mut scenario = Scenario::clean_focused();
    let card = scenario.host.card();

    for i in 1..24 {
        for j in 1..24 {
            let at = pt(
                card.x + card.width * i as f32 / 24.0,
                card.y + card.height * j as f32 / 24.0,
            );
            assert!(
                scenario.host.is_inside_card(at),
                "{at:?} should be inside the card"
            );

            let outcome = scenario.host.on_pointer(
                at,
                CardTarget::Padding,
                &mut scenario.focus,
                &mut scenario.registry,
            );
            assert!(
                outcome.stays_open(),
                "a padding click at {at:?} dismissed the dialog: {outcome:?}"
            );
        }
    }
    assert!(scenario.host.is_open());
}

#[test]
fn test_no_point_inside_the_card_can_dismiss_a_dirty_modal() {
    let mut scenario = Scenario::dirty();
    let card = scenario.host.card();

    for i in 1..16 {
        for j in 1..16 {
            let at = pt(
                card.x + card.width * i as f32 / 16.0,
                card.y + card.height * j as f32 / 16.0,
            );
            let outcome = scenario.host.on_pointer(
                at,
                CardTarget::Control(fid("card.primary")),
                &mut scenario.focus,
                &mut scenario.registry,
            );
            assert!(outcome.stays_open(), "{at:?} dismissed a dirty dialog");
        }
    }
    assert!(scenario.host.is_open() && scenario.host.is_dirty());
}

#[test]
fn test_the_close_control_dismisses_a_clean_modal() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::CloseButton,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::Dismissed {
            cause: DismissCause::CloseButton
        }
    );
}

#[test]
fn test_the_close_control_refuses_a_dirty_modal() {
    let mut scenario = Scenario::dirty();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::CloseButton,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::DismissRefused {
            cause: DismissCause::CloseButton
        }
    );
    assert!(scenario.host.is_open());
}

// ---------------------------------------------------------------------------
// 7. The error banner — UIF-MOD-003
// ---------------------------------------------------------------------------

#[test]
fn test_a_modal_opens_with_the_error_it_was_given() {
    let scenario = Scenario::open(errored_spec());
    let error = scenario.host.error().expect("the banner must be showing");

    assert_eq!(error.code(), "MOD-5001");
    assert_eq!(error.message(), "Save failed");
    assert_eq!(
        error.location(),
        Some(&ErrorLocation::Position {
            line: 12,
            column: 7
        })
    );
    assert_eq!(error.recovery(), &Recovery::Retry);
    assert!(scenario.host.has_error());
}

#[test]
fn test_a_modal_without_an_error_has_no_banner() {
    let scenario = Scenario::open(standard_spec());
    assert!(!scenario.host.has_error());
    assert_eq!(scenario.host.error(), None);
}

#[test]
fn test_the_error_persists_across_tab_cycles() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    for _ in 0..20 {
        scenario
            .host
            .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        assert_eq!(scenario.host.error().cloned(), before);
    }
}

#[test]
fn test_the_error_persists_across_shift_tab_cycles() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    for _ in 0..20 {
        scenario.host.on_key(
            KeyClass::ShiftTab,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        assert_eq!(scenario.host.error().cloned(), before);
    }
}

#[test]
fn test_the_error_persists_across_absorbed_clicks() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    let targets = [
        CardTarget::Padding,
        CardTarget::Disabled,
        CardTarget::Control(fid("card.primary")),
        CardTarget::Control(fid("bg.tree")),
        CardTarget::Control(fid("card.cancel")),
    ];
    for (i, target) in targets.iter().enumerate() {
        let card = scenario.host.card();
        let at = pt(card.x + 10.0 + i as f32, card.y + 10.0);
        scenario.host.on_pointer(
            at,
            target.clone(),
            &mut scenario.focus,
            &mut scenario.registry,
        );
        assert_eq!(
            scenario.host.error().cloned(),
            before,
            "{target:?} changed the banner"
        );
    }
}

#[test]
fn test_the_error_persists_across_a_refused_escape() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    for _ in 0..5 {
        scenario.host.on_key(
            KeyClass::Escape,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        assert_eq!(scenario.host.error().cloned(), before);
    }
}

#[test]
fn test_the_error_persists_across_a_refused_scrim_click() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    for _ in 0..5 {
        let at = scenario.scrim_point();
        scenario.host.on_pointer(
            at,
            CardTarget::Padding,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        assert_eq!(scenario.host.error().cloned(), before);
    }
}

#[test]
fn test_only_an_explicit_clear_or_a_new_error_changes_the_banner() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    for _ in 0..5 {
        let card = scenario.host.card();
        let scrim = scenario.scrim_point();

        scenario
            .host
            .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        scenario.host.on_key(
            KeyClass::ShiftTab,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        scenario.host.on_pointer(
            pt(card.x + 4.0, card.y + 4.0),
            CardTarget::Padding,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        scenario.host.on_pointer(
            pt(card.x + 8.0, card.y + 8.0),
            CardTarget::Control(fid("card.primary")),
            &mut scenario.focus,
            &mut scenario.registry,
        );
        scenario.host.on_key(
            KeyClass::Escape,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        scenario.host.on_pointer(
            scrim,
            CardTarget::Padding,
            &mut scenario.focus,
            &mut scenario.registry,
        );
        scenario.host.resize(
            rect(0.0, 0.0, 1440.0, 900.0),
            UNMEASURED,
            &mut scenario.registry,
        );
        scenario.host.sync_members(
            vec![
                FocusMember::new("card.field"),
                FocusMember::new("card.cancel"),
            ],
            &mut scenario.focus,
        );

        assert_eq!(
            scenario.host.error().cloned(),
            before,
            "an interaction changed the banner"
        );
    }

    scenario.host.clear_error();
    assert!(!scenario.host.has_error());
}

#[test]
fn test_set_error_supersedes_the_previous_one() {
    let mut scenario = Scenario::open(errored_spec());
    let first = scenario.host.error().cloned();

    let second = ModalError::new("MOD-5002", "Authentication failed")
        .at(ErrorLocation::Region("Tools list row 3".into()))
        .with_recovery(Recovery::FocusControl(fid("card.field")));

    assert_eq!(scenario.host.set_error(second.clone()), first);
    assert_eq!(scenario.host.error(), Some(&second));
}

#[test]
fn test_clear_error_returns_what_it_removed() {
    let mut scenario = Scenario::open(errored_spec());
    let before = scenario.host.error().cloned();

    assert_eq!(scenario.host.clear_error(), before);
    assert_eq!(scenario.host.clear_error(), None);
}

#[test]
fn test_acknowledge_recovery_clears_the_banner() {
    let mut scenario =
        Scenario::open(standard_spec().with_error(
            ModalError::new("MOD-5003", "Bad input").with_recovery(Recovery::Acknowledge),
        ));
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::ErrorRecovery,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::Absorbed {
            target: CardTarget::ErrorRecovery
        }
    );
    assert!(!scenario.host.has_error());
    assert!(scenario.host.is_open());
}

#[test]
fn test_retry_recovery_keeps_the_banner_and_asks_for_a_retry() {
    let mut scenario = Scenario::open(errored_spec());
    let before = scenario.host.error().cloned();
    let at = scenario.card_centre();

    scenario.host.on_pointer(
        at,
        CardTarget::ErrorRecovery,
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(
        scenario.host.error().cloned(),
        before,
        "a retry must not hide the error"
    );
    assert!(scenario.host.retry_pending());
}

#[test]
fn test_take_retry_hands_back_the_error_it_was_requested_for() {
    let mut scenario = Scenario::open(errored_spec());
    let at = scenario.card_centre();

    assert_eq!(scenario.host.take_retry(), None);
    scenario.host.on_pointer(
        at,
        CardTarget::ErrorRecovery,
        &mut scenario.focus,
        &mut scenario.registry,
    );

    let taken = scenario.host.take_retry();
    assert_eq!(taken, scenario.host.error().cloned());
    assert!(!scenario.host.retry_pending());
    assert_eq!(scenario.host.take_retry(), None);
}

#[test]
fn test_a_new_error_clears_a_pending_retry() {
    let mut scenario = Scenario::open(errored_spec());
    let at = scenario.card_centre();
    scenario.host.on_pointer(
        at,
        CardTarget::ErrorRecovery,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert!(scenario.host.retry_pending());

    scenario
        .host
        .set_error(ModalError::new("MOD-5004", "Still failing"));
    assert!(!scenario.host.retry_pending());
}

#[test]
fn test_focus_control_recovery_moves_focus_and_keeps_the_banner() {
    let mut scenario = Scenario::open(
        standard_spec().with_error(
            ModalError::new("MOD-5005", "Field invalid")
                .with_recovery(Recovery::FocusControl(fid("card.cancel"))),
        ),
    );
    let before = scenario.host.error().cloned();
    let at = scenario.card_centre();

    scenario.host.on_pointer(
        at,
        CardTarget::ErrorRecovery,
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(scenario.focus.current(), Some(&fid("card.cancel")));
    assert_eq!(scenario.host.error().cloned(), before);
}

#[test]
fn test_a_recovery_click_with_no_error_absorbs_and_changes_nothing() {
    let mut scenario = Scenario::clean_focused();
    let at = scenario.card_centre();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::ErrorRecovery,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::Absorbed {
            target: CardTarget::ErrorRecovery
        }
    );
    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
    assert!(!scenario.host.has_error());
}

#[test]
fn test_the_error_survives_a_focus_group_rebuild() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    scenario
        .host
        .sync_members(vec![FocusMember::new("card.new")], &mut scenario.focus);
    assert_eq!(scenario.host.error().cloned(), before);
}

#[test]
fn test_the_error_survives_a_resize() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );
    let before = scenario.host.error().cloned();

    scenario.host.resize(
        rect(0.0, 0.0, 800.0, 600.0),
        UNMEASURED,
        &mut scenario.registry,
    );
    assert_eq!(scenario.host.error().cloned(), before);
}

#[test]
fn test_a_clean_modal_with_an_error_still_dismisses_on_scrim() {
    // The documented reading: `UIF-MOD-003` says a failure must not close
    // the modal, not that the user cannot. The banner goes with the modal.
    let mut scenario = Scenario::open(errored_spec());
    let at = scenario.scrim_point();

    let outcome = scenario.host.on_pointer(
        at,
        CardTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert!(outcome.is_dismissed());
    assert!(!scenario.host.is_open());
}

// ---------------------------------------------------------------------------
// 8. Nested modals — the confirm-over-modal case
// ---------------------------------------------------------------------------

/// Open a confirmation card over the scenario's modal.
fn open_confirm(scenario: &mut Scenario) -> ModalHost {
    let spec = ModalSpec::new("confirm", "Discard changes?", ModalSizing::about())
        .with_controls(vec![
            FocusMember::new("confirm.discard"),
            FocusMember::new("confirm.keep"),
        ])
        .with_primary("confirm.keep");

    ModalHost::open(
        spec,
        scenario.host.viewport(),
        Size::new(0.0, 200.0),
        &mut scenario.focus,
        &mut scenario.registry,
    )
}

#[test]
fn test_the_topmost_trap_governs_a_second_modal() {
    let scenario = &mut Scenario::dirty();
    let upper = open_confirm(scenario);

    assert_eq!(scenario.focus.trap_depth(), 2);
    assert_eq!(scenario.focus.active_trap(), Some(upper.group_id()));
    assert_eq!(scenario.focus.current(), Some(&fid("confirm.keep")));
}

#[test]
fn test_tabbing_the_lower_modal_cannot_escape_the_upper_trap() {
    let mut scenario = Scenario::dirty();
    let upper = open_confirm(&mut scenario);

    for _ in 0..12 {
        scenario
            .host
            .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        let current = scenario.focus.current().unwrap();
        assert!(
            current.as_str().starts_with("confirm."),
            "the lower modal's Tab reached {current} while the confirmation was up"
        );
        assert_eq!(scenario.focus.active_trap(), Some(upper.group_id()));
    }
}

#[test]
fn test_the_upper_modal_owns_the_pointer() {
    let mut scenario = Scenario::dirty();
    let upper = open_confirm(&mut scenario);
    let card = upper.card();

    let at = pt(card.x + card.width / 2.0, card.y + card.height / 2.0);
    let route = scenario.registry.route_pointer(at);
    assert_eq!(route.target(), &PointerTarget::Surface(sid("confirm")));
}

#[test]
fn test_dismissing_the_upper_modal_restores_focus_to_the_lower_one() {
    let mut scenario = Scenario::dirty();
    scenario
        .host
        .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
    let lower_focus = scenario.focus.current().cloned();
    assert!(lower_focus.is_some());

    let mut upper = open_confirm(&mut scenario);
    assert_ne!(scenario.focus.current(), lower_focus.as_ref());

    let outcome = upper.close(&mut scenario.focus, &mut scenario.registry);
    assert!(outcome.is_dismissed());
    assert_eq!(scenario.focus.current(), lower_focus.as_ref());
    assert_eq!(scenario.focus.trap_depth(), 1);
    assert_eq!(scenario.focus.active_trap(), Some(scenario.host.group_id()));
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_dirty_upper_modal_refuses_and_the_lower_modal_is_untouched() {
    let mut scenario = Scenario::dirty();
    let mut upper = open_confirm(&mut scenario);
    upper.set_dirty(true);

    let outcome = upper.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::DismissRefused {
            cause: DismissCause::Escape
        }
    );
    assert!(upper.is_open());
    assert!(scenario.host.is_open());
    assert_eq!(scenario.focus.trap_depth(), 2);
}

#[test]
fn test_popping_through_the_stack_restores_the_original_focus() {
    let mut scenario = Scenario::dirty();
    let mut upper = open_confirm(&mut scenario);

    upper.close(&mut scenario.focus, &mut scenario.registry);
    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    assert_eq!(scenario.focus.current(), Some(&fid("bg.tree")));
    assert_eq!(scenario.focus.trap_depth(), 0);
    assert!(!scenario.focus.is_trapped());
    assert!(!scenario.registry.contains(&sid("editor")));
    assert!(!scenario.registry.contains(&sid("confirm")));
}

#[test]
fn test_popping_everything_with_nothing_focused_before_clears_focus() {
    let mut scenario = Scenario::open(dirty_spec());
    let mut upper = open_confirm(&mut scenario);

    upper.close(&mut scenario.focus, &mut scenario.registry);
    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    assert_eq!(scenario.focus.current(), None);
    assert_eq!(scenario.focus.trap_depth(), 0);
}

#[test]
fn test_the_confirm_over_modal_sequence_end_to_end() {
    let mut scenario = Scenario::at(
        errored_spec().with_dirty(true),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tree"),
    );

    // Escape is refused: the edit is unsaved.
    assert!(
        scenario
            .host
            .on_key(
                KeyClass::Escape,
                &mut scenario.focus,
                &mut scenario.registry
            )
            .is_refused()
    );

    // The host stacks a confirmation.
    let mut confirm = open_confirm(&mut scenario);
    assert_eq!(scenario.focus.current(), Some(&fid("confirm.keep")));

    // The user keeps editing: the confirmation closes, the editor stays.
    confirm.close(&mut scenario.focus, &mut scenario.registry);
    assert!(scenario.host.is_open() && scenario.host.is_dirty());
    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));
    assert!(scenario.host.has_error());

    // The save lands and the edit is clean; Escape now goes through.
    scenario.host.clear_error();
    scenario.host.set_dirty(false);
    assert!(
        scenario
            .host
            .on_key(
                KeyClass::Escape,
                &mut scenario.focus,
                &mut scenario.registry
            )
            .is_dismissed()
    );
    assert_eq!(scenario.focus.current(), Some(&fid("bg.tree")));
}

// ---------------------------------------------------------------------------
// 9. Teardown, focus restoration, the frame, determinism
// ---------------------------------------------------------------------------

#[test]
fn test_closing_restores_focus_to_the_exact_pre_modal_control() {
    let mut scenario = Scenario::at(
        standard_spec(),
        rect(0.0, 0.0, 1920.0, 1080.0),
        Some("bg.tabs"),
    );

    scenario
        .host
        .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(scenario.focus.current(), Some(&fid("card.option")));

    let outcome = scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        ModalOutcome::Dismissed {
            cause: DismissCause::Programmatic
        }
    );
    assert_eq!(scenario.focus.current(), Some(&fid("bg.tabs")));
}

#[test]
fn test_closing_with_nothing_focused_before_clears_rather_than_inventing_focus() {
    let mut scenario = Scenario::at(standard_spec(), rect(0.0, 0.0, 1920.0, 1080.0), None);
    assert_eq!(scenario.focus.current(), Some(&fid("card.field")));

    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(scenario.focus.current(), None);
}

#[test]
fn test_closing_unregisters_the_modal_focus_group() {
    let mut scenario = Scenario::clean_focused();
    let group = scenario.host.group_id().clone();
    assert!(scenario.focus.group(&group).is_some());

    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert!(scenario.focus.group(&group).is_none());
    assert!(scenario.focus.group(&gid("bg")).is_some());
}

#[test]
fn test_closing_unregisters_the_surface() {
    let mut scenario = Scenario::clean_focused();
    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    assert!(!scenario.registry.contains(&sid("editor")));
    assert!(scenario.registry.is_empty());
}

#[test]
fn test_closing_leaves_the_trap_stack_where_it_found_it() {
    let mut scenario = Scenario::clean_focused();
    assert_eq!(scenario.focus.trap_depth(), 1);

    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(scenario.focus.trap_depth(), 0);
    assert_eq!(scenario.focus.active_trap(), None);
}

#[test]
fn test_closing_twice_is_a_no_op() {
    let mut scenario = Scenario::clean_focused();
    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    let outcome = scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        ModalOutcome::NoOp {
            reason: ModalNoOp::NotOpen
        }
    );
}

#[test]
fn test_a_closed_host_reports_not_open_for_every_event() {
    let mut scenario = Scenario::clean_focused();
    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    for class in KeyClass::ALL {
        assert_eq!(
            scenario
                .host
                .on_key(class, &mut scenario.focus, &mut scenario.registry),
            ModalOutcome::NoOp {
                reason: ModalNoOp::NotOpen
            },
            "a closed host reacted to {class:?}"
        );
    }

    for target in [
        CardTarget::Padding,
        CardTarget::Disabled,
        CardTarget::CloseButton,
        CardTarget::ErrorRecovery,
        CardTarget::Control(fid("card.field")),
    ] {
        assert_eq!(
            scenario.host.on_pointer(
                pt(1.0, 1.0),
                target.clone(),
                &mut scenario.focus,
                &mut scenario.registry
            ),
            ModalOutcome::NoOp {
                reason: ModalNoOp::NotOpen
            },
            "a closed host reacted to a pointer press on {target:?}"
        );
    }
}

#[test]
fn test_untrapped_key_classes_are_left_to_the_bands_below() {
    let mut scenario = Scenario::clean_focused();
    let before = scenario.focus.current().cloned();

    for class in [KeyClass::ArrowNavigation, KeyClass::Space] {
        assert_eq!(
            ZBand::Modal.key_disposition(class),
            KeyDisposition::PassThrough,
            "{class:?} is not trapped by Band 3"
        );
        assert_eq!(
            scenario
                .host
                .on_key(class, &mut scenario.focus, &mut scenario.registry),
            ModalOutcome::NoOp {
                reason: ModalNoOp::KeyNotTrapped
            },
            "the host acted on {class:?}, which Band 3 does not trap"
        );
        assert_eq!(scenario.focus.current(), before.as_ref());
    }
}

#[test]
fn test_enter_is_trapped_but_owned_by_the_focused_control() {
    let mut scenario = Scenario::clean_focused();
    let before = scenario.focus.current().cloned();

    assert_eq!(
        ZBand::Modal.key_disposition(KeyClass::Enter),
        KeyDisposition::Swallow
    );
    assert_eq!(
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry),
        ModalOutcome::NoOp {
            reason: ModalNoOp::KeyOwnedByControl
        }
    );
    assert_eq!(scenario.focus.current(), before.as_ref());
}

#[test]
fn test_a_modal_not_registered_cannot_be_reached_by_the_escape_chain() {
    let mut scenario = Scenario::clean_focused();
    scenario.registry.unregister(&sid("editor"));

    let outcome = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        ModalOutcome::NoOp {
            reason: ModalNoOp::NotRegistered
        }
    );
    assert!(
        scenario.host.is_open(),
        "an unreachable host must not close"
    );
}

#[test]
fn test_resize_refreshes_the_surface_region_and_keeps_the_stacking_position() {
    let mut scenario = Scenario::clean_focused();
    scenario.registry.register(
        sid("other"),
        SurfaceKind::Modal,
        vec![rect(0.0, 0.0, 10.0, 10.0)],
    );

    let viewport = rect(0.0, 0.0, 1024.0, 768.0);
    scenario
        .host
        .resize(viewport, UNMEASURED, &mut scenario.registry);

    assert_eq!(scenario.host.viewport(), viewport);
    assert_eq!(
        scenario.registry.get(&sid("editor")).unwrap().regions(),
        &[viewport]
    );
    assert_eq!(
        scenario.registry.topmost().map(|s| s.id().as_str()),
        Some("other"),
        "re-registering must not restack the modal"
    );
}

#[test]
fn test_a_resize_on_a_closed_host_leaves_the_registry_alone() {
    let mut scenario = Scenario::clean_focused();
    scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);

    scenario.host.resize(
        rect(0.0, 0.0, 800.0, 600.0),
        UNMEASURED,
        &mut scenario.registry,
    );
    assert!(scenario.registry.is_empty());
}

#[test]
fn test_the_frame_reads_every_value_from_the_tokens() {
    let palette = Palette::dark();
    let frame = ModalFrame::resolve(&palette);

    assert_eq!(frame.surface, palette.surface_overlay);
    assert_eq!(frame.border, palette.border_strong);
    assert_eq!(frame.border_width, shape::BORDER_HAIRLINE);
    assert_eq!(frame.radius, shape::RADIUS_CARD);
    assert_eq!(frame.divider, palette.border_subtle);
    assert_eq!(frame.content_padding, space::LG);
    assert_eq!(frame.footer_padding_horizontal, space::LG);
    assert_eq!(frame.footer_padding_vertical, space::MD);
    assert_eq!(frame.close_target, size::HIT_TARGET_MIN);
}

#[test]
fn test_the_scrim_is_the_token_composition_not_a_literal() {
    let palette = Palette::dark();
    let frame = ModalFrame::resolve(&palette);

    assert_eq!(frame.scrim, scrim_for(&palette));
    assert_eq!(frame.scrim, palette.surface_scrim);
    assert_eq!(frame.scrim.a, SCRIM_ALPHA);
}

#[test]
fn test_the_scrim_bounds_are_the_viewport() {
    let viewport = rect(5.0, 7.0, 1234.0, 567.0);
    let scenario = Scenario::at(standard_spec(), viewport, None);
    let card = scenario.host.card();

    assert_eq!(scenario.host.scrim_bounds(), viewport);
    assert_eq!(
        scenario
            .host
            .hit_test(pt(card.x + card.width / 2.0, card.y + card.height / 2.0)),
        ModalHit::Card
    );
    assert_eq!(
        scenario
            .host
            .hit_test(pt(viewport.x - 1.0, viewport.y - 1.0)),
        ModalHit::Scrim
    );
}

#[test]
fn test_the_same_script_produces_the_same_outcomes_twice() {
    fn run() -> (
        Vec<ModalOutcome>,
        FocusState,
        SurfaceRegistry,
        Option<ModalError>,
    ) {
        let mut scenario = Scenario::at(
            errored_spec().with_dirty(true),
            rect(0.0, 0.0, 1440.0, 900.0),
            Some("bg.tree"),
        );
        let mut outcomes = Vec::new();

        let scrim = scenario.scrim_point();
        let centre = scenario.card_centre();

        outcomes.push(scenario.host.on_key(
            KeyClass::Tab,
            &mut scenario.focus,
            &mut scenario.registry,
        ));
        outcomes.push(scenario.host.on_key(
            KeyClass::ShiftTab,
            &mut scenario.focus,
            &mut scenario.registry,
        ));
        outcomes.push(scenario.host.on_key(
            KeyClass::Escape,
            &mut scenario.focus,
            &mut scenario.registry,
        ));
        outcomes.push(scenario.host.on_pointer(
            scrim,
            CardTarget::Padding,
            &mut scenario.focus,
            &mut scenario.registry,
        ));
        outcomes.push(scenario.host.on_pointer(
            centre,
            CardTarget::Control(fid("card.primary")),
            &mut scenario.focus,
            &mut scenario.registry,
        ));
        scenario.host.set_dirty(false);
        outcomes.push(scenario.host.on_key(
            KeyClass::Escape,
            &mut scenario.focus,
            &mut scenario.registry,
        ));

        let error = scenario.host.error().cloned();
        (outcomes, scenario.focus, scenario.registry, error)
    }

    let first = run();
    let second = run();

    assert_eq!(
        first.0, second.0,
        "the same script produced different outcomes"
    );
    assert_eq!(
        first.1, second.1,
        "the same script left focus in different places"
    );
    assert_eq!(
        first.2, second.2,
        "the same script left a different registry"
    );
    assert_eq!(first.3, second.3);
}

#[test]
fn test_reopening_with_the_same_spec_produces_the_same_geometry() {
    let viewport = rect(0.0, 0.0, 1366.0, 768.0);
    let first = Scenario::at(standard_spec(), viewport, None);
    let second = Scenario::at(standard_spec(), viewport, None);

    assert_eq!(first.host.card(), second.host.card());
    assert_eq!(first.focus, second.focus);
    assert_eq!(first.registry, second.registry);
}

#[test]
fn test_the_spec_carries_what_it_was_built_with() {
    let spec = dirty_spec();
    assert_eq!(spec.id(), &sid("editor"));
    assert_eq!(spec.title(), "Inline Document Editor");
    assert_eq!(spec.sizing(), ModalSizing::editor());
    assert!(spec.dirty());
    assert_eq!(spec.error(), None);
    assert_eq!(spec.primary(), Some(&fid("card.field")));
    assert_eq!(spec.controls().len(), CARD.len());
    assert!(!spec.controls()[3].enabled);
}

// ===========================================================================
// 10. User resize (owner ruling D-3)
// ===========================================================================
//
// A card the user has dragged is no longer the §4.6.2 fill: the wanted size is
// the user's, and only the two framework bounds remain — the card's own floor
// and the gutter. These are pure arithmetic, so they are checked directly.

/// The roomy viewport every sizing test here starts from.
const ROOMY: Size = Size::new(1200.0, 800.0);

/// The floors a Tools-class card is allowed down to.
const FLOORS: ResizeFloors = ResizeFloors::new(EDITOR_WIDTH_MIN, TOOLS_HEIGHT_MIN);

#[test]
fn test_a_user_size_that_fits_is_taken_verbatim() {
    let size = FLOORS.resolve(Size::new(600.0, 500.0), ROOMY);

    assert_eq!(size, Size::new(600.0, 500.0));
}

#[test]
fn test_a_user_size_below_the_width_floor_is_raised_to_it() {
    let size = FLOORS.resolve(Size::new(100.0, 500.0), ROOMY);

    assert_eq!(size.width, EDITOR_WIDTH_MIN);
    assert_eq!(size.height, 500.0);
}

#[test]
fn test_a_user_size_below_the_height_floor_is_raised_to_it() {
    let size = FLOORS.resolve(Size::new(600.0, 40.0), ROOMY);

    assert_eq!(size.width, 600.0);
    assert_eq!(size.height, TOOLS_HEIGHT_MIN);
}

#[test]
fn test_a_user_size_beyond_the_gutter_is_pulled_back_to_it() {
    for viewport in [ROOMY, Size::new(1366.0, 768.0)] {
        let size = FLOORS.resolve(Size::new(9_000.0, 9_000.0), viewport);

        assert_eq!(
            size,
            Size::new(
                viewport.width - VIEWPORT_GUTTER,
                viewport.height - VIEWPORT_GUTTER
            ),
            "a drag past the gutter must stop at the gutter on {viewport:?}"
        );
    }
}

#[test]
fn test_the_initial_fill_is_not_a_ceiling_on_a_user_size() {
    // The 80% fill is the card's INITIAL size, not its maximum: a dragged card
    // is allowed past it, and only the gutter stops it. `ROOMY`'s fill is
    // 960x640, so both numbers here are above it.
    let size = FLOORS.resolve(Size::new(1_100.0, 780.0), ROOMY);

    assert_eq!(size.width, 1_100.0, "the width may exceed the fill");
    assert_eq!(
        size.height,
        ROOMY.height - VIEWPORT_GUTTER,
        "the gutter still bounds it"
    );
}

#[test]
fn test_the_gutter_beats_the_floor_when_the_viewport_cannot_hold_it() {
    // 540x300 leaves 508x268 after the gutter — below both floors. The card
    // shrinks rather than overflow (§4.6.2 gutter rule, last).
    let viewport = Size::new(540.0, 300.0);

    let size = FLOORS.resolve(Size::new(1_000.0, 1_000.0), viewport);

    assert_eq!(size, Size::new(508.0, 268.0));
    assert!(size.width < EDITOR_WIDTH_MIN);
    assert!(size.height < TOOLS_HEIGHT_MIN);
}

#[test]
fn test_a_non_finite_user_size_collapses_to_the_floor() {
    let size = FLOORS.resolve(Size::new(f32::NAN, f32::INFINITY), ROOMY);

    assert_eq!(size, Size::new(EDITOR_WIDTH_MIN, TOOLS_HEIGHT_MIN));
    assert!(size.width.is_finite() && size.height.is_finite());
}

#[test]
fn test_a_zero_viewport_cannot_produce_a_negative_size() {
    let size = FLOORS.resolve(Size::new(600.0, 500.0), Size::ZERO);

    assert_eq!(size, Size::ZERO);
}
