//! Tests for `ui_iced/overlay.rs`.
//!
//! iced widget rendering cannot be driven headlessly without the simulator,
//! so these tests cover the state and mapping layer the widgets are built
//! from: the overlay-slot transitions, the context-menu item → row
//! projection, the anchor/clamp geometry, and the confirm/cancel →
//! `Message` mapping. Where a widget's public shape *is* observable without
//! a renderer — its `Length` size hint — that is asserted too, which is what
//! pins the layering contracts.

use super::*;
use iced::{Element, Point, Size};

/// The app-side message vocabulary the primitives are wired to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TestMessage {
    Confirm,
    Cancel,
    Close,
}

/// A caller-defined modal vocabulary — the primitive must stay generic over
/// whatever the panel invents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestModal {
    Save,
    Rename,
}

/// The viewport the geometry tests clamp against.
fn viewport() -> Size {
    Size::new(800.0, 600.0)
}

/// A freshly closed slot with the modal kind pinned.
fn closed_slot() -> OverlaySlot<TestModal> {
    OverlaySlot::closed()
}

// --- overlay slot: initial state -------------------------------------------

#[test]
fn slot_defaults_to_closed() {
    let slot: OverlaySlot<TestModal> = OverlaySlot::default();

    assert!(slot.is_closed());
    assert!(!slot.is_context_menu());
    assert!(!slot.is_modal());
    assert_eq!(slot.anchor(), None);
    assert_eq!(slot.modal_kind(), None);
}

#[test]
fn closed_constructor_matches_default() {
    assert_eq!(OverlaySlot::<TestModal>::closed(), OverlaySlot::default());
}

// --- overlay slot: opening --------------------------------------------------

#[test]
fn open_context_menu_records_the_anchor() {
    let mut slot = closed_slot();
    let anchor = Point::new(123.0, 45.0);

    assert!(slot.open_context_menu(anchor));

    assert!(slot.is_context_menu());
    assert!(!slot.is_modal());
    assert_eq!(slot.anchor(), Some(anchor));
    assert_eq!(slot.modal_kind(), None);
}

#[test]
fn open_modal_records_the_kind() {
    let mut slot = closed_slot();

    slot.open_modal(TestModal::Rename);

    assert!(slot.is_modal());
    assert!(!slot.is_context_menu());
    assert_eq!(slot.modal_kind(), Some(&TestModal::Rename));
    assert_eq!(slot.anchor(), None);
}

#[test]
fn a_second_context_menu_replaces_the_first_anchor() {
    let mut slot = closed_slot();
    slot.open_context_menu(Point::new(10.0, 10.0));

    assert!(slot.open_context_menu(Point::new(90.0, 70.0)));
    assert_eq!(slot.anchor(), Some(Point::new(90.0, 70.0)));
}

// --- overlay slot: modality -------------------------------------------------

#[test]
fn context_menu_cannot_open_while_a_modal_is_up() {
    let mut slot = closed_slot();
    slot.open_modal(TestModal::Save);

    assert!(
        !slot.open_context_menu(Point::new(5.0, 5.0)),
        "a blocking modal owns the input, so the request must be refused"
    );
    assert_eq!(
        slot,
        OverlaySlot::Modal(TestModal::Save),
        "the refused request must leave the modal untouched"
    );
}

#[test]
fn opening_a_modal_replaces_an_open_context_menu() {
    let mut slot = closed_slot();
    slot.open_context_menu(Point::new(40.0, 40.0));

    slot.open_modal(TestModal::Save);

    assert!(slot.is_modal());
    assert!(!slot.is_context_menu());
    assert_eq!(slot.anchor(), None);
}

#[test]
fn opening_a_second_modal_replaces_the_first_kind() {
    let mut slot = closed_slot();
    slot.open_modal(TestModal::Save);
    slot.open_modal(TestModal::Rename);

    assert_eq!(slot.modal_kind(), Some(&TestModal::Rename));
}

// --- overlay slot: dismissal ----------------------------------------------

#[test]
fn close_reports_true_when_an_overlay_was_open() {
    let mut context_menu = closed_slot();
    context_menu.open_context_menu(Point::new(1.0, 1.0));
    assert!(context_menu.close());
    assert!(context_menu.is_closed());

    let mut modal_slot = closed_slot();
    modal_slot.open_modal(TestModal::Save);
    assert!(modal_slot.close());
    assert!(modal_slot.is_closed());
}

