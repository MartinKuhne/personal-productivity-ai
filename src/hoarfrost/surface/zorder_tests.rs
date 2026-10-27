//! Tests for [`zorder`] — the four z-bands, pointer routing and strict
//! `Escape` unwinding (`UIF-ZORD-*`, `UIF-WIN-003`, `UIF-VER-003`).
//!
//! Seven groups, in order:
//!
//! 1. **The band table.** Every column of §5 — backdrop, pointer rule,
//!    blocking, promotion, keyboard disposition, escape rank — asserted
//!    per band against the spec rather than inferred from behaviour.
//! 2. **The registry** — band ordering under arbitrary insertion order,
//!    topmost resolution, and what re-registering does and must not do to
//!    the stacking order.
//! 3. **Z-promotion** — Band 1 reorders within Band 1 and nothing else.
//! 4. **Pointer routing** — the pass-through rules, blocking, and a grid
//!    sweep over three stacked scenarios: Bands 0+1 only, Bands 0+1+2,
//!    and all four.
//! 5. **Escape unwinding** — the delegation chain, including a drive
//!    through a real [`MenuCascade`] so the seam `INC-16` left is
//!    exercised rather than simulated.
//! 6. **Scrim composition and determinism.**
//! 7. **The Pre-Rank composition** — `UIF-WIN-003` step 1 running ahead
//!    of the walk above, including that an uncaptured chain is identical
//!    to the plain walk in groups 5 and 6.
//!
//! # Why the routing sweep is split into three scenarios
//!
//! The bands are not peers at a click. A Band 2 menu swallows every
//! outside click as a dismissal, and a Band 3 scrim swallows every point
//! at all, so "which surface gets (x, y)" has a different answer set for
//! each combination of open bands. Running one sweep over every band open
//! at once would only ever produce "the card or the scrim" and would hide
//! the Band 1 pass-through entirely. Each scenario therefore sweeps the
//! grid against the answer that scenario must produce.

use std::cell::Cell;

use super::*;
use crate::binding::pre_rank::CaptureId;
use crate::binding::pre_rank::DragKind;
use crate::focus::FocusState;
use crate::surface::menu::cascade::MenuCascade;
use crate::surface::menu::cascade::MenuTreeKind;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::MenuId;
use crate::surface::menu::model::Row;

use iced::Size;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(pt(x, y), Size::new(w, h))
}

fn sid(name: &str) -> SurfaceId {
    SurfaceId::new(name)
}

/// Register one surface, discarding the registered surface's reference.
fn add(reg: &mut SurfaceRegistry, name: &str, kind: SurfaceKind, regions: Vec<Rectangle>) {
    reg.register(sid(name), kind, regions);
}

/// The registry's contents, bottom to top, as names.
fn names(reg: &SurfaceRegistry) -> Vec<String> {
    reg.surfaces()
        .iter()
        .map(|surface| surface.id().as_str().to_owned())
        .collect()
}

/// The names in one band, bottom to top.
fn names_in(reg: &SurfaceRegistry, band: ZBand) -> Vec<String> {
    reg.surfaces_in(band)
        .map(|surface| surface.id().as_str().to_owned())
        .collect()
}

/// A responder that dismisses whatever it is asked about.
fn dismiss_everything() -> impl FnMut(&Surface) -> UnwindAnswer {
    |_| UnwindAnswer::Dismissed
}

/// A responder driven by a per-surface script; anything not listed
/// declines with [`DeclineReason::CannotUnwind`].
fn scripted(answers: Vec<(&'static str, UnwindAnswer)>) -> impl FnMut(&Surface) -> UnwindAnswer {
    move |surface: &Surface| {
        answers
            .iter()
            .find(|(name, _)| *name == surface.id().as_str())
            .map_or(
                UnwindAnswer::Declined(DeclineReason::CannotUnwind),
                |(_, answer)| *answer,
            )
    }
}

/// Drain the escape chain until it stops consuming, capped so a bug that
/// never terminates fails instead of hanging. The final element is the
/// `None` that says nothing more would be consumed.
fn drain(
    reg: &mut SurfaceRegistry,
    cap: usize,
    mut responder: impl FnMut(&Surface) -> UnwindAnswer,
) -> Vec<Option<String>> {
    let mut consumed = Vec::new();
    for _ in 0..cap {
        let outcome = reg.escape(&mut responder);
        let hit = outcome.consumed_by().map(|id| id.as_str().to_owned());
        let stop = !outcome.is_consumed();
        consumed.push(hit);
        if stop {
            break;
        }
    }
    consumed
}

/// A menu tree three levels deep: root → submenu → submenu.
fn deep_menu() -> Menu {
    Menu::new(
        "root",
        vec![
            Row::action("first", "First"),
            Row::submenu(
                "sub",
                "Submenu",
                Menu::new(
                    "sub",
                    vec![
                        Row::action("second", "Second"),
                        Row::submenu(
                            "deep",
                            "Deeper",
                            Menu::new("deep", vec![Row::action("third", "Third")]),
                        ),
                    ],
                ),
            ),
        ],
    )
}

/// Open [`deep_menu`] three levels deep and return the cascade with focus.
fn open_three_levels() -> (MenuCascade, FocusState) {
    let tree = deep_menu();
    let mut focus = FocusState::new();
    let mut cascade = MenuCascade::new();
    cascade.open(&tree, MenuTreeKind::Context, &mut focus);
    cascade.move_focus(1, &mut focus);
    cascade.open_focused_submenu(&mut focus);
    cascade.move_focus(1, &mut focus);
    cascade.open_focused_submenu(&mut focus);
    assert_eq!(cascade.depth(), 3, "fixture must be three cards deep");
    (cascade, focus)
}

/// Bands 0 and 1 only: the workspace fills the window, a filter sits in
/// the top-left, and two movable cards sit side by side in the lower half.
fn base_registry() -> SurfaceRegistry {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "workspace",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 1000., 700.)],
    );
    add(
        &mut reg,
        "filter",
        SurfaceKind::SearchFilter,
        vec![rect(0., 0., 200., 24.)],
    );
    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(10., 400., 300., 200.)],
    );
    add(
        &mut reg,
        "debug",
        SurfaceKind::MovableCard,
        vec![rect(400., 400., 300., 200.)],
    );
    reg
}

/// [`base_registry`] plus a Band 2 menu card in the upper left.
fn menu_registry() -> SurfaceRegistry {
    let mut reg = base_registry();
    add(
        &mut reg,
        "menu",
        SurfaceKind::MenuCard,
        vec![rect(20., 40., 180., 160.)],
    );
    reg
}

/// [`menu_registry`] plus a Band 3 modal in the middle — one surface in
/// every band, plus a second Band 1 card.
fn full_registry() -> SurfaceRegistry {
    let mut reg = menu_registry();
    add(
        &mut reg,
        "modal",
        SurfaceKind::Modal,
        vec![rect(400., 250., 200., 160.)],
    );
    reg
}

/// The escape order `full_registry` must produce: menu band first, then
/// modal, then Band 1 top-down, then Band 0 top-down.
const FULL_ESCAPE_ORDER: [&str; 6] = ["menu", "modal", "debug", "logs", "filter", "workspace"];

// ---------------------------------------------------------------------------
// 1. The band table
// ---------------------------------------------------------------------------

#[test]
fn test_band_index_matches_the_spec_band_numbers() {
    assert_eq!(ZBand::Workspace.index(), 0);
    assert_eq!(ZBand::UtilityOverlay.index(), 1);
    assert_eq!(ZBand::Menu.index(), 2);
    assert_eq!(ZBand::Modal.index(), 3);
}

#[test]
fn test_band_all_is_ordered_bottom_to_top() {
    assert_eq!(
        ZBand::ALL,
        [
            ZBand::Workspace,
            ZBand::UtilityOverlay,
            ZBand::Menu,
            ZBand::Modal
        ]
    );
    assert!(ZBand::Workspace < ZBand::UtilityOverlay);
    assert!(ZBand::UtilityOverlay < ZBand::Menu);
    assert!(ZBand::Menu < ZBand::Modal);
}

