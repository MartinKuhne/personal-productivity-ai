//! Tests for [`super::expander`] - the disclosure's zero-shift header, the
//! `GestureSlot` transcript its toggle produces, the tab-reachability contract a
//! collapsed body owes, and the contrast figures its paint is measured at
//! (`UIF-COMP-003`).

use crate::binding::GestureStart;
use crate::composite::expander::{
    AccordionPolicy, BODY_BORDER_WIDTH, BODY_PADDING, CHEVRON_BOX, CHEVRON_ROTATE_COLLAPSED,
    CHEVRON_ROTATE_EXPANDED, CHEVRON_SWEEP, CHEVRON_TITLE_GAP, DisclosureKey, DisclosureToggle,
    HEADER_HEIGHT, HEADER_HEIGHT_COMPACT, HEADER_PADDING_X, HeaderRung, TITLE_RUNG,
    TRAILING_TARGET, apply_accordion_policy, apply_key, body_bounds, body_bounds_at,
    body_content_bounds, body_layer, body_style, chevron_bounds, chevron_rotation, focus_members,
    header_bounds, header_bounds_at, header_layer, resolve_header_style, title_bounds,
    total_height, total_height_at,
};
use crate::focus::{FocusId, FocusState, GroupId};
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::contrast::{contrast_ratio, meets_disabled_text_band};
use crate::token::{shape, size, space};
use iced::{Point, Rectangle};

/// A contrast ratio compared at the four decimal places the pinned figures are
/// written to.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

/// Whether a header rung clears the crate's hit-target floor.
///
/// A function rather than a bare comparison so the assertion is not between two
/// compile-time constants, which would test the arithmetic rather than the rung.
fn clears_hit_floor(height: f32) -> bool {
    height >= size::HIT_TARGET_MIN
}

fn bounds(width: f32, height: f32) -> Rectangle {
    Rectangle::new(Point::new(10.0, 40.0), iced::Size::new(width, height))
}

fn palette() -> Palette {
    Palette::dark()
}

// ---------------------------------------------------------------------------
// 1 - the header rung
// ---------------------------------------------------------------------------

#[test]
fn the_standard_header_rung_is_the_control_rung() {
    assert_eq!(HEADER_HEIGHT, 32.0);
    assert_eq!(HEADER_HEIGHT, size::CONTROL_MD);
}

#[test]
fn the_compact_header_rung_is_the_small_control_rung() {
    assert_eq!(HEADER_HEIGHT_COMPACT, 28.0);
    assert_eq!(HEADER_HEIGHT_COMPACT, size::CONTROL_SM);
}

#[test]
fn both_rungs_clear_the_hit_target_floor() {
    assert!(clears_hit_floor(HEADER_HEIGHT));
    assert!(clears_hit_floor(HEADER_HEIGHT_COMPACT));
}

#[test]
fn the_rungs_report_their_own_heights() {
    assert_eq!(HeaderRung::Standard.height(), HEADER_HEIGHT);
    assert_eq!(HeaderRung::Compact.height(), HEADER_HEIGHT_COMPACT);
    assert_eq!(HeaderRung::default(), HeaderRung::Standard);
}

#[test]
fn the_header_spans_the_full_width_at_the_top_of_its_bounds() {
    let b = bounds(320.0, 200.0);
    let header = header_bounds(b);
    assert_eq!(header.x, b.x);
    assert_eq!(header.y, b.y);
    assert_eq!(header.width, 320.0);
    assert_eq!(header.height, HEADER_HEIGHT);
}

#[test]
fn a_negative_width_clamps_to_zero_rather_than_wrapping() {
    let b = Rectangle::new(Point::new(5.0, 5.0), iced::Size::new(-40.0, 100.0));
    assert_eq!(header_bounds(b).width, 0.0);
}

// ---------------------------------------------------------------------------
// 2 - the zero-shift invariant
// ---------------------------------------------------------------------------

#[test]
fn the_header_box_is_identical_open_and_closed() {
    // The invariant is structural: `header_bounds_at` has no open parameter, so
    // the only way to observe a difference is to route one in through the rung.
    // This asserts the observable fact for both rungs anyway.
    for rung in [HeaderRung::Standard, HeaderRung::Compact] {
        let b = bounds(480.0, 400.0);
        let closed = header_bounds_at(b, rung);
        let open = header_bounds_at(b, rung);
        assert_eq!(closed, open, "header shifted at {rung:?}");
    }
}

