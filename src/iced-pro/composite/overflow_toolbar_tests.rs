//! Tests for the responsive overflow toolbar — `UIF-COMP-008`.
//!
//! Six groups, in order:
//!
//! 1. **Declaration.** The eviction order is the rank order, and the
//!    tie-break is the declared one.
//! 2. **The solver.** Where the reserve bites, and what happens at the
//!    degenerate widths.
//! 3. **The solver's *properties*.** Purity and monotonicity.
//! 4. **Zero vertical shift.** The height cannot see the split.
//! 5. **State survives overflow.** Identity, enabled and `ActionPending`
//!    follow the item into the menu, in both postures.
//! 6. **Keyboard and the host.** Tab reaches the trigger, the menu roves,
//!    Escape returns, and a foreign Band-2 flyout is retired.

use super::*;
use crate::focus::GroupId;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The canonical five-item strip.
///
/// Ranks are deliberately not monotone along the strip: the most
/// expendable item sits in the middle and the two ends are the most
/// essential, so a solver that evicted by position would disagree with
/// one that evicted by declaration on every width.
fn strip() -> Vec<OverflowItem<String>> {
    vec![
        OverflowItem::new(
            "a".to_string(),
            "Alpha".to_string(),
            40.0,
            OverflowRank::PRIMARY,
        ),
        OverflowItem::new(
            "b".to_string(),
            "Bravo".to_string(),
            40.0,
            OverflowRank::SECONDARY,
        ),
        OverflowItem::new(
            "c".to_string(),
            "Charlie".to_string(),
            40.0,
            OverflowRank::DISPOSABLE,
        ),
        OverflowItem::new(
            "d".to_string(),
            "Delta".to_string(),
            40.0,
            OverflowRank::SECONDARY,
        ),
        OverflowItem::new(
            "e".to_string(),
            "Echo".to_string(),
            40.0,
            OverflowRank::PRIMARY,
        ),
    ]
}

/// The width the canonical strip needs fully inline.
fn full_width() -> f32 {
    5.0 * 40.0 + 4.0 * ITEM_GAP
}

/// A focus state with the trigger registered.
fn focus_with_trigger() -> FocusState {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new("bar"),
        vec![FocusMember::new(trigger_focus_id())],
    );
    focus
}

/// The ids behind the trigger, for a layout over `items`.
fn overflow_ids(items: &[OverflowItem<String>], width: f32) -> Vec<String> {
    overflow_menu(items, &fit(items, width))
        .rows()
        .iter()
        .map(|row| row.key().to_string())
        .collect()
}

/// The rest state, spelled out once.
fn rest() -> VisualState {
    VisualState {
        base: BaseState::Rest,
        focused: false,
    }
}

// ---------------------------------------------------------------------------
// 1. Declaration
// ---------------------------------------------------------------------------

#[test]
fn the_reserve_is_the_trigger_plus_its_clearance() {
    assert_eq!(TRIGGER_RESERVE, 36.0);
    assert_eq!(TRIGGER_SIZE, 28.0);
}

#[test]
fn eviction_follows_declared_rank_not_layout_position() {
    let items = strip();
    let order = eviction_order(&items);
    // DISPOSABLE first, then the SECONDARY pair rightmost-first, then the
    // PRIMARY pair rightmost-first.
    assert_eq!(order, vec![2, 3, 1, 4, 0]);
    // Index 0 is the very last thing to leave, despite sitting at the
    // front of the strip. Position had nothing to say about it.
    assert_eq!(order.last(), Some(&0));
    // And the strip's front is not its first casualty either.
    assert_ne!(order.first(), Some(&0));
}

#[test]
fn equal_ranks_evict_rightmost_first() {
    let items = vec![
        OverflowItem::new(
            "l".to_string(),
            "L".to_string(),
            40.0,
            OverflowRank::SECONDARY,
        ),
        OverflowItem::new(
            "m".to_string(),
            "M".to_string(),
            40.0,
            OverflowRank::SECONDARY,
        ),
        OverflowItem::new(
            "r".to_string(),
            "R".to_string(),
            40.0,
            OverflowRank::SECONDARY,
        ),
    ];
    assert_eq!(eviction_order(&items), vec![2, 1, 0]);
}