#[test]
fn test_backdrop_table_matches_spec_section_5() {
    assert_eq!(ZBand::Workspace.backdrop(), Backdrop::None);
    assert_eq!(ZBand::UtilityOverlay.backdrop(), Backdrop::None);
    assert_eq!(
        ZBand::Menu.backdrop(),
        Backdrop::TransparentCatcher,
        "Band 2 is the full-window transparent click-away catcher"
    );
    assert_eq!(ZBand::Modal.backdrop(), Backdrop::DimmedScrim);
}

#[test]
fn test_pointer_rule_table_matches_spec_section_5() {
    assert_eq!(ZBand::Workspace.pointer_rule(), PointerRule::Direct);
    assert_eq!(
        ZBand::UtilityOverlay.pointer_rule(),
        PointerRule::CaptureInsidePassOutside
    );
    assert_eq!(
        ZBand::Menu.pointer_rule(),
        PointerRule::CaptureInsideDismissOutside
    );
    assert_eq!(
        ZBand::Modal.pointer_rule(),
        PointerRule::CaptureInsideAbsorbOutside
    );
}

#[test]
fn test_only_band_3_blocks_lower_bands() {
    assert!(!ZBand::Workspace.blocks_lower_bands());
    assert!(!ZBand::UtilityOverlay.blocks_lower_bands());
    assert!(
        !ZBand::Menu.blocks_lower_bands(),
        "Band 2 swallows the outside click by dismissing itself, not by \
         standing between the pointer and everything below"
    );
    assert!(ZBand::Modal.blocks_lower_bands());
}

#[test]
fn test_only_band_1_promotes_on_click() {
    let promoting: Vec<ZBand> = ZBand::ALL
        .into_iter()
        .filter(|band| band.promotes_on_click())
        .collect();
    assert_eq!(promoting, vec![ZBand::UtilityOverlay]);
}

#[test]
fn test_escape_rank_puts_the_menu_band_above_the_modal_band() {
    assert!(
        ZBand::Menu.escape_rank() > ZBand::Modal.escape_rank(),
        "UIF-WIN-003 unwinds menus before dialogs, which is the reverse \
         of the elevation in ZBand"
    );
}

#[test]
fn test_escape_rank_order_is_menu_modal_utility_workspace() {
    let mut ordered = ZBand::ALL.to_vec();
    ordered.sort_by_key(|band| std::cmp::Reverse(band.escape_rank()));
    assert_eq!(
        ordered,
        vec![
            ZBand::Menu,
            ZBand::Modal,
            ZBand::UtilityOverlay,
            ZBand::Workspace
        ]
    );
}

#[test]
fn test_keyboard_disposition_full_table() {
    use KeyClass::{ArrowNavigation, Enter, Escape, ShiftTab, Space, Tab};
    use KeyDisposition::{PassThrough, Swallow, SwallowWhenFocused};

    let table: [(ZBand, [(KeyClass, KeyDisposition); 6]); 4] = [
        (
            ZBand::Workspace,
            [
                (Tab, Swallow),
                (ShiftTab, Swallow),
                (ArrowNavigation, PassThrough),
                (Enter, PassThrough),
                (Space, PassThrough),
                (Escape, PassThrough),
            ],
        ),
        (
            ZBand::UtilityOverlay,
            [
                (Tab, PassThrough),
                (ShiftTab, PassThrough),
                (ArrowNavigation, PassThrough),
                (Enter, PassThrough),
                (Space, PassThrough),
                (Escape, SwallowWhenFocused),
            ],
        ),
        (
            ZBand::Menu,
            [
                (Tab, Swallow),
                (ShiftTab, Swallow),
                (ArrowNavigation, Swallow),
                (Enter, Swallow),
                (Space, Swallow),
                (Escape, Swallow),
            ],
        ),
        (
            ZBand::Modal,
            [
                (Tab, Swallow),
                (ShiftTab, Swallow),
                (ArrowNavigation, PassThrough),
                (Enter, Swallow),
                (Space, PassThrough),
                (Escape, Swallow),
            ],
        ),
    ];

    for (band, rows) in table {
        assert_eq!(rows.len(), KeyClass::ALL.len());
        for class in KeyClass::ALL {
            let expected = rows
                .iter()
                .find(|(row_class, _)| *row_class == class)
                .map(|(_, disposition)| *disposition);
            let expected = expected.unwrap_or_else(|| panic!("{band:?} has no row for {class:?}"));
            assert_eq!(
                band.key_disposition(class),
                expected,
                "{band:?} / {class:?}"
            );
        }
    }
}

#[test]
fn test_swallowed_keys_per_band() {
    use KeyClass::{ArrowNavigation, Enter, Escape, ShiftTab, Space, Tab};

    assert_eq!(
        ZBand::Workspace.swallowed_keys(),
        vec![Tab, ShiftTab],
        "Band 0 swallows Tab because traversal *is* its keyboard routing"
    );
    assert!(
        ZBand::UtilityOverlay.swallowed_keys().is_empty(),
        "Band 1's only claim is conditional and must not be reported as unconditional"
    );
    assert_eq!(
        ZBand::Menu.swallowed_keys(),
        vec![Tab, ShiftTab, ArrowNavigation, Enter, Space, Escape]
    );
    assert_eq!(
        ZBand::Modal.swallowed_keys(),
        vec![Tab, ShiftTab, Enter, Escape]
    );
}

#[test]
fn test_swallow_when_focused_counts_as_swallowed_but_not_unconditionally() {
    let disposition = ZBand::UtilityOverlay.key_disposition(KeyClass::Escape);
    assert_eq!(disposition, KeyDisposition::SwallowWhenFocused);
    assert!(disposition.swallowed());
    assert!(
        !ZBand::UtilityOverlay
            .swallowed_keys()
            .contains(&KeyClass::Escape)
    );
}

#[test]
fn test_pass_through_is_not_swallowed() {
    for band in ZBand::ALL {
        for class in KeyClass::ALL {
            let disposition = band.key_disposition(class);
            assert_eq!(
                disposition.swallowed(),
                matches!(
                    disposition,
                    KeyDisposition::Swallow | KeyDisposition::SwallowWhenFocused
                ),
                "{band:?} / {class:?}"
            );
        }
    }
}

#[test]
fn test_key_class_all_covers_the_six_classes_the_table_names() {
    assert_eq!(KeyClass::ALL.len(), 6);
    for class in KeyClass::ALL {
        let hits = KeyClass::ALL
            .iter()
            .filter(|other| **other == class)
            .count();
        assert_eq!(
            hits, 1,
            "{class:?} must appear exactly once in KeyClass::ALL"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Kind mapping and the registry
// ---------------------------------------------------------------------------

#[test]
fn test_surface_kind_band_mapping() {
    assert_eq!(SurfaceKind::Workspace.band(), ZBand::Workspace);
    assert_eq!(SurfaceKind::SearchFilter.band(), ZBand::Workspace);
    assert_eq!(SurfaceKind::MovableCard.band(), ZBand::UtilityOverlay);
    assert_eq!(SurfaceKind::MenuCard.band(), ZBand::Menu);
    assert_eq!(SurfaceKind::Modal.band(), ZBand::Modal);
    assert_eq!(SurfaceKind::InputDialog.band(), ZBand::Modal);
}

#[test]
fn test_a_surface_reports_its_kind_and_band() {
    let surface = Surface::new(
        sid("x"),
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 10., 10.)],
    );
    assert_eq!(surface.id().as_str(), "x");
    assert_eq!(surface.kind(), SurfaceKind::MovableCard);
    assert_eq!(surface.band(), ZBand::UtilityOverlay);
    assert_eq!(surface.regions().len(), 1);
    assert!(surface.has_regions());
}

#[test]
fn test_empty_registry_is_empty_and_has_no_topmost() {
    let reg = SurfaceRegistry::new();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
    assert!(reg.topmost().is_none());
    assert!(reg.highest_open_band().is_none());
    assert!(reg.keyboard_owner().is_none());
    for band in ZBand::ALL {
        assert!(reg.band_is_empty(band));
        assert_eq!(reg.count_in(band), 0);
    }
}

#[test]
fn test_register_keeps_band_order_regardless_of_insertion_order() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "modal",
        SurfaceKind::Modal,
        vec![rect(0., 0., 10., 10.)],
    );
    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 10., 10.)],
    );
    add(
        &mut reg,
        "workspace",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 10., 10.)],
    );
    add(
        &mut reg,
        "menu",
        SurfaceKind::MenuCard,
        vec![rect(0., 0., 10., 10.)],
    );

    assert_eq!(
        names(&reg),
        vec!["workspace", "logs", "menu", "modal"],
        "the list must be sorted by band no matter what order it was filled in"
    );
}