#[test]
fn mounting_a_body_never_moves_the_header() {
    let b = bounds(480.0, 400.0);
    let before = header_bounds(b);
    let _body = body_bounds(b, true, 128.0);
    assert_eq!(header_bounds(b), before);
}

#[test]
fn the_body_starts_exactly_at_the_headers_bottom_edge() {
    let b = bounds(480.0, 400.0);
    let header = header_bounds(b);
    let body = body_bounds(b, true, 128.0).expect("open body");
    assert_eq!(body.y, header.y + header.height);
    assert_eq!(body.x, b.x);
    assert_eq!(body.width, b.width);
    assert_eq!(body.height, 128.0);
}

#[test]
fn the_body_never_overlaps_the_header() {
    let b = bounds(480.0, 400.0);
    let header = header_bounds(b);
    let body = body_bounds(b, true, 128.0).expect("open body");
    assert!(
        body.y >= header.y + header.height,
        "body at {} overlapped header ending at {}",
        body.y,
        header.y + header.height
    );
}

#[test]
fn a_collapsed_body_has_no_rectangle_at_all() {
    assert!(body_bounds(bounds(480.0, 400.0), false, 128.0).is_none());
    assert!(body_bounds_at(bounds(480.0, 400.0), HeaderRung::Compact, false, 128.0).is_none());
}

#[test]
fn a_negative_content_height_clamps_to_zero_rather_than_inverting() {
    let body = body_bounds(bounds(480.0, 400.0), true, -50.0).expect("open body");
    assert_eq!(body.height, 0.0);
}

#[test]
fn the_body_content_is_inset_by_the_body_padding() {
    let b = bounds(480.0, 400.0);
    let body = body_bounds(b, true, 200.0).expect("open body");
    let inner = body_content_bounds(body);
    assert_eq!(BODY_PADDING, space::MD);
    assert_eq!(inner.x, body.x + BODY_PADDING);
    assert_eq!(inner.y, body.y + BODY_PADDING);
    assert_eq!(inner.width, body.width - BODY_PADDING * 2.0);
    assert_eq!(inner.height, body.height - BODY_PADDING * 2.0);
}

#[test]
fn a_body_smaller_than_its_padding_clamps_instead_of_going_negative() {
    let tiny = Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(4.0, 6.0));
    let inner = body_content_bounds(tiny);
    assert_eq!(inner.width, 0.0);
    assert_eq!(inner.height, 0.0);
}

#[test]
fn total_height_carries_the_body_only_when_mounted() {
    assert_eq!(total_height(false, 128.0), HEADER_HEIGHT);
    assert_eq!(total_height(true, 128.0), HEADER_HEIGHT + 128.0);
    assert_eq!(
        total_height_at(HeaderRung::Compact, true, 100.0),
        HEADER_HEIGHT_COMPACT + 100.0
    );
    assert_eq!(
        total_height_at(HeaderRung::Compact, false, 100.0),
        HEADER_HEIGHT_COMPACT
    );
}

// ---------------------------------------------------------------------------
// 3 - the chevron
// ---------------------------------------------------------------------------

#[test]
fn the_chevron_box_is_the_medium_icon_rung() {
    assert_eq!(CHEVRON_BOX, size::ICON_MD);
    assert_eq!(CHEVRON_BOX, 16.0);
}

#[test]
fn the_chevron_rotates_through_a_right_angle() {
    assert_eq!(chevron_rotation(false), CHEVRON_ROTATE_COLLAPSED);
    assert_eq!(chevron_rotation(true), CHEVRON_ROTATE_EXPANDED);
    assert_eq!(CHEVRON_SWEEP, 90.0);
    assert_eq!(
        CHEVRON_SWEEP,
        CHEVRON_ROTATE_EXPANDED - CHEVRON_ROTATE_COLLAPSED
    );
}

#[test]
fn the_chevron_box_is_centred_in_the_header() {
    let header = header_bounds(bounds(320.0, 200.0));
    let chevron = chevron_bounds(header);
    assert_eq!(chevron.x, header.x + HEADER_PADDING_X);
    assert_eq!(chevron.width, CHEVRON_BOX);
    assert_eq!(chevron.height, CHEVRON_BOX);
    assert_eq!(
        chevron.y + chevron.height / 2.0,
        header.y + header.height / 2.0,
        "chevron is not vertically centred"
    );
}