#[test]
fn close_reports_false_when_nothing_was_open() {
    let mut slot = closed_slot();

    assert!(
        !slot.close(),
        "nothing was open, so a dismiss must not claim to have consumed the key"
    );
    assert!(slot.is_closed());
}

#[test]
fn closing_twice_only_reports_once() {
    let mut slot = closed_slot();
    slot.open_modal(TestModal::Save);

    assert!(slot.close());
    assert!(!slot.close());
}

#[test]
fn a_dismissed_slot_can_open_again() {
    let mut slot = closed_slot();
    slot.open_modal(TestModal::Save);
    assert!(slot.close());

    // The modal-blocking rule must not leak past dismissal.
    assert!(slot.open_context_menu(Point::new(7.0, 8.0)));
    assert_eq!(slot.anchor(), Some(Point::new(7.0, 8.0)));
}

// --- geometry ---------------------------------------------------------------

/// A stand-in card box: 200px wide, `rows` rows of 24px inside 4px of
/// padding.
///
/// These are the numbers the removed legacy flat-card model produced. They
/// stay here, local to the tests, because [`clamp_anchor`] is generic over
/// any box and the clamp behaviour under test does not depend on where the
/// size came from.
fn card(rows: usize) -> Size {
    Size::new(200.0, 4.0 * 2.0 + 24.0 * rows as f32)
}

#[test]
fn clamp_anchor_leaves_a_fully_visible_box_untouched() {
    let size = card(3);
    let anchor = Point::new(10.0, 10.0);

    assert_eq!(clamp_anchor(anchor, size, viewport()), anchor);
}

#[test]
fn clamp_anchor_pulls_the_box_back_from_the_right_edge() {
    let size = card(3);

    let clamped = clamp_anchor(Point::new(795.0, 10.0), size, viewport());

    assert_eq!(clamped.x, viewport().width - size.width);
    assert_eq!(clamped.y, 10.0);
}

#[test]
fn clamp_anchor_pulls_the_box_back_from_the_bottom_edge() {
    let size = card(3);

    let clamped = clamp_anchor(Point::new(10.0, 599.0), size, viewport());

    assert_eq!(clamped.x, 10.0);
    assert_eq!(clamped.y, viewport().height - size.height);
}

#[test]
fn clamp_anchor_pulls_back_on_both_axes_at_the_corner() {
    let size = card(3);

    let clamped = clamp_anchor(Point::new(4000.0, 4000.0), size, viewport());

    assert_eq!(
        clamped,
        Point::new(
            viewport().width - size.width,
            viewport().height - size.height
        )
    );
}

#[test]
fn clamp_anchor_never_goes_negative_when_the_viewport_is_smaller_than_the_box() {
    let tiny = Size::new(20.0, 20.0);

    let clamped = clamp_anchor(Point::new(500.0, 500.0), card(30), tiny);

    assert_eq!(
        clamped,
        Point::new(0.0, 0.0),
        "the lower bound must win so the card's top-left corner stays on screen"
    );
}

#[test]
fn clamp_anchor_clamps_a_negative_anchor_to_the_origin() {
    let clamped = clamp_anchor(Point::new(-50.0, -25.0), card(2), viewport());

    assert_eq!(clamped, Point::new(0.0, 0.0));
}

#[test]
fn clamp_anchor_is_idempotent() {
    let size = card(5);
    let once = clamp_anchor(Point::new(900.0, 700.0), size, viewport());
    let twice = clamp_anchor(once, size, viewport());

    assert_eq!(once, twice);
}

// --- confirm / cancel mapping -----------------------------------------------

fn pair(confirm_enabled: bool) -> (Option<TestMessage>, TestMessage) {
    let (confirm, cancel) = confirm_cancel_pair(|outcome| match outcome {
        ConfirmCancel::Confirm => TestMessage::Confirm,
        ConfirmCancel::Cancel => TestMessage::Cancel,
    });

    (confirm_press(confirm_enabled, confirm), cancel)
}

#[test]
fn confirm_cancel_pair_produces_both_messages() {
    let (confirm, cancel) = pair(true);

    assert_eq!(confirm, Some(TestMessage::Confirm));
    assert_eq!(cancel, TestMessage::Cancel);
}