#[test]
fn test_topmost_resolves_across_mixed_open_surfaces() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "workspace",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 10., 10.)],
    );
    assert_eq!(reg.topmost().map(|s| s.id().as_str()), Some("workspace"));

    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 10., 10.)],
    );
    assert_eq!(reg.topmost().map(|s| s.id().as_str()), Some("logs"));

    add(
        &mut reg,
        "menu",
        SurfaceKind::MenuCard,
        vec![rect(0., 0., 10., 10.)],
    );
    assert_eq!(reg.topmost().map(|s| s.id().as_str()), Some("menu"));

    add(
        &mut reg,
        "modal",
        SurfaceKind::Modal,
        vec![rect(0., 0., 10., 10.)],
    );
    assert_eq!(reg.topmost().map(|s| s.id().as_str()), Some("modal"));
    assert_eq!(reg.highest_open_band(), Some(ZBand::Modal));
    assert_eq!(reg.keyboard_owner(), Some(ZBand::Modal));
}

#[test]
fn test_reregistering_refreshes_regions_and_keeps_position() {
    let mut reg = full_registry();
    let before = names(&reg);

    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(500., 500., 50., 50.)],
    );

    assert_eq!(names(&reg), before, "a rebuild must not reorder");
    assert_eq!(
        reg.get(&sid("logs")).map(|s| s.regions().to_vec()),
        Some(vec![rect(500., 500., 50., 50.)])
    );
}

#[test]
fn test_reregistering_in_the_same_band_can_change_kind() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "left",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 10., 10.)],
    );
    add(
        &mut reg,
        "right",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 10., 10.)],
    );
    add(
        &mut reg,
        "left",
        SurfaceKind::SearchFilter,
        vec![rect(0., 0., 10., 10.)],
    );

    assert_eq!(
        reg.get(&sid("left")).map(Surface::kind),
        Some(SurfaceKind::SearchFilter)
    );
    assert_eq!(names(&reg), vec!["left", "right"]);
}

#[test]
fn test_reregistering_into_a_different_band_moves_it_to_the_top_of_that_band() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "thing",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 10., 10.)],
    );
    add(
        &mut reg,
        "other",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 10., 10.)],
    );
    add(
        &mut reg,
        "top",
        SurfaceKind::Modal,
        vec![rect(0., 0., 10., 10.)],
    );

    add(
        &mut reg,
        "thing",
        SurfaceKind::Modal,
        vec![rect(0., 0., 10., 10.)],
    );

    assert_eq!(names(&reg), vec!["other", "top", "thing"]);
    assert_eq!(reg.count_in(ZBand::Workspace), 1);
    assert_eq!(reg.count_in(ZBand::Modal), 2);
}

#[test]
fn test_unregister_removes_and_returns_the_surface() {
    let mut reg = full_registry();
    let removed = reg.unregister(&sid("menu")).expect("menu is registered");
    assert_eq!(removed.kind(), SurfaceKind::MenuCard);
    assert!(!reg.contains(&sid("menu")));
    assert_eq!(reg.count_in(ZBand::Menu), 0);
    assert_eq!(reg.topmost().map(|s| s.id().as_str()), Some("modal"));
}

#[test]
fn test_unregister_unknown_is_none() {
    let mut reg = full_registry();
    assert!(reg.unregister(&sid("nope")).is_none());
    assert_eq!(reg.len(), 6);
}

#[test]
fn test_unregistering_a_lower_surface_leaves_the_rest_in_order() {
    let mut reg = full_registry();
    reg.unregister(&sid("logs"));
    assert_eq!(
        names(&reg),
        vec!["workspace", "filter", "debug", "menu", "modal"]
    );
}

#[test]
fn test_surfaces_in_band_yields_only_that_band_in_order() {
    let reg = full_registry();
    assert_eq!(
        names_in(&reg, ZBand::Workspace),
        vec!["workspace", "filter"]
    );
    assert_eq!(names_in(&reg, ZBand::UtilityOverlay), vec!["logs", "debug"]);
    assert_eq!(names_in(&reg, ZBand::Menu), vec!["menu"]);
    assert_eq!(names_in(&reg, ZBand::Modal), vec!["modal"]);
}

#[test]
fn test_clear_empties_everything() {
    let mut reg = full_registry();
    reg.clear();
    assert!(reg.is_empty());
    assert!(reg.topmost().is_none());
}

// ---------------------------------------------------------------------------
// 3. Z-promotion
// ---------------------------------------------------------------------------

#[test]
fn test_band_1_promotion_reorders_within_band_1_only() {
    let mut reg = full_registry();
    let before = names(&reg);

    assert!(reg.promote(&sid("logs")));

    assert_eq!(
        names_in(&reg, ZBand::UtilityOverlay),
        vec!["debug", "logs"],
        "the clicked card moves to the top of Band 1"
    );
    assert_eq!(
        names_in(&reg, ZBand::Workspace),
        vec!["workspace", "filter"],
        "Band 0 is untouched"
    );
    assert_eq!(names_in(&reg, ZBand::Menu), vec!["menu"]);
    assert_eq!(names_in(&reg, ZBand::Modal), vec!["modal"]);
    assert_eq!(reg.len(), before.len());
}

#[test]
fn test_promoting_one_card_then_another_swaps_them_back() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "a", SurfaceKind::MovableCard, vec![]);
    add(&mut reg, "b", SurfaceKind::MovableCard, vec![]);
    assert_eq!(names_in(&reg, ZBand::UtilityOverlay), vec!["a", "b"]);

    reg.promote(&sid("a"));
    assert_eq!(names_in(&reg, ZBand::UtilityOverlay), vec!["b", "a"]);

    reg.promote(&sid("b"));
    assert_eq!(names_in(&reg, ZBand::UtilityOverlay), vec!["a", "b"]);
}

#[test]
fn test_promote_already_top_returns_false() {
    let mut reg = full_registry();
    assert!(!reg.promote(&sid("debug")), "debug is already Band 1's top");
    assert_eq!(names_in(&reg, ZBand::UtilityOverlay), vec!["logs", "debug"]);
}

#[test]
fn test_promote_unknown_id_returns_false() {
    let mut reg = full_registry();
    assert!(!reg.promote(&sid("ghost")));
    assert_eq!(reg.len(), 6);
}

#[test]
fn test_promotion_never_moves_a_surface_between_bands() {
    let mut reg = full_registry();
    let before: Vec<(ZBand, usize)> = ZBand::ALL
        .into_iter()
        .map(|band| (band, reg.count_in(band)))
        .collect();

    reg.promote(&sid("logs"));
    reg.promote(&sid("workspace"));
    reg.promote(&sid("menu"));

    let after: Vec<(ZBand, usize)> = ZBand::ALL
        .into_iter()
        .map(|band| (band, reg.count_in(band)))
        .collect();
    assert_eq!(before, after);
}

#[test]
fn test_pointer_down_promotes_a_band_1_card() {
    let mut reg = base_registry();
    let route = reg.pointer_down(pt(100., 450.));
    assert_eq!(route.target(), &PointerTarget::Surface(sid("logs")));
    assert_eq!(names_in(&reg, ZBand::UtilityOverlay), vec!["debug", "logs"]);
}

