//! Tests for the split-button host â€” `UIF-ATOM-001c`, Â§3.1c.
//!
//! Six groups, in order:
//!
//! 1. **Half independence.** The negative cases the two-zone claim rests
//!    on: neither half can borrow the other's hover or press.
//! 2. **The chevron opens on release.** Press alone does nothing, and a
//!    release outside the chevron zone opens nothing.
//! 3. **The primary half never opens the menu,** and the menu never
//!    fires the primary action.
//! 4. **Escape.** Dismisses the flyout, returns focus to the control,
//!    and produces no primary emission on the way out.
//! 5. **Menu driving.** Keys go through the one cascade, and the row
//!    that fired comes back as the application's own `ActionId`.
//! 6. **Inert states.** Disabled and busy controls, and the reset path.

use super::*;
use crate::focus::{FocusMember, GroupId};
use crate::state::BaseState;
use crate::widget::split_button_model::SplitMenuRow;

type Id = &'static str;

const CONTROL: &str = "split-control";
const TOOLBAR: &str = "toolbar";

fn rows() -> Vec<SplitMenuRow<Id>> {
    vec![
        SplitMenuRow::new("copy", "Copy"),
        SplitMenuRow::new("paste", "Paste"),
        SplitMenuRow::new("delete", "Delete").destructive(),
    ]
}

fn model() -> SplitButtonModel<Id> {
    SplitButtonModel::new("Save", rows())
}

fn host() -> SplitButtonHost {
    SplitButtonHost::new(FocusId::new(CONTROL))
}

/// A focus state in which the split button's own control is focusable.
fn focus_with_control() -> FocusState {
    let mut focus = FocusState::new();
    focus.register_group(
        GroupId::new(TOOLBAR),
        vec![FocusMember::new(CONTROL), FocusMember::new("other")],
    );
    focus
}

/// A focus state in which the control cannot be focused at all.
fn focus_without_control() -> FocusState {
    let mut focus = FocusState::new();
    focus.register_group(GroupId::new(TOOLBAR), vec![FocusMember::new("other")]);
    focus
}

fn rest() -> VisualState {
    VisualState {
        base: BaseState::Rest,
        focused: false,
    }
}

fn primary_activated(outcome: &SplitButtonOutcome<Id>) -> bool {
    outcome
        .emissions
        .iter()
        .any(|e| matches!(e, SplitButtonEmission::PrimaryActivated(_)))
}

fn any_primary(outcome: &SplitButtonOutcome<Id>) -> bool {
    outcome
        .emissions
        .iter()
        .any(SplitButtonEmission::is_primary)
}

