//! Tests for the teaching-popover host — `UIF-ATOM-007`, §3.7, and the
//! z-order contract in §5.
//!
//! Five groups:
//!
//! 1. **Opening.** What the host registers, in what order, and the two
//!    refusals.
//! 2. **Dismissal precedence.** The four closers and the one thing that
//!    is deliberately not a closer.
//! 3. **The escape contract.** That the callout answers rank 3, that its
//!    `Escape` is consumed, and that `unwind_answer` never touches the
//!    registry.
//! 4. **The modal focus trap survives.** That a callout opened inside a
//!    modal nests its trap rather than replacing it, and that closing
//!    restores what the modal held.
//! 5. **The menu's roving focus survives.** That a Band 2 neighbour is
//!    not disturbed by the callout's mount or teardown.

use super::*;
use crate::focus::{FocusId, FocusMember, FocusState, GroupId};
use crate::surface::modal::{ModalHost, ModalSizing, ModalSpec};
use crate::surface::zorder::{
    DeclineReason, EscapeOutcome, PointerTarget, SurfaceId, SurfaceKind, SurfaceRegistry,
    UnwindAnswer, ZBand,
};
use iced::{Point, Rectangle, Size};

/// The action type the tests use.
type Action = &'static str;

fn viewport() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(1200.0, 800.0))
}

fn trigger() -> Rectangle {
    Rectangle::new(Point::new(100.0, 100.0), Size::new(120.0, 28.0))
}

fn card_size() -> Size {
    Size::new(300.0, 140.0)
}

fn popover_id() -> SurfaceId {
    SurfaceId::new("teach")
}

fn model_with_action() -> PopoverModel<Action> {
    PopoverModel::new("Heads up", "This is the thing.").with_action("Got it", "ack")
}