#[test]
fn test_pointer_down_promotion_is_what_makes_the_second_card_come_forward() {
    let mut reg = base_registry();
    assert_eq!(
        reg.route_pointer(pt(500., 450.)).target(),
        &PointerTarget::Surface(sid("debug"))
    );
    let _ = reg.pointer_down(pt(100., 450.));
    // `logs` is now on top, but `debug` still owns the point only `debug` covers.
    assert_eq!(
        reg.route_pointer(pt(500., 450.)).target(),
        &PointerTarget::Surface(sid("debug"))
    );
    assert_eq!(
        reg.route_pointer(pt(100., 450.)).target(),
        &PointerTarget::Surface(sid("logs"))
    );
}

#[test]
fn test_pointer_down_does_not_promote_outside_band_1() {
    let mut reg = base_registry();
    let before = names(&reg);

    let _ = reg.pointer_down(pt(900., 100.)); // the workspace
    let _ = reg.pointer_down(pt(100., 10.)); // the filter
    let _ = reg.pointer_down(pt(999., 699.)); // dead workspace

    assert_eq!(names(&reg), before, "only Band 1 reorders on click");
}

#[test]
fn test_promotion_survives_a_re_register() {
    let mut reg = base_registry();
    let _ = reg.pointer_down(pt(100., 450.));
    assert_eq!(names_in(&reg, ZBand::UtilityOverlay), vec!["debug", "logs"]);

    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(10., 400., 300., 200.)],
    );
    assert_eq!(
        names_in(&reg, ZBand::UtilityOverlay),
        vec!["debug", "logs"],
        "a promoted card must not fall back to the bottom when it rebuilds"
    );
}

// ---------------------------------------------------------------------------
// 4. Pointer routing
// ---------------------------------------------------------------------------

#[test]
fn test_empty_registry_routes_every_point_to_the_workspace() {
    let reg = SurfaceRegistry::new();
    for x in [0., 250., 500., 999., -100.] {
        for y in [0., 250., 500., 699., -100.] {
            let route = reg.route_pointer(pt(x, y));
            assert!(
                route.reaches_workspace(),
                "({x}, {y}) should reach the workspace, got {:?}",
                route.target()
            );
            assert!(route.passed_through().is_empty());
        }
    }
}

#[test]
fn test_band_0_surface_receives_inside_and_workspace_outside() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "pane",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 100., 100.)],
    );

    assert_eq!(
        reg.route_pointer(pt(50., 50.)).target(),
        &PointerTarget::Surface(sid("pane"))
    );
    assert_eq!(
        reg.route_pointer(pt(500., 500.)).target(),
        &PointerTarget::Workspace
    );
}

#[test]
fn test_band_1_inside_hits_the_card() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(10., 10., 100., 100.)],
    );
    let route = reg.route_pointer(pt(50., 50.));
    assert_eq!(route.target(), &PointerTarget::Surface(sid("logs")));
    assert!(route.passed_through().is_empty());
}

#[test]
fn test_band_1_outside_passes_through_to_band_0() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "pane",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 1000., 700.)],
    );
    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(10., 10., 100., 100.)],
    );

    let route = reg.route_pointer(pt(500., 500.));
    assert_eq!(route.target(), &PointerTarget::Surface(sid("pane")));
    assert_eq!(route.passed_through(), [ZBand::UtilityOverlay]);
}

#[test]
fn test_band_1_outside_with_no_band_0_surface_still_reaches_the_workspace() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(10., 10., 100., 100.)],
    );
    let route = reg.route_pointer(pt(500., 500.));
    assert_eq!(route.target(), &PointerTarget::Workspace);
    assert_eq!(route.passed_through(), [ZBand::UtilityOverlay]);
}

#[test]
fn test_band_2_outside_dismisses_band_2_and_never_reaches_the_workspace() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "pane",
        SurfaceKind::Workspace,
        vec![rect(0., 0., 1000., 700.)],
    );
    add(
        &mut reg,
        "menu",
        SurfaceKind::MenuCard,
        vec![rect(10., 10., 100., 100.)],
    );

    let route = reg.route_pointer(pt(500., 500.));
    assert_eq!(route.target(), &PointerTarget::DismissBand(ZBand::Menu));
    assert!(route.is_band_rule());
    assert!(
        !route.reaches_workspace(),
        "the click-away catcher consumes the click"
    );
}

#[test]
fn test_band_2_inside_hits_the_card() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "menu",
        SurfaceKind::MenuCard,
        vec![rect(10., 10., 100., 100.)],
    );
    assert_eq!(
        reg.route_pointer(pt(50., 50.)).target(),
        &PointerTarget::Surface(sid("menu"))
    );
}

#[test]
fn test_band_2_outside_does_not_reach_the_band_1_card_below_it() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "logs",
        SurfaceKind::MovableCard,
        vec![rect(300., 300., 100., 100.)],
    );
    add(
        &mut reg,
        "menu",
        SurfaceKind::MenuCard,
        vec![rect(10., 10., 100., 100.)],
    );

    let route = reg.route_pointer(pt(350., 350.));
    assert_eq!(
        route.target(),
        &PointerTarget::DismissBand(ZBand::Menu),
        "the Band 2 catcher is full-window: the click dismisses the menu and \
         never arrives at the Band 1 card underneath it"
    );
}

#[test]
fn test_band_3_inside_hits_the_modal() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "modal",
        SurfaceKind::Modal,
        vec![rect(100., 100., 200., 200.)],
    );
    assert_eq!(
        reg.route_pointer(pt(150., 150.)).target(),
        &PointerTarget::Surface(sid("modal"))
    );
}

#[test]
fn test_band_3_outside_hits_the_scrim_and_never_a_lower_band() {
    let reg = full_registry();
    for point in [pt(0., 0.), pt(50., 50.), pt(350., 350.), pt(900., 600.)] {
        let route = reg.route_pointer(point);
        assert_eq!(
            route.target(),
            &PointerTarget::Scrim(sid("modal")),
            "{point:?} must land on the scrim"
        );
        assert!(route.is_band_rule());
    }
}

#[test]
fn test_a_modal_blocks_band_0_and_band_1_completely_across_a_grid() {
    let reg = full_registry();
    let blocked: Vec<SurfaceId> = ["workspace", "filter", "logs", "debug", "menu"]
        .iter()
        .map(|name| sid(name))
        .collect();

    for x in (0..=1000).step_by(25) {
        for y in (0..=700).step_by(25) {
            let target = reg.route_pointer(pt(x as f32, y as f32)).target().clone();
            if let PointerTarget::Surface(id) = &target {
                assert!(
                    !blocked.contains(id),
                    "a lower band received a pointer event at ({x}, {y}) while a modal was open"
                );
            }
            assert!(
                matches!(target, PointerTarget::Surface(_) | PointerTarget::Scrim(_)),
                "with a modal open every point must resolve to the card or the scrim, got {target:?}"
            );
        }
    }
}

#[test]
fn test_topmost_overlapping_card_wins_the_hit() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "under",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 100., 100.)],
    );
    add(
        &mut reg,
        "over",
        SurfaceKind::MovableCard,
        vec![rect(50., 50., 100., 100.)],
    );
    assert_eq!(
        reg.route_pointer(pt(75., 75.)).target(),
        &PointerTarget::Surface(sid("over"))
    );
}

#[test]
fn test_a_lower_card_is_hit_where_only_it_covers() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "under",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 100., 100.)],
    );
    add(
        &mut reg,
        "over",
        SurfaceKind::MovableCard,
        vec![rect(50., 50., 100., 100.)],
    );
    assert_eq!(
        reg.route_pointer(pt(10., 10.)).target(),
        &PointerTarget::Surface(sid("under"))
    );
}

#[test]
fn test_promotion_changes_which_overlapping_card_is_hit() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "a",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 100., 100.)],
    );
    add(
        &mut reg,
        "b",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 100., 100.)],
    );
    assert_eq!(
        reg.route_pointer(pt(10., 10.)).target(),
        &PointerTarget::Surface(sid("b"))
    );
    reg.promote(&sid("a"));
    assert_eq!(
        reg.route_pointer(pt(10., 10.)).target(),
        &PointerTarget::Surface(sid("a"))
    );
}

