//! Tests for [`input_dialog`] — the §4.5 box model, the four `UIF-INP-*`
//! invariants and the destructive-parity rule.
//!
//! Ten groups, in order:
//!
//! 1. **Token traces** — every constant in the box model resolves to the
//!    token §4.5.1 names, and the card height is the arithmetic the
//!    diagram prints.
//! 2. **Placement** — `UIF-INP-001`, driven as pure model arithmetic
//!    across real viewports down to the `640 × 480` floor, asserting the
//!    card clamps and never flips.
//! 3. **Zero-shift** — `UIF-INP-003`'s headline: the footer does not
//!    move, at whole-footer granularity, across every error state and
//!    every control kind.
//! 4. **Auto-focus and stem selection** — `UIF-INP-004`'s first half.
//! 5. **Validation rules** — each rule broken and satisfied on its own,
//!    so a regression names itself.
//! 6. **Enter** — reveal, stay open, keep focus in the text box.
//! 7. **Card-level absorption** — `UIF-INP-002`, including the grid
//!    sweeps that show no interior point can dismiss.
//! 8. **Escape and the menu above** — the **G-13** ordering, through a
//!    real cascade.
//! 9. **Destructive parity** — `UIF-INP-004`'s second half.
//! 10. **Teardown, focus restoration and determinism.**
//!
//! # Why the zero-shift tests compare whole rectangles
//!
//! The invariant is stated at "whole-footer granularity", so the tests
//! compare the footer rectangle and the card rectangle as units rather
//! than checking a height number. A one-pixel nudge anywhere in the box
//! model fails the card comparison even if it somehow passed a height
//! check, and the footer comparison localises the failure to the row the
//! user would actually see jump.

use super::*;

use crate::surface::menu::cascade::MenuCascade;
use crate::surface::menu::cascade::MenuTreeKind;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::MenuId;
use crate::surface::menu::model::Row;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The background focus group a real window would already have registered.
const BACKGROUND: [&str; 3] = ["bg.navbar", "bg.tree", "bg.tabs"];

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

fn desktop() -> Rectangle {
    rect(0.0, 0.0, 1920.0, 1080.0)
}

/// The `UIF-WIN-001` minimum viewport.
fn minimum_viewport() -> Rectangle {
    rect(0.0, 0.0, 640.0, 480.0)
}

/// Every viewport a placement sweep runs over, largest to smallest.
fn viewport_sweep() -> Vec<Rectangle> {
    vec![
        rect(0.0, 0.0, 2560.0, 1440.0),
        rect(0.0, 0.0, 1920.0, 1080.0),
        rect(0.0, 0.0, 1440.0, 900.0),
        rect(0.0, 0.0, 1280.0, 800.0),
        rect(0.0, 0.0, 1024.0, 768.0),
        rect(0.0, 0.0, 800.0, 600.0),
        minimum_viewport(),
    ]
}

fn rename_spec() -> InputDialogSpec {
    InputDialogSpec::rename("dlg.rename", "C:/notes/projects", "report.md")
}

fn create_spec() -> InputDialogSpec {
    InputDialogSpec::create("dlg.create", "Create Directory", "C:/notes")
}

fn move_spec(count: usize) -> InputDialogSpec {
    let folders = (0..count)
        .map(|index| format!("/destination/{index}"))
        .collect();
    InputDialogSpec::move_item("dlg.move", "report.md", folders)
}

fn delete_single() -> InputDialogSpec {
    InputDialogSpec::delete_confirmation("dlg.del", DeleteTargets::Single("report.md".into()))
}

fn delete_many(count: usize) -> InputDialogSpec {
    InputDialogSpec::delete_confirmation("dlg.del", DeleteTargets::Many(count))
}

/// A host opened over a window that already has background focus groups.
struct Scenario {
    host: InputDialogHost,
    focus: FocusState,
    registry: SurfaceRegistry,
}

impl Scenario {
    fn open(spec: InputDialogSpec) -> Self {
        Self::at(spec, pt(500.0, 400.0), false, desktop(), Some("bg.tree"))
    }

    fn at(
        spec: InputDialogSpec,
        point: Point,
        keyboard_invoked: bool,
        viewport: Rectangle,
        prefocus: Option<&str>,
    ) -> Self {
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
                "the pre-dialog control must be focusable"
            );
        }

        let host = InputDialogHost::open(
            spec,
            point,
            keyboard_invoked,
            viewport,
            &mut focus,
            &mut registry,
        );
        Self {
            host,
            focus,
            registry,
        }
    }

    /// A point inside the card — its centre.
    fn card_centre(&self) -> Point {
        let card = self.host.card();
        pt(card.x + card.width / 2.0, card.y + card.height / 2.0)
    }

    /// A point on the scrim — the viewport's top-left corner, which no
    /// card placed at these coordinates covers.
    fn scrim_point(&self) -> Point {
        pt(1.0, 1.0)
    }
}

/// Assert a rectangle to a tolerance, so the tests read as the numbers
/// the spec prints rather than as bit patterns.
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

/// Assert two rectangles are identical.
fn assert_same_rect(before: Rectangle, after: Rectangle, what: &str) {
    assert_eq!(
        before.to_bits(),
        after.to_bits(),
        "{what} must not move: {before:?} -> {after:?}"
    );
}

/// A trait-free way to compare rectangles bit-for-bit.
trait ToBits {
    fn to_bits(&self) -> (u32, u32, u32, u32);
}

impl ToBits for Rectangle {
    fn to_bits(&self) -> (u32, u32, u32, u32) {
        (
            self.x.to_bits(),
            self.y.to_bits(),
            self.width.to_bits(),
            self.height.to_bits(),
        )
    }
}

/// Assert the whole box model is identical between two layouts.
fn assert_layout_identical(before: &InputLayout, after: &InputLayout, what: &str) {
    assert_same_rect(before.card, after.card, &format!("{what}: card"));
    assert_same_rect(before.title, after.title, &format!("{what}: title"));
    assert_same_rect(before.context, after.context, &format!("{what}: context"));
    assert_same_rect(before.control, after.control, &format!("{what}: control"));
    assert_same_rect(
        before.validation,
        after.validation,
        &format!("{what}: validation slot"),
    );
    assert_same_rect(before.footer, after.footer, &format!("{what}: footer"));
    assert_same_rect(before.cancel, after.cancel, &format!("{what}: cancel"));
    assert_same_rect(before.ok, after.ok, &format!("{what}: ok"));
}

/// Assert every row of a layout sits exactly where the §4.5.1 diagram
/// puts it, given a card origin and a control height.
fn assert_box_model(layout: &InputLayout, card_x: f32, card_y: f32, control_height: f32) {
    let inner = CARD_WIDTH - 2.0 * CARD_PADDING;
    assert_rect(layout.title, card_x + 16.0, card_y + 16.0, inner, 24.0);
    assert_rect(
        layout.context,
        card_x + 16.0,
        card_y + 48.0,
        inner,
        CONTEXT_ROW_HEIGHT,
    );
    assert_rect(
        layout.control,
        card_x + 16.0,
        card_y + 76.0,
        inner,
        control_height,
    );
    assert_rect(
        layout.validation,
        card_x + 16.0,
        card_y + 116.0,
        inner,
        VALIDATION_SLOT_HEIGHT,
    );
    assert_rect(
        layout.footer,
        card_x + 16.0,
        card_y + 144.0,
        inner,
        FOOTER_HEIGHT,
    );
}

/// Assert two adjacent rows are separated by exactly one [`ROW_GAP`].
fn assert_gap(upper: Rectangle, lower: Rectangle) {
    assert!(
        (lower.y - (upper.y + upper.height) - ROW_GAP).abs() < 1e-4,
        "expected a {ROW_GAP}px gap, got {:?}",
        lower.y - (upper.y + upper.height)
    );
}

// ---------------------------------------------------------------------------
// 1. Token traces (§4.5.1)
// ---------------------------------------------------------------------------

#[test]
fn test_card_width_is_the_three_hundred_twenty_pixel_spec_figure() {
    assert_eq!(CARD_WIDTH, 320.0);
}

#[test]
fn test_card_padding_is_the_dialog_token() {
    assert_eq!(CARD_PADDING, space::LG);
    assert_eq!(CARD_PADDING, 16.0);
}

#[test]
fn test_row_gap_is_the_sibling_token() {
    assert_eq!(ROW_GAP, space::SM);
    assert_eq!(ROW_GAP, 8.0);
}

#[test]
fn test_title_row_is_the_title_sm_line_height() {
    assert_eq!(TITLE_ROW_HEIGHT, typography::TITLE_SM.line_height);
    assert_eq!(TITLE_ROW_HEIGHT, 24.0);
}

#[test]
fn test_context_row_is_the_body_line_height() {
    assert_eq!(CONTEXT_ROW_HEIGHT, typography::BODY.line_height);
    assert_eq!(CONTEXT_ROW_HEIGHT, 20.0);
}

#[test]
fn test_text_entry_is_the_standard_control_rung() {
    assert_eq!(TEXT_ENTRY_HEIGHT, size::CONTROL_MD);
    assert_eq!(TEXT_ENTRY_HEIGHT, 32.0);
}

#[test]
fn test_folder_row_is_the_compact_control_rung() {
    assert_eq!(FOLDER_ROW_HEIGHT, size::CONTROL_SM);
    assert_eq!(FOLDER_ROW_HEIGHT, 28.0);
}

#[test]
fn test_validation_slot_is_twenty_pixels_and_never_shrinks() {
    assert_eq!(VALIDATION_SLOT_HEIGHT, typography::BODY.line_height);
    assert_eq!(VALIDATION_SLOT_HEIGHT, 20.0);
}

#[test]
fn test_footer_is_the_standard_control_rung() {
    assert_eq!(FOOTER_HEIGHT, size::CONTROL_MD);
}