#[test]
fn a_disabled_confirm_emits_nothing() {
    let (confirm, cancel) = pair(false);

    assert_eq!(
        confirm, None,
        "an invalid dialog must leave the confirm button inert"
    );
    assert_eq!(
        cancel,
        TestMessage::Cancel,
        "cancel must keep working even when confirm is disabled"
    );
}

#[test]
fn confirm_cancel_outcomes_map_to_dialog_actions() {
    assert_eq!(
        ConfirmCancel::Confirm.to_dialog_action(),
        DialogAction::Confirm
    );
    assert_eq!(
        ConfirmCancel::Cancel.to_dialog_action(),
        DialogAction::Cancel
    );
}

#[test]
fn the_two_outcomes_are_distinct() {
    assert_ne!(ConfirmCancel::Confirm, ConfirmCancel::Cancel);
}

// --- widget shape contracts (observable without a renderer) ------------------

#[test]
fn both_blockers_fill_the_available_area() {
    let transparent: Element<'_, TestMessage> = input_blocker(TestMessage::Close).into();
    let scrim: Element<'_, TestMessage> = scrim_blocker(TestMessage::Close).into();

    assert_eq!(
        transparent.as_widget().size(),
        Size::new(Length::Fill, Length::Fill)
    );
    assert_eq!(
        scrim.as_widget().size(),
        Size::new(Length::Fill, Length::Fill)
    );
}

#[test]
fn scrim_keeps_the_default_cursor_visible() {
    assert_eq!(
        SCRIM_INTERACTION,
        iced::mouse::Interaction::Idle,
        "a modal scrim should block underlying interaction without hiding the cursor"
    );
}

#[test]
fn menu_click_away_blocker_keeps_the_default_cursor_visible() {
    assert_eq!(
        MENU_BLOCKER_INTERACTION,
        iced::mouse::Interaction::Idle,
        "the menu blocker must dismiss outside clicks without hiding the pointer"
    );
}

#[test]
fn modal_fills_its_parent() {
    let card: Element<'_, TestMessage> = text("dialog").into();

    let modal_element = modal(card, TestMessage::Close);

    assert_eq!(
        modal_element.as_widget().size(),
        Size::new(Length::Fill, Length::Fill)
    );
}

#[test]
fn modal_at_fills_its_parent_like_modal() {
    // The anchored sibling of `modal` keeps the same
    // full-area scrim contract; only the card placement differs.
    let card: Element<'_, TestMessage> = text("anchored dialog").into();

    let anchored = modal_at(card, Point::new(40.0, 60.0), TestMessage::Close);

    assert_eq!(
        anchored.as_widget().size(),
        Size::new(Length::Fill, Length::Fill),
        "the anchored modal still needs the whole area to swallow clicks"
    );
}

#[test]
fn context_menu_trigger_inherits_the_wrapped_content_size() {
    let row: Element<'_, TestMessage> = text("row").into();
    let row_size = row.as_widget().size();

    let trigger: Element<'_, TestMessage> = context_menu_trigger(row, TestMessage::Close).into();

    assert_eq!(
        trigger.as_widget().size(),
        row_size,
        "the trigger must not resize the row it wraps"
    );
}

#[test]
fn confirm_cancel_row_builds_for_both_enabled_states() {
    let to_label = |outcome: ConfirmCancel| match outcome {
        ConfirmCancel::Confirm => strings::SAVE_BUTTON.to_string(),
        ConfirmCancel::Cancel => strings::CANCEL_BUTTON.to_string(),
    };

    let enabled: Row<'_, String> = confirm_cancel_buttons(strings::SAVE_BUTTON, true, to_label);
    let disabled: Row<'_, String> = confirm_cancel_buttons(strings::OK_BUTTON, false, to_label);

    let enabled: Element<'_, String> = enabled.into();
    let disabled: Element<'_, String> = disabled.into();

    // Both allocate the same two-button shape, so toggling dialog validity
    // never changes the widget-tree shape.
    assert_eq!(enabled.as_widget().size(), disabled.as_widget().size());
    assert_eq!(
        enabled.as_widget().size(),
        Size::new(Length::Shrink, Length::Shrink)
    );
}