#[test]
fn test_a_multi_region_surface_hits_in_either_region() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "torn",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 50., 50.), rect(200., 200., 50., 50.)],
    );
    assert_eq!(
        reg.route_pointer(pt(25., 25.)).target(),
        &PointerTarget::Surface(sid("torn"))
    );
    assert_eq!(
        reg.route_pointer(pt(225., 225.)).target(),
        &PointerTarget::Surface(sid("torn"))
    );
    assert_eq!(
        reg.route_pointer(pt(125., 125.)).target(),
        &PointerTarget::Workspace
    );
}

#[test]
fn test_a_surface_without_regions_is_never_hit() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "filter", SurfaceKind::SearchFilter, vec![]);
    assert_eq!(
        reg.route_pointer(pt(0., 0.)).target(),
        &PointerTarget::Workspace
    );
    assert!(
        reg.contains(&sid("filter")),
        "it is still registered and stacked"
    );
}

#[test]
fn test_right_and_bottom_edges_are_outside_the_rectangle() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "card",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 10., 10.)],
    );

    assert_eq!(
        reg.route_pointer(pt(0., 0.)).target(),
        &PointerTarget::Surface(sid("card")),
        "the top-left corner is inside"
    );
    assert_eq!(
        reg.route_pointer(pt(9.999, 5.)).target(),
        &PointerTarget::Surface(sid("card"))
    );
    assert_eq!(
        reg.route_pointer(pt(10., 5.)).target(),
        &PointerTarget::Workspace,
        "iced excludes the right edge, so abutting cards do not overlap"
    );
    assert_eq!(
        reg.route_pointer(pt(5., 10.)).target(),
        &PointerTarget::Workspace,
        "iced excludes the bottom edge"
    );
}

#[test]
fn test_a_nan_point_hits_nothing() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "card",
        SurfaceKind::MovableCard,
        vec![rect(0., 0., 100., 100.)],
    );
    let route = reg.route_pointer(pt(f32::NAN, 50.));
    assert_eq!(route.target(), &PointerTarget::Workspace);
    assert_eq!(route.passed_through(), [ZBand::UtilityOverlay]);
}

#[test]
fn test_sweep_with_bands_0_and_1_only_passes_every_outside_point_down() {
    let reg = base_registry();
    let workspace = rect(0., 0., 1000., 700.);
    let logs = rect(10., 400., 300., 200.);
    let debug = rect(400., 400., 300., 200.);
    let filter = rect(0., 0., 200., 24.);

    for x in (0..=1000).step_by(20) {
        for y in (0..=700).step_by(20) {
            let point = pt(x as f32, y as f32);
            let route = reg.route_pointer(point);
            let expected = if logs.contains(point) {
                PointerTarget::Surface(sid("logs"))
            } else if debug.contains(point) {
                PointerTarget::Surface(sid("debug"))
            } else if filter.contains(point) {
                PointerTarget::Surface(sid("filter"))
            } else if workspace.contains(point) {
                PointerTarget::Surface(sid("workspace"))
            } else {
                PointerTarget::Workspace
            };
            assert_eq!(route.target(), &expected, "({x}, {y})");

            let inside_band_1 = logs.contains(point) || debug.contains(point);
            let expected_passed: &[ZBand] = if inside_band_1 {
                &[]
            } else {
                &[ZBand::UtilityOverlay]
            };
            assert_eq!(route.passed_through(), expected_passed, "({x}, {y})");
        }
    }
}

#[test]
fn test_sweep_with_a_menu_open_hits_only_the_menu_or_dismisses_the_band() {
    let reg = menu_registry();
    let menu = rect(20., 40., 180., 160.);

    for x in (0..=1000).step_by(20) {
        for y in (0..=700).step_by(20) {
            let point = pt(x as f32, y as f32);
            let route = reg.route_pointer(point);
            let expected = if menu.contains(point) {
                PointerTarget::Surface(sid("menu"))
            } else {
                PointerTarget::DismissBand(ZBand::Menu)
            };
            assert_eq!(route.target(), &expected, "({x}, {y})");
            assert!(route.passed_through().is_empty(), "({x}, {y})");
        }
    }
}

#[test]
fn test_sweep_with_a_modal_open_hits_only_the_card_or_the_scrim() {
    let reg = full_registry();
    let modal = rect(400., 250., 200., 160.);

    for x in (0..=1000).step_by(20) {
        for y in (0..=700).step_by(20) {
            let point = pt(x as f32, y as f32);
            let route = reg.route_pointer(point);
            let expected = if modal.contains(point) {
                PointerTarget::Surface(sid("modal"))
            } else {
                PointerTarget::Scrim(sid("modal"))
            };
            assert_eq!(route.target(), &expected, "({x}, {y})");
            assert!(route.passed_through().is_empty(), "({x}, {y})");
        }
    }
}

#[test]
fn test_a_shrunk_modal_card_puts_the_scrim_over_every_other_surface() {
    // Same registry, but the modal card is one pixel in the corner, so
    // every interesting point is outside it and the scrim owns them all.
    let mut reg = full_registry();
    reg.register(sid("modal"), SurfaceKind::Modal, vec![rect(0., 0., 1., 1.)]);

    for (x, y) in [
        (30., 50.),
        (100., 450.),
        (500., 450.),
        (900., 100.),
        (999., 699.),
    ] {
        let route = reg.route_pointer(pt(x, y));
        assert_eq!(
            route.target(),
            &PointerTarget::Scrim(sid("modal")),
            "({x}, {y}) must land on the scrim, not on the surface painted there"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Escape unwinding
// ---------------------------------------------------------------------------

#[test]
fn test_escape_on_empty_registry_is_a_defined_noop() {
    let mut reg = SurfaceRegistry::new();
    let outcome = reg.escape(dismiss_everything());
    assert!(!outcome.is_consumed());
    assert!(outcome.refusal().is_some_and(EscapeRefusal::is_empty_stack));
    assert_eq!(outcome.consumed_by(), None);
    assert!(outcome.declined_before().is_empty());
}

#[test]
fn test_escape_never_calls_the_responder_on_an_empty_registry() {
    let calls = Cell::new(0usize);
    let mut reg = SurfaceRegistry::new();
    let _ = reg.escape(|_| {
        calls.set(calls.get() + 1);
        UnwindAnswer::Dismissed
    });
    assert_eq!(calls.get(), 0);
}

#[test]
fn test_a_dismissed_surface_is_removed_from_the_registry() {
    let mut reg = SurfaceRegistry::new();
    add(
        &mut reg,
        "modal",
        SurfaceKind::Modal,
        vec![rect(0., 0., 10., 10.)],
    );
    let outcome = reg.escape(dismiss_everything());
    assert!(outcome.is_consumed());
    assert!(outcome.dismissed());
    assert_eq!(outcome.consumed_by(), Some(&sid("modal")));
    assert!(!reg.contains(&sid("modal")));
    assert!(reg.is_empty());
}

#[test]
fn test_an_unwound_surface_stays_registered() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "filter", SurfaceKind::SearchFilter, vec![]);
    let outcome = reg.escape(scripted(vec![("filter", UnwindAnswer::Unwound)]));
    assert!(outcome.is_consumed());
    assert!(
        !outcome.dismissed(),
        "clearing a filter unwinds a level without dismissing the field"
    );
    assert!(reg.contains(&sid("filter")));
}

#[test]
fn test_a_declined_surface_stays_registered_and_is_recorded() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "logs", SurfaceKind::MovableCard, vec![]);
    let outcome = reg.escape(scripted(vec![(
        "logs",
        UnwindAnswer::Declined(DeclineReason::NotFocused),
    )]));
    assert!(!outcome.is_consumed());
    assert!(reg.contains(&sid("logs")));

    let declined = outcome.refusal().and_then(|refusal| match refusal {
        EscapeRefusal::AllDeclined { declined } => Some(declined.clone()),
        EscapeRefusal::EmptyStack => None,
    });
    assert_eq!(
        declined,
        Some(vec![Declination {
            surface: sid("logs"),
            reason: DeclineReason::NotFocused,
        }])
    );
}