#[test]
fn the_chevron_box_is_identical_in_both_faces_so_rotation_moves_no_target() {
    let header = header_bounds(bounds(320.0, 200.0));
    let closed = chevron_bounds(header);
    let open = chevron_bounds(header);
    assert_eq!(closed, open);
    // The two settled angles differ; the box they are drawn in does not.
    assert_ne!(chevron_rotation(false), chevron_rotation(true));
}

#[test]
fn the_chevron_stays_inside_the_header_at_the_compact_rung() {
    let header = header_bounds_at(bounds(320.0, 200.0), HeaderRung::Compact);
    let chevron = chevron_bounds(header);
    assert!(chevron.y >= header.y);
    assert!(chevron.y + chevron.height <= header.y + header.height);
}

// ---------------------------------------------------------------------------
// 4 - the title slot
// ---------------------------------------------------------------------------

#[test]
fn the_title_starts_after_the_chevron_and_the_gap() {
    let header = header_bounds(bounds(320.0, 200.0));
    let title = title_bounds(header, 0.0);
    assert_eq!(
        title.x,
        header.x + HEADER_PADDING_X + CHEVRON_BOX + CHEVRON_TITLE_GAP
    );
    assert_eq!(title.height, header.height);
}

#[test]
fn the_title_shrinks_for_the_trailing_slot() {
    let header = header_bounds(bounds(320.0, 200.0));
    let bare = title_bounds(header, 0.0);
    let with_trailing = title_bounds(header, TRAILING_TARGET);
    assert_eq!(
        bare.width - with_trailing.width,
        TRAILING_TARGET,
        "the trailing target is not `size.hit_target.min`"
    );
    assert_eq!(TRAILING_TARGET, size::HIT_TARGET_MIN);
}

#[test]
fn the_title_clamps_to_zero_when_the_trailing_slot_eats_it() {
    let header = header_bounds(bounds(60.0, 200.0));
    assert_eq!(title_bounds(header, 500.0).width, 0.0);
}

#[test]
fn a_negative_trailing_width_is_treated_as_zero() {
    let header = header_bounds(bounds(320.0, 200.0));
    assert_eq!(
        title_bounds(header, -20.0).width,
        title_bounds(header, 0.0).width
    );
}

#[test]
fn the_title_rung_is_the_compact_ui_rung() {
    assert_eq!(TITLE_RUNG, &crate::token::typography::UI_COMPACT);
}

// ---------------------------------------------------------------------------
// 5 - the toggle transcript
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_toggle_is_closed_with_nothing_pressed() {
    let toggle = DisclosureToggle::new(false);
    assert!(!toggle.is_open());
    assert!(!toggle.is_pressed());
}

#[test]
fn releasing_a_pressed_toggle_flips_it_once() {
    let mut toggle = DisclosureToggle::new(false);
    let _ = toggle.press();
    assert!(toggle.is_pressed());
    let commit = toggle.release().expect("commit");
    assert!(*commit.value());
    assert!(toggle.is_open());
    assert!(!toggle.is_pressed());
}

#[test]
fn a_release_with_no_press_commits_nothing() {
    let mut toggle = DisclosureToggle::new(false);
    assert!(toggle.release().is_none());
    assert!(!toggle.is_open());
}

#[test]
fn a_second_press_is_refused_and_the_first_snapshot_governs() {
    let mut toggle = DisclosureToggle::new(false);
    let _ = toggle.press();
    let second = toggle.press();
    assert_eq!(
        second,
        GestureStart::AlreadyOpen,
        "nested presses must not re-capture the snapshot"
    );
    // Cancelling therefore reverts to the value before the *first* press.
    let outcome = toggle.cancel().expect("cancel");
    assert!(!*outcome.restored());
    assert!(!toggle.is_open());
}

#[test]
fn cancelling_restores_the_value_the_press_started_from() {
    let mut toggle = DisclosureToggle::new(true);
    let _ = toggle.press();
    let outcome = toggle.cancel().expect("cancel");
    assert!(*outcome.restored());
    assert!(toggle.is_open());
}

#[test]
fn cancelling_with_nothing_pending_emits_nothing() {
    let mut toggle = DisclosureToggle::new(true);
    assert!(toggle.cancel().is_none());
    assert!(toggle.is_open());
}

#[test]
fn press_release_round_trips_back_to_the_start() {
    let mut toggle = DisclosureToggle::new(false);
    let _ = toggle.press();
    toggle.release();
    assert!(toggle.is_open());
    let _ = toggle.press();
    toggle.release();
    assert!(!toggle.is_open());
}