#[test]
fn test_card_border_is_the_hairline_token() {
    assert_eq!(card_border(), shape::BORDER_HAIRLINE);
    assert_eq!(card_border(), 1.0);
}

#[test]
fn test_folder_list_caps_at_two_hundred_pixels() {
    assert_eq!(FOLDER_LIST_MAX_HEIGHT, 200.0);
    assert_eq!(folder_list_height(100), FOLDER_LIST_MAX_HEIGHT);
}

#[test]
fn test_folder_list_height_grows_one_row_at_a_time_until_the_cap() {
    assert_eq!(folder_list_height(1), 28.0);
    assert_eq!(folder_list_height(2), 56.0);
    assert_eq!(folder_list_height(7), 196.0);
    assert_eq!(folder_list_height(8), 200.0);
}

#[test]
fn test_an_empty_folder_list_still_reserves_one_row() {
    assert_eq!(folder_list_height(0), FOLDER_ROW_HEIGHT);
}

#[test]
fn test_text_entry_card_is_exactly_one_hundred_ninety_two_tall() {
    let control = InputControl::TextEntry {
        initial: "report.md".into(),
        selection: Selection::EMPTY,
    };
    assert_eq!(InputLayout::card_height(&control), 192.0);
    assert_eq!(InputLayout::card_size(&control).height, 192.0);
}

#[test]
fn test_confirmation_card_collapses_the_control_row() {
    let control = InputControl::Confirmation {
        summary: "report.md".into(),
    };
    assert_eq!(CONFIRMATION_CONTROL_HEIGHT, 0.0);
    assert_eq!(InputLayout::card_height(&control), 160.0);
}

#[test]
fn test_card_width_is_invariant_across_control_kinds() {
    let text = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };
    let list = InputControl::FolderList {
        folders: vec!["a".into(), "b".into()],
        selected: 0,
    };
    let confirm = InputControl::Confirmation {
        summary: "x".into(),
    };
    assert_eq!(InputLayout::card_size(&text).width, CARD_WIDTH);
    assert_eq!(InputLayout::card_size(&list).width, CARD_WIDTH);
    assert_eq!(InputLayout::card_size(&confirm).width, CARD_WIDTH);
}

#[test]
fn test_box_model_rows_stack_at_their_specified_offsets() {
    let control = InputControl::TextEntry {
        initial: "report.md".into(),
        selection: Selection::EMPTY,
    };
    let layout = InputLayout::solve(
        rect(100.0, 50.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );
    assert_box_model(&layout, 100.0, 50.0, TEXT_ENTRY_HEIGHT);
}

#[test]
fn test_every_adjacent_row_is_separated_by_one_gap() {
    let control = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };
    let layout = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );
    assert_gap(layout.title, layout.context);
    assert_gap(layout.context, layout.control);
    assert_gap(layout.control, layout.validation);
    assert_gap(layout.validation, layout.footer);
}

#[test]
fn test_rows_do_not_overlap() {
    let control = InputControl::FolderList {
        folders: vec!["a".into(), "b".into(), "c".into()],
        selected: 0,
    };
    let height = InputLayout::card_height(&control);
    let layout = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, height),
        &control,
        FooterButtons::default(),
    );
    let rows = [
        layout.title,
        layout.context,
        layout.control,
        layout.validation,
        layout.footer,
    ];
    for pair in rows.windows(2) {
        assert!(
            pair[1].y >= pair[0].y + pair[0].height,
            "rows overlap: {:?} then {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn test_inner_width_is_the_card_less_two_paddings() {
    let control = InputControl::Confirmation {
        summary: "x".into(),
    };
    let layout = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, 160.0),
        &control,
        FooterButtons::default(),
    );
    assert_eq!(layout.title.width, CARD_WIDTH - 2.0 * CARD_PADDING);
    assert_eq!(layout.title.width, 288.0);
}

#[test]
fn test_footer_buttons_are_right_aligned_with_an_eight_pixel_gap() {
    let control = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };
    let layout = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );

    assert!(
        (layout.ok.x + layout.ok.width - (layout.footer.x + layout.footer.width)).abs() < 1e-4,
        "Ok must sit flush with the footer's right edge"
    );
    assert!(
        (layout.footer.x + layout.footer.width - layout.ok.x - layout.ok.width).abs() < 1e-4,
        "Ok must be flush right"
    );
    assert!(
        (layout.ok.x - (layout.cancel.x + layout.cancel.width) - ROW_GAP).abs() < 1e-4,
        "the button gap must be {ROW_GAP}px"
    );
    assert_eq!(layout.ok.height, FOOTER_HEIGHT);
    assert_eq!(layout.cancel.height, FOOTER_HEIGHT);
}

#[test]
fn test_footer_button_positions_at_default_widths() {
    let control = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };
    let layout = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );
    assert_rect(layout.ok, 240.0, 144.0, BUTTON_MIN_WIDTH, 32.0);
    assert_rect(layout.cancel, 168.0, 144.0, BUTTON_MIN_WIDTH, 32.0);
}

#[test]
fn test_wider_labels_push_the_pair_left_but_keep_the_flush_right_edge() {
    let control = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };
    let buttons = FooterButtons {
        cancel: 80.0,
        ok: 120.0,
    };
    let layout = InputLayout::solve(rect(0.0, 0.0, CARD_WIDTH, 192.0), &control, buttons);
    assert_rect(layout.ok, 184.0, 144.0, 120.0, 32.0);
    assert_rect(layout.cancel, 96.0, 144.0, 80.0, 32.0);
    assert!(
        (layout.ok.x + layout.ok.width - (CARD_WIDTH - CARD_PADDING)).abs() < 1e-4,
        "the pair must still end at the inner right edge"
    );
}

#[test]
fn test_button_min_width_is_the_default_for_both_buttons() {
    assert_eq!(FooterButtons::default().cancel, BUTTON_MIN_WIDTH);
    assert_eq!(FooterButtons::default().ok, BUTTON_MIN_WIDTH);
}

#[test]
fn test_layout_is_relative_to_the_card_not_the_origin() {
    let control = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };
    let at_origin = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );
    let shifted = InputLayout::solve(
        rect(500.0, 250.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );
    assert_eq!(shifted.footer.x - at_origin.footer.x, 500.0);
    assert_eq!(shifted.footer.y - at_origin.footer.y, 250.0);
}

// ---------------------------------------------------------------------------
// 2. Placement — UIF-INP-001
// ---------------------------------------------------------------------------

#[test]
fn test_pointer_invoked_dialog_opens_two_pixels_off_the_trigger() {
    let scenario = Scenario::at(
        create_spec(),
        pt(500.0, 400.0),
        false,
        desktop(),
        Some("bg.tree"),
    );
    assert_rect(scenario.host.card(), 502.0, 402.0, CARD_WIDTH, 192.0);
}

#[test]
fn test_keyboard_invoked_dialog_centres_in_the_viewport() {
    let scenario = Scenario::at(
        create_spec(),
        pt(500.0, 400.0),
        true,
        desktop(),
        Some("bg.tree"),
    );
    let expected_x = (1920.0 - CARD_WIDTH) / 2.0;
    let expected_y = (1080.0 - 192.0) / 2.0;
    assert_rect(
        scenario.host.card(),
        expected_x,
        expected_y,
        CARD_WIDTH,
        192.0,
    );
}

#[test]
fn test_keyboard_placement_ignores_the_trigger_point() {
    let a = Scenario::at(create_spec(), pt(10.0, 10.0), true, desktop(), None);
    let b = Scenario::at(create_spec(), pt(1900.0, 1070.0), true, desktop(), None);
    assert_same_rect(a.host.card(), b.host.card(), "centred card");
}

#[test]
fn test_card_clamps_at_the_right_edge_without_flipping() {
    let scenario = Scenario::at(create_spec(), pt(1900.0, 400.0), false, desktop(), None);
    let placement = scenario.host.placement();
    assert_eq!(placement.origin.x, 1920.0 - 8.0 - CARD_WIDTH);
    assert!(
        !placement.flipped_horizontal,
        "UIF-INP-001 clamps, never flips"
    );
    assert!(placement.clamped_horizontal);
}

#[test]
fn test_card_clamps_at_the_bottom_edge_without_flipping() {
    let scenario = Scenario::at(create_spec(), pt(500.0, 1075.0), false, desktop(), None);
    let placement = scenario.host.placement();
    assert_eq!(placement.origin.y, 1080.0 - 8.0 - 192.0);
    assert!(
        !placement.flipped_vertical,
        "UIF-INP-001 clamps, never flips"
    );
    assert!(placement.clamped_vertical);
}

#[test]
fn test_card_clamps_at_the_left_and_top_margins() {
    let scenario = Scenario::at(create_spec(), pt(-500.0, -500.0), false, desktop(), None);
    assert_eq!(scenario.host.card().x, 8.0);
    assert_eq!(scenario.host.card().y, 8.0);
}

#[test]
fn test_a_negative_trigger_point_lands_on_the_near_margin() {
    let scenario = Scenario::at(create_spec(), pt(-100.0, -100.0), false, desktop(), None);
    assert!(scenario.host.card().x >= 8.0);
    assert!(scenario.host.card().y >= 8.0);
}

#[test]
fn test_placement_never_flips_on_any_axis_at_any_trigger_point() {
    for viewport in viewport_sweep() {
        for x in [
            -200.0,
            0.0,
            100.0,
            viewport.width / 2.0,
            viewport.width - 1.0,
            viewport.width + 500.0,
        ] {
            for y in [
                -200.0,
                0.0,
                100.0,
                viewport.height / 2.0,
                viewport.height - 1.0,
                viewport.height + 500.0,
            ] {
                let scenario = Scenario::at(create_spec(), pt(x, y), false, viewport, None);
                let placement = scenario.host.placement();
                assert!(
                    !placement.was_flipped(),
                    "a cursor-proximal dialog must never flip at ({x}, {y}) in {viewport:?}"
                );
            }
        }
    }
}