#[test]
fn test_all_declined_reports_every_decliner_in_asked_order() {
    let mut reg = full_registry();
    let outcome = reg.escape(|surface| {
        UnwindAnswer::Declined(match surface.band() {
            ZBand::Workspace => DeclineReason::NotOpen,
            ZBand::UtilityOverlay => DeclineReason::NotFocused,
            ZBand::Menu => DeclineReason::CannotUnwind,
            ZBand::Modal => DeclineReason::CannotUnwind,
        })
    });
    let EscapeOutcome::Ignored(EscapeRefusal::AllDeclined { declined }) = &outcome else {
        panic!("every surface declined, so nothing should have consumed the key: {outcome:?}");
    };
    let order: Vec<&str> = declined.iter().map(|d| d.surface.as_str()).collect();
    assert_eq!(order, FULL_ESCAPE_ORDER);
    assert_eq!(reg.len(), 6, "a declined Escape removes nothing");
}

#[test]
fn test_escape_unwinds_submenu_root_menu_then_modal_in_strict_reverse_z_order() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "root-menu", SurfaceKind::MenuCard, vec![]);
    add(&mut reg, "submenu", SurfaceKind::MenuCard, vec![]);
    add(&mut reg, "modal", SurfaceKind::Modal, vec![]);

    let sequence = drain(&mut reg, 10, dismiss_everything());
    assert_eq!(
        sequence,
        vec![
            Some("submenu".to_owned()),
            Some("root-menu".to_owned()),
            Some("modal".to_owned()),
            None,
        ],
        "UIF-WIN-003: deepest submenu → root menu → topmost dialog → nothing"
    );
    assert!(reg.is_empty());
}

#[test]
fn test_escape_unwinds_the_search_filter_last() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "workspace", SurfaceKind::Workspace, vec![]);
    add(&mut reg, "filter", SurfaceKind::SearchFilter, vec![]);
    add(&mut reg, "logs", SurfaceKind::MovableCard, vec![]);
    add(&mut reg, "menu", SurfaceKind::MenuCard, vec![]);
    add(&mut reg, "modal", SurfaceKind::Modal, vec![]);

    let sequence = drain(&mut reg, 10, dismiss_everything());
    assert_eq!(
        sequence,
        vec![
            Some("menu".to_owned()),
            Some("modal".to_owned()),
            Some("logs".to_owned()),
            Some("filter".to_owned()),
            Some("workspace".to_owned()),
            None,
        ]
    );
}

#[test]
fn test_the_whole_registry_drains_in_escape_order() {
    let mut reg = full_registry();
    let sequence = drain(&mut reg, 20, dismiss_everything());
    let expected: Vec<Option<String>> = FULL_ESCAPE_ORDER
        .iter()
        .map(|name| Some((*name).to_owned()))
        .chain(std::iter::once(None))
        .collect();
    assert_eq!(sequence, expected);
    assert!(reg.is_empty());
}

#[test]
fn test_declines_before_the_consumer_are_reported() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "modal", SurfaceKind::Modal, vec![]);
    add(&mut reg, "menu", SurfaceKind::MenuCard, vec![]);

    // The menu band is asked first; it declines, so the modal consumes.
    let outcome = reg.escape(|surface| {
        if surface.kind() == SurfaceKind::MenuCard {
            UnwindAnswer::Declined(DeclineReason::CannotUnwind)
        } else {
            UnwindAnswer::Dismissed
        }
    });
    assert_eq!(outcome.consumed_by(), Some(&sid("modal")));
    assert!(outcome.dismissed());
    let declined = outcome.declined_before();
    assert_eq!(declined.len(), 1);
    assert_eq!(declined[0].surface.as_str(), "menu");
    assert_eq!(declined[0].reason, DeclineReason::CannotUnwind);
    assert!(
        reg.contains(&sid("menu")),
        "a surface that declined stays put; only the consumer is removed"
    );
    assert!(!reg.contains(&sid("modal")));
}

#[test]
fn test_a_band_1_card_that_is_not_focused_declines_and_the_key_moves_on() {
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "logs", SurfaceKind::MovableCard, vec![]);
    add(&mut reg, "filter", SurfaceKind::SearchFilter, vec![]);

    let outcome = reg.escape(|surface| match surface.kind() {
        SurfaceKind::MovableCard => UnwindAnswer::Declined(DeclineReason::NotFocused),
        _ => UnwindAnswer::Dismissed,
    });
    assert_eq!(outcome.consumed_by(), Some(&sid("filter")));
    assert!(reg.contains(&sid("logs")));
}

#[test]
fn test_from_cascade_close_maps_notopen_to_a_decline() {
    let outcome = CascadeOutcome::ignored(IgnoredReason::NotOpen);
    assert_eq!(
        UnwindAnswer::from_cascade_close(&outcome),
        UnwindAnswer::Declined(DeclineReason::NotOpen)
    );
}

#[test]
fn test_from_cascade_close_maps_a_closed_card_to_dismissed() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let tree = deep_menu();
    cascade.open(&tree, MenuTreeKind::Context, &mut focus);
    cascade.move_focus(1, &mut focus);
    cascade.open_focused_submenu(&mut focus);

    let outcome = cascade.close_deepest(&mut focus);
    assert_eq!(
        UnwindAnswer::from_cascade_close(&outcome),
        UnwindAnswer::Dismissed
    );
}

#[test]
fn test_from_cascade_close_maps_other_refusals_to_cannot_unwind() {
    for reason in [
        IgnoredReason::AlreadyOpen {
            card: MenuId::new("x"),
        },
        IgnoredReason::UnknownCard {
            card: MenuId::new("x"),
        },
        IgnoredReason::NothingFocused {
            card: MenuId::new("x"),
        },
        IgnoredReason::NoFocusableRows {
            card: MenuId::new("x"),
        },
        IgnoredReason::NotSubmenu {
            card: MenuId::new("x"),
            row: 1,
        },
        IgnoredReason::RowOutOfRange {
            card: MenuId::new("x"),
            row: 9,
        },
    ] {
        let outcome = CascadeOutcome::ignored(reason);
        assert_eq!(
            UnwindAnswer::from_cascade_close(&outcome),
            UnwindAnswer::Declined(DeclineReason::CannotUnwind)
        );
    }
}

#[test]
fn test_a_cascade_driven_escape_unwinds_deepest_first_then_the_modal() {
    let (mut cascade, mut focus) = open_three_levels();
    let ids: Vec<String> = cascade
        .open_cards()
        .iter()
        .map(|card| card.id().as_str().to_owned())
        .collect();

    let mut reg = SurfaceRegistry::new();
    for card in cascade.open_cards() {
        add(&mut reg, card.id().as_str(), SurfaceKind::MenuCard, vec![]);
    }
    add(&mut reg, "modal", SurfaceKind::Modal, vec![]);

    let mut responder = |surface: &Surface| {
        if surface.kind() == SurfaceKind::MenuCard {
            UnwindAnswer::from_cascade_close(&cascade.close_deepest(&mut focus))
        } else {
            UnwindAnswer::Dismissed
        }
    };

    let mut sequence = Vec::new();
    for _ in 0..6 {
        let outcome = reg.escape(&mut responder);
        let consumed = outcome.consumed_by().map(|id| id.as_str().to_owned());
        let stop = !outcome.is_consumed();
        sequence.push(consumed);
        if stop {
            break;
        }
    }

    assert_eq!(
        sequence,
        vec![
            Some(ids[2].clone()),
            Some(ids[1].clone()),
            Some(ids[0].clone()),
            Some("modal".to_owned()),
            None,
        ],
        "deepest submenu → middle → root menu → topmost dialog → nothing"
    );
    assert!(!cascade.is_open());
    assert!(reg.is_empty());
}