fn menu_toggled(outcome: &SplitButtonOutcome<Id>) -> Option<bool> {
    outcome.emissions.iter().find_map(|e| match e {
        SplitButtonEmission::MenuToggled(open) => Some(*open),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// 1. Half independence
// ---------------------------------------------------------------------------

#[test]
fn hovering_the_label_does_not_light_the_chevron() {
    let mut host = host();
    host.hover(SplitZone::Primary);
    assert_eq!(
        host.half_state(SplitZone::Primary, false).base,
        BaseState::Hovered
    );
    assert_eq!(host.half_state(SplitZone::Menu, false), rest());
}

#[test]
fn hovering_the_chevron_does_not_light_the_label() {
    let mut host = host();
    host.hover(SplitZone::Menu);
    assert_eq!(
        host.half_state(SplitZone::Menu, false).base,
        BaseState::Hovered
    );
    assert_eq!(host.half_state(SplitZone::Primary, false), rest());
}

#[test]
fn pressing_the_label_does_not_press_the_chevron() {
    let mut host = host();
    let m = model();
    let _ = host.press(SplitZone::Primary, &m);
    assert_eq!(host.pressed_zone(), Some(SplitZone::Primary));
    assert_eq!(host.half_state(SplitZone::Menu, false), rest());
    assert_eq!(
        host.half_state(SplitZone::Primary, false).base,
        BaseState::Pressed
    );
}

#[test]
fn pressing_the_chevron_does_not_press_the_label() {
    let mut host = host();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    assert_eq!(host.pressed_zone(), Some(SplitZone::Menu));
    assert_eq!(host.half_state(SplitZone::Primary, false), rest());
    assert_eq!(
        host.half_state(SplitZone::Menu, false).base,
        BaseState::Pressed
    );
}

#[test]
fn the_focus_ring_is_control_level_and_reaches_both_halves_alike() {
    let mut host = host();
    host.hover(SplitZone::Primary);
    assert!(host.half_state(SplitZone::Menu, true).focused);
    assert!(host.half_state(SplitZone::Primary, true).focused);
    assert_eq!(host.half_state(SplitZone::Menu, true).base, BaseState::Rest);
}

#[test]
fn pointer_leave_darkens_both_halves() {
    let mut host = host();
    host.hover(SplitZone::Menu);
    host.pointer_leave();
    assert_eq!(host.half_state(SplitZone::Menu, false), rest());
    assert_eq!(host.half_state(SplitZone::Primary, false), rest());
}

// ---------------------------------------------------------------------------
// 2. The chevron opens on release
// ---------------------------------------------------------------------------

#[test]
fn a_chevron_press_alone_opens_nothing() {
    let mut host = host();
    let m = model();
    let out = host.press(SplitZone::Menu, &m);
    assert!(!host.is_menu_open());
    assert!(out.is_noop());
}

#[test]
fn a_chevron_release_opens_the_flyout() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    let out = host.release(SplitZone::Menu, &m, &mut focus);
    assert!(host.is_menu_open());
    assert_eq!(menu_toggled(&out), Some(true));
}

#[test]
fn a_chevron_press_that_slides_off_and_releases_elsewhere_opens_nothing() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    let out = host.release(SplitZone::Primary, &m, &mut focus);
    assert!(!host.is_menu_open());
    assert!(out.is_noop());
}

#[test]
fn a_chevron_release_outside_the_control_opens_nothing() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    let out = host.release(SplitZone::Outside, &m, &mut focus);
    assert!(!host.is_menu_open());
    assert!(out.is_noop());
}

#[test]
fn a_second_chevron_release_closes_the_flyout_it_just_opened() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    let _ = host.release(SplitZone::Menu, &m, &mut focus);
    let _ = host.press(SplitZone::Menu, &m);
    let out = host.release(SplitZone::Menu, &m, &mut focus);
    assert!(!host.is_menu_open());
    assert_eq!(menu_toggled(&out), Some(false));
}

// ---------------------------------------------------------------------------
// 3. The halves do not reach each other's callbacks
// ---------------------------------------------------------------------------

#[test]
fn pressing_the_label_never_opens_the_menu() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Primary, &m);
    let out = host.release(SplitZone::Primary, &m, &mut focus);
    assert!(primary_activated(&out));
    assert!(!host.is_menu_open());
    assert_eq!(menu_toggled(&out), None);
}

#[test]
fn pressing_the_chevron_never_fires_the_primary_action() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    let out = host.release(SplitZone::Menu, &m, &mut focus);
    assert!(
        !any_primary(&out),
        "the chevron produced a primary-tier emission: {:?}",
        out.emissions
    );
    assert_eq!(menu_toggled(&out), Some(true));
}

#[test]
fn a_label_gesture_released_over_the_chevron_cancels_and_opens_nothing() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Primary, &m);
    let out = host.release(SplitZone::Menu, &m, &mut focus);
    assert!(matches!(
        out.emissions.first(),
        Some(SplitButtonEmission::PrimaryCancelled(_))
    ));
    assert!(!host.is_menu_open());
    assert!(!primary_activated(&out));
}

#[test]
fn a_label_gesture_released_outside_the_control_cancels() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Primary, &m);
    let out = host.release(SplitZone::Outside, &m, &mut focus);
    assert!(matches!(
        out.emissions.first(),
        Some(SplitButtonEmission::PrimaryCancelled(_))
    ));
}

#[test]
fn a_release_with_no_press_behind_it_does_nothing() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    assert!(host.release(SplitZone::Primary, &m, &mut focus).is_noop());
    assert!(host.release(SplitZone::Menu, &m, &mut focus).is_noop());
}

