//! Tests for the breadcrumb trail's elision solver and host — `UIF-COMP-008`.
//!
//! Six groups, in order:
//!
//! 1. **Roles.** What `crumb_role` says at each position, including the
//!    one- and two-crumb degeneracies where root and terminal collide.
//! 2. **The solver.** When nothing collapses, when the middle collapses,
//!    and the invariant that root and terminal never do.
//! 3. **The solver's *properties*.** Purity, prefix-shaped hidden sets,
//!    and monotonicity across a width sweep — the claims the increment is
//!    actually made of.
//! 4. **Rectangles.** That the separator lives in the gap and therefore
//!    cannot be a click target.
//! 5. **Focus.** Hidden crumbs are not tab stops; the trigger is.
//! 6. **Colour and the host.** Every contrast pair recomputed, and the
//!    `[…]` menu holding exactly the hidden crumbs.

use super::*;
use crate::focus::GroupId;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A trail of `n` crumbs, each `40px` wide.
fn trail(n: usize) -> Vec<BreadcrumbItem<String>> {
    (0..n)
        .map(|index| BreadcrumbItem::new(format!("c{index}"), format!("Crumb {index}"), 40.0))
        .collect()
}

/// The width a trail of `n` 40px crumbs needs to sit fully inline.
fn full_width(n: usize) -> f32 {
    40.0 * n as f32 + SEGMENT_GAP * n.saturating_sub(1) as f32
}

/// A focus state with the trigger registered, so the trap has somewhere to
/// come back to.
fn focus_with_trigger() -> FocusState {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("bar"),
        vec![FocusMember::new(ellipsis_focus_id())],
    );
    focus
}

/// The rest state, spelled out once.
fn rest() -> VisualState {
    VisualState {
        base: BaseState::Rest,
        focused: false,
    }
}

// ---------------------------------------------------------------------------
// 1. Roles
// ---------------------------------------------------------------------------

#[test]
fn role_of_first_is_root_even_in_a_one_crumb_trail() {
    assert_eq!(crumb_role(0, 1), CrumbRole::Root);
}

#[test]
fn role_of_last_is_terminal_and_first_wins_the_collision() {
    // Index 0 of a one-crumb trail is also the last crumb. Root wins,
    // because "first always visible" is stated about the root.
    assert_eq!(crumb_role(0, 1), CrumbRole::Root);
    assert_eq!(crumb_role(1, 2), CrumbRole::Terminal);
}

#[test]
fn only_interior_indices_are_middle() {
    assert_eq!(crumb_role(1, 3), CrumbRole::Middle);
    assert_eq!(crumb_role(2, 5), CrumbRole::Middle);
    assert_eq!(crumb_role(0, 5), CrumbRole::Root);
    assert_eq!(crumb_role(4, 5), CrumbRole::Terminal);
}

// ---------------------------------------------------------------------------
// 2. The solver
// ---------------------------------------------------------------------------

#[test]
fn an_empty_trail_lays_out_as_nothing() {
    let layout = collapse::<String>(&[], 100.0);
    assert!(layout.plans().is_empty());
    assert_eq!(layout.hidden_count(), 0);
    assert_eq!(layout.trigger_width(), 0.0);
    assert_eq!(layout.total_width(), 0.0);
}

#[test]
fn a_two_crumb_trail_is_never_elided_however_narrow() {
    // There is no middle run. Eliding anything would break the
    // always-visible rule rather than relieve the pressure, so the solver
    // reports the shortfall instead.
    let items = trail(2);
    let layout = collapse(&items, 1.0);
    assert_eq!(layout.hidden_count(), 0);
    assert!(!layout.is_elided());
    assert!(!layout.minimum_fits(), "1px cannot hold two 40px crumbs");
    assert!(layout.plans().iter().all(CrumbPlan::is_inline));
}

#[test]
fn a_trail_that_fits_stays_fully_inline() {
    let items = trail(4);
    let layout = collapse(&items, full_width(4));
    assert_eq!(layout.hidden_count(), 0);
    assert_eq!(layout.inline_count(), 4);
    assert_eq!(layout.trigger_width(), 0.0);
    assert!(layout.fits());
    assert!(layout.minimum_fits());
}

#[test]
fn one_pixel_short_collapses_the_middle() {
    let items = trail(4);
    let layout = collapse(&items, full_width(4) - 1.0);
    assert!(layout.is_elided());
    assert_eq!(layout.trigger_width(), ELLIPSIS_TRIGGER_WIDTH);
}