#[test]
fn test_a_spent_cascade_declines_and_the_key_keeps_travelling() {
    let (mut cascade, mut focus) = open_three_levels();
    let mut reg = SurfaceRegistry::new();
    for card in cascade.open_cards() {
        add(&mut reg, card.id().as_str(), SurfaceKind::MenuCard, vec![]);
    }
    cascade.close_tree(&mut focus);
    assert!(!cascade.is_open());

    let outcome =
        reg.escape(|_surface| UnwindAnswer::from_cascade_close(&cascade.close_deepest(&mut focus)));

    // Every card answers NotOpen, so nothing consumes the key and the
    // registry is untouched.
    assert!(!outcome.is_consumed());
    assert_eq!(reg.len(), 3);
    let EscapeOutcome::Ignored(EscapeRefusal::AllDeclined { declined }) = &outcome else {
        panic!("expected all-declined, got {outcome:?}");
    };
    assert_eq!(declined.len(), 3);
    assert!(
        declined
            .iter()
            .all(|declination| declination.reason == DeclineReason::NotOpen)
    );
}

// ---------------------------------------------------------------------------
// 6. Scrim composition and determinism
// ---------------------------------------------------------------------------

#[test]
fn test_scrim_alpha_is_the_spec_value() {
    assert_eq!(SCRIM_ALPHA, 0.55);
}

#[test]
fn test_scrim_composes_the_base_token_with_the_alpha_constant() {
    let palette = Palette::dark();
    assert_eq!(
        scrim(palette.surface_base),
        palette.surface_scrim,
        "the composition must reproduce the `color.surface.scrim` token exactly"
    );
}

#[test]
fn test_scrim_for_reads_the_surface_base_token() {
    let palette = Palette::dark();
    assert_eq!(scrim_for(&palette), palette.surface_scrim);
    assert_eq!(scrim_for(&palette), scrim(palette.surface_base));
}

#[test]
fn test_scrim_preserves_the_base_channels_for_any_colour() {
    let base = Color::from_rgb8(123, 45, 67);
    let composed = scrim(base);
    assert_eq!(composed.r, base.r);
    assert_eq!(composed.g, base.g);
    assert_eq!(composed.b, base.b);
    assert_eq!(composed.a, SCRIM_ALPHA);
}

#[test]
fn test_scrim_is_translucent_not_opaque() {
    let scrim = scrim_for(&Palette::dark());
    assert!(scrim.a > 0.0 && scrim.a < 1.0);
    assert!(
        scrim.a < 1.0,
        "an opaque scrim would hide the workspace instead of dimming it"
    );
}

#[test]
fn test_routing_is_deterministic_across_repeats() {
    let reg = full_registry();
    for _ in 0..3 {
        for x in (0..=1000).step_by(50) {
            for y in (0..=700).step_by(50) {
                let point = pt(x as f32, y as f32);
                assert_eq!(reg.route_pointer(point), reg.route_pointer(point));
            }
        }
    }
}

#[test]
fn test_a_cloned_registry_routes_identically() {
    let original = full_registry();
    let clone = original.clone();
    for x in (0..=1000).step_by(40) {
        for y in (0..=700).step_by(40) {
            let point = pt(x as f32, y as f32);
            assert_eq!(original.route_pointer(point), clone.route_pointer(point));
        }
    }
}

#[test]
fn test_escape_sequence_is_reproducible_from_a_clone() {
    let mut a = full_registry();
    let mut b = a.clone();
    let first = drain(&mut a, 20, dismiss_everything());
    let second = drain(&mut b, 20, dismiss_everything());
    assert_eq!(first, second);
    assert_eq!(a, b);
}

#[test]
fn test_a_cloned_registry_can_be_mutated_independently() {
    let original = base_registry();
    let mut clone = original.clone();
    clone.promote(&sid("logs"));
    assert_eq!(
        names_in(&original, ZBand::UtilityOverlay),
        vec!["logs", "debug"]
    );
    assert_eq!(
        names_in(&clone, ZBand::UtilityOverlay),
        vec!["debug", "logs"]
    );
}

// ---------------------------------------------------------------------------
// 7. The Pre-Rank composition (`UIF-WIN-003` step 1)
// ---------------------------------------------------------------------------
//
// The rule under test, §3.0.4.2: "Pressing `Escape` while any
// pointer-captured drag … is active cancels that drag first (`Pre-Rank`),
// even if keyboard focus resides elsewhere."
//
// Every test here pairs a registry with a
// [`PreRankCoordinator`] and drives one `Escape` through
// [`escape_with_pre_rank`]. The two things that must always be true are
// that the rank walk did **not** also run, and that with no capture held it
// ran exactly as it did before the rule existed.

/// A workspace slider mid-scrub, with keyboard focus parked in the search
/// filter — the configuration the rule exists for.
///
/// The slider is not a surface, so nothing in the registry can see it. The
/// ledger is the only place its capture is recorded.
fn dragging_ledger() -> PreRankCoordinator {
    let mut captures = PreRankCoordinator::new();
    let _ = captures.begin(DragCapture::new("volume-slider", DragKind::Slider));
    captures
}

/// Drain the full chain — Pre-Rank first, then the ranks — until it stops
/// consuming, reporting which subject took each key.
fn drain_pre_ranked(
    captures: &mut PreRankCoordinator,
    reg: &mut SurfaceRegistry,
    cap: usize,
    mut cancel_capture: impl FnMut(&DragCapture) -> bool,
    mut responder: impl FnMut(&Surface) -> UnwindAnswer,
) -> Vec<String> {
    let mut taken_by = Vec::new();
    for _ in 0..cap {
        let escape = escape_with_pre_rank(captures, reg, &mut cancel_capture, &mut responder);
        let taken = match &escape {
            PreRankedEscape::DragCancelled { capture } => {
                format!("drag:{}", capture.id())
            }
            PreRankedEscape::RankWalked { outcome, .. } => match outcome.consumed_by() {
                Some(id) => format!("surface:{id}"),
                None => "none".to_owned(),
            },
        };
        let stop = escape.drag_cancelled() || !escape.outcome().is_some_and(|o| o.is_consumed());
        taken_by.push(taken);
        if stop {
            break;
        }
    }
    taken_by
}

#[test]
fn test_pre_rank_cancels_the_drag_and_never_walks_the_ranks() {
    let mut reg = full_registry();
    let before = reg.clone();
    let mut captures = dragging_ledger();

    let asked = Cell::new(0usize);
    let escape = escape_with_pre_rank(
        &mut captures,
        &mut reg,
        |_| true,
        |_| {
            asked.set(asked.get() + 1);
            UnwindAnswer::Dismissed
        },
    );

    assert!(
        escape.drag_cancelled(),
        "the drag owns the keypress, not the surface that holds focus"
    );
    assert_eq!(
        escape,
        PreRankedEscape::DragCancelled {
            capture: DragCapture::new("volume-slider", DragKind::Slider)
        }
    );
    assert_eq!(
        asked.get(),
        0,
        "the rank walk must not run at all in the same keypress"
    );
    assert_eq!(reg, before, "no surface was dismissed or reordered");
    assert!(
        !captures.is_captured(),
        "the capture is spent once cancelled"
    );
    assert_eq!(
        escape.outcome(),
        None,
        "there is no surface outcome to report"
    );
}

#[test]
fn test_pre_rank_outranks_the_menu_band_and_the_modal_band_together() {
    // The menu is Escape Rank 3 and the modal Rank 2; the drag outranks both,
    // so neither is asked.
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "modal", SurfaceKind::Modal, vec![]);
    add(&mut reg, "menu", SurfaceKind::MenuCard, vec![]);
    let before = reg.clone();
    let mut captures = dragging_ledger();

    let escape = escape_with_pre_rank(
        &mut captures,
        &mut reg,
        |_| true,
        |_| UnwindAnswer::Dismissed,
    );

    assert_eq!(
        escape,
        PreRankedEscape::DragCancelled {
            capture: DragCapture::new("volume-slider", DragKind::Slider)
        },
        "Pre-Rank runs ahead of every escape rank"
    );
    assert_eq!(reg, before);
}