#[test]
fn test_card_stays_inside_the_eight_pixel_margin_wherever_the_trigger_is() {
    for viewport in viewport_sweep() {
        for x in [0.0, viewport.width / 3.0, viewport.width - 1.0] {
            for y in [0.0, viewport.height / 3.0, viewport.height - 1.0] {
                let scenario = Scenario::at(create_spec(), pt(x, y), false, viewport, None);
                let card = scenario.host.card();
                assert!(
                    card.x >= viewport.x + 8.0 - 1e-4,
                    "left margin violated at ({x}, {y}) in {viewport:?}: {card:?}"
                );
                assert!(
                    card.y >= viewport.y + 8.0 - 1e-4,
                    "top margin violated at ({x}, {y}) in {viewport:?}: {card:?}"
                );
                assert!(
                    card.x + card.width <= viewport.x + viewport.width - 8.0 + 1e-4,
                    "right margin violated at ({x}, {y}) in {viewport:?}: {card:?}"
                );
                assert!(
                    card.y + card.height <= viewport.y + viewport.height - 8.0 + 1e-4,
                    "bottom margin violated at ({x}, {y}) in {viewport:?}: {card:?}"
                );
            }
        }
    }
}

#[test]
fn test_placement_holds_at_the_minimum_viewport() {
    let scenario = Scenario::at(
        create_spec(),
        pt(600.0, 500.0),
        false,
        minimum_viewport(),
        None,
    );
    let card = scenario.host.card();
    assert_eq!(card.width, CARD_WIDTH);
    assert!(card.x + card.width <= 640.0 - 8.0 + 1e-4);
    assert!(card.y + card.height <= 480.0 - 8.0 + 1e-4);
}

#[test]
fn test_centred_placement_holds_at_the_minimum_viewport() {
    let scenario = Scenario::at(create_spec(), pt(0.0, 0.0), true, minimum_viewport(), None);
    assert_rect(scenario.host.card(), 160.0, 144.0, CARD_WIDTH, 192.0);
}

#[test]
fn test_card_width_is_independent_of_viewport_width() {
    for viewport in viewport_sweep() {
        let scenario = Scenario::at(create_spec(), pt(0.0, 0.0), false, viewport, None);
        assert_eq!(scenario.host.card().width, CARD_WIDTH);
    }
}

#[test]
fn test_a_viewport_narrower_than_the_card_pins_it_to_the_near_margin() {
    let scenario = Scenario::at(
        create_spec(),
        pt(150.0, 10.0),
        false,
        rect(0.0, 0.0, 200.0, 150.0),
        None,
    );
    let placement = scenario.host.placement();
    assert_eq!(placement.origin.x, 8.0);
    assert!(placement.clamped_horizontal);
    assert_eq!(scenario.host.card().width, CARD_WIDTH);
}

#[test]
fn test_a_viewport_shorter_than_the_card_reports_the_truncation() {
    let scenario = Scenario::at(
        create_spec(),
        pt(10.0, 200.0),
        false,
        rect(0.0, 0.0, 800.0, 150.0),
        None,
    );
    let placement = scenario.host.placement();
    assert!(placement.clamped_vertical);
    assert!(placement.scrollable);
}

#[test]
fn test_non_finite_trigger_point_is_sanitised() {
    let scenario = Scenario::at(
        create_spec(),
        pt(f32::NAN, f32::INFINITY),
        false,
        desktop(),
        None,
    );
    assert_finite(scenario.host.card().x, "card x");
    assert_finite(scenario.host.card().y, "card y");
}

#[test]
fn test_a_degenerate_viewport_still_produces_finite_geometry() {
    let scenario = Scenario::at(
        create_spec(),
        pt(10.0, 10.0),
        false,
        rect(0.0, 0.0, 0.0, 0.0),
        None,
    );
    assert_finite(scenario.host.card().x, "card x");
    assert_finite(scenario.host.card().y, "card y");
    assert_finite(scenario.host.card().width, "card width");
    assert_eq!(scenario.host.card().width, CARD_WIDTH);
}

#[test]
fn test_the_dialog_owns_no_clamping_of_its_own() {
    // The placement is exactly what the shared solver says it is. If the
    // host grew a second clamping implementation this would diverge.
    let point = pt(1900.0, 400.0);
    let size = InputLayout::card_size(&InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    });
    let direct = anchor::solve(
        Anchor::CursorProximal {
            point,
            keyboard_invoked: false,
        },
        size,
        desktop(),
    );
    let scenario = Scenario::at(create_spec(), point, false, desktop(), None);
    assert_same_rect(direct.rect(), scenario.host.card(), "solver agreement");
}

// ---------------------------------------------------------------------------
// 3. Zero-shift — UIF-INP-003
// ---------------------------------------------------------------------------

#[test]
fn test_revealing_an_error_does_not_move_the_footer() {
    let mut scenario = Scenario::open(create_spec());
    let before = *scenario.host.layout();

    let verdict = scenario.host.set_value(String::new(), &mut scenario.focus);
    assert!(!verdict.is_accept());
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(scenario.host.revealed().is_some(), "Enter must reveal");

    assert_same_rect(before.footer, scenario.host.layout().footer, "footer");
}

#[test]
fn test_revealing_an_error_does_not_move_the_card() {
    let mut scenario = Scenario::open(create_spec());
    let before = scenario.host.card();

    scenario.host.set_value("".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);

    assert_same_rect(before, scenario.host.card(), "card");
}

#[test]
fn test_the_whole_box_model_is_identical_across_an_error_transition() {
    let mut scenario = Scenario::open(rename_spec());
    let before = *scenario.host.layout();

    scenario
        .host
        .set_value("bad/name".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    let after = *scenario.host.layout();

    assert_layout_identical(&before, &after, "rename error");
}

#[test]
fn test_clearing_an_error_does_not_move_anything_either() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    let errored = *scenario.host.layout();

    scenario
        .host
        .set_value("good-name".into(), &mut scenario.focus);
    assert!(
        scenario.host.revealed().is_none(),
        "a valid value clears the slot"
    );
    assert_layout_identical(&errored, scenario.host.layout(), "clearing");
}

#[test]
fn test_no_error_message_of_any_kind_moves_the_footer() {
    let messages = [
        "",
        " ",
        "a/b",
        "a\\b",
        "a:b",
        "a*b",
        "a?b",
        "a\"b",
        "a<b",
        "a>b",
        "a|b",
        "a\u{7}b",
        " leading",
        "trailing ",
        "a rather long validation message that would need wrapping",
    ];

    for message in messages {
        let mut scenario = Scenario::open(create_spec());
        let before = *scenario.host.layout();

        scenario.host.set_value(message.into(), &mut scenario.focus);
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);

        assert_same_rect(
            before.footer,
            scenario.host.layout().footer,
            &format!("footer after {message:?}"),
        );
        assert_same_rect(
            before.validation,
            scenario.host.layout().validation,
            &format!("slot after {message:?}"),
        );
        assert_same_rect(
            before.card,
            scenario.host.card(),
            &format!("card after {message:?}"),
        );
    }
}

#[test]
fn test_the_slot_is_reserved_for_every_control_kind() {
    let specs = [
        create_spec(),
        rename_spec(),
        move_spec(3),
        delete_single(),
        delete_many(7),
    ];
    for spec in specs {
        let scenario = Scenario::open(spec.clone());
        let slot = scenario.host.layout().validation;
        assert_eq!(
            slot.height, VALIDATION_SLOT_HEIGHT,
            "the slot must be reserved whatever the control"
        );
        assert!(slot.height > 0.0, "a reserved slot cannot be zero height");
    }
}

#[test]
fn test_the_slot_sits_between_the_control_and_the_footer_whatever_the_control() {
    for spec in [create_spec(), move_spec(12), delete_single()] {
        let scenario = Scenario::open(spec.clone());
        let layout = scenario.host.layout();
        assert_gap(layout.control, layout.validation);
        assert_gap(layout.validation, layout.footer);
    }
}

#[test]
fn test_a_long_folder_list_does_not_disturb_the_slot_or_footer_relative_position() {
    let scenario = Scenario::open(move_spec(50));
    let layout = scenario.host.layout();
    assert_eq!(layout.control.height, FOLDER_LIST_MAX_HEIGHT);
    assert!(
        (layout.footer.y - layout.validation.y - layout.validation.height - ROW_GAP).abs() < 1e-4,
        "the footer must sit one gap below the slot regardless of list length"
    );
}

#[test]
fn test_the_layout_function_cannot_be_told_an_error_is_showing() {
    // `InputLayout::solve` takes the card, the control and the button
    // widths — nothing else. Calling it twice with the same three inputs
    // is the same call, which is the guarantee restated as a test.
    let control = InputControl::TextEntry {
        initial: String::new(),
        selection: Selection::EMPTY,
    };
    let a = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );
    let b = InputLayout::solve(
        rect(0.0, 0.0, CARD_WIDTH, 192.0),
        &control,
        FooterButtons::default(),
    );
    assert_layout_identical(&a, &b, "pure solve");
}

#[test]
fn test_repeated_invalid_enters_never_drift_the_layout() {
    let mut scenario = Scenario::open(create_spec());
    let before = *scenario.host.layout();

    for _ in 0..25 {
        scenario.host.set_value("".into(), &mut scenario.focus);
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    }

    assert_layout_identical(&before, scenario.host.layout(), "25 rejected enters");
}

// ---------------------------------------------------------------------------
// 4. Auto-focus and stem selection — UIF-INP-004
// ---------------------------------------------------------------------------

#[test]
fn test_a_text_dialog_focuses_its_entry_on_open() {
    let scenario = Scenario::open(create_spec());
    assert_eq!(
        scenario.focus.current(),
        Some(&fid("dlg.create:entry")),
        "the caret must land in the text field"
    );
}