#[test]
fn set_open_drives_the_flag_and_reports_a_commit() {
    let mut toggle = DisclosureToggle::new(false);
    let commit = toggle.set_open(true);
    assert!(*commit.value());
    assert!(toggle.is_open());
    let commit = toggle.set_open(true);
    assert!(*commit.value(), "a repeat commit still names the value");
}

#[test]
fn set_open_settles_a_stuck_press_rather_than_leaving_it_open() {
    let mut toggle = DisclosureToggle::new(false);
    let _ = toggle.press();
    assert!(toggle.is_pressed());
    toggle.set_open(true);
    assert!(
        !toggle.is_pressed(),
        "a keyboard toggle must settle the open slot"
    );
}

// ---------------------------------------------------------------------------
// 6 - keyboard activation
// ---------------------------------------------------------------------------

#[test]
fn toggle_flips_either_way() {
    let mut open = DisclosureToggle::new(false);
    assert!(
        *apply_key(&mut open, DisclosureKey::Toggle)
            .expect("commit")
            .value()
    );

    let mut closed = DisclosureToggle::new(true);
    assert!(
        !*apply_key(&mut closed, DisclosureKey::Toggle)
            .expect("commit")
            .value()
    );
}

#[test]
fn right_arrow_expands_and_left_arrow_collapses() {
    let mut toggle = DisclosureToggle::new(false);
    assert!(
        *apply_key(&mut toggle, DisclosureKey::Expand)
            .expect("commit")
            .value()
    );
    assert!(
        !*apply_key(&mut toggle, DisclosureKey::Collapse)
            .expect("commit")
            .value()
    );
}

#[test]
fn a_key_that_would_change_nothing_commits_nothing() {
    let mut open = DisclosureToggle::new(true);
    assert!(apply_key(&mut open, DisclosureKey::Expand).is_none());
    assert!(open.is_open());

    let mut closed = DisclosureToggle::new(false);
    assert!(apply_key(&mut closed, DisclosureKey::Collapse).is_none());
    assert!(!closed.is_open());
}

// ---------------------------------------------------------------------------
// 7 - tab reachability
// ---------------------------------------------------------------------------

#[test]
fn a_collapsed_body_contributes_only_disabled_members() {
    let body = vec![FocusId::new("body.a"), FocusId::new("body.b")];
    let members = focus_members("header", &body, false, false);
    assert_eq!(members.len(), 3);
    assert!(members[0].enabled, "the header is always focusable");
    assert!(
        members[1..].iter().all(|m| !m.enabled),
        "a collapsed body must not offer live tab stops"
    );
}

#[test]
fn an_open_body_contributes_live_members() {
    let body = vec![FocusId::new("body.a"), FocusId::new("body.b")];
    let members = focus_members("header", &body, true, false);
    assert!(members.iter().all(|m| m.enabled));
}

#[test]
fn a_disabled_disclosure_disables_every_member_including_the_header() {
    let body = vec![FocusId::new("body.a")];
    for open in [false, true] {
        let members = focus_members("header", &body, open, true);
        assert!(
            members.iter().all(|m| !m.enabled),
            "a disabled disclosure leaked a live member while open={open}"
        );
    }
}

#[test]
fn a_collapsed_body_is_not_tab_reachable_in_a_real_focus_walk() {
    let body = vec![
        FocusId::new("collapsed.body.a"),
        FocusId::new("collapsed.body.b"),
    ];
    let group = GroupId::new("disclosure");
    let mut state = FocusState::new();
    state.register_group(
        group.clone(),
        focus_members("collapsed.header", &body, false, false),
    );

    let mut visited = Vec::new();
    visited.push(
        state
            .focus_first_in_group(&group)
            .expect("the header is reachable"),
    );
    for _ in 0..8 {
        if let Some(next) = state.focus_next() {
            visited.push(next);
        }
    }

    let reached_body = visited
        .iter()
        .filter(|id| id.as_str().starts_with("collapsed.body."))
        .count();
    assert_eq!(
        reached_body, 0,
        "tabbing reached a collapsed body: {visited:?}"
    );
    assert!(
        visited.iter().any(|id| id.as_str() == "collapsed.header"),
        "the header itself was not reachable: {visited:?}"
    );
}