#[test]
fn root_and_terminal_are_inline_at_every_width() {
    let items = trail(6);
    for width in 0..400 {
        let layout = collapse(&items, width as f32);
        assert!(
            layout.plan(0).is_some_and(|plan| plan.is_inline()),
            "root elided at width {width}"
        );
        assert!(
            layout.plan(5).is_some_and(|plan| plan.is_inline()),
            "terminal elided at width {width}"
        );
    }
}

#[test]
fn the_minimum_trail_is_root_terminal_and_trigger() {
    let items = trail(5);
    // Root + terminal + trigger, with two gaps on each join.
    let minimum = 40.0 + SEGMENT_GAP + 40.0 + SEGMENT_GAP + ELLIPSIS_TRIGGER_WIDTH;
    let layout = collapse(&items, minimum);
    assert_eq!(layout.inline_count(), 2);
    assert_eq!(layout.hidden_count(), 3);
    assert_eq!(layout.total_width(), minimum);
    assert!(layout.minimum_fits());
}

#[test]
fn a_width_below_the_minimum_reports_rather_than_eliding_the_terminals() {
    let items = trail(5);
    let layout = collapse(&items, 20.0);
    // The invariant holds; the shortfall is what is reported.
    assert_eq!(layout.hidden_count(), 3);
    assert!(!layout.minimum_fits());
}