#[test]
fn test_a_rename_dialog_focuses_its_entry_on_open() {
    let scenario = Scenario::open(rename_spec());
    assert_eq!(scenario.focus.current(), Some(&fid("dlg.rename:entry")));
}

#[test]
fn test_a_rename_selects_the_stem_not_the_extension() {
    let spec = rename_spec();
    let InputControl::TextEntry { selection, .. } = &spec.control else {
        panic!("a rename spec carries a text entry");
    };
    assert_eq!(selection.slice("report.md"), "report");
    assert_eq!(selection.start, 0);
    assert_eq!(selection.end, 6);
}

#[test]
fn test_a_rename_selects_the_stem_through_the_host() {
    let scenario = Scenario::open(rename_spec());
    let InputControl::TextEntry { initial, selection } = scenario.host.control() else {
        panic!("a rename host carries a text entry");
    };
    assert_eq!(initial, "report.md");
    assert_eq!(selection.slice(initial), "report");
}

#[test]
fn test_a_name_with_no_extension_selects_everything() {
    let selection = Selection::rename_stem("notes");
    assert_eq!(selection, Selection::all("notes"));
    assert_eq!(selection.slice("notes"), "notes");
}

#[test]
fn test_a_dotfile_selects_its_whole_name() {
    let selection = Selection::rename_stem(".gitignore");
    assert_eq!(selection.slice(".gitignore"), ".gitignore");
}

#[test]
fn test_a_double_extension_selects_before_the_last_dot_only() {
    let selection = Selection::rename_stem("archive.tar.gz");
    assert_eq!(selection.slice("archive.tar.gz"), "archive.tar");
}

#[test]
fn test_a_trailing_dot_keeps_the_dot_out_of_the_stem() {
    let selection = Selection::rename_stem("archive.");
    assert_eq!(selection.slice("archive."), "archive");
}

#[test]
fn test_an_empty_initial_selects_nothing() {
    assert_eq!(Selection::rename_stem(""), Selection::EMPTY);
    assert!(Selection::rename_stem("").is_empty());
}

#[test]
fn test_stem_selection_counts_characters_not_bytes() {
    let value = "café.md";
    let selection = Selection::rename_stem(value);
    assert_eq!(selection.end, 4, "four characters before the dot");
    assert_eq!(selection.slice(value), "café");
}

#[test]
fn test_selection_slice_handles_inverted_and_out_of_range() {
    let inverted = Selection { start: 5, end: 2 };
    assert_eq!(inverted.slice("abcdef"), "");
    let past_end = Selection { start: 0, end: 99 };
    assert_eq!(past_end.slice("abc"), "abc");
    let empty = Selection::EMPTY;
    assert_eq!(empty.slice("abc"), "");
}

#[test]
fn test_a_folder_list_focuses_the_list_not_a_button() {
    let scenario = Scenario::open(move_spec(4));
    assert_eq!(scenario.focus.current(), Some(&fid("dlg.move:list")));
}

#[test]
fn test_a_confirmation_focuses_cancel_not_the_danger_button() {
    let scenario = Scenario::open(delete_single());
    assert_eq!(scenario.focus.current(), Some(&fid("dlg.del:cancel")));
    assert_ne!(scenario.focus.current(), Some(&fid("dlg.del:ok")));
}

#[test]
fn test_focus_never_lands_on_the_primary_for_a_text_dialog() {
    for spec in [create_spec(), rename_spec()] {
        let scenario = Scenario::open(spec.clone());
        assert_ne!(scenario.focus.current(), Some(scenario.host.ok_id()));
    }
}

#[test]
fn test_the_dialog_takes_focus_from_the_background() {
    let mut before = FocusState::new();
    before.register_group(
        gid("bg"),
        BACKGROUND
            .iter()
            .map(|label| FocusMember::new(*label))
            .collect(),
    );
    before.focus(&fid("bg.tree"));
    assert_eq!(before.current(), Some(&fid("bg.tree")));

    let scenario = Scenario::at(
        create_spec(),
        pt(100.0, 100.0),
        false,
        desktop(),
        Some("bg.tree"),
    );
    assert_ne!(scenario.focus.current(), Some(&fid("bg.tree")));
}

#[test]
fn test_the_trap_is_active_and_names_the_dialog_group() {
    let scenario = Scenario::open(create_spec());
    assert!(scenario.focus.is_trapped());
    assert_eq!(scenario.focus.active_trap(), Some(scenario.host.group_id()));
    assert_eq!(scenario.focus.trap_depth(), 1);
}

#[test]
fn test_the_group_id_is_derived_from_the_surface_id() {
    let spec = create_spec();
    assert_eq!(
        group_for(&spec.id),
        GroupId::new("input:dlg.create".to_string())
    );
}

// ---------------------------------------------------------------------------
// 5. Validation rules — UIF-INP-003
// ---------------------------------------------------------------------------

#[test]
fn test_an_empty_name_is_rejected() {
    let verdict = ValidationRule::Name.check("");
    assert_eq!(verdict.message().map(|m| m.code.as_str()), Some(CODE_EMPTY));
}

#[test]
fn test_whitespace_only_is_rejected_as_empty() {
    for value in [" ", "   ", "\t"] {
        let verdict = ValidationRule::Name.check(value);
        assert_eq!(
            verdict.message().map(|m| m.code.as_str()),
            Some(CODE_EMPTY),
            "{value:?} must read as empty"
        );
    }
}

#[test]
fn test_leading_and_trailing_whitespace_is_rejected_apart_from_empty() {
    for value in [" leading", "trailing ", " both "] {
        let verdict = ValidationRule::Name.check(value);
        assert_eq!(
            verdict.message().map(|m| m.code.as_str()),
            Some(CODE_EDGESPACING),
            "{value:?} must trip the edge-spacing rule, not the empty rule"
        );
    }
}

#[test]
fn test_control_characters_are_rejected() {
    for value in ["a\u{1}b", "a\u{7}b", "a\nb", "a\tb"] {
        let verdict = ValidationRule::Name.check(value);
        assert_eq!(
            verdict.message().map(|m| m.code.as_str()),
            Some(CODE_CONTROL),
            "{value:?} must trip the control-character rule"
        );
    }
}

#[test]
fn test_every_illegal_basename_character_is_rejected() {
    for c in ILLEGAL_BASENAME_CHARS.chars() {
        let value = format!("a{c}b");
        let verdict = ValidationRule::Name.check(&value);
        assert_eq!(
            verdict.message().map(|m| m.code.as_str()),
            Some(CODE_ILLEGAL_CHAR),
            "{c:?} must be illegal in a basename"
        );
    }
}

#[test]
fn test_a_plain_legal_name_is_accepted() {
    for value in ["report", "report.md", "notes-2026", "a_b.c", "Ünïcødé"] {
        assert!(
            ValidationRule::Name.check(value).is_accept(),
            "{value:?} must be accepted"
        );
    }
}

#[test]
fn test_rename_rejects_the_name_it_replaces() {
    let rule = ValidationRule::Rename {
        current: "report.md".into(),
    };
    assert_eq!(
        rule.check("report.md").message().map(|m| m.code.as_str()),
        Some(CODE_UNCHANGED)
    );
}

#[test]
fn test_rename_rejects_an_unchanged_name_despite_whitespace() {
    let rule = ValidationRule::Rename {
        current: " report.md ".into(),
    };
    assert_eq!(
        rule.check("report.md").message().map(|m| m.code.as_str()),
        Some(CODE_UNCHANGED)
    );
}

#[test]
fn test_rename_accepts_a_genuinely_different_name() {
    let rule = ValidationRule::Rename {
        current: "report.md".into(),
    };
    assert!(rule.check("summary.md").is_accept());
    assert!(rule.check("report.markdown").is_accept());
}

#[test]
fn test_rename_checks_the_basename_rules_first() {
    let rule = ValidationRule::Rename {
        current: "a/b".into(),
    };
    assert_eq!(
        rule.check("a/b").message().map(|m| m.code.as_str()),
        Some(CODE_ILLEGAL_CHAR),
        "an illegal character outranks the unchanged-name rule"
    );
}

#[test]
fn test_a_rename_to_an_empty_name_is_the_empty_rule_not_the_unchanged_rule() {
    let rule = ValidationRule::Rename {
        current: "x".into(),
    };
    assert_eq!(
        rule.check("").message().map(|m| m.code.as_str()),
        Some(CODE_EMPTY)
    );
}

#[test]
fn test_a_destination_must_actually_be_chosen() {
    assert_eq!(
        ValidationRule::Destination
            .check("")
            .message()
            .map(|m| m.code.as_str()),
        Some(CODE_NO_DESTINATION)
    );
    assert!(ValidationRule::Destination.check("/dest/0").is_accept());
}

#[test]
fn test_a_confirmation_over_nothing_is_rejected() {
    assert_eq!(
        ValidationRule::Confirm { count: 0 }
            .check("")
            .message()
            .map(|m| m.code.as_str()),
        Some(CODE_NOTHING_TO_CONFIRM)
    );
}

#[test]
fn test_a_confirmation_over_any_items_is_accepted() {
    for count in 1..=100 {
        assert!(
            ValidationRule::Confirm { count }.check("").is_accept(),
            "count {count} must be confirmable"
        );
    }
}

#[test]
fn test_the_always_rule_accepts_everything_including_garbage() {
    for value in ["", " ", "a/b", "\u{0}"] {
        assert!(
            ValidationRule::Always.check(value).is_accept(),
            "{value:?} must be accepted by Always"
        );
    }
}

#[test]
fn test_ok_is_disabled_at_open_for_an_invalid_initial_value() {
    let scenario = Scenario::open(create_spec());
    assert!(!scenario.host.ok_is_enabled());
    assert!(
        !scenario.focus.is_focusable(scenario.host.ok_id()),
        "the focus model must agree that Ok is dead"
    );
}