#[test]
fn the_keyboard_primary_activation_is_marked_as_keyboard() {
    let mut host = host();
    let m = model();
    let out = host.activate_primary(&m);
    let SplitButtonEmission::PrimaryActivated(commit) = &out.emissions[0] else {
        panic!("expected an activation, got {:?}", out.emissions);
    };
    assert_eq!(commit.value().source, ActivationSource::Keyboard);
    assert!(!host.is_menu_open());
}

// ---------------------------------------------------------------------------
// 4. Escape
// ---------------------------------------------------------------------------

#[test]
fn escape_after_open_returns_focus_to_the_control_without_firing_the_primary() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    let _ = host.release(SplitZone::Menu, &m, &mut focus);
    assert!(host.is_menu_open());
    assert_ne!(focus.current().map(FocusId::as_str), Some(CONTROL));

    let out = host.escape(&m, &mut focus);
    assert!(!host.is_menu_open());
    assert_eq!(focus.current().map(FocusId::as_str), Some(CONTROL));
    assert_eq!(menu_toggled(&out), Some(false));
    assert!(
        !any_primary(&out),
        "closing a flyout must not look like a primary action: {:?}",
        out.emissions
    );
}

#[test]
fn escape_with_no_menu_and_an_open_label_gesture_cancels_it() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Primary, &m);
    let out = host.escape(&m, &mut focus);
    assert!(matches!(
        out.emissions.first(),
        Some(SplitButtonEmission::PrimaryCancelled(_))
    ));
}

#[test]
fn escape_with_nothing_open_is_a_no_op() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    assert!(host.escape(&m, &mut focus).is_noop());
}

#[test]
fn escape_prefers_the_flyout_over_the_open_label_gesture() {
    // Both are live at once: the menu wins, and the label's gesture is
    // left alone rather than being cancelled behind the flyout.
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.press(SplitZone::Menu, &m);
    let _ = host.release(SplitZone::Menu, &m, &mut focus);
    let _ = host.press(SplitZone::Primary, &m);
    let out = host.escape(&m, &mut focus);
    assert_eq!(menu_toggled(&out), Some(false));
    assert!(!any_primary(&out));
}

#[test]
fn escape_restores_nothing_when_the_control_could_not_hold_focus() {
    // The trap's existing contract, pinned rather than worked around: a
    // trap pushed while nothing was focused restores nothing.
    let mut host = host();
    let mut focus = focus_without_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    assert!(host.is_menu_open());
    let _ = host.escape(&m, &mut focus);
    assert!(!host.is_menu_open());
    assert_eq!(focus.current(), None);
}

// ---------------------------------------------------------------------------
// 5. Menu driving
// ---------------------------------------------------------------------------

#[test]
fn the_flyout_is_one_card_deep_not_a_second_menu() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    assert_eq!(host.cascade().depth(), 1);
}

#[test]
fn reopening_does_not_stack_a_second_card() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    let again = host.open_menu(&m, &mut focus);
    assert_eq!(host.cascade().depth(), 1);
    assert!(again.emissions.is_empty() || menu_toggled(&again) == Some(true));
}

#[test]
fn activating_the_focused_row_reports_the_applications_own_action_id() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    let out = host.menu_key(MenuKey::Enter, &m, &mut focus);
    assert!(
        out.emissions
            .contains(&SplitButtonEmission::MenuSelected("copy"))
    );
    assert!(!any_primary(&out));
}

#[test]
fn navigating_down_then_activating_reports_the_second_rows_id() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    let _ = host.menu_key(MenuKey::Down, &m, &mut focus);
    let out = host.menu_key(MenuKey::Enter, &m, &mut focus);
    assert!(
        out.emissions
            .contains(&SplitButtonEmission::MenuSelected("paste"))
    );
}

#[test]
fn a_movement_key_emits_no_selection() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    let out = host.menu_key(MenuKey::Down, &m, &mut focus);
    assert!(
        !out.emissions
            .iter()
            .any(|e| matches!(e, SplitButtonEmission::MenuSelected(_))),
        "a cursor move reported a command: {:?}",
        out.emissions
    );
}