#[test]
fn moving_an_item_along_the_strip_does_not_change_its_eviction_rank() {
    let before = strip();
    let mut after = strip();
    // Swap the two PRIMARY ends. Their relative eviction order is still
    // rightmost-first, and neither moves ahead of the DISPOSABLE item.
    after.swap(0, 4);
    let before_order = eviction_order(&before);
    let after_order = eviction_order(&after);
    assert_eq!(before_order.first(), after_order.first());
    assert_eq!(
        before_order
            .iter()
            .filter(|i| before[**i].rank == OverflowRank::PRIMARY)
            .count(),
        after_order
            .iter()
            .filter(|i| after[**i].rank == OverflowRank::PRIMARY)
            .count()
    );
    // The disposable item is still first out in both.
    assert_eq!(before_order[0], 2);
    assert_eq!(after_order[0], 2);
}

// ---------------------------------------------------------------------------
// 2. The solver
// ---------------------------------------------------------------------------

#[test]
fn an_empty_strip_lays_out_as_nothing() {
    let layout = fit::<String>(&[], 100.0);
    assert!(layout.plans().is_empty());
    assert_eq!(layout.overflow_count(), 0);
    assert_eq!(layout.trigger_width(), 0.0);
    assert!(layout.fits());
}

#[test]
fn a_strip_that_fits_keeps_everything_inline() {
    let items = strip();
    let layout = fit(&items, full_width());
    assert!(layout.fits());
    assert_eq!(layout.inline_count(), 5);
    assert_eq!(layout.trigger_width(), 0.0);
}

#[test]
fn the_reserve_bites_one_pixel_past_the_fit() {
    let items = strip();
    let layout = fit(&items, full_width() - 1.0);
    assert!(layout.is_overflowing());
    // The most expendable item is the one that goes.
    assert_eq!(layout.overflow_indices(), vec![2]);
    assert!(
        layout.total_width() <= full_width(),
        "the strip with its trigger must still fit the width that forced eviction"
    );
}

#[test]
fn eviction_proceeds_through_the_declared_order_as_width_shrinks() {
    // Inline widths as items leave: 5 → 216, 4 → 172, 3 → 128, 2 → 84,
    // 1 → 40. The strip needs `inline <= available - 36`.
    let items = strip();
    assert_eq!(fit(&items, full_width()).overflow_indices(), vec![]);
    assert_eq!(fit(&items, 215.0).overflow_indices(), vec![2]);
    assert_eq!(fit(&items, 180.0).overflow_indices(), vec![2, 3]);
    assert_eq!(fit(&items, 140.0).overflow_indices(), vec![1, 2, 3]);
    assert_eq!(fit(&items, 100.0).overflow_indices(), vec![1, 2, 3, 4]);
}

#[test]
fn a_strip_too_narrow_for_anything_still_offers_the_trigger() {
    let items = strip();
    let layout = fit(&items, 10.0);
    assert_eq!(layout.overflow_count(), 5);
    assert_eq!(layout.inline_count(), 0);
    assert_eq!(layout.trigger_width(), TRIGGER_SIZE);
    // Nothing is unreachable: the menu holds all five.
    assert_eq!(overflow_menu(&items, &layout).rows().len(), 5);
}

#[test]
fn a_zero_width_strip_still_terminates() {
    let items = strip();
    let layout = fit(&items, 0.0);
    assert_eq!(layout.overflow_count(), 5);
}