#[test]
fn test_a_rename_opens_with_ok_disabled_because_the_name_is_unchanged() {
    let scenario = Scenario::open(rename_spec());
    assert!(!scenario.host.ok_is_enabled());
}

#[test]
fn test_a_valid_initial_value_opens_with_ok_enabled() {
    let scenario = Scenario::open(move_spec(3));
    assert!(scenario.host.ok_is_enabled());
    assert!(scenario.focus.is_focusable(scenario.host.ok_id()));
}

#[test]
fn test_typing_a_valid_value_enables_ok() {
    let mut scenario = Scenario::open(create_spec());
    assert!(!scenario.host.ok_is_enabled());
    let verdict = scenario
        .host
        .set_value("new-folder".into(), &mut scenario.focus);
    assert!(verdict.is_accept());
    assert!(scenario.host.ok_is_enabled());
    assert!(scenario.focus.is_focusable(scenario.host.ok_id()));
}

#[test]
fn test_typing_an_invalid_value_disables_ok() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("fine".into(), &mut scenario.focus);
    assert!(scenario.host.ok_is_enabled());
    let verdict = scenario
        .host
        .set_value("bad/path".into(), &mut scenario.focus);
    assert!(!verdict.is_accept());
    assert!(!scenario.host.ok_is_enabled());
}

#[test]
fn test_becoming_valid_clears_a_revealed_message() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(scenario.host.revealed().is_some());

    scenario.host.set_value("good".into(), &mut scenario.focus);
    assert!(scenario.host.revealed().is_none());
}

#[test]
fn test_becoming_invalid_does_not_reveal_by_itself() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("good".into(), &mut scenario.focus);
    scenario
        .host
        .set_value("bad/path".into(), &mut scenario.focus);
    assert!(
        scenario.host.revealed().is_none(),
        "only Enter reveals; typing must not"
    );
}

#[test]
fn test_the_verdict_and_the_focus_model_never_disagree() {
    let mut scenario = Scenario::open(create_spec());
    for value in ["", "a", "a/b", "ok", " ", "x|y"] {
        let verdict = scenario.host.set_value(value.into(), &mut scenario.focus);
        assert_eq!(
            verdict.is_accept(),
            scenario.focus.is_focusable(scenario.host.ok_id()),
            "verdict and focus model disagree on {value:?}"
        );
    }
}

#[test]
fn test_validation_is_pure() {
    let rule = ValidationRule::Rename {
        current: "a.md".into(),
    };
    for _ in 0..10 {
        assert_eq!(rule.check("a.md"), rule.check("a.md"));
        assert_eq!(rule.check("b.md"), rule.check("b.md"));
    }
}

// ---------------------------------------------------------------------------
// 6. Enter — UIF-INP-003
// ---------------------------------------------------------------------------

#[test]
fn test_enter_on_an_invalid_value_reveals_the_error() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(matches!(outcome, InputOutcome::ErrorRevealed { .. }));
}

#[test]
fn test_enter_on_an_invalid_value_keeps_the_dialog_open() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(scenario.host.is_open());
    assert!(scenario.registry.contains(scenario.host.id()));
}

#[test]
fn test_enter_on_an_invalid_value_keeps_focus_in_the_text_box() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(scenario.focus.current(), Some(&fid("dlg.create:entry")));
}

#[test]
fn test_enter_pulls_focus_back_to_the_entry_from_anywhere_in_the_card() {
    let mut scenario = Scenario::open(create_spec());
    scenario.focus.focus(&fid("dlg.create:cancel"));
    scenario
        .host
        .set_value("bad/path".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        scenario.focus.current(),
        Some(&fid("dlg.create:entry")),
        "Enter must land the caret back in the text box"
    );
}

#[test]
fn test_the_trap_survives_a_rejected_enter() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(scenario.focus.is_trapped());
    assert_eq!(scenario.focus.trap_depth(), 1);
    assert_eq!(scenario.focus.active_trap(), Some(scenario.host.group_id()));
}

#[test]
fn test_repeated_rejected_enters_are_stable() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    for _ in 0..10 {
        let outcome =
            scenario
                .host
                .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
        assert!(matches!(outcome, InputOutcome::ErrorRevealed { .. }));
        assert!(scenario.host.is_open());
    }
    assert_eq!(scenario.registry.count_in(ZBand::Modal), 1);
}

#[test]
fn test_enter_on_a_valid_value_accepts_and_closes() {
    let mut scenario = Scenario::open(create_spec());
    scenario
        .host
        .set_value("new-folder".into(), &mut scenario.focus);
    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        InputOutcome::Accepted {
            value: "new-folder".into()
        }
    );
    assert!(!scenario.host.is_open());
    assert!(!scenario.registry.contains(scenario.host.id()));
}

#[test]
fn test_recover_after_a_rejected_enter_and_accept() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(scenario.host.revealed().is_some());

    scenario.host.set_value("fixed".into(), &mut scenario.focus);
    assert!(scenario.host.revealed().is_none());

    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        InputOutcome::Accepted {
            value: "fixed".into()
        }
    );
}

#[test]
fn test_enter_on_a_rename_rejects_the_unchanged_name() {
    let mut scenario = Scenario::open(rename_spec());
    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(matches!(outcome, InputOutcome::ErrorRevealed { .. }));
    assert_eq!(
        scenario.host.revealed().map(|m| m.code.as_str()),
        Some(CODE_UNCHANGED)
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_enter_on_a_confirmation_accepts_it() {
    let mut scenario = Scenario::open(delete_single());
    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        InputOutcome::Accepted {
            value: "report.md".into()
        }
    );
}

#[test]
fn test_enter_on_an_empty_confirmation_stays_open() {
    let mut scenario = Scenario::open(delete_many(0));
    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(matches!(outcome, InputOutcome::ErrorRevealed { .. }));
    assert!(scenario.host.is_open());
}

#[test]
fn test_enter_after_close_is_a_no_op() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("x".into(), &mut scenario.focus);
    let _ = scenario
        .host
        .cancel(&mut scenario.focus, &mut scenario.registry);
    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        InputOutcome::NoOp {
            reason: InputNoOp::NotOpen
        }
    );
}

// ---------------------------------------------------------------------------
// 6b. The answer tiers (§3.0.4.2, `INC-36`)
// ---------------------------------------------------------------------------
//
// The dialog's answer used to be a bare enum. It is now carried on a tier,
// which says what the caller is supposed to *do* with it: a `Commit` is
// persist-this, a `Cancel` is throw-it-away. `submit` and `cancel` remain
// as the unwrapped shapes so no existing caller had to change.

#[test]
fn test_a_valid_answer_arrives_on_the_commit_tier() {
    let mut scenario = Scenario::open(rename_spec());
    scenario
        .host
        .set_value("renamed.md".into(), &mut scenario.focus);

    let answer = scenario
        .host
        .commit_answer(&mut scenario.focus, &mut scenario.registry)
        .expect("a valid value commits");

    assert_eq!(
        answer.value(),
        &InputAnswer::Accepted {
            value: "renamed.md".into()
        }
    );
    assert!(
        !scenario.host.is_open(),
        "the `Commit` tier is the terminal answer: the card is gone"
    );
}

#[test]
fn test_an_invalid_answer_emits_no_tier_at_all() {
    let mut scenario = Scenario::open(rename_spec());
    scenario.host.set_value(String::new(), &mut scenario.focus);

    assert!(
        scenario
            .host
            .commit_answer(&mut scenario.focus, &mut scenario.registry)
            .is_none(),
        "a rejected value must not put anything on the wire, let alone a `Commit`"
    );
    assert!(
        scenario.host.is_open(),
        "the card stays open on the invalid branch"
    );
    assert!(
        scenario.host.revealed().is_some(),
        "and the reason is shown where the user just tried to confirm"
    );
}

#[test]
fn test_a_discard_arrives_on_the_cancel_tier() {
    let mut scenario = Scenario::open(rename_spec());
    scenario
        .host
        .set_value("half-typed.md".into(), &mut scenario.focus);

    let answer = scenario
        .host
        .cancel_answer(&mut scenario.focus, &mut scenario.registry)
        .expect("a discard always settles");

    assert_eq!(answer.restored(), &InputAnswer::Cancelled);
    assert!(!scenario.host.is_open());
}

#[test]
fn test_submit_and_cancel_are_the_unwrapped_shapes_of_the_tiers() {
    let mut committing = Scenario::open(rename_spec());
    committing
        .host
        .set_value("same.md".into(), &mut committing.focus);
    let tiered = committing
        .host
        .commit_answer(&mut committing.focus, &mut committing.registry)
        .map(Commit::into_value);

    let mut legacy = Scenario::open(rename_spec());
    legacy.host.set_value("same.md".into(), &mut legacy.focus);
    let plain = legacy.host.submit(&mut legacy.focus, &mut legacy.registry);

    assert_eq!(
        tiered, plain,
        "`submit` is `commit_answer` unwrapped, unchanged"
    );

    let mut cancelling = Scenario::open(rename_spec());
    let tiered_cancel = cancelling
        .host
        .cancel_answer(&mut cancelling.focus, &mut cancelling.registry)
        .map(Cancel::into_restored);

    let mut legacy_cancel = Scenario::open(rename_spec());
    let plain_cancel = legacy_cancel
        .host
        .cancel(&mut legacy_cancel.focus, &mut legacy_cancel.registry);

    assert_eq!(
        tiered_cancel, plain_cancel,
        "`cancel` is `cancel_answer` unwrapped, unchanged"
    );
}