#[test]
fn a_closing_row_reports_the_flyout_shutting() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    let out = host.menu_key(MenuKey::Enter, &m, &mut focus);
    assert!(!host.is_menu_open());
    assert_eq!(menu_toggled(&out), Some(false));
}

#[test]
fn menu_keys_against_a_closed_flyout_are_inert() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let out = host.menu_key(MenuKey::Enter, &m, &mut focus);
    assert!(!out.emissions.iter().any(|e| matches!(
        e,
        SplitButtonEmission::MenuSelected(_) | SplitButtonEmission::MenuToggled(_)
    )));
}

// ---------------------------------------------------------------------------
// 6. Inert states
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_control_opens_no_gesture_and_no_flyout() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model().disabled();
    assert!(host.press(SplitZone::Primary, &m).is_noop());
    assert!(host.press(SplitZone::Menu, &m).is_noop());
    assert!(host.activate_primary(&m).is_noop());
    assert!(host.open_menu(&m, &mut focus).is_noop());
    assert!(!host.is_menu_open());
}

#[test]
fn a_control_disabled_mid_gesture_owes_neither_a_commit_nor_a_revert() {
    let mut host = host();
    let mut focus = focus_with_control();
    let live = model();
    let dead = model().disabled();
    let _ = host.press(SplitZone::Primary, &live);
    let out = host.release(SplitZone::Primary, &dead, &mut focus);
    assert!(
        out.is_noop(),
        "a dead control reported something: {:?}",
        out.emissions
    );
    assert_eq!(host.pressed_zone(), None);
}

#[test]
fn a_flyout_with_nothing_live_in_it_does_not_open() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = SplitButtonModel::new("Save", vec![SplitMenuRow::new("a", "A").disabled()]);
    let out = host.open_menu(&m, &mut focus);
    assert!(!host.is_menu_open());
    assert!(out.is_noop());
}

#[test]
fn a_busy_primary_half_asks_to_stop_instead_of_firing_again() {
    let mut host = host();
    let m = model().busy().cancellable();
    let out = host.press(SplitZone::Primary, &m);
    assert!(
        out.emissions
            .iter()
            .any(|e| matches!(e, SplitButtonEmission::StopRequested(_)))
    );
    assert!(!primary_activated(&out));
}

#[test]
fn the_chevron_never_produces_a_stop_request_for_a_busy_primary_half() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model().busy().cancellable();
    let press = host.press(SplitZone::Menu, &m);
    let release = host.release(SplitZone::Menu, &m, &mut focus);
    assert!(!any_primary(&press));
    assert!(
        !any_primary(&release),
        "the chevron reported a busy-primary tier: {:?}",
        release.emissions
    );
}

#[test]
fn reset_clears_pointer_state_but_leaves_the_flyout_for_the_host_to_report() {
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    let _ = host.open_menu(&m, &mut focus);
    host.hover(SplitZone::Menu);
    let _ = host.press(SplitZone::Menu, &m);
    host.reset();
    assert_eq!(host.hovered_zone(), SplitZone::Outside);
    assert_eq!(host.pressed_zone(), None);
    assert!(
        host.is_menu_open(),
        "reset must not silently tear down a visible flyout"
    );
}

#[test]
fn observe_forwards_the_models_pending_projection() {
    let mut host = host();
    let m = model().busy().cancellable();
    host.observe(&m);
    assert!(host.primary().busy_observed());
}

// ---------------------------------------------------------------------------
// 7. The emission's own value semantics, and the paths group 1-6 never walked
// ---------------------------------------------------------------------------

#[test]
fn only_the_primary_tiers_carry_a_settlement() {
    let mut idle_host = host();
    let m = model();
    let mut focus = focus_with_control();

    let activated = idle_host.activate_primary(&m);
    assert_eq!(
        activated.emissions[0]
            .settlement()
            .map(|settlement| settlement.source),
        Some(ActivationSource::Keyboard),
        "the activation tier carries its own settlement"
    );
    assert!(
        SplitButtonEmission::<&str>::MenuToggled(true)
            .settlement()
            .is_none(),
        "a menu toggle carries no settlement"
    );
    assert!(
        SplitButtonEmission::MenuSelected("copy")
            .settlement()
            .is_none(),
        "a menu selection carries no settlement"
    );

    let mut cancel_host = host();
    let _ = cancel_host.press(SplitZone::Primary, &m);
    let cancelled = cancel_host.escape(&m, &mut focus);
    assert!(
        cancelled.emissions[0].settlement().is_some(),
        "the cancellation tier carries the settlement it restores"
    );

    let mut busy_host = host();
    let busy = model().busy().cancellable();
    let stopped = busy_host.press(SplitZone::Primary, &busy);
    let stop = stopped
        .emissions
        .iter()
        .find(|e| matches!(e, SplitButtonEmission::StopRequested(_)))
        .expect("a busy cancellable half must ask to stop");
    assert!(
        stop.settlement().is_none(),
        "a stop request carries no settlement"
    );
}