#[test]
fn an_open_body_is_tab_reachable() {
    let body = vec![FocusId::new("open.body.a"), FocusId::new("open.body.b")];
    let group = GroupId::new("disclosure");
    let mut state = FocusState::new();
    state.register_group(
        group.clone(),
        focus_members("open.header", &body, true, false),
    );

    let mut visited = Vec::new();
    visited.push(state.focus_first_in_group(&group).expect("first"));
    for _ in 0..2 {
        if let Some(next) = state.focus_next() {
            visited.push(next);
        }
    }

    let reached: Vec<&str> = visited
        .iter()
        .filter(|id| id.as_str().starts_with("open.body."))
        .map(|id| id.as_str())
        .collect();
    assert_eq!(reached, vec!["open.body.a", "open.body.b"]);
}

#[test]
fn collapsing_mid_focus_does_not_leave_a_live_stop_behind() {
    let body = vec![FocusId::new("switch.body.a")];
    let group = GroupId::new("disclosure");
    let mut state = FocusState::new();
    state.register_group(
        group.clone(),
        focus_members("switch.header", &body, true, false),
    );
    state.focus(&FocusId::new("switch.body.a"));

    // The body collapses; re-register the same ids at `open == false`.
    state.register_group(
        group.clone(),
        focus_members("switch.header", &body, false, false),
    );
    let reached = state
        .focusable_members(&group)
        .iter()
        .filter(|id| id.as_str().starts_with("switch.body."))
        .count();
    assert_eq!(reached, 0, "a collapsed body kept a focusable member");
}

// ---------------------------------------------------------------------------
// 8 - accordion policy
// ---------------------------------------------------------------------------

#[test]
fn independent_policy_touches_no_sibling() {
    let mut toggles = vec![
        DisclosureToggle::new(true),
        DisclosureToggle::new(true),
        DisclosureToggle::new(false),
    ];
    let commits = apply_accordion_policy(AccordionPolicy::Independent, &mut toggles, 0);
    assert!(commits.is_empty());
    assert!(toggles[1].is_open());
}

#[test]
fn single_open_policy_closes_the_other_open_siblings() {
    let mut toggles = vec![
        DisclosureToggle::new(true),
        DisclosureToggle::new(true),
        DisclosureToggle::new(false),
    ];
    let commits = apply_accordion_policy(AccordionPolicy::SingleOpen, &mut toggles, 0);
    assert_eq!(commits.len(), 1, "only the other *open* sibling commits");
    assert!(!*commits[0].value());
    assert!(!toggles[1].is_open());
    assert!(toggles[0].is_open(), "the opened one stays open");
}

#[test]
fn single_open_policy_leaves_a_sibling_under_its_own_press_alone() {
    let mut toggles = vec![DisclosureToggle::new(true), DisclosureToggle::new(true)];
    let _ = toggles[1].press();
    let commits = apply_accordion_policy(AccordionPolicy::SingleOpen, &mut toggles, 0);
    assert!(
        commits.is_empty(),
        "a sibling mid-press must not be force-closed"
    );
    assert!(toggles[1].is_open());
    assert!(toggles[1].is_pressed());
}

#[test]
fn closing_the_only_open_sibling_is_a_no_op() {
    let mut toggles = vec![DisclosureToggle::new(false), DisclosureToggle::new(false)];
    let commits = apply_accordion_policy(AccordionPolicy::SingleOpen, &mut toggles, 0);
    assert!(commits.is_empty());
}

#[test]
fn an_out_of_range_index_closes_every_open_sibling() {
    // Nothing is exempt, which is the honest reading: there is no "opener" to keep.
    let mut toggles = vec![DisclosureToggle::new(true), DisclosureToggle::new(true)];
    let commits = apply_accordion_policy(AccordionPolicy::SingleOpen, &mut toggles, 99);
    assert_eq!(commits.len(), 2);
}

#[test]
fn the_default_policy_is_independent() {
    assert_eq!(AccordionPolicy::default(), AccordionPolicy::Independent);
    assert!(!AccordionPolicy::Independent.closes_siblings());
    assert!(AccordionPolicy::SingleOpen.closes_siblings());
}

// ---------------------------------------------------------------------------
// 9 - contrast, recomputed from raw sRGB
// ---------------------------------------------------------------------------

#[test]
fn the_title_clears_its_resting_fill() {
    let p = palette();
    let ratio = contrast_ratio(p.text_primary, p.surface_raised);
    assert!(
        approx(ratio, 12.3755),
        "text.primary on surface.raised measured {ratio}, expected 12.3755"
    );
    assert!(ratio >= 4.5);
}