#[test]
fn test_the_pending_projection_drives_the_primary_cta() {
    let mut scenario = Scenario::open(rename_spec());
    assert_eq!(scenario.host.pending(), ActionPending::idle());
    assert!(!scenario.host.is_busy());

    scenario
        .host
        .set_pending(ActionPending::indeterminate().with_cancellable(true));
    assert!(scenario.host.is_busy());
    assert!(scenario.host.request_cancel().is_some());
    assert!(
        scenario.host.request_cancel().is_none(),
        "one stop request per busy period"
    );

    scenario.host.set_pending(ActionPending::idle());
    assert!(!scenario.host.is_busy());
    assert!(
        scenario.host.request_cancel().is_none(),
        "nothing running, nothing to stop"
    );
}

#[test]
fn test_the_first_valid_accept_closes_the_card_so_it_cannot_double_fire() {
    // The `Ok` control carries the same activation gate every other action
    // control has, and that gate is what suppresses a duplicate press
    // (`dialog_button_tests`). Through *this* host the suppression branch
    // is unreachable by construction: the first valid accept tears the
    // card down, so there is no second press inside the window to
    // withhold. This test pins the reason rather than pretending otherwise.
    let mut scenario = Scenario::open(rename_spec());
    scenario
        .host
        .set_pending(ActionPending::idle().with_cancellable(true));
    scenario
        .host
        .set_value("once.md".into(), &mut scenario.focus);

    assert!(
        scenario
            .host
            .commit_answer(&mut scenario.focus, &mut scenario.registry)
            .is_some(),
        "the first accept dispatches"
    );
    assert!(!scenario.host.is_open(), "and closes the card with it");
    assert!(
        scenario
            .host
            .commit_answer(&mut scenario.focus, &mut scenario.registry)
            .is_none(),
        "a closed card has nothing left to dispatch"
    );
}

#[test]
fn test_teardown_releases_the_cta_gate() {
    let mut scenario = Scenario::open(rename_spec());
    scenario
        .host
        .set_pending(ActionPending::idle().with_cancellable(true));
    scenario
        .host
        .set_value("done.md".into(), &mut scenario.focus);
    scenario
        .host
        .commit_answer(&mut scenario.focus, &mut scenario.registry)
        .expect("the accept dispatches");

    let ephemeral = scenario.host.ephemeral();
    assert!(
        !ephemeral.get().is_pending(),
        "the gate is released on teardown, so nothing is left wedged"
    );
    assert!(!ephemeral.get().cancel_requested());
}

// ---------------------------------------------------------------------------
// 7. Card-level absorption — UIF-INP-002
// ---------------------------------------------------------------------------