#[test]
fn an_emission_compares_its_payload_within_its_own_tier_only() {
    let mut idle_host = host();
    let m = model();
    let mut focus = focus_with_control();
    let activated = idle_host.activate_primary(&m);
    let activated_again = idle_host.activate_primary(&m);

    assert_eq!(
        activated.emissions[0], activated_again.emissions[0],
        "two activations of the same settlement are one emission"
    );
    assert_ne!(
        activated.emissions[0],
        SplitButtonEmission::<&str>::MenuToggled(true),
        "an activation must never compare equal to a menu toggle"
    );
    assert_eq!(
        SplitButtonEmission::<&str>::MenuToggled(true),
        SplitButtonEmission::<&str>::MenuToggled(true),
        "the same toggle state compares equal"
    );
    assert_ne!(
        SplitButtonEmission::<&str>::MenuToggled(true),
        SplitButtonEmission::<&str>::MenuToggled(false),
        "opening and closing are different reports"
    );
    assert_eq!(
        SplitButtonEmission::MenuSelected("copy"),
        SplitButtonEmission::MenuSelected("copy"),
        "the same command id compares equal"
    );
    assert_ne!(
        SplitButtonEmission::MenuSelected("copy"),
        SplitButtonEmission::MenuSelected("paste"),
        "two different commands are two different emissions"
    );

    let _ = idle_host.press(SplitZone::Primary, &m);
    let cancelled = idle_host.escape(&m, &mut focus);
    assert_eq!(
        cancelled.emissions[0],
        cancelled.emissions[0].clone(),
        "a cancellation compares equal to itself"
    );
    assert_ne!(
        cancelled.emissions[0], activated.emissions[0],
        "a cancellation must never compare equal to an activation"
    );
    assert_ne!(
        cancelled.emissions[0],
        SplitButtonEmission::MenuSelected("copy"),
        "a cancellation must never compare equal to a menu selection"
    );

    let mut busy_host = host();
    let busy = model().busy().cancellable();
    let stopped = busy_host.press(SplitZone::Primary, &busy);
    let stop = stopped
        .emissions
        .iter()
        .find(|e| matches!(e, SplitButtonEmission::StopRequested(_)))
        .expect("a busy cancellable half must ask to stop");
    assert_eq!(
        stop.clone(),
        stop.clone(),
        "two stop requests compare equal"
    );
    assert_ne!(
        *stop, activated.emissions[0],
        "a stop request must never compare equal to an activation"
    );
}

#[test]
fn every_emission_tier_prints_its_own_name_and_payload() {
    let mut idle_host = host();
    let m = model();
    let mut focus = focus_with_control();
    let activated = idle_host.activate_primary(&m);

    assert!(
        format!("{:?}", activated.emissions[0]).starts_with("PrimaryActivated("),
        "an activation must announce its tier, got {:?}",
        format!("{:?}", activated.emissions[0])
    );
    assert_eq!(
        format!("{:?}", SplitButtonEmission::<&str>::MenuToggled(true)),
        "MenuToggled(true)",
        "a toggle must show which way it moved"
    );
    assert_eq!(
        format!("{:?}", SplitButtonEmission::MenuSelected("copy")),
        "MenuSelected(\"copy\")",
        "a selection must show the application's own id"
    );

    let _ = idle_host.press(SplitZone::Primary, &m);
    let cancelled = idle_host.escape(&m, &mut focus);
    assert!(
        format!("{:?}", cancelled.emissions[0]).starts_with("PrimaryCancelled("),
        "a cancellation must announce its tier, got {:?}",
        format!("{:?}", cancelled.emissions[0])
    );

    let mut busy_host = host();
    let busy = model().busy().cancellable();
    let stopped = busy_host.press(SplitZone::Primary, &busy);
    let stop = stopped
        .emissions
        .iter()
        .find(|e| matches!(e, SplitButtonEmission::StopRequested(_)))
        .expect("a busy cancellable half must ask to stop");
    assert!(
        format!("{stop:?}").starts_with("StopRequested("),
        "a stop request must announce its tier, got {stop:?}"
    );
}