#[test]
fn the_chevron_clears_its_resting_fill() {
    let p = palette();
    let ratio = contrast_ratio(p.text_muted, p.surface_raised);
    assert!(
        approx(ratio, 7.1553),
        "text.muted on surface.raised measured {ratio}, expected 7.1553"
    );
}

#[test]
fn the_disabled_title_sits_inside_the_disabled_band() {
    let p = palette();
    let ratio = contrast_ratio(p.text_disabled, p.surface_raised);
    assert!(
        approx(ratio, 5.5643),
        "text.disabled on surface.raised measured {ratio}, expected 5.5643"
    );
    assert!(
        meets_disabled_text_band(p.text_disabled, p.surface_raised),
        "the disabled title left the [4.0, 6.0] band"
    );
}

#[test]
fn the_body_text_clears_the_body_fill() {
    let p = palette();
    let ratio = contrast_ratio(p.text_primary, p.surface_base);
    assert!(
        approx(ratio, 13.1590),
        "text.primary on surface.base measured {ratio}, expected 13.1590"
    );
}

#[test]
fn the_body_uses_base_rather_than_sunken_so_mounting_moves_no_boundary() {
    let p = palette();
    let style = body_style(&p);
    assert_eq!(style.background, p.surface_base);
    assert_ne!(style.background, p.surface_sunken);
    assert_eq!(style.border_color, p.border_subtle);
    assert_eq!(style.border_width, BODY_BORDER_WIDTH);
    assert_eq!(BODY_BORDER_WIDTH, shape::BORDER_HAIRLINE);
}

// ---------------------------------------------------------------------------
// 10 - header paint states
// ---------------------------------------------------------------------------

#[test]
fn the_resting_header_is_raised_with_a_muted_chevron() {
    let p = palette();
    let style = resolve_header_style(BaseState::Rest, &p);
    assert_eq!(style.background, p.surface_raised);
    assert_eq!(style.title_colour, p.text_primary);
    assert_eq!(style.chevron_colour, p.text_muted);
}

#[test]
fn hover_lifts_the_fill_and_brightens_the_chevron() {
    let p = palette();
    let style = resolve_header_style(BaseState::Hovered, &p);
    assert_eq!(style.background, p.surface_hover);
    assert_eq!(style.chevron_colour, p.text_primary);
}

#[test]
fn press_deepens_the_fill() {
    let p = palette();
    let style = resolve_header_style(BaseState::Pressed, &p);
    assert_eq!(style.background, p.surface_active);
    assert_eq!(style.chevron_colour, p.text_primary);
}

#[test]
fn the_three_live_states_are_all_distinct() {
    let p = palette();
    let rest = resolve_header_style(BaseState::Rest, &p).background;
    let hover = resolve_header_style(BaseState::Hovered, &p).background;
    let pressed = resolve_header_style(BaseState::Pressed, &p).background;
    assert_ne!(rest, hover);
    assert_ne!(hover, pressed);
    assert_ne!(rest, pressed);
}

#[test]
fn a_disabled_header_stays_visible_on_the_resting_surface() {
    let p = palette();
    let style = resolve_header_style(BaseState::Disabled, &p);
    assert_eq!(style.background, p.surface_raised);
    assert_eq!(style.title_colour, p.text_disabled);
    assert_eq!(style.chevron_colour, p.text_disabled);
    assert_ne!(
        style.title_colour.a, 0.0,
        "a dead header must still be readable"
    );
}

#[test]
fn the_header_layer_paints_its_resolved_fill_at_the_widget_radius() {
    let p = palette();
    let header = header_bounds(bounds(320.0, 200.0));
    let style = resolve_header_style(BaseState::Rest, &p);
    let layer = header_layer(header, &style);
    assert_eq!(layer.bounds, header);
    assert_eq!(layer.fill, style.background);
}

#[test]
fn the_body_layer_consumes_an_already_resolved_rectangle() {
    let p = palette();
    let body = body_bounds(bounds(480.0, 400.0), true, 200.0).expect("open body");
    let style = body_style(&p);
    let layer = body_layer(body, &style);
    assert_eq!(layer.bounds, body);
    assert_eq!(layer.fill, style.background);
    assert_eq!(layer.border.width, BODY_BORDER_WIDTH);
}