#[test]
fn a_measure_below_the_floor_is_clamped_not_collapsed() {
    let items = vec![
        OverflowItem::new(
            "x".to_string(),
            "X".to_string(),
            0.0,
            OverflowRank::SECONDARY,
        ),
        OverflowItem::new(
            "y".to_string(),
            "Y".to_string(),
            -50.0,
            OverflowRank::SECONDARY,
        ),
    ];
    let layout = fit(&items, 1_000.0);
    for plan in layout.plans() {
        assert_eq!(
            plan.width, MIN_ITEM_WIDTH,
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
    let items = strip();
    assert_eq!(fit(&items, 137.0), fit(&items, 137.0));
}

#[test]
fn widening_never_swaps_one_overflow_set_for_another() {
    let items = strip();
    let mut previous: Vec<usize> = Vec::new();
    let mut first = true;
    for width in 0..=260 {
        let overflow = fit(&items, width as f32).overflow_indices();
        if first {
            previous = overflow;
            first = false;
            continue;
        }
        for index in &overflow {
            assert!(
                previous.contains(index),
                "widening to {width} put {index} back inline while {} is still overflowed",
                previous.len()
            );
        }
        assert!(
            overflow.len() <= previous.len(),
            "overflow set grew when widening to {width}"
        );
        previous = overflow;
    }
}

#[test]
fn the_overflow_set_is_a_prefix_of_the_eviction_order() {
    // `overflow_indices` reports declaration order, so the prefix claim is
    // about the *set*: what is overflowed is exactly the first `n` of the
    // eviction order, for however many `n` the width demanded.
    let items = strip();
    let order = eviction_order(&items);
    for width in 0..=260 {
        let mut overflow = fit(&items, width as f32).overflow_indices();
        overflow.sort_unstable();
        let mut expected: Vec<usize> = order.iter().take(overflow.len()).copied().collect();
        expected.sort_unstable();
        assert_eq!(
            overflow, expected,
            "not a prefix of the eviction order at {width}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Zero vertical shift
// ---------------------------------------------------------------------------

#[test]
fn the_height_function_cannot_see_the_split() {
    // `strip_height` takes no arguments. There is nothing to assert about
    // a value that cannot vary; the signature is the proof. This test
    // exists to make the absence visible at the call site.
    assert_eq!(strip_height(), STRIP_HEIGHT);
}

#[test]
fn every_painted_element_is_at_most_the_strip_height_tall() {
    let items = strip();
    for width in [0.0, 60.0, 120.0, 180.0, full_width(), 1_000.0] {
        let layout = fit(&items, width);
        for (_, bounds) in item_rects(&layout, 0.0, 0.0) {
            assert!(
                bounds.height <= STRIP_HEIGHT,
                "inline item overflows the strip"
            );
        }
        if let Some(trigger) = trigger_rect(&layout, 0.0, 0.0) {
            assert!(
                trigger.height <= STRIP_HEIGHT,
                "trigger overflows the strip"
            );
        }
    }
}

#[test]
fn the_strip_height_is_identical_across_every_eviction_count() {
    // `strip_height()` is nullary, so nothing here can move it - and that
    // is the claim. The widths below sweep the strip from "everything
    // inline" to "only the last item standing" so the eviction count
    // really does vary while the height provably cannot.
    let items = strip();
    let heights: Vec<f32> = (0..6)
        .map(|step| {
            let layout = fit(&items, 40.0 + step as f32 * 56.0);
            assert_eq!(
                layout.overflow_count(),
                items.len() - layout.inline_count(),
                "step {step}: placement does not partition the items"
            );
            strip_height()
        })
        .collect();
    assert!(heights.iter().all(|h| *h == STRIP_HEIGHT));
    assert_eq!(heights.len(), 6);
}

#[test]
fn the_trigger_is_centred_in_the_strip_rather_than_growing_it() {
    let items = strip();
    let layout = fit(&items, 120.0);
    let trigger = trigger_rect(&layout, 0.0, 100.0).expect("overflowing strip has a trigger");
    assert_eq!(trigger.y, 100.0 + (STRIP_HEIGHT - TRIGGER_SIZE) / 2.0);
}

// ---------------------------------------------------------------------------
// 5. State survives overflow
// ---------------------------------------------------------------------------

#[test]
fn an_overflowed_item_keeps_its_identity() {
    let items = strip();
    let layout = fit(&items, 140.0);
    let keys = overflow_ids(&items, 140.0);
    for index in layout.overflow_indices() {
        assert!(
            keys.contains(&items[index].id),
            "item {} lost its identity in the menu",
            items[index].id
        );
    }
}

#[test]
fn an_overflowed_disabled_item_stays_disabled_in_the_menu() {
    let mut items = strip();
    items[2].enabled = false;
    let menu = overflow_menu(&items, &fit(&items, 180.0));
    let row = menu
        .rows()
        .iter()
        .find(|row| row.key() == "c")
        .expect("c overflows at this width");
    assert!(row.is_disabled());
    assert!(!row.navigable());
}

#[test]
fn a_busy_item_is_reachable_in_the_menu_rather_than_greyed_out() {
    let mut items = strip();
    items[2].pending = ActionPending::indeterminate().with_cancellable(true);
    let menu = overflow_menu(&items, &fit(&items, 180.0));
    let row = menu
        .rows()
        .iter()
        .find(|row| row.key() == "c")
        .expect("c overflows at this width");
    // A disabled row would be dropped from the focus roster entirely, so
    // busy must not be expressed as disabled.
    assert!(row.is_enabled(), "a busy item must stay navigable");
    assert!(row.navigable());
    assert!(row_tracks_action(&items[2].pending));
}

#[test]
fn a_busy_item_keeps_the_same_state_in_both_postures() {
    let mut items = strip();
    let busy = ActionPending::determinate(0.5).with_cancellable(true);
    items[2].pending = busy;

    let inline = fit(&items, 1_000.0);
    let overflow = fit(&items, 120.0);

    assert!(inline.plan(2).unwrap().is_inline());
    assert!(overflow.plan(2).unwrap().is_overflow());
    // Placement is the only thing that changed.
    assert_eq!(items[2].pending, busy);
    assert!(items[2].enabled);
    assert_eq!(items[2].label, "Charlie");
}

#[test]
fn activating_a_busy_overflowed_item_asks_it_to_stop_instead_of_firing_again() {
    let mut items = strip();
    items[2].pending = ActionPending::indeterminate().with_cancellable(true);
    let layout = fit(&items, 180.0);

    let mut host = OverflowToolbarHost::new(trigger_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_menu(&items, &layout, &mut focus);
    let outcome = host.menu_key(MenuKey::Enter, &items, &mut focus);

    let fired = outcome
        .emissions
        .iter()
        .any(|emission| matches!(emission, ToolbarEmission::Activated(_)));
    let stopped = outcome
        .emissions
        .iter()
        .any(|emission| matches!(emission, ToolbarEmission::StopRequested(_)));
    assert!(!fired, "a busy item must not dispatch a second time");
    assert!(stopped, "the second press should read as a request to stop");
}

#[test]
fn activating_an_idle_overflowed_item_dispatches_once() {
    let items = strip();
    let layout = fit(&items, 180.0);
    let mut host = OverflowToolbarHost::new(trigger_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_menu(&items, &layout, &mut focus);
    let outcome = host.menu_key(MenuKey::Enter, &items, &mut focus);
    let activated: Vec<&RowSettlement> = outcome
        .emissions
        .iter()
        .filter_map(|emission| match emission {
            ToolbarEmission::Activated(commit) => Some(commit.value()),
            _ => None,
        })
        .collect();
    assert_eq!(activated.len(), 1);
    assert_eq!(activated[0].row_key, "c");
    assert!(activated[0].dispatched);
}

#[test]
fn the_menu_reflects_item_state_on_each_open() {
    let mut items = strip();
    let layout = fit(&items, 180.0);

    let first = overflow_menu(&items, &layout);
    assert!(
        first
            .rows()
            .iter()
            .find(|r| r.key() == "c")
            .unwrap()
            .is_enabled()
    );

    items[2].enabled = false;
    items[2].label = "Charlie (off)".to_string();

    let second = overflow_menu(&items, &layout);
    let row = second.rows().iter().find(|r| r.key() == "c").unwrap();
    assert!(row.is_disabled());
    assert_eq!(row.label(), "Charlie (off)");
}

// ---------------------------------------------------------------------------
// 6. Keyboard and the host
// ---------------------------------------------------------------------------

#[test]
fn tab_reaches_the_inline_items_and_the_trigger() {
    let items = strip();
    let layout = fit(&items, 140.0);
    let members = navigable_members(&layout);
    let ids: Vec<String> = members.iter().map(|m| m.id.to_string()).collect();
    assert!(ids.contains(&TRIGGER_FOCUS_ID.to_string()));
    assert_eq!(members.len(), layout.inline_count() + 1);
}

#[test]
fn a_strip_that_fits_has_no_trigger_stop() {
    let items = strip();
    let layout = fit(&items, 1_000.0);
    let ids: Vec<String> = navigable_members(&layout)
        .iter()
        .map(|m| m.id.to_string())
        .collect();
    assert!(!ids.contains(&TRIGGER_FOCUS_ID.to_string()));
}

#[test]
fn opening_a_strip_with_nothing_overflowed_opens_nothing() {
    let items = strip();
    let layout = fit(&items, 1_000.0);
    let mut host = OverflowToolbarHost::new(trigger_focus_id());
    let mut focus = focus_with_trigger();
    let outcome = host.open_menu(&items, &layout, &mut focus);
    assert!(outcome.emissions.is_empty());
    assert!(!host.is_menu_open());
}

#[test]
fn escape_closes_the_menu_and_returns_focus_to_the_trigger() {
    let items = strip();
    let layout = fit(&items, 120.0);
    let mut host = OverflowToolbarHost::new(trigger_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_menu(&items, &layout, &mut focus);
    assert_ne!(
        focus.current().map(|id| id.to_string()),
        Some(TRIGGER_FOCUS_ID.to_string())
    );

    let _ = host.escape::<String>(&mut focus);
    assert!(!host.is_menu_open());
    assert_eq!(
        focus.current().map(|id| id.to_string()),
        Some(TRIGGER_FOCUS_ID.to_string()),
        "Escape must land the caret back on the ⋯"
    );
}

#[test]
fn the_menu_roves_over_its_rows_in_order() {
    let items = strip();
    let layout = fit(&items, 100.0);
    let expected = layout.overflow_indices();
    let mut host = OverflowToolbarHost::new(trigger_focus_id());
    let mut focus = focus_with_trigger();
    let _ = host.open_menu(&items, &layout, &mut focus);

    assert_eq!(host.item_for_row(0), Some(expected[0]));
    let _ = host.menu_key::<String>(MenuKey::Down, &items, &mut focus);
    let _ = host.menu_key::<String>(MenuKey::Down, &items, &mut focus);
    // Down twice from the first row lands on the third overflowed item.
    let outcome = host.menu_key(MenuKey::Enter, &items, &mut focus);
    let activated: Vec<&RowSettlement> = outcome
        .emissions
        .iter()
        .filter_map(|emission| match emission {
            ToolbarEmission::Activated(commit) => Some(commit.value()),
            _ => None,
        })
        .collect();
    assert_eq!(activated.len(), 1);
    assert_eq!(activated[0].row_key, items[expected[2]].id);
}

#[test]
fn a_disabled_overflowed_item_is_stepped_over() {
    let mut items = strip();
    items[2].enabled = false;
    let layout = fit(&items, 100.0);
    let menu = overflow_menu(&items, &layout);
    assert_eq!(menu.navigable_indices().len(), menu.rows().len() - 1);
}

#[test]
fn opening_the_overflow_retires_a_foreign_band_two_flyout() {
    // The host opens through `MenuCascade::open`, which closes whatever
    // tree was already showing. That is the mechanism the Band-2
    // single-flyout rule rests on: a foreign flyout sharing the cascade
    // is retired by the overflow opening, not by the overflow noticing.
    let items = strip();
    let layout = fit(&items, 120.0);
    let mut host = OverflowToolbarHost::new(trigger_focus_id());
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

    let _ = host.open_menu(&items, &layout, &mut focus);
    assert_eq!(
        host.cascade().root_id().map(|id| id.to_string()),
        Some(MENU_ID.to_string()),
        "the foreign flyout should have been retired"
    );
}

#[test]
fn the_trigger_reports_its_pointer_posture() {
    let mut host = OverflowToolbarHost::new(trigger_focus_id());
    assert_eq!(host.trigger_state(false).base, BaseState::Rest);
    host.hover_trigger();
    assert_eq!(host.trigger_state(false).base, BaseState::Hovered);
    host.press_trigger();
    assert_eq!(host.trigger_state(false).base, BaseState::Pressed);
    host.reset();
    assert_eq!(host.trigger_state(false).base, BaseState::Rest);
    assert!(host.trigger_state(true).focused);
}

#[test]
fn releasing_the_trigger_toggles_the_menu() {
    let items = strip();
    let layout = fit(&items, 120.0);
    let mut host = OverflowToolbarHost::new(trigger_focus_id());
    let mut focus = focus_with_trigger();

    host.press_trigger();
    let _ = host.release_trigger(&items, &layout, &mut focus);
    assert!(host.is_menu_open());
    let _ = host.release_trigger(&items, &layout, &mut focus);
    assert!(!host.is_menu_open());
}

#[test]
fn the_trigger_glyph_clears_the_text_band_in_its_attended_states() {
    let palette = Palette::dark();
    let hovered = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };
    let pressed = VisualState {
        base: BaseState::Pressed,
        focused: false,
    };
    assert!(contrast::meets_text_contrast(
        trigger_glyph_colour(&palette, rest()),
        trigger_backdrop(&palette, rest())
    ));
    assert!(contrast::meets_text_contrast(
        trigger_glyph_colour(&palette, hovered),
        trigger_backdrop(&palette, hovered)
    ));
    assert!(contrast::meets_text_contrast(
        trigger_glyph_colour(&palette, pressed),
        trigger_backdrop(&palette, pressed)
    ));
    assert!(trigger_contrast(&palette, rest()) > 4.5);
}

#[test]
fn a_disabled_item_in_the_menu_sits_in_the_muted_band() {
    let palette = Palette::dark();
    assert!(disabled_row_in_band(&palette, palette.surface_overlay));
    assert!(disabled_row_in_band(&palette, palette.surface_hover));
}