#[test]
fn test_a_click_on_card_padding_is_absorbed() {
    let mut scenario = Scenario::open(create_spec());
    let outcome = scenario.host.on_pointer(
        scenario.card_centre(),
        InputTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::Absorbed {
            target: InputTarget::Disabled
        }
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_click_on_a_disabled_ok_is_absorbed_and_reveals_nothing() {
    let mut scenario = Scenario::open(create_spec());
    let ok = {
        let layout = scenario.host.layout();
        pt(layout.ok.x + 1.0, layout.ok.y + 1.0)
    };
    let outcome = scenario.host.on_pointer(
        ok,
        InputTarget::Ok,
        &mut scenario.focus,
        &mut scenario.registry,
    );

    assert_eq!(
        outcome,
        InputOutcome::Absorbed {
            target: InputTarget::Disabled
        }
    );
    assert!(
        scenario.host.revealed().is_none(),
        "a click on a disabled Ok must not reveal the error"
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_no_interior_point_of_the_card_can_dismiss_it() {
    let mut scenario = Scenario::open(create_spec());
    let card = scenario.host.card();
    let mut x = card.x;
    while x < card.x + card.width {
        let mut y = card.y;
        while y < card.y + card.height {
            let outcome = scenario.host.on_pointer(
                pt(x, y),
                InputTarget::Padding,
                &mut scenario.focus,
                &mut scenario.registry,
            );
            assert!(
                matches!(outcome, InputOutcome::Absorbed { .. }),
                "interior point ({x}, {y}) must be absorbed, got {outcome:?}"
            );
            assert!(scenario.host.is_open(), "dismissed at ({x}, {y})");
            y += 8.0;
        }
        x += 8.0;
    }
}

#[test]
fn test_no_interior_point_can_dismiss_even_when_reported_as_ok() {
    let mut scenario = Scenario::open(create_spec());
    let card = scenario.host.card();
    let mut x = card.x;
    while x < card.x + card.width {
        let mut y = card.y;
        while y < card.y + card.height {
            let outcome = scenario.host.on_pointer(
                pt(x, y),
                InputTarget::Ok,
                &mut scenario.focus,
                &mut scenario.registry,
            );
            assert!(
                matches!(outcome, InputOutcome::Absorbed { .. }),
                "a disabled Ok reported at ({x}, {y}) must be absorbed"
            );
            assert!(scenario.host.is_open());
            y += 16.0;
        }
        x += 16.0;
    }
}

#[test]
fn test_a_click_on_an_enabled_ok_accepts() {
    let mut scenario = Scenario::open(create_spec());
    scenario
        .host
        .set_value("good-name".into(), &mut scenario.focus);
    let ok = {
        let layout = scenario.host.layout();
        pt(layout.ok.x + 1.0, layout.ok.y + 1.0)
    };
    let outcome = scenario.host.on_pointer(
        ok,
        InputTarget::Ok,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::Accepted {
            value: "good-name".into()
        }
    );
}

#[test]
fn test_a_click_on_cancel_cancels() {
    let mut scenario = Scenario::open(create_spec());
    let cancel = {
        let layout = scenario.host.layout();
        pt(layout.cancel.x + 1.0, layout.cancel.y + 1.0)
    };
    let outcome = scenario.host.on_pointer(
        cancel,
        InputTarget::Cancel,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::Cancelled {
            cause: DismissCause::CancelButton
        }
    );
    assert!(!scenario.host.is_open());
}

#[test]
fn test_a_click_on_the_entry_focuses_it() {
    let mut scenario = Scenario::open(rename_spec());
    scenario.focus.focus(&fid("dlg.rename:cancel"));
    let entry = {
        let layout = scenario.host.layout();
        pt(layout.control.x + 1.0, layout.control.y + 1.0)
    };
    let outcome = scenario.host.on_pointer(
        entry,
        InputTarget::Entry,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::FocusMoved {
            focused: fid("dlg.rename:entry")
        }
    );
}

#[test]
fn test_a_click_on_a_folder_row_selects_it() {
    let mut scenario = Scenario::open(move_spec(5));
    let outcome = scenario.host.on_pointer(
        scenario.card_centre(),
        InputTarget::FolderRow(2),
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::FocusMoved {
            focused: fid("dlg.move:list")
        }
    );
    assert_eq!(scenario.host.selected(), 2);
    assert_eq!(scenario.host.value(), "/destination/2");
}

#[test]
fn test_an_out_of_range_folder_row_clamps_instead_of_failing() {
    let mut scenario = Scenario::open(move_spec(3));
    scenario.host.on_pointer(
        scenario.card_centre(),
        InputTarget::FolderRow(99),
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(scenario.host.selected(), 2);
    assert_eq!(scenario.host.value(), "/destination/2");
}

#[test]
fn test_a_scrim_click_cancels_the_dialog() {
    let mut scenario = Scenario::open(create_spec());
    let outcome = scenario.host.on_pointer(
        scenario.scrim_point(),
        InputTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::Cancelled {
            cause: DismissCause::Scrim
        }
    );
    assert!(!scenario.host.is_open());
}

#[test]
fn test_geometry_outranks_a_report_of_being_inside_the_card() {
    let mut scenario = Scenario::open(create_spec());
    let outside = scenario.scrim_point();
    let outcome = scenario.host.on_pointer(
        outside,
        InputTarget::Cancel,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::Cancelled {
            cause: DismissCause::Scrim
        },
        "a Cancel report outside the card takes the scrim path"
    );
}

#[test]
fn test_a_control_the_focus_model_refuses_is_downgraded_to_absorbed() {
    let mut scenario = Scenario::open(rename_spec());
    scenario
        .focus
        .set_member_enabled(scenario.host.group_id(), scenario.host.primary_id(), false);
    let entry = {
        let layout = scenario.host.layout();
        pt(layout.control.x + 1.0, layout.control.y + 1.0)
    };
    let outcome = scenario.host.on_pointer(
        entry,
        InputTarget::Entry,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::Absorbed {
            target: InputTarget::Disabled
        },
        "a control the focus model will not accept must not take focus"
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_an_absorbed_click_never_reaches_the_workspace() {
    let mut scenario = Scenario::open(create_spec());
    scenario.registry.register(
        sid("ws.tree"),
        SurfaceKind::Workspace,
        vec![rect(0.0, 0.0, 300.0, 1080.0)],
    );
    let route = scenario.registry.route_pointer(scenario.card_centre());
    assert!(
        !route.reaches_workspace(),
        "a click inside a Band 3 card must not reach Band 0"
    );
}

#[test]
fn test_absorption_leaves_the_error_state_untouched() {
    let mut scenario = Scenario::open(create_spec());
    scenario
        .host
        .set_value("bad/path".into(), &mut scenario.focus);
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    let revealed = scenario.host.revealed().cloned();
    assert!(revealed.is_some());

    scenario.host.on_pointer(
        scenario.card_centre(),
        InputTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(scenario.host.revealed().cloned(), revealed);
}

// ---------------------------------------------------------------------------
// 8. Escape and the menu above — G-13
// ---------------------------------------------------------------------------

#[test]
fn test_escape_cancels_the_dialog() {
    let mut scenario = Scenario::open(create_spec());
    let outcome = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::Cancelled {
            cause: DismissCause::Escape
        }
    );
    assert!(!scenario.host.is_open());
}

#[test]
fn test_escape_goes_through_the_same_route_as_the_cancel_button() {
    let mut a = Scenario::open(create_spec());
    let mut b = Scenario::open(create_spec());

    let escaped = a
        .host
        .on_key(KeyClass::Escape, &mut a.focus, &mut a.registry);
    let cancel = {
        let layout = b.host.layout();
        b.host.on_pointer(
            pt(layout.cancel.x + 1.0, layout.cancel.y + 1.0),
            InputTarget::Cancel,
            &mut b.focus,
            &mut b.registry,
        )
    };

    match (escaped, cancel) {
        (
            InputOutcome::Cancelled {
                cause: DismissCause::Escape,
            },
            InputOutcome::Cancelled {
                cause: DismissCause::CancelButton,
            },
        ) => {}
        other => panic!("both must be plain cancellations, got {other:?}"),
    }
    assert_eq!(a.host.is_open(), b.host.is_open());
    assert_eq!(
        a.registry.count_in(ZBand::Modal),
        b.registry.count_in(ZBand::Modal)
    );
}

#[test]
fn test_escape_restores_focus_to_the_background() {
    let mut scenario = Scenario::at(
        create_spec(),
        pt(500.0, 400.0),
        false,
        desktop(),
        Some("bg.tree"),
    );
    assert_ne!(scenario.focus.current(), Some(&fid("bg.tree")));
    scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(scenario.focus.current(), Some(&fid("bg.tree")));
}

#[test]
fn test_escape_after_close_is_a_no_op() {
    let mut scenario = Scenario::open(create_spec());
    let _ = scenario
        .host
        .cancel(&mut scenario.focus, &mut scenario.registry);
    let outcome = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::NoOp {
            reason: InputNoOp::NotOpen
        }
    );
}

#[test]
fn test_escape_with_nothing_registered_reports_not_registered() {
    let spec = create_spec();
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut host = InputDialogHost::open(
        spec,
        pt(100.0, 100.0),
        false,
        desktop(),
        &mut focus,
        &mut registry,
    );
    registry.unregister(host.id());
    let outcome = host.on_key(KeyClass::Escape, &mut focus, &mut registry);
    assert_eq!(
        outcome,
        InputOutcome::NoOp {
            reason: InputNoOp::NotRegistered
        }
    );
}

#[test]
fn test_the_menu_band_outranks_the_dialog_band_for_escape() {
    assert!(ZBand::Menu.escape_rank() > ZBand::Modal.escape_rank());
    assert_eq!(SurfaceKind::InputDialog.band(), ZBand::Modal);
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

/// A dialog open over a window that already has a Band 2 menu tree open.
fn with_menu_above(spec: InputDialogSpec) -> (Scenario, MenuCascade) {
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
    assert!(cascade.is_open(), "the menu tree must be open");

    for card in cascade.open_cards() {
        registry.register(
            sid(card.id().as_str()),
            SurfaceKind::MenuCard,
            vec![rect(100.0, 100.0, 200.0, 120.0)],
        );
    }

    let host = InputDialogHost::open(
        spec,
        pt(500.0, 400.0),
        false,
        desktop(),
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
/// dialog through its own answer — the shape a host drives.
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

#[test]
fn test_a_menu_above_takes_the_first_escape_and_the_dialog_stays_open() {
    let (mut scenario, mut cascade) = with_menu_above(create_spec());

    let outcome = drive_escape(&mut scenario, &mut cascade);
    assert!(outcome.is_consumed(), "the menu must consume the key");
    assert_ne!(
        outcome.consumed_by(),
        Some(scenario.host.id()),
        "the dialog must not be the first consumer"
    );
    assert!(
        scenario.host.is_open(),
        "G-13: the first Escape closes the menu, not the dialog"
    );
}

#[test]
fn test_the_dialog_closes_on_the_escape_after_the_menu_is_gone() {
    let (mut scenario, mut cascade) = with_menu_above(create_spec());

    drive_escape(&mut scenario, &mut cascade);
    assert!(scenario.host.is_open());

    let second = drive_escape(&mut scenario, &mut cascade);
    assert_eq!(second.consumed_by(), Some(scenario.host.id()));
    assert!(!scenario.host.is_open());
}

#[test]
fn test_the_dialog_is_asked_after_every_menu_card() {
    let (mut scenario, mut cascade) = with_menu_above(create_spec());
    let menu_count = scenario.registry.count_in(ZBand::Menu);
    assert!(menu_count > 0);

    drive_escape(&mut scenario, &mut cascade);
    let remaining = scenario.registry.count_in(ZBand::Menu);
    assert!(
        remaining < menu_count,
        "one menu level must have closed before the dialog was reached"
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_the_dialog_never_refuses_to_unwind() {
    let (mut scenario, _cascade) = with_menu_above(create_spec());
    let answer = scenario.host.unwind_answer(&mut scenario.focus);
    assert_eq!(answer, UnwindAnswer::Dismissed);
    assert!(!scenario.host.is_open());
}

#[test]
fn test_escape_via_closes_only_the_dialog() {
    let (mut scenario, cascade) = with_menu_above(create_spec());
    let menu_before = scenario.registry.count_in(ZBand::Menu);

    let outcome = scenario
        .host
        .escape_via(&mut scenario.registry, &mut scenario.focus);
    assert_eq!(outcome.consumed_by(), Some(scenario.host.id()));
    assert_eq!(
        scenario.registry.count_in(ZBand::Menu),
        menu_before,
        "escape_via must not touch the menu band"
    );
    drop(cascade);
}

// ---------------------------------------------------------------------------
// 9. Destructive parity — UIF-INP-004
// ---------------------------------------------------------------------------

#[test]
fn test_a_single_delete_shows_the_item_name() {
    let spec = delete_single();
    assert_eq!(spec.context, "report.md");
    assert_eq!(DeleteTargets::Single("report.md".into()).count(), 1);
}

#[test]
fn test_a_multi_delete_shows_the_count() {
    let spec = delete_many(7);
    assert_eq!(spec.context, "7 items");
    assert_eq!(DeleteTargets::Many(7).count(), 7);
}

#[test]
fn test_a_multi_delete_of_one_reads_as_one_item() {
    assert_eq!(DeleteTargets::Many(1).summary(), "1 items");
}

#[test]
fn test_both_delete_shapes_are_destructive() {
    assert_eq!(delete_single().intent, ButtonIntent::Danger);
    assert_eq!(delete_many(4).intent, ButtonIntent::Danger);
    assert!(delete_single().intent.is_destructive());
}

#[test]
fn test_non_destructive_dialogs_are_not_danger() {
    assert_eq!(create_spec().intent, ButtonIntent::Standard);
    assert_eq!(rename_spec().intent, ButtonIntent::Standard);
    assert_eq!(move_spec(2).intent, ButtonIntent::Standard);
    assert!(!create_spec().intent.is_destructive());
}

#[test]
fn test_a_delete_card_shares_the_fixed_rows_of_a_rename_card() {
    let rename = Scenario::open(rename_spec());
    let delete = Scenario::open(delete_single());

    let r = rename.host.layout();
    let d = delete.host.layout();

    // One card width, one set of row heights, one gap. The anatomy is a
    // single function, not two that happen to look alike.
    assert_eq!(r.card.width, d.card.width);
    assert_eq!(r.title.height, d.title.height);
    assert_eq!(r.context.height, d.context.height);
    assert_eq!(r.validation.height, d.validation.height);
    assert_eq!(r.footer.height, d.footer.height);
    assert_eq!(r.cancel.width, d.cancel.width);
    assert_eq!(r.ok.width, d.ok.width);

    // Rows above the control sit at identical offsets from the card top.
    assert_eq!(r.title.y - r.card.y, d.title.y - d.card.y);
    assert_eq!(r.context.y - r.card.y, d.context.y - d.card.y);

    // Rows below the control are displaced by exactly the control-height
    // difference, which is the whole extent of the difference between
    // the two cards.
    let delta = r.control.height - d.control.height;
    assert_eq!(
        (r.validation.y - r.card.y) - (d.validation.y - d.card.y),
        delta
    );
    assert_eq!((r.footer.y - r.card.y) - (d.footer.y - d.card.y), delta);
}

#[test]
fn test_the_only_layout_difference_is_the_control_row() {
    let rename = Scenario::open(rename_spec());
    let delete = Scenario::open(delete_single());

    assert_ne!(
        rename.host.layout().control.height,
        delete.host.layout().control.height
    );
    assert_eq!(
        delete.host.layout().control.height,
        CONFIRMATION_CONTROL_HEIGHT
    );
}

#[test]
fn test_both_shapes_answer_with_the_same_answer_type() {
    let mut rename = Scenario::open(rename_spec());
    let mut delete = Scenario::open(delete_single());

    rename.host.set_value("new.md".into(), &mut rename.focus);
    let a = rename.host.submit(&mut rename.focus, &mut rename.registry);
    let b = delete.host.submit(&mut delete.focus, &mut delete.registry);

    assert_eq!(
        a,
        Some(InputAnswer::Accepted {
            value: "new.md".into()
        })
    );
    assert_eq!(
        b,
        Some(InputAnswer::Accepted {
            value: "report.md".into()
        })
    );
}

#[test]
fn test_both_shapes_cancel_the_same_way() {
    let mut rename = Scenario::open(rename_spec());
    let mut delete = Scenario::open(delete_single());

    let a = rename.host.cancel(&mut rename.focus, &mut rename.registry);
    let b = delete.host.cancel(&mut delete.focus, &mut delete.registry);
    assert_eq!(a, b);
    assert_eq!(a, Some(InputAnswer::Cancelled));
}

#[test]
fn test_a_destructive_card_still_lives_in_band_three() {
    let scenario = Scenario::open(delete_many(3));
    assert_eq!(scenario.registry.count_in(ZBand::Modal), 1);
    assert_eq!(
        scenario
            .registry
            .get(scenario.host.id())
            .map(|surface| surface.kind()),
        Some(SurfaceKind::InputDialog)
    );
}

#[test]
fn test_deleting_nothing_is_refused_and_stays_open() {
    let mut scenario = Scenario::open(delete_many(0));
    assert!(!scenario.host.ok_is_enabled());
    let outcome =
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(matches!(outcome, InputOutcome::ErrorRevealed { .. }));
    assert!(scenario.host.is_open());
    assert_eq!(
        scenario.host.revealed().map(|m| m.code.as_str()),
        Some(CODE_NOTHING_TO_CONFIRM)
    );
}

#[test]
fn test_a_destructive_card_uses_the_same_validation_slot_as_everyone() {
    let mut scenario = Scenario::open(delete_many(0));
    let before = *scenario.host.layout();
    scenario
        .host
        .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
    assert!(scenario.host.revealed().is_some());
    assert_layout_identical(&before, scenario.host.layout(), "destructive zero-shift");
}

#[test]
fn test_a_confirmation_absorbs_interior_clicks_like_every_other_card() {
    let mut scenario = Scenario::open(delete_single());
    let card = scenario.host.card();
    let outcome = scenario.host.on_pointer(
        pt(card.x + card.width / 2.0, card.y + card.height / 2.0),
        InputTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert!(matches!(outcome, InputOutcome::Absorbed { .. }));
    assert!(scenario.host.is_open());
}

// ---------------------------------------------------------------------------
// 10. Teardown, focus restoration and determinism
// ---------------------------------------------------------------------------

#[test]
fn test_teardown_pops_the_trap_and_unregisters_the_group() {
    let mut scenario = Scenario::open(create_spec());
    let group = scenario.host.group_id().clone();
    assert!(scenario.focus.group(&group).is_some());

    let _ = scenario
        .host
        .cancel(&mut scenario.focus, &mut scenario.registry);

    assert!(!scenario.focus.is_trapped());
    assert!(scenario.focus.group(&group).is_none());
}

#[test]
fn test_the_trap_is_popped_before_the_group_goes_away() {
    let mut scenario = Scenario::open(create_spec());
    let group = scenario.host.group_id().clone();
    let _ = scenario
        .host
        .cancel(&mut scenario.focus, &mut scenario.registry);
    // Both gone, and the depth came back to the background's zero rather
    // than being left stranded over a group that no longer exists.
    assert_eq!(scenario.focus.trap_depth(), 0);
    assert_ne!(scenario.focus.active_trap(), Some(&group));
}

#[test]
fn test_closing_frees_the_z_slot() {
    let mut scenario = Scenario::open(create_spec());
    assert_eq!(scenario.registry.count_in(ZBand::Modal), 1);
    let _ = scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(scenario.registry.count_in(ZBand::Modal), 0);
}

#[test]
fn test_a_second_close_is_a_no_op() {
    let mut scenario = Scenario::open(create_spec());
    let _ = scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    let outcome = scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        outcome,
        InputOutcome::NoOp {
            reason: InputNoOp::NotOpen
        }
    );
}

#[test]
fn test_submit_after_close_returns_none() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("x".into(), &mut scenario.focus);
    let _ = scenario
        .host
        .close(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        scenario
            .host
            .submit(&mut scenario.focus, &mut scenario.registry),
        None
    );
}

#[test]
fn test_cancel_after_close_returns_none() {
    let mut scenario = Scenario::open(create_spec());
    let _ = scenario
        .host
        .cancel(&mut scenario.focus, &mut scenario.registry);
    assert_eq!(
        scenario
            .host
            .cancel(&mut scenario.focus, &mut scenario.registry),
        None
    );
}

#[test]
fn test_pointer_after_close_is_a_no_op() {
    let mut scenario = Scenario::open(create_spec());
    let _ = scenario
        .host
        .cancel(&mut scenario.focus, &mut scenario.registry);
    let outcome = scenario.host.on_pointer(
        scenario.card_centre(),
        InputTarget::Padding,
        &mut scenario.focus,
        &mut scenario.registry,
    );
    assert_eq!(
        outcome,
        InputOutcome::NoOp {
            reason: InputNoOp::NotOpen
        }
    );
}

#[test]
fn test_the_background_keeps_its_own_group_through_the_whole_cycle() {
    let mut scenario = Scenario::open(create_spec());
    scenario.host.set_value("x".into(), &mut scenario.focus);
    let _ = scenario
        .host
        .cancel(&mut scenario.focus, &mut scenario.registry);
    assert!(scenario.focus.group(&gid("bg")).is_some());
    assert_eq!(
        scenario
            .focus
            .focusable_members(&gid("bg"))
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        vec!["bg.navbar", "bg.tree", "bg.tabs"]
    );
}

#[test]
fn test_opening_the_same_spec_twice_produces_an_identical_host() {
    let a = Scenario::open(rename_spec());
    let b = Scenario::open(rename_spec());
    assert_eq!(a.host, b.host);
}

#[test]
fn test_host_state_is_deterministic_across_a_full_interaction_cycle() {
    let run = || {
        let mut scenario = Scenario::open(create_spec());
        scenario.host.set_value("".into(), &mut scenario.focus);
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
        scenario.host.set_value("ok".into(), &mut scenario.focus);
        scenario
            .host
            .on_key(KeyClass::Enter, &mut scenario.focus, &mut scenario.registry);
        scenario.host
    };
    assert_eq!(run(), run());
}

#[test]
fn test_repeated_open_close_cycles_leave_no_residue() {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    focus.register_group(
        gid("bg"),
        BACKGROUND
            .iter()
            .map(|label| FocusMember::new(*label))
            .collect(),
    );

    for _ in 0..10 {
        let mut host = InputDialogHost::open(
            create_spec(),
            pt(100.0, 100.0),
            false,
            desktop(),
            &mut focus,
            &mut registry,
        );
        assert_eq!(registry.count_in(ZBand::Modal), 1);
        let _ = host.cancel(&mut focus, &mut registry);
    }

    assert_eq!(registry.count_in(ZBand::Modal), 0);
    assert!(registry.is_empty(), "no surface survives the cycle");
    assert_eq!(focus.trap_depth(), 0);
    assert_eq!(focus.groups().len(), 1);
}

#[test]
fn test_two_dialogs_do_not_share_a_focus_group() {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();

    let a = InputDialogHost::open(
        InputDialogSpec::create("dlg.a", "Create", "/a"),
        pt(100.0, 100.0),
        false,
        desktop(),
        &mut focus,
        &mut registry,
    );
    let b = InputDialogHost::open(
        InputDialogSpec::create("dlg.b", "Create", "/b"),
        pt(600.0, 100.0),
        false,
        desktop(),
        &mut focus,
        &mut registry,
    );
    assert_ne!(a.group_id(), b.group_id());
    assert_eq!(registry.count_in(ZBand::Modal), 2);
}

#[test]
fn test_control_ids_are_scoped_to_the_surface() {
    let spec = create_spec();
    assert_eq!(spec.control_id(ENTRY_ROLE), fid("dlg.create:entry"));
    assert_eq!(spec.control_id(LIST_ROLE), fid("dlg.create:list"));
    assert_eq!(spec.control_id(CANCEL_ROLE), fid("dlg.create:cancel"));
    assert_eq!(spec.control_id(OK_ROLE), fid("dlg.create:ok"));
}

#[test]
fn test_the_host_exposes_the_same_control_ids_as_its_spec() {
    let scenario = Scenario::open(create_spec());
    assert_eq!(*scenario.host.ok_id(), scenario.host.control_id(OK_ROLE));
    assert_eq!(
        *scenario.host.cancel_id(),
        scenario.host.control_id(CANCEL_ROLE)
    );
    assert_eq!(
        *scenario.host.primary_id(),
        scenario.host.control_id(ENTRY_ROLE)
    );
}

#[test]
fn test_the_scrim_bounds_are_the_whole_viewport() {
    let scenario = Scenario::at(create_spec(), pt(100.0, 100.0), false, desktop(), None);
    assert_eq!(scenario.host.scrim_bounds(), desktop());
    assert_eq!(scenario.host.viewport(), desktop());
}

#[test]
fn test_tab_cycles_only_within_the_card() {
    let mut scenario = Scenario::open(move_spec(3));
    let mut visited = Vec::new();
    for _ in 0..9 {
        scenario
            .host
            .on_key(KeyClass::Tab, &mut scenario.focus, &mut scenario.registry);
        if let Some(current) = scenario.focus.current() {
            visited.push(current.as_str().to_string());
        }
    }
    assert!(
        visited.iter().all(|id| id.starts_with("dlg.move:")),
        "tab must never leave the card: {visited:?}"
    );
    assert!(
        visited.iter().any(|id| id.ends_with(":cancel")),
        "the cycle must include the footer"
    );
    assert!(
        visited.iter().any(|id| id.ends_with(":list")),
        "the cycle must include the list control"
    );
}

#[test]
fn test_arrow_keys_and_space_are_not_claimed_by_the_band() {
    // §5: Band 3 traps `Tab`/`Shift+Tab`, `Enter` and `Escape`. Arrows
    // and `Space` belong to whatever control inside the card holds focus,
    // so the host reports them as not trapped and touches nothing.
    let mut scenario = Scenario::open(move_spec(3));
    let before = scenario.focus.current().cloned();

    for class in [KeyClass::ArrowNavigation, KeyClass::Space] {
        assert_eq!(
            ZBand::Modal.key_disposition(class),
            KeyDisposition::PassThrough
        );
        let outcome = scenario
            .host
            .on_key(class, &mut scenario.focus, &mut scenario.registry);
        assert_eq!(
            outcome,
            InputOutcome::NoOp {
                reason: InputNoOp::KeyNotTrapped
            },
            "{class:?} must not be claimed by the band"
        );
        assert_eq!(
            scenario.focus.current().cloned(),
            before,
            "{class:?} must not move focus"
        );
    }
}