/// Open a callout and return it with the collaborators it was opened into.
fn opened(model: PopoverModel<Action>) -> (PopoverHost<Action>, FocusState, SurfaceRegistry) {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let host = PopoverHost::open(
        popover_id(),
        model,
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("a wanted callout with a focusable footer opens");
    (host, focus, registry)
}

// ---------------------------------------------------------------------------
// 1. Opening
// ---------------------------------------------------------------------------

#[test]
fn opening_mounts_the_group_the_trap_the_focus_and_the_surface() {
    let (host, focus, registry) = opened(model_with_action());

    assert!(host.is_open());
    assert_eq!(focus.trap_depth(), 1);
    assert_eq!(focus.active_trap(), Some(host.group_id()));
    assert_eq!(focus.current(), Some(&primary_focus_id()));
    assert!(registry.contains(&popover_id()));
    assert_eq!(
        registry.get(&popover_id()).map(|s| s.kind()),
        Some(SurfaceKind::TeachingPopover),
    );
}

#[test]
fn the_surface_is_registered_over_the_card_not_the_trigger() {
    let (host, _, registry) = opened(model_with_action());
    let surface = registry.get(&popover_id()).expect("registered");
    assert_eq!(surface.regions(), &[host.card()]);
    assert!(surface.contains(Point::new(host.card().x + 1.0, host.card().y + 1.0)));
}

#[test]
fn the_band_is_two_and_the_escape_rank_is_three() {
    // §3.7's "Z-Band 2 (Escape Rank 3)" reconciles with the existing
    // enum exactly — no new band, no retuned rank.
    let kind = SurfaceKind::TeachingPopover;
    assert_eq!(kind.band(), ZBand::Menu);
    assert_eq!(kind.band().index(), 2);
    assert_eq!(kind.band().escape_rank(), 3);
}

#[test]
fn opening_refuses_a_model_that_does_not_want_to_be_open() {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let closed = PopoverModel::<Action>::new("T", "B").with_open(false);
    let result = PopoverHost::open(
        popover_id(),
        closed,
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    );
    assert_eq!(result.err(), Some(OpenRefusal::NotRequested));
    assert_eq!(focus.trap_depth(), 0);
    assert!(
        registry.is_empty(),
        "a refused open leaves no surface behind"
    );
}

#[test]
fn the_footer_roster_always_has_something_for_the_trap() {
    // The `NoFocusableControl` refusal is unreachable through the public
    // model because the dismiss button is framework-supplied. Assert the
    // precondition rather than the dead branch.
    for with_action in [true, false] {
        let mut model = PopoverModel::<Action>::new("T", "B");
        if with_action {
            model = model.with_action("Go", "go");
        }
        let (host, focus, _registry) = opened(model);
        assert!(
            focus.is_focused(&host.model().initial_focus()),
            "with_action({with_action}) must land the keyboard on the initial control",
        );
    }
}

#[test]
fn a_callout_without_an_action_lands_on_dismiss() {
    let (host, focus, _) = opened(PopoverModel::<Action>::new("T", "B"));
    assert_eq!(focus.current(), Some(&dismiss_focus_id()));
    assert_eq!(host.footer_ids(), vec![dismiss_focus_id()]);
}

// ---------------------------------------------------------------------------
// 2. Dismissal precedence
// ---------------------------------------------------------------------------

#[test]
fn the_primary_action_fires_and_closes_the_callout() {
    let (mut host, mut focus, _registry) = opened(model_with_action());
    let event = host.activate_primary(&mut focus);
    assert_eq!(event, PopoverEvent::Action("ack"));
    assert!(!host.is_open());
    assert_eq!(focus.trap_depth(), 0, "the trap goes with the callout");
}

#[test]
fn a_primary_press_on_a_actionless_callout_is_ignored_and_stays_open() {
    let (mut host, mut focus, _registry) = opened(PopoverModel::<Action>::new("T", "B"));
    let event = host.activate_primary(&mut focus);
    assert_eq!(event, PopoverEvent::Ignored(IgnoredReason::NoPrimaryAction));
    assert!(host.is_open());
    assert_eq!(focus.trap_depth(), 1);
}

#[test]
fn the_dismiss_button_closes_without_an_action() {
    let (mut host, mut focus, _registry) = opened(model_with_action());
    let event = host.dismiss_dismiss_button(&mut focus);
    assert_eq!(event, PopoverEvent::Dismissed(DismissCause::DismissButton));
    assert!(!host.is_open());
}

#[test]
fn a_pointer_inside_the_card_is_left_to_the_button_under_it() {
    // The host must not act on an interior click, or a footer press
    // would be handled twice: once by the button and once as a
    // card-level event.
    let (mut host, mut focus, _registry) = opened(model_with_action());
    let inside = Point::new(host.card().x + 10.0, host.card().y + 10.0);
    let event = host.on_pointer(inside, &mut focus);
    assert_eq!(event, PopoverEvent::Ignored(IgnoredReason::InsideCard));
    assert!(host.is_open());
}

#[test]
fn a_pointer_outside_the_card_dismisses_as_click_away() {
    let (mut host, mut focus, _registry) = opened(model_with_action());
    let event = host.on_pointer(Point::new(5.0, 700.0), &mut focus);
    assert_eq!(event, PopoverEvent::Dismissed(DismissCause::ClickOutside));
    assert!(!host.is_open());
}

#[test]
fn the_band_routes_an_outside_click_to_the_dismiss_band() {
    // The Band 2 `CaptureInsideDismissOutside` rule, read off the
    // registry rather than asserted by the host alone.
    let (host, _focus, registry) = opened(model_with_action());
    let route = registry.route_pointer(Point::new(5.0, 700.0));
    assert_eq!(route.target(), &PointerTarget::DismissBand(ZBand::Menu));
    let inside = registry.route_pointer(Point::new(host.card().x + 5.0, host.card().y + 5.0));
    assert_eq!(inside.target(), &PointerTarget::Surface(popover_id()));
}

#[test]
fn no_dismissal_cause_is_pointer_leave() {
    // The absence is the point. §3.7 gives the tooltip "dismisses on
    // pointer leave" and gives the popover no such rule, because the
    // cursor has to cross the gap to reach the footer.
    let causes = [
        DismissCause::PrimaryAction,
        DismissCause::DismissButton,
        DismissCause::Escape,
        DismissCause::ClickOutside,
        DismissCause::Programmatic,
    ];
    for cause in causes {
        let label = format!("{cause:?}");
        assert!(
            !label.to_lowercase().contains("leave") && !label.to_lowercase().contains("hover"),
            "a hover- or leave-triggered dismissal would break the \
             pointer's path to the footer: {label}",
        );
    }
}

#[test]
fn acting_on_a_closed_callout_is_ignored() {
    let (mut host, mut focus, _registry) = opened(model_with_action());
    assert_eq!(
        host.close(&mut focus, &mut SurfaceRegistry::new()),
        PopoverEvent::Dismissed(DismissCause::Programmatic),
    );
    assert_eq!(
        host.activate_primary(&mut focus),
        PopoverEvent::Ignored(IgnoredReason::NotOpen),
    );
    assert_eq!(
        host.on_pointer(Point::new(5.0, 5.0), &mut focus),
        PopoverEvent::Ignored(IgnoredReason::NotOpen),
    );
}

#[test]
fn every_closer_reports_itself_as_closing() {
    assert!(
        PopoverEvent::<Action>::Action("a").closed_the_callout(),
        "acting closes"
    );
    assert!(PopoverEvent::<Action>::Dismissed(DismissCause::Escape).closed_the_callout());
    assert!(!PopoverEvent::<Action>::Ignored(IgnoredReason::NotOpen).closed_the_callout());
}

// ---------------------------------------------------------------------------
// 3. The escape contract
// ---------------------------------------------------------------------------

#[test]
fn escape_is_consumed_and_closes_the_callout() {
    let (mut host, mut focus, _registry) = opened(model_with_action());
    let event = host.on_escape(&mut focus);
    assert_eq!(event, PopoverEvent::Dismissed(DismissCause::Escape));
    assert!(!host.is_open());
    assert_eq!(focus.trap_depth(), 0);
}

#[test]
fn the_unwind_answer_is_dismissed_while_open_and_declined_after() {
    let (mut host, mut focus, _registry) = opened(model_with_action());
    assert_eq!(host.unwind_answer(&mut focus), UnwindAnswer::Dismissed);
    assert_eq!(
        host.unwind_answer(&mut focus),
        UnwindAnswer::Declined(DeclineReason::NotOpen),
    );
}

#[test]
fn unwind_answer_does_not_touch_the_registry() {
    // The escape walk indexes the surface list and removes the entry it
    // was told closed. A host that unregistered from inside the answer
    // would shift the list under its feet.
    let (mut host, mut focus, registry) = opened(model_with_action());
    let before = registry.len();
    let _ = host.unwind_answer(&mut focus);
    assert_eq!(
        registry.len(),
        before,
        "unwind_answer must leave the registry exactly as it found it",
    );
    assert!(registry.contains(&popover_id()));
}

#[test]
fn escape_via_consumes_the_key_for_this_surface() {
    let (mut host, mut focus, mut registry) = opened(model_with_action());
    let outcome = host.escape_via(&mut registry, &mut focus);
    assert!(outcome.is_consumed());
    assert_eq!(outcome.consumed_by(), Some(&popover_id()));
    assert!(outcome.dismissed());
    assert_eq!(
        outcome.refusal(),
        None,
        "an open callout never refuses the key it was ranked first to take",
    );
}

#[test]
fn escape_via_declines_once_closed() {
    let (mut host, mut focus, mut registry) = opened(model_with_action());
    assert_eq!(
        host.close(&mut focus, &mut registry),
        PopoverEvent::Dismissed(DismissCause::Programmatic),
    );
    let outcome = host.escape_via(&mut registry, &mut focus);
    assert!(
        !outcome.is_consumed(),
        "a closed callout must let the key travel, not swallow it",
    );
}

#[test]
fn close_unregisters_the_surface_and_pops_the_trap() {
    let (mut host, mut focus, mut registry) = opened(model_with_action());
    let event = host.close(&mut focus, &mut registry);
    assert_eq!(event, PopoverEvent::Dismissed(DismissCause::Programmatic));
    assert!(!registry.contains(&popover_id()));
    assert_eq!(focus.trap_depth(), 0);
    assert_eq!(focus.group(&group_for()), None, "the footer group is gone");
}

// ---------------------------------------------------------------------------
// 4. The modal focus trap survives
// ---------------------------------------------------------------------------

/// Open a modal with two controls, focus on the second one.
///
/// The registry the modal registers into is returned alongside it: a
/// callout opened "inside" this modal must share that registry, or the
/// escape-ordering assertions below would be walking a stack the modal
/// was never on.
fn modal_focus() -> (ModalHost, FocusState, SurfaceRegistry) {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let spec = ModalSpec::new("editor", "Editor", ModalSizing::about())
        .with_controls(vec![
            FocusMember::new("m:one"),
            FocusMember::new("m:two"),
            FocusMember::disabled("m:three"),
        ])
        .with_primary(FocusId::new("m:two"));
    let host = ModalHost::open(
        spec,
        viewport(),
        Size::new(400.0, 300.0),
        &mut focus,
        &mut registry,
    );
    (host, focus, registry)
}

#[test]
fn a_callout_inside_a_modal_pushes_its_trap_rather_than_replacing_it() {
    let (modal, mut focus, mut registry) = modal_focus();
    assert_eq!(focus.trap_depth(), 1);
    let modal_held = focus.current().cloned().expect("modal holds focus");

    let callout = PopoverHost::open(
        popover_id(),
        model_with_action(),
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("opens inside the modal's trap");

    assert_eq!(
        focus.trap_depth(),
        2,
        "the callout nests; the modal's trap is still underneath",
    );
    assert_eq!(focus.active_trap(), Some(callout.group_id()));
    assert_eq!(focus.current(), Some(&primary_focus_id()));
    assert!(modal.is_open(), "the modal is unaffected by the mount");
    assert_ne!(
        focus.current(),
        Some(&modal_held),
        "focus moved into the callout as it should",
    );
}

#[test]
fn escaping_the_callout_restores_what_the_modal_held() {
    let (modal, mut focus, mut registry) = modal_focus();
    let modal_held = focus.current().cloned().expect("modal holds focus");
    let mut callout = PopoverHost::open(
        popover_id(),
        model_with_action(),
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("opens inside the modal's trap");

    let event = callout.on_escape(&mut focus);
    assert_eq!(event, PopoverEvent::Dismissed(DismissCause::Escape));
    assert_eq!(focus.trap_depth(), 1, "the modal's trap is back on top");
    assert_eq!(
        focus.current(),
        Some(&modal_held),
        "the keyboard goes back exactly where the modal had it",
    );
    assert!(
        modal.is_open(),
        "dismissing the explanation must not destroy \
        the thing being explained"
    );
    assert_eq!(focus.active_trap(), Some(modal.group_id()));
}

#[test]
fn the_modal_still_traps_after_the_callout_closes() {
    let (_modal, mut focus, mut registry) = modal_focus();
    let mut callout = PopoverHost::open(
        popover_id(),
        model_with_action(),
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("opens");

    assert_eq!(
        callout.close(&mut focus, &mut registry),
        PopoverEvent::Dismissed(DismissCause::Programmatic),
    );

    // The modal's trap is live and confines traversal to the modal's
    // own roster — the callout's ids are not reachable.
    let moved = focus.focus_next().expect("the modal has somewhere to go");
    assert!(
        focus.in_active_scope(&moved),
        "traversal stays inside the modal's scope",
    );
    assert_ne!(moved, primary_focus_id());
    assert_ne!(moved, dismiss_focus_id());
}

#[test]
fn the_callout_cannot_steal_focus_outside_the_modal_after_it_closes() {
    let (_modal, mut focus, mut registry) = modal_focus();
    let mut callout = PopoverHost::open(
        popover_id(),
        model_with_action(),
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("opens");
    assert_eq!(
        callout.close(&mut focus, &mut registry),
        PopoverEvent::Dismissed(DismissCause::Programmatic),
    );
    assert!(
        !focus.focus(&primary_focus_id()),
        "the callout's footer ids are gone; the modal's trap must reject them",
    );
}

// ---------------------------------------------------------------------------
// 5. The menu's roving focus survives
// ---------------------------------------------------------------------------

#[test]
fn the_callout_unwinds_before_a_modal_because_of_its_rank() {
    let (modal, mut focus, mut registry) = modal_focus();
    let mut callout = PopoverHost::open(
        popover_id(),
        model_with_action(),
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("opens");

    // One Escape: the rank-3 callout takes it, the rank-2 modal does not.
    let outcome = callout.escape_via(&mut registry, &mut focus);
    assert_eq!(outcome.consumed_by(), Some(&popover_id()));
    assert!(!callout.is_open());
    assert!(modal.is_open());
    assert_eq!(
        outcome.refusal(),
        None,
        "the walk stopped at the callout and never reached the modal",
    );
}

#[test]
fn a_menu_registered_in_band_two_is_untouched_by_the_callout() {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let menu_group = GroupId::new("menu:root");
    focus.register_group(
        menu_group.clone(),
        vec![
            FocusMember::new("menu:row-1"),
            FocusMember::new("menu:row-2"),
        ],
    );
    focus.push_trap(menu_group.clone());
    focus.focus_first_in_group(&menu_group);
    registry.register(
        SurfaceId::new("menu"),
        SurfaceKind::MenuCard,
        vec![Rectangle::new(
            Point::new(0.0, 0.0),
            Size::new(200.0, 200.0),
        )],
    );

    let mut callout = PopoverHost::open(
        popover_id(),
        model_with_action(),
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("opens on top of the menu");

    assert_eq!(focus.trap_depth(), 2);
    assert_eq!(focus.active_trap(), Some(callout.group_id()));

    assert_eq!(
        callout.close(&mut focus, &mut registry),
        PopoverEvent::Dismissed(DismissCause::Programmatic),
    );

    assert_eq!(
        focus.active_trap(),
        Some(&menu_group),
        "the menu's trap is back on top and unchanged",
    );
    assert!(
        focus.group(&menu_group).is_some(),
        "the menu's roster must survive the callout's teardown",
    );
    assert!(registry.contains(&SurfaceId::new("menu")));
    assert!(!registry.contains(&popover_id()));
}

#[test]
fn within_band_two_the_newest_surface_takes_escape_first() {
    // Same rank, so the walk orders by index descending. The callout
    // registered last must be the one that answers.
    let mut registry = SurfaceRegistry::new();
    registry.register(SurfaceId::new("menu"), SurfaceKind::MenuCard, vec![]);
    registry.register(
        SurfaceId::new("teach"),
        SurfaceKind::TeachingPopover,
        vec![],
    );
    assert_eq!(registry.topmost().map(|s| s.id().as_str()), Some("teach"));
    assert_eq!(registry.keyboard_owner(), Some(ZBand::Menu));

    let outcome = registry.escape(|surface| {
        if surface.id().as_str() == "teach" {
            UnwindAnswer::Dismissed
        } else {
            UnwindAnswer::Declined(DeclineReason::NotOpen)
        }
    });
    assert_eq!(outcome.consumed_by(), Some(&SurfaceId::new("teach")));
    assert!(
        outcome.declined_before().is_empty(),
        "the menu must never have been asked",
    );
}

#[test]
fn a_declined_callout_lets_the_key_reach_the_modal() {
    let (mut modal, mut focus, mut registry) = modal_focus();
    let mut callout = PopoverHost::open(
        popover_id(),
        model_with_action(),
        trigger(),
        card_size(),
        viewport(),
        &mut focus,
        &mut registry,
    )
    .expect("opens");
    assert_eq!(
        callout.close(&mut focus, &mut registry),
        PopoverEvent::Dismissed(DismissCause::Programmatic),
    );

    let outcome = modal.escape_via(&mut registry, &mut focus);
    assert!(
        !modal.is_open(),
        "the key travelled past the closed callout and reached the modal, \
         which dismisses because it is clean",
    );
    assert!(
        !registry.contains(&popover_id()),
        "the closed callout is no longer on the stack either",
    );
    match outcome {
        EscapeOutcome::Consumed { surface, .. } => assert_eq!(&surface, modal.id()),
        EscapeOutcome::Ignored(refusal) => {
            panic!("expected the modal to take the key: {refusal:?}")
        }
    }
}

#[test]
fn the_group_id_is_shared_so_only_one_callout_can_own_it() {
    // The framework allows one teaching popover at a time (Band 2's
    // single-flyout rule), so the group is a fixed id rather than one
    // derived per callout.
    assert_eq!(group_for(), GroupId::new("popover"));
    let (host, _, _) = opened(model_with_action());
    assert_eq!(host.group_id(), &group_for());
}