#[test]
fn a_measure_below_the_floor_is_clamped_not_collapsed() {
    let items = vec![
        BreadcrumbItem::new("root".to_string(), "Root".to_string(), 4.0),
        BreadcrumbItem::new("mid".to_string(), "Mid".to_string(), 0.0),
        BreadcrumbItem::new("term".to_string(), "Term".to_string(), -12.0),
    ];
    let layout = collapse(&items, 1.0);
    for plan in layout.plans() {
        assert_eq!(
            plan.width, MIN_SEGMENT_WIDTH,
            "clamped at index {}",
            plan.index
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Solver properties
// ---------------------------------------------------------------------------

#[test]
fn the_solver_is_pure() {
    let items = trail(7);
    let first = collapse(&items, 150.0);
    let second = collapse(&items, 150.0);
    assert_eq!(first, second, "same inputs must give the same layout");
}

#[test]
fn the_hidden_set_is_a_prefix_of_the_middle_run() {
    let items = trail(8);
    for width in 0..500 {
        let hidden = collapse(&items, width as f32).hidden_indices();
        if hidden.is_empty() {
            continue;
        }
        let expected: Vec<usize> = (1..1 + hidden.len()).collect();
        assert_eq!(
            hidden, expected,
            "hidden set is not a front-of-middle prefix at {width}"
        );
    }
}

#[test]
fn widening_never_collapses_a_different_arbitrary_set() {
    // The monotone claim: hidden(w) ⊇ hidden(w') whenever w < w'.
    let items = trail(9);
    let mut previous: Vec<usize> = Vec::new();
    let mut first = true;
    for width in (0..=520).step_by(3) {
        let hidden = collapse(&items, width as f32).hidden_indices();
        if first {
            previous = hidden;
            first = false;
            continue;
        }
        for index in &hidden {
            assert!(
                previous.contains(index),
                "widening to {width} un-hid {index} while {} is still hidden",
                previous.len()
            );
        }
        // Because the hidden set is a prefix, superset-of is the same as
        // "at least as long", so the sweep can only shrink it.
        assert!(
            hidden.len() <= previous.len(),
            "hidden set grew from {} to {} when widening to {width}",
            previous.len(),
            hidden.len()
        );
        previous = hidden;
    }
}

#[test]
fn widening_restores_hidden_crumbs_from_the_terminal_end_first() {
    // The kept run is anchored at the terminal, so the first crumb to
    // come back is the one nearest the terminal.
    let items = trail(6);
    let tight = collapse(&items, 120.0).hidden_indices();
    let loose = collapse(&items, 120.0 + 40.0 + SEGMENT_GAP * 2.0).hidden_indices();
    assert_eq!(tight, vec![1, 2, 3, 4]);
    assert_eq!(loose, vec![1, 2, 3]);
}

#[test]
fn the_trigger_only_exists_when_there_is_something_to_reach() {
    let items = trail(4);
    assert_eq!(
        trigger_rect(&collapse(&items, 10.0), 0.0, 0.0).map(|r| r.width),
        Some(ELLIPSIS_TRIGGER_WIDTH)
    );
    assert_eq!(trigger_rect(&collapse(&items, 1_000.0), 0.0, 0.0), None);
}

// ---------------------------------------------------------------------------
// 4. Rectangles — the separator is not a target
// ---------------------------------------------------------------------------

#[test]
fn separators_sit_in_the_gap_and_never_inside_a_crumb() {
    let items = trail(5);
    let layout = collapse(&items, 1_000.0);
    let crumbs = segment_rects(&layout, 0.0, 0.0);
    let separators = separator_rects(&layout, 0.0, 0.0);

    assert_eq!(separators.len(), crumbs.len() - 1);
    for separator in &separators {
        assert!(
            (separator.width - SEGMENT_GAP).abs() < 1.0e-4,
            "separator is {} wide, not the gap",
            separator.width
        );
        for (_, crumb) in &crumbs {
            let overlaps =
                separator.x < crumb.x + crumb.width && separator.x + separator.width > crumb.x;
            assert!(
                !overlaps,
                "separator {:?} overlaps crumb {:?}",
                separator, crumb
            );
        }
    }
}

#[test]
fn an_elided_trail_has_fewer_separators_than_it_had_crumbs() {
    let items = trail(6);
    let layout = collapse(&items, 140.0);
    let crumbs = segment_rects(&layout, 0.0, 0.0);
    assert_eq!(separator_rects(&layout, 0.0, 0.0).len(), crumbs.len() - 1);
    assert_eq!(crumbs.len(), layout.inline_count());
}

#[test]
fn the_trigger_sits_after_the_last_inline_crumb_with_one_gap_between() {
    let items = trail(5);
    let layout = collapse(&items, 140.0);
    let crumbs = segment_rects(&layout, 0.0, 0.0);
    let trigger = trigger_rect(&layout, 0.0, 0.0).expect("elided trail has a trigger");
    let last = crumbs.last().expect("root is always inline");
    assert!(
        (trigger.x - (last.1.x + last.1.width + SEGMENT_GAP)).abs() < 1.0e-4,
        "trigger is not one gap behind the last crumb"
    );
}

#[test]
fn the_bar_height_does_not_depend_on_the_elision() {
    // The trail's height is a constant of the bar, not a function of the
    // layout, so collapsing cannot shift anything vertically. The rect
    // walk is the part that could actually move: every painted box has to
    // stay inside the same 28px band at every width, elided or not.
    const _: () = assert!(SEGMENT_HEIGHT <= BAR_HEIGHT);
    const _: () = assert!(ELLIPSIS_TRIGGER_WIDTH <= BAR_HEIGHT);

    let items = trail(8);
    for width in [40.0, 120.0, 260.0, 1_000.0] {
        let layout = collapse(&items, width);
        let painted = segment_rects(&layout, 0.0, 0.0)
            .into_iter()
            .map(|(_, seg)| seg.y + seg.height)
            .chain(trigger_rect(&layout, 0.0, 0.0).map(|tr| tr.y + tr.height))
            .fold(0.0_f32, f32::max);
        assert!(
            painted <= BAR_HEIGHT,
            "at {width}px something is painted below the bar's own height"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Focus
// ---------------------------------------------------------------------------

#[test]
fn a_hidden_crumb_is_not_a_tab_stop() {
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let members = navigable_members(&layout);
    let ids: Vec<String> = members.iter().map(|m| m.id.to_string()).collect();
    for index in layout.hidden_indices() {
        assert!(
            !ids.contains(&format!("{CRUMB_FOCUS_PREFIX}:{index}")),
            "hidden crumb {index} is still focusable"
        );
    }
}

#[test]
fn an_elided_trail_adds_exactly_one_stop_for_the_trigger() {
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let members = navigable_members(&layout);
    assert_eq!(members.len(), layout.inline_count() + 1);
    assert_eq!(members.last().unwrap().id, ellipsis_focus_id());
}

#[test]
fn a_full_trail_has_no_trigger_stop() {
    let items = trail(3);
    let layout = collapse(&items, 1_000.0);
    assert_eq!(navigable_members(&layout).len(), 3);
}

// ---------------------------------------------------------------------------
// 6. Colour
// ---------------------------------------------------------------------------

#[test]
fn every_resting_crumb_clears_the_text_band() {
    let palette = Palette::dark();
    for role in [CrumbRole::Root, CrumbRole::Middle, CrumbRole::Terminal] {
        assert!(
            crumb_meets_text(&palette, role),
            "{role:?} resting label fails UIF-TOK-006 rule 1"
        );
    }
}

#[test]
fn ancestors_are_muted_and_the_current_location_is_strong() {
    let palette = Palette::dark();
    assert_eq!(
        crumb_colour(&palette, CrumbRole::Root, rest()),
        palette.text_muted
    );
    assert_eq!(
        crumb_colour(&palette, CrumbRole::Middle, rest()),
        palette.text_muted
    );
    assert_eq!(
        crumb_colour(&palette, CrumbRole::Terminal, rest()),
        palette.text_strong
    );
}

#[test]
fn a_hovered_crumb_takes_the_ghost_columns_strong_label() {
    let palette = Palette::dark();
    let hovered = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };
    for role in [CrumbRole::Root, CrumbRole::Middle, CrumbRole::Terminal] {
        assert_eq!(crumb_colour(&palette, role, hovered), palette.text_strong);
        assert_eq!(crumb_fill(&palette, hovered), palette.surface_hover);
        assert!(
            contrast::meets_text_contrast(
                crumb_colour(&palette, role, hovered),
                palette.surface_hover
            ),
            "{role:?} hovered label fails on surface.hover"
        );
    }
}

#[test]
fn a_disabled_crumb_reads_disabled_whatever_its_role() {
    let palette = Palette::dark();
    let dead = VisualState {
        base: BaseState::Disabled,
        focused: false,
    };
    for role in [CrumbRole::Root, CrumbRole::Middle, CrumbRole::Terminal] {
        assert_eq!(crumb_colour(&palette, role, dead), palette.text_disabled);
        assert!(
            disabled_crumb_in_band(&palette, role),
            "{role:?} disabled label leaves the UIF-TOK-006 rule 2 band"
        );
    }
}

#[test]
fn the_separator_measures_where_the_spec_printed_it() {
    let palette = Palette::dark();
    let measured = separator_contrast(&palette);
    assert!(
        (measured - SEPARATOR_CONTRAST_SPEC).abs() < 0.01,
        "separator measures {measured:.4} against the spec's {SEPARATOR_CONTRAST_SPEC}"
    );
    assert!(separator_meets_text(&palette));
}

#[test]
fn the_trigger_glyph_clears_the_text_band_at_rest_and_on_hover() {
    let palette = Palette::dark();
    let hovered = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };
    assert!(contrast::meets_text_contrast(
        trigger_glyph_colour(&palette, rest()),
        bar_backdrop(&palette, rest())
    ));
    assert!(contrast::meets_text_contrast(
        trigger_glyph_colour(&palette, hovered),
        bar_backdrop(&palette, hovered)
    ));
    assert!(trigger_contrast(&palette, rest()) > 4.5);
}

// ---------------------------------------------------------------------------
// 6b. The host
// ---------------------------------------------------------------------------

#[test]
fn the_overflow_menu_holds_exactly_the_hidden_crumbs() {
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let menu = overflow_menu(&items, &layout);
    let keys: Vec<String> = menu
        .rows()
        .iter()
        .map(|row| row.key().to_string())
        .collect();
    let expected: Vec<String> = layout
        .hidden_indices()
        .into_iter()
        .map(|index| format!("c{index}"))
        .collect();
    assert_eq!(keys, expected);
}

#[test]
fn opening_a_trail_with_nothing_hidden_opens_nothing() {
    let items = trail(2);
    let layout = collapse(&items, 1_000.0);
    let mut host = BreadcrumbHost::new(ellipsis_focus_id());
    let mut focus = focus_with_trigger();
    let outcome = host.open_overflow(&items, &layout, &mut focus);
    assert!(outcome.emissions.is_empty());
    assert!(!host.is_menu_open());
}

#[test]
fn opening_the_overflow_moves_focus_into_the_menu() {
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let mut host = BreadcrumbHost::new(ellipsis_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_overflow(&items, &layout, &mut focus);
    assert!(host.is_menu_open());
    assert_ne!(
        focus.current().map(|id| id.to_string()),
        Some(ELLIPSIS_FOCUS_ID.to_string()),
        "the menu takes focus so its rows are reachable"
    );
}

#[test]
fn escape_closes_the_menu_and_returns_focus_to_the_trigger() {
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let mut host = BreadcrumbHost::new(ellipsis_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_overflow(&items, &layout, &mut focus);
    let _ = host.escape::<String>(&mut focus);
    assert!(!host.is_menu_open());
    assert_eq!(
        focus.current().map(|id| id.to_string()),
        Some(ELLIPSIS_FOCUS_ID.to_string()),
        "Escape must land the caret back on the […]"
    );
}

#[test]
fn selecting_a_row_navigates_to_that_crumb() {
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let mut host = BreadcrumbHost::new(ellipsis_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_overflow(&items, &layout, &mut focus);

    let outcome = host.menu_key(MenuKey::Enter, &items, &mut focus);
    let activated: Vec<String> = outcome
        .emissions
        .iter()
        .filter_map(|emission| match emission {
            BreadcrumbEmission::CrumbActivated(id) => Some(id.clone()),
            _ => None,
        })
        .collect();
    // The first navigable row is the first hidden crumb.
    assert_eq!(activated, vec!["c1".to_string()]);
}

#[test]
fn walking_down_and_activating_reaches_the_last_hidden_crumb() {
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let hidden = layout.hidden_indices();
    let mut host = BreadcrumbHost::new(ellipsis_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_overflow(&items, &layout, &mut focus);

    let last = hidden.len() - 1;
    for _ in 0..last {
        let _ = host.menu_key::<String>(MenuKey::Down, &items, &mut focus);
    }
    let outcome = host.menu_key(MenuKey::Enter, &items, &mut focus);
    let activated: Vec<String> = outcome
        .emissions
        .iter()
        .filter_map(|emission| match emission {
            BreadcrumbEmission::CrumbActivated(id) => Some(id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(activated, vec![format!("c{}", hidden[last])]);
}

#[test]
fn activating_an_inline_crumb_reports_its_identity_and_role() {
    let items = trail(4);
    let outcome = BreadcrumbHost::activate_crumb(&items, 3);
    match outcome.emissions.as_slice() {
        [BreadcrumbEmission::CrumbActivated(id)] => assert_eq!(id, "c3"),
        other => panic!("expected one activation, got {other:?}"),
    }
    assert_eq!(crumb_role(3, 4), CrumbRole::Terminal);
}

#[test]
fn activating_a_missing_crumb_is_a_noop() {
    let items = trail(2);
    let outcome = BreadcrumbHost::activate_crumb(&items, 9);
    assert!(outcome.emissions.is_empty());
}

#[test]
fn the_menu_is_rebuilt_from_live_items_so_a_label_change_shows() {
    let mut items = trail(4);
    let layout = collapse(&items, 90.0);
    let before: Vec<String> = overflow_menu(&items, &layout)
        .rows()
        .iter()
        .map(|row| row.label().to_string())
        .collect();
    items[1].label = "Renamed".to_string();
    let after: Vec<String> = overflow_menu(&items, &layout)
        .rows()
        .iter()
        .map(|row| row.label().to_string())
        .collect();
    assert_eq!(before[0], "Crumb 1");
    assert_eq!(after[0], "Renamed");
}

#[test]
fn opening_a_second_tree_in_the_same_cascade_retires_the_first() {
    // The host opens through `MenuCascade::open`, which closes whatever
    // tree was already showing. That is the mechanism the Band-2
    // single-flyout rule rests on: a foreign flyout sharing the cascade is
    // retired by the overflow opening, not by the overflow noticing.
    let items = trail(6);
    let layout = collapse(&items, 120.0);
    let mut host = BreadcrumbHost::new(ellipsis_focus_id());
    let mut focus = focus_with_trigger();

    let foreign = Menu::new("foreign.flyout", vec![Row::action("f0", "Foreign")]);
    {
        let cascade = host.cascade_mut();
        let _ = cascade.open(&foreign, MenuTreeKind::Fixed, &mut focus);
    }
    assert_eq!(
        host.cascade().root_id().map(|id| id.to_string()),
        Some("foreign.flyout".to_string())
    );

    let _ = host.open_overflow(&items, &layout, &mut focus);
    assert_eq!(
        host.cascade().root_id().map(|id| id.to_string()),
        Some(MENU_ID.to_string()),
        "the foreign flyout should have been retired"
    );
}