#[test]
fn test_pre_rank_fires_when_the_drag_and_the_focus_are_the_same_surface() {
    // The rule is unconditional: whether focus sits on the dragging control
    // or a hundred surfaces away changes nothing. Same registry shape, drag
    // registered by the control inside the surface that holds focus.
    let mut reg = SurfaceRegistry::new();
    add(&mut reg, "settings", SurfaceKind::Modal, vec![]);
    let before = reg.clone();

    let mut captures = PreRankCoordinator::new();
    let _ = captures.begin(DragCapture::new("settings.brightness", DragKind::Slider));

    let asked = Cell::new(0usize);
    let escape = escape_with_pre_rank(
        &mut captures,
        &mut reg,
        |_| true,
        |_| {
            asked.set(asked.get() + 1);
            UnwindAnswer::Dismissed
        },
    );

    assert!(escape.drag_cancelled());
    assert_eq!(asked.get(), 0, "the host modal is not asked either");
    assert_eq!(reg, before);
}

#[test]
fn test_a_stale_capture_lets_the_escape_reach_the_ranks() {
    // A capture whose control had nothing to cancel must not swallow the key.
    let mut reg = full_registry();
    let mut captures = dragging_ledger();

    let escape = escape_with_pre_rank(&mut captures, &mut reg, |_| false, dismiss_everything());

    match &escape {
        PreRankedEscape::RankWalked {
            outcome, pre_rank, ..
        } => {
            assert_eq!(
                *pre_rank,
                PreRankOutcome::Stale {
                    capture: DragCapture::new("volume-slider", DragKind::Slider)
                }
            );
            assert_eq!(outcome.consumed_by(), Some(&sid("menu")));
        }
        other => panic!("a dead capture must not consume the key: {other:?}"),
    }
    assert!(
        !reg.contains(&sid("volume-slider")),
        "the drag was never a surface anyway"
    );
    assert!(!captures.is_captured());
}

#[test]
fn test_the_second_escape_after_a_pre_rank_cancel_walks_the_ranks() {
    let mut reg = full_registry();
    let mut captures = dragging_ledger();

    let first = escape_with_pre_rank(&mut captures, &mut reg, |_| true, dismiss_everything());
    assert!(first.drag_cancelled());

    let second = escape_with_pre_rank(&mut captures, &mut reg, |_| true, dismiss_everything());
    assert!(
        !second.drag_cancelled(),
        "the capture was spent on the first keypress"
    );
    assert_eq!(
        second.outcome().and_then(|o| o.consumed_by()),
        Some(&sid("menu"))
    );
    assert_eq!(
        second.pre_rank(),
        PreRankOutcome::NotCaptured,
        "the second Escape sees an empty ledger"
    );
}

#[test]
fn test_releasing_the_capture_restores_normal_escape_behaviour() {
    let mut reg = full_registry();
    let mut captures = dragging_ledger();

    assert!(captures.release(&CaptureId::new("volume-slider")));

    let escape = escape_with_pre_rank(&mut captures, &mut reg, |_| true, dismiss_everything());
    assert!(!escape.drag_cancelled());
    assert_eq!(
        escape.outcome().and_then(|o| o.consumed_by()),
        Some(&sid("menu"))
    );
    assert_eq!(escape.pre_rank(), PreRankOutcome::NotCaptured);
}

#[test]
fn test_an_uncaptured_chain_is_identical_to_a_plain_escape_walk() {
    // Byte-for-byte: the same registry drained one way through
    // `SurfaceRegistry::escape` and the other through the full chain with an
    // empty ledger must produce the same consumers, in the same order, and
    // end in the same registry.
    let mut plain = full_registry();
    let mut chained = full_registry();
    let mut captures = PreRankCoordinator::new();

    let plain_sequence = drain(&mut plain, 20, dismiss_everything());
    let chained_sequence = drain_pre_ranked(
        &mut captures,
        &mut chained,
        20,
        |_| true,
        dismiss_everything(),
    );

    let expected: Vec<String> = FULL_ESCAPE_ORDER
        .iter()
        .map(|name| format!("surface:{name}"))
        .chain(std::iter::once("none".to_owned()))
        .collect();
    assert_eq!(
        chained_sequence, expected,
        "the Pre-Rank must not disturb the documented order"
    );

    // The stronger form: tag the plain walk's own answers and compare them,
    // element for element, rather than comparing each to a hand-written list.
    let tagged_plain: Vec<String> = plain_sequence
        .iter()
        .map(|hit| match hit {
            Some(name) => format!("surface:{name}"),
            None => "none".to_owned(),
        })
        .collect();
    assert_eq!(
        chained_sequence, tagged_plain,
        "with no capture held the full chain must be the plain walk"
    );
    assert_eq!(plain, chained, "both walks end in the same registry");
    assert!(!captures.is_captured());
}

#[test]
fn test_an_uncaptured_empty_registry_still_reports_the_empty_stack() {
    let mut reg = SurfaceRegistry::new();
    let mut captures = PreRankCoordinator::new();
    let asked = Cell::new(0usize);

    let escape = escape_with_pre_rank(
        &mut captures,
        &mut reg,
        |_| true,
        |_| {
            asked.set(asked.get() + 1);
            UnwindAnswer::Dismissed
        },
    );

    let PreRankedEscape::RankWalked { outcome, pre_rank } = &escape else {
        panic!("nothing was captured, so the rank walk should have run: {escape:?}");
    };
    assert_eq!(*pre_rank, PreRankOutcome::NotCaptured);
    assert!(outcome.refusal().is_some_and(EscapeRefusal::is_empty_stack));
    assert_eq!(asked.get(), 0);
}

#[test]
fn test_all_declined_still_accumulates_through_the_full_chain() {
    let mut reg = full_registry();
    let mut captures = PreRankCoordinator::new();

    let escape = escape_with_pre_rank(
        &mut captures,
        &mut reg,
        |_| true,
        |_| UnwindAnswer::Declined(DeclineReason::NotFocused),
    );

    let PreRankedEscape::RankWalked { outcome, .. } = &escape else {
        panic!("no capture was held: {escape:?}");
    };
    let order: Vec<&str> = outcome
        .declined_before()
        .iter()
        .map(|d| d.surface.as_str())
        .collect();
    assert_eq!(order, FULL_ESCAPE_ORDER);
    assert_eq!(reg.len(), 6, "a declined Escape removes nothing");
}

#[test]
fn test_pre_ranked_escape_reports_which_step_took_the_key() {
    let capture = DragCapture::new("volume-slider", DragKind::Slider);
    let cancelled = PreRankedEscape::DragCancelled {
        capture: capture.clone(),
    };
    assert!(cancelled.drag_cancelled());
    assert_eq!(cancelled.outcome(), None);
    assert_eq!(
        cancelled.pre_rank(),
        PreRankOutcome::Cancelled {
            capture: capture.clone()
        }
    );

    let walked = PreRankedEscape::RankWalked {
        outcome: EscapeOutcome::Ignored(EscapeRefusal::EmptyStack),
        pre_rank: PreRankOutcome::NotCaptured,
    };
    assert!(!walked.drag_cancelled());
    assert!(walked.outcome().is_some());
    assert_eq!(walked.pre_rank(), PreRankOutcome::NotCaptured);
}

#[test]
fn test_a_drag_cancelled_escape_never_reaches_a_registered_surface() {
    // The strongest form of the focus-independence claim: whatever the
    // registry holds, and whatever it would have answered, a held capture
    // means no surface is consulted.
    for kind in [
        SurfaceKind::Modal,
        SurfaceKind::InputDialog,
        SurfaceKind::MenuCard,
        SurfaceKind::TeachingPopover,
        SurfaceKind::MovableCard,
        SurfaceKind::ToastStack,
        SurfaceKind::SearchFilter,
        SurfaceKind::Workspace,
    ] {
        let mut reg = SurfaceRegistry::new();
        add(&mut reg, "focused", kind, vec![]);
        let before = reg.clone();
        let mut captures = dragging_ledger();

        let asked = Cell::new(0usize);
        let escape = escape_with_pre_rank(
            &mut captures,
            &mut reg,
            |_| true,
            |surface| {
                asked.set(asked.get() + 1);
                panic!("the rank walk must not consult {surface:?} while a drag is held")
            },
        );

        assert!(
            escape.drag_cancelled(),
            "{kind:?} stole a Pre-Rank keypress"
        );
        assert_eq!(asked.get(), 0, "{kind:?} was consulted");
        assert_eq!(reg, before, "{kind:?} was dismissed");
    }
}