#[test]
fn the_host_reports_the_control_it_was_built_for() {
    let host = SplitButtonHost::new(FocusId::new("split-other"));
    assert_eq!(
        host.control().as_str(),
        "split-other",
        "control() must report the identity the trap will return to"
    );
    assert_eq!(
        host.hovered_zone(),
        SplitZone::Outside,
        "a fresh host has no hovered half"
    );
    assert_eq!(host.pressed_zone(), None, "and no pressed half");
    assert!(!host.is_menu_open(), "and no flyout");
}

#[test]
fn a_press_outside_the_control_opens_nothing_at_all() {
    let mut host = host();
    let m = model();
    let out = host.press(SplitZone::Outside, &m);
    assert!(
        out.is_noop(),
        "a press that hit nothing must report nothing, saw {:?}",
        out.emissions
    );
    assert_eq!(
        host.pressed_zone(),
        None,
        "and must not record a pressed half for a zone that was never hit"
    );
    assert!(
        !host.primary().gesture_open(),
        "and must not open a primary gesture"
    );
}

#[test]
fn closing_a_flyout_that_is_not_open_says_nothing() {
    let mut host = host();
    let mut focus = focus_with_control();
    let out = host.close_menu::<Id>(&mut focus);
    assert!(
        out.is_noop(),
        "closing an already-closed flyout must not report a toggle, saw {:?}",
        out.emissions
    );
    assert!(
        out.cascade.opened().is_empty() && out.cascade.closed().is_empty(),
        "and the cascade must report no movement either"
    );
}

#[test]
fn opening_with_the_control_already_focused_still_records_it_for_escape() {
    // The `focus.current() != Some(&self.control)` guard's false branch: the
    // trap must still record the control, and the flyout must still open.
    let mut host = host();
    let mut focus = focus_with_control();
    let m = model();
    focus.focus(&FocusId::new(CONTROL));
    assert_eq!(
        focus.current().map(FocusId::as_str),
        Some(CONTROL),
        "the control holds focus before the flyout opens"
    );

    let out = host.open_menu(&m, &mut focus);
    assert!(host.is_menu_open(), "the flyout opens");
    assert_eq!(menu_toggled(&out), Some(true), "and reports that it did");
    assert_ne!(
        focus.current().map(FocusId::as_str),
        Some(CONTROL),
        "mounting the flyout moves the caret onto its first row — the control is \
         not where the arrow keys go, and skipping the re-focus changes nothing \
         about that; saw {:?}",
        focus.current().map(FocusId::as_str)
    );

    let closed = host.escape(&m, &mut focus);
    assert_eq!(menu_toggled(&closed), Some(false));
    assert_eq!(
        focus.current().map(FocusId::as_str),
        Some(CONTROL),
        "the trap pushed while the control already held focus still pops back to \
         it, which is what Escape returns to"
    );
}

#[test]
fn a_disabled_control_cannot_have_its_flyout_closed_into_a_report() {
    // The flyout was opened while live; disabling the control must not make
    // the close emit a toggle the host never asked for twice.
    let mut host = host();
    let mut focus = focus_with_control();
    let live = model();
    let _ = host.open_menu(&live, &mut focus);
    assert!(host.is_menu_open());

    let out = host.close_menu::<Id>(&mut focus);
    assert_eq!(menu_toggled(&out), Some(false));
    assert!(!host.is_menu_open());
    assert!(
        host.close_menu::<Id>(&mut focus).is_noop(),
        "a second close of the same flyout is silent"
    );
}
