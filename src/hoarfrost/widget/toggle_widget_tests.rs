//! Unit tests for live Boolean & Choice Controls: `Checkbox`, `RadioGroup`,
//! and `ToggleSwitch` (`UIF-ATOM-003`, §3.3).
//!
//! # Why the cases are grouped
//!
//! Every renderer-backed case used to build its own headless renderer, and a
//! renderer costs more than everything else in such a test put together
//! (~620 ms of wgpu setup against ~0.1 s of process). The cases are therefore
//! grouped **by control and by claim**: one renderer per control family per
//! group, with the tree swapped through [`Harness::set_view`] instead of a
//! second harness. Nothing was dropped to get there — each case below is
//! named in the message its assertions carry, so a failure still says which
//! gesture or which state failed, not merely which group it now shares.
//!
//! Three properties make a shared harness safe to read:
//!
//! * [`Capture`] frames are stateless — every raster rebuilds its interface
//!   from an empty cache — so one capture session serves every frame in a
//!   group, and a guard that compares a frame with itself re-reads the frame
//!   the positive case already drew.
//! * An interaction [`Harness`] *does* carry widget state from case to case
//!   (iced reconciles trees by tag, and these cases all build one control at
//!   one position). Each case therefore ends with the button released and
//!   drops focus through [`Harness::unfocus`] before the next one starts, so
//!   no gesture is handed to the case after it. The one piece of state a
//!   case cannot reset — the radio group's roving index — is only read after
//!   that case's own first key press has set it, and the option it lands on
//!   is the option a fresh tree reaches from the selection.
//! * A view closure stored in a harness borrows its captures for as long as
//!   the harness lives, so every model and counter a case's closure borrows is
//!   declared **before** the harness that renders it.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::binding::Controlled;
use crate::focus::FocusId;
use crate::identity::{role, sub_part_id};
use crate::test_helpers::{
    Candidate, Capture, Frame, Harness, KIND_CONTAINER, KIND_FOCUSABLE, KIND_TEXT,
    MIN_CHANGED_PIXELS, PIXEL_SCALE, assert_panics, assert_repainted, centre, focus_one, moved,
    pressed,
};
use crate::token::color::Palette;
use crate::token::size;
use crate::widget::toggle;
use crate::widget::toggle_model::{
    CheckState, CheckboxModel, ChildSummary, RadioGroupModel, RadioOption, RadioOrientation,
    SwitchModel,
};
use crate::widget::toggle_widget::{
    CHECK_MARK, Checkbox, INDETERMINATE_MARK, RadioGroup, ToggleSwitch, option_rectangles,
};

use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::mouse;
use iced::widget::Id;
use iced::{Event, Point, Rectangle, Size};
use iced_test::renderer::Renderer as HeadlessRenderer;

/// The focus key every checkbox interaction case drives. One key per family:
/// the tree the harness reuses is then the same widget in every case.
const CB_KEY: &str = "cb_probe";
/// The focus key every switch interaction case drives.
const SW_KEY: &str = "sw_probe";
/// The focus key every radio interaction case drives.
const RG_KEY: &str = "rg_probe";

/// The key the checkbox draw-across-states sweep renders under.
const CB_DRAW: &str = "cb_draw";
/// The key the switch draw-across-states sweep renders under.
const SW_DRAW: &str = "sw_draw";
/// The key the radio draw-across-states sweep renders under.
const RG_DRAW: &str = "rg_draw";

/// The key the checkbox repaint sweep publishes under.
const CB_REPAINT: &str = "cb_rp";
/// The key the switch repaint sweep publishes under.
const SW_REPAINT: &str = "sw_rp";
/// The enabled switch the disabled-switch pointer control is measured against.
const SW_LIVE: &str = "sw_rp_live";
/// The key the radio repaint sweep publishes under.
const RG_REPAINT: &str = "rg_rp";

fn key_press(key: Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(key),
        modified_key: Key::Named(key),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestMsg {
    Checked(CheckState),
    Radio(String),
    Switched(bool),
}

// ===========================================================================
// Model surface (`UIF-ATOM-003`, §3.3)
//
// No renderer: these read the builders and the geometry helper only.
// ===========================================================================

#[test]
fn test_checkbox_creation_and_properties() {
    // -- the standalone path: the control owns the value it was handed
    let cb = Checkbox::<TestMsg>::standalone("Agree", CheckState::Unchecked, "cb_agree", |s| {
        Some(TestMsg::Checked(s))
    });

    assert_eq!(
        cb.label_text(),
        "Agree",
        "standalone: the caption is the one handed to the builder"
    );
    assert_eq!(
        cb.current_state(),
        CheckState::Unchecked,
        "standalone: the state is the one handed to the builder"
    );
    assert!(
        cb.is_enabled(),
        "standalone: a freshly built checkbox is enabled"
    );

    let custom_palette = Palette::dark();
    let cb = cb
        .with_palette(custom_palette)
        .with_children(ChildSummary {
            enabled: 2,
            checked: 2,
        })
        .disabled(true);

    assert!(
        !cb.is_enabled(),
        "standalone: `disabled(true)` takes the control out of play"
    );

    // -- the controlled path: a host owns it, and says it is disabled
    let model = CheckboxModel::new("Controlled")
        .with_state(CheckState::Checked)
        .disabled();
    let cb = Checkbox::<TestMsg>::controlled(Controlled::new(&model), "cb_ctl", |c| {
        Some(TestMsg::Checked(c.into_value()))
    });

    assert_eq!(
        cb.label_text(),
        "Controlled",
        "controlled: the caption comes from the model"
    );
    assert_eq!(
        cb.current_state(),
        CheckState::Checked,
        "controlled: the state comes from the model, not from a builder argument"
    );
    assert!(
        !cb.is_enabled(),
        "controlled: a disabled model makes the control disabled"
    );
}

#[test]
fn test_switch_creation_and_properties() {
    // -- the standalone path
    let switch = ToggleSwitch::<TestMsg>::standalone("WiFi", false, "sw_wifi", |on| {
        Some(TestMsg::Switched(on))
    });

    assert_eq!(
        switch.label_text(),
        "WiFi",
        "standalone: the caption is the one handed to the builder"
    );
    assert!(
        !switch.is_on(),
        "standalone: the polarity is the one handed in"
    );
    assert!(
        switch.is_enabled(),
        "standalone: a freshly built switch is enabled"
    );
    assert!(
        !switch.is_busy(),
        "standalone: a freshly built switch is not busy"
    );

    let switch = switch
        .with_texts("Enabled", "Disabled")
        .with_palette(Palette::dark())
        .busy(true)
        .disabled(true);

    assert!(
        switch.is_busy(),
        "standalone: `busy(true)` locks the switch"
    );
    assert!(
        !switch.is_enabled(),
        "standalone: `disabled(true)` takes the switch out of play"
    );
    assert_eq!(
        switch.state_text(),
        Some("Disabled"),
        "standalone: the off text is the one handed to `with_texts`"
    );

    // -- the controlled path, on and busy
    let model = SwitchModel::new("Bluetooth")
        .with_on(true)
        .with_texts("On", "Off")
        .busy();
    let switch = ToggleSwitch::<TestMsg>::controlled(Controlled::new(&model), "sw_bt", |commit| {
        Some(TestMsg::Switched(commit.into_value()))
    });

    assert_eq!(
        switch.label_text(),
        "Bluetooth",
        "controlled: the caption comes from the model"
    );
    assert!(
        switch.is_on(),
        "controlled: the polarity comes from the model"
    );
    assert!(
        switch.is_busy(),
        "controlled: the busy flag comes from the model"
    );
    assert_eq!(
        switch.state_text(),
        Some("On"),
        "controlled: an on-and-busy switch reads its on text"
    );
}

#[test]
fn test_radio_group_creation_and_properties() {
    let options = vec![
        RadioOption::new("opt1", "Option 1"),
        RadioOption::new("opt2", "Option 2").disabled(),
        RadioOption::new("opt3", "Option 3"),
    ];
    let model = RadioGroupModel::new(options)
        .with_selected("opt1")
        .with_orientation(RadioOrientation::HorizontalWrap);

    let group =
        RadioGroup::<TestMsg, &str>::controlled(Controlled::new(&model), "rg_prop", |commit| {
            Some(TestMsg::Radio(commit.into_value().to_string()))
        })
        .with_palette(Palette::dark());

    assert_eq!(
        group.model.get().selected,
        Some("opt1"),
        "a group built `with_selected` reports that selection through its model"
    );
}

#[test]
fn test_option_rectangles_geometry() {
    let options = vec![RadioOption::new("a", "A"), RadioOption::new("b", "B")];
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 48.0));
    let label_widths = [10.0, 10.0];

    let vertical = option_rectangles(bounds, RadioOrientation::Vertical, &options, &label_widths);
    assert_eq!(
        vertical.len(),
        2,
        "vertical: one rectangle per option, got {vertical:?}"
    );
    assert_eq!(
        vertical[0].y, 0.0,
        "vertical: the first option starts at the top"
    );
    assert_eq!(
        vertical[0].height,
        size::HIT_TARGET_MIN,
        "vertical: every option row is a hit target tall"
    );
    assert_eq!(
        vertical[1].y,
        size::HIT_TARGET_MIN,
        "vertical: the second option sits directly below the first"
    );

    let horizontal = option_rectangles(
        bounds,
        RadioOrientation::HorizontalWrap,
        &options,
        &label_widths,
    );
    assert_eq!(
        horizontal.len(),
        2,
        "horizontal-wrap: one rectangle per option, got {horizontal:?}"
    );
    assert_eq!(
        horizontal[0].y, 0.0,
        "horizontal-wrap: the first option starts on the first row"
    );
    assert_eq!(
        horizontal[0].y, 0.0,
        "horizontal-wrap: the row the first option landed on is the top row"
    );
    assert!(
        horizontal[1].x > horizontal[0].x,
        "horizontal-wrap: the second option follows the first along the row"
    );
}

#[test]
fn test_constants_and_glyphs() {
    assert_eq!(CHECK_MARK, "✓", "the checked mark is the tick");
    assert_eq!(
        INDETERMINATE_MARK, "\u{2212}",
        "the indeterminate mark is the dash, not the tick"
    );
}

// ===========================================================================
// Interaction: the pointer and the keyboard (`UIF-ATOM-003`, §3.3)
//
// One harness per control family. Each case swaps the tree, drives its own
// gesture against its own counter, and leaves the control quiescent: button
// released, focus dropped.
// ===========================================================================

#[test]
fn test_a_checkbox_answers_the_pointer_and_the_keyboard() {
    // Everything a case's view closure borrows is declared up front: the
    // harness outlives every case, so its captures have to outlive it too.
    let controlled_model = CheckboxModel::new("Controlled Checkbox");
    let click_fired = Arc::new(AtomicUsize::new(0));
    let controlled_fired = Arc::new(AtomicUsize::new(0));
    let space_fired = Arc::new(AtomicUsize::new(0));
    let leaf_fired = Arc::new(AtomicUsize::new(0));
    let all_fired = Arc::new(AtomicUsize::new(0));
    let partial_fired = Arc::new(AtomicUsize::new(0));
    let disabled_fired = Arc::new(AtomicUsize::new(0));

    let mut harness = Harness::headless(|| checkbox_tree(CB_KEY, CheckState::Unchecked));

    // -- a click that ends over the box toggles; one that drags out does not
    harness.set_view(|| {
        let fired = Arc::clone(&click_fired);
        Checkbox::standalone("Click me", CheckState::Unchecked, CB_KEY, move |s| {
            if s == CheckState::Checked {
                fired.fetch_add(1, Ordering::SeqCst);
            }
            Some(TestMsg::Checked(s))
        })
        .into()
    });

    let target = Point::new(10.0, 10.0);
    harness.hover(target);
    harness.render();
    assert_eq!(
        click_fired.load(Ordering::SeqCst),
        0,
        "pointer: hovering a checkbox must not toggle it"
    );

    harness.press(target);
    harness.render();
    assert_eq!(
        click_fired.load(Ordering::SeqCst),
        0,
        "pointer: pressing a checkbox must not toggle it"
    );

    harness.release(target);
    harness.render();
    assert_eq!(
        click_fired.load(Ordering::SeqCst),
        1,
        "pointer: releasing over the box toggles exactly once"
    );

    // Press and drag out should not fire
    harness.press(target);
    harness.hover(Point::new(500.0, 500.0));
    harness.release(Point::new(500.0, 500.0));
    harness.render();
    assert_eq!(
        click_fired.load(Ordering::SeqCst),
        1,
        "pointer: a press that leaves the box before releasing toggles nothing"
    );

    // -- the controlled path answers the same click
    harness.set_view(|| {
        let fired = Arc::clone(&controlled_fired);
        Checkbox::controlled(Controlled::new(&controlled_model), CB_KEY, move |commit| {
            if commit.value() == &CheckState::Checked {
                fired.fetch_add(1, Ordering::SeqCst);
            }
            Some(TestMsg::Checked(commit.into_value()))
        })
        .into()
    });

    let target = Point::new(12.0, 12.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(
        controlled_fired.load(Ordering::SeqCst),
        1,
        "controlled: a click commits the checked state exactly once"
    );

    // -- Space toggles a focused checkbox; Enter does not
    harness.set_view(|| {
        let fired = Arc::clone(&space_fired);
        Checkbox::standalone("Space me", CheckState::Unchecked, CB_KEY, move |s| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Checked(s))
        })
        .into()
    });

    harness.unfocus();
    focus_one(&mut harness, CB_KEY);
    harness.render();

    // Space key when focused triggers toggle
    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(msgs.len(), 1, "keyboard: Space publishes one message");
    assert_eq!(
        space_fired.load(Ordering::SeqCst),
        1,
        "keyboard: Space toggles a focused checkbox once"
    );

    // Other keys (e.g. Enter) do not toggle
    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert!(enter_msgs.is_empty(), "keyboard: Enter publishes nothing");
    assert_eq!(
        space_fired.load(Ordering::SeqCst),
        1,
        "keyboard: Enter toggles nothing"
    );

    // -- the tri-state rule is decided by the children it was handed
    //
    // 1. Leaf Indeterminate -> Checked
    harness.set_view(|| {
        let fired = Arc::clone(&leaf_fired);
        Checkbox::standalone("Leaf", CheckState::Indeterminate, CB_KEY, move |s| {
            if s == CheckState::Checked {
                fired.fetch_add(1, Ordering::SeqCst);
            }
            Some(TestMsg::Checked(s))
        })
        .with_children(ChildSummary::LEAF)
        .into()
    });
    let at = Point::new(10.0, 10.0);
    harness.hover(at);
    harness.press(at);
    harness.release(at);
    harness.render();
    assert_eq!(
        leaf_fired.load(Ordering::SeqCst),
        1,
        "tri-state: a leaf indeterminate checkbox turns checked"
    );

    // 2. Parent Indeterminate with all enabled children checked -> Unchecked
    harness.set_view(|| {
        let fired = Arc::clone(&all_fired);
        Checkbox::standalone("Parent All", CheckState::Indeterminate, CB_KEY, move |s| {
            if s == CheckState::Unchecked {
                fired.fetch_add(1, Ordering::SeqCst);
            }
            Some(TestMsg::Checked(s))
        })
        .with_children(ChildSummary {
            enabled: 3,
            checked: 3,
        })
        .into()
    });
    harness.hover(at);
    harness.press(at);
    harness.release(at);
    harness.render();
    assert_eq!(
        all_fired.load(Ordering::SeqCst),
        1,
        "tri-state: an indeterminate parent whose children are all checked turns unchecked"
    );

    // 3. Parent Indeterminate with partial children checked -> Checked
    harness.set_view(|| {
        let fired = Arc::clone(&partial_fired);
        Checkbox::standalone(
            "Parent Partial",
            CheckState::Indeterminate,
            CB_KEY,
            move |s| {
                if s == CheckState::Checked {
                    fired.fetch_add(1, Ordering::SeqCst);
                }
                Some(TestMsg::Checked(s))
            },
        )
        .with_children(ChildSummary {
            enabled: 3,
            checked: 2,
        })
        .into()
    });
    harness.hover(at);
    harness.press(at);
    harness.release(at);
    harness.render();
    assert_eq!(
        partial_fired.load(Ordering::SeqCst),
        1,
        "tri-state: an indeterminate parent with children still unchecked turns checked"
    );

    // -- a disabled checkbox answers nothing, to the pointer or the keyboard
    harness.set_view(|| {
        let fired = Arc::clone(&disabled_fired);
        Checkbox::standalone("Disabled", CheckState::Unchecked, CB_KEY, move |s| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Checked(s))
        })
        .disabled(true)
        .into()
    });

    harness.unfocus();
    let target = Point::new(10.0, 10.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(
        disabled_fired.load(Ordering::SeqCst),
        0,
        "disabled: the pointer toggles nothing"
    );

    // Space key when disabled does nothing
    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert!(msgs.is_empty(), "disabled: Space publishes nothing");
    assert_eq!(
        disabled_fired.load(Ordering::SeqCst),
        0,
        "disabled: Space toggles nothing"
    );
}

#[test]
fn test_a_radio_group_answers_the_pointer_and_the_keyboard() {
    // Declared before the harness: every view closure below borrows one of
    // these, and the harness outlives every case.
    let click_model = RadioGroupModel::new(vec![
        RadioOption::new("first".to_string(), "First"),
        RadioOption::new("second".to_string(), "Second"),
        RadioOption::new("third".to_string(), "Third").disabled(),
    ]);
    let roving_model = RadioGroupModel::new(vec![
        RadioOption::new("a".to_string(), "Option A"),
        RadioOption::new("b".to_string(), "Option B").disabled(), // disabled!
        RadioOption::new("c".to_string(), "Option C"),
    ])
    .with_selected("a".to_string());
    let disabled_model = RadioGroupModel::new(vec![
        RadioOption::new("x".to_string(), "X"),
        RadioOption::new("y".to_string(), "Y"),
    ])
    .disabled();
    let clicked = Arc::new(std::sync::Mutex::new(None));
    let roved = Arc::new(std::sync::Mutex::new(None));
    let group_disabled = Arc::new(std::sync::Mutex::new(None));

    let mut harness = Harness::headless(|| {
        RadioGroup::<TestMsg, String>::controlled(Controlled::new(&click_model), RG_KEY, |_| None)
            .into()
    });

    // -- clicking an option chooses it; a disabled option is not choosable
    harness.set_view(|| {
        let selected = Arc::clone(&clicked);
        RadioGroup::controlled(Controlled::new(&click_model), RG_KEY, move |commit| {
            *selected.lock().unwrap() = Some(commit.value().clone());
            Some(TestMsg::Radio(commit.into_value()))
        })
        .into()
    });

    // Click option 0 ("first") -> y in [0, 24)
    let pt0 = Point::new(10.0, 10.0);
    harness.hover(pt0);
    harness.press(pt0);
    harness.release(pt0);
    harness.render();
    assert_eq!(
        clicked.lock().unwrap().as_deref(),
        Some("first"),
        "pointer: the option clicked is the option chosen"
    );

    // Click option 1 ("second") -> y in [24, 48)
    let pt1 = Point::new(10.0, 34.0);
    harness.hover(pt1);
    harness.press(pt1);
    harness.release(pt1);
    harness.render();
    assert_eq!(
        clicked.lock().unwrap().as_deref(),
        Some("second"),
        "pointer: a second click moves the selection"
    );

    // Click option 2 ("third", disabled) -> y in [48, 72) - must be ignored!
    *clicked.lock().unwrap() = None;
    let pt2 = Point::new(10.0, 58.0);
    harness.hover(pt2);
    harness.press(pt2);
    harness.release(pt2);
    harness.render();
    assert_eq!(
        *clicked.lock().unwrap(),
        None,
        "pointer: a disabled option cannot be chosen"
    );

    // -- the arrows rove over the enabled options and wrap at both ends
    //
    // This case keeps a harness of its own. `RadioGroup` publishes its roving
    // tab stop only while the tree carries none, and a tree survives `set_view`,
    // so the stop the click case left on the option it chose would still be in
    // force here: the first `ArrowDown` would rove from that stale option
    // instead of from this model's `"a"` selection. A fresh tree is the only
    // way this case starts where it was written to start.
    let mut roving = Harness::headless(|| {
        let selected = Arc::clone(&roved);
        RadioGroup::controlled(Controlled::new(&roving_model), RG_KEY, move |commit| {
            *selected.lock().unwrap() = Some(commit.value().clone());
            Some(TestMsg::Radio(commit.into_value()))
        })
        .into()
    });

    focus_one(&mut roving, RG_KEY);
    roving.render();

    // ArrowDown roves forward: skips disabled 'b' and lands on 'c'!
    let d1 = roving.pointer(&[key_press(Named::ArrowDown)], mouse::Cursor::Unavailable);
    assert_eq!(d1.len(), 1, "keyboard: ArrowDown publishes one commit");
    assert_eq!(
        roved.lock().unwrap().as_deref(),
        Some("c"),
        "keyboard: ArrowDown skips the disabled option"
    );

    // ArrowDown wraps around to 'a'!
    let d2 = roving.pointer(&[key_press(Named::ArrowDown)], mouse::Cursor::Unavailable);
    assert_eq!(
        d2.len(),
        1,
        "keyboard: the wrapping ArrowDown publishes one commit"
    );
    assert_eq!(
        roved.lock().unwrap().as_deref(),
        Some("a"),
        "keyboard: ArrowDown wraps from the last option to the first"
    );

    // ArrowUp roves backward: wraps to 'c'!
    let u1 = roving.pointer(&[key_press(Named::ArrowUp)], mouse::Cursor::Unavailable);
    assert_eq!(
        u1.len(),
        1,
        "keyboard: the wrapping ArrowUp publishes one commit"
    );
    assert_eq!(
        roved.lock().unwrap().as_deref(),
        Some("c"),
        "keyboard: ArrowUp wraps from the first option to the last"
    );

    // -- a disabled group answers nothing, to the pointer or the arrows
    harness.set_view(|| {
        let selected = Arc::clone(&group_disabled);
        RadioGroup::controlled(Controlled::new(&disabled_model), RG_KEY, move |commit| {
            *selected.lock().unwrap() = Some(commit.value().clone());
            Some(TestMsg::Radio(commit.into_value()))
        })
        .into()
    });

    harness.unfocus();
    let pt = Point::new(10.0, 10.0);
    harness.hover(pt);
    harness.press(pt);
    harness.release(pt);
    harness.render();
    assert_eq!(
        *group_disabled.lock().unwrap(),
        None,
        "disabled group: the pointer chooses nothing"
    );

    // Arrow key does nothing when group is disabled
    let msgs = harness.pointer(&[key_press(Named::ArrowDown)], mouse::Cursor::Unavailable);
    assert!(
        msgs.is_empty(),
        "disabled group: ArrowDown publishes nothing"
    );
    assert_eq!(
        *group_disabled.lock().unwrap(),
        None,
        "disabled group: ArrowDown chooses nothing"
    );
}

#[test]
fn test_a_switch_answers_the_pointer_and_the_keyboard() {
    let controlled_model = SwitchModel::new("Controlled Switch").with_on(false);
    let click_fired = Arc::new(std::sync::Mutex::new(None));
    let controlled_fired = Arc::new(std::sync::Mutex::new(None));
    let space_fired = Arc::new(std::sync::Mutex::new(None));
    let busy_fired = Arc::new(AtomicUsize::new(0));
    let disabled_fired = Arc::new(AtomicUsize::new(0));

    let mut harness = Harness::headless(|| switch_tree(SW_KEY, false, false));

    // -- a click on the track toggles; a press that drags out does not
    harness.set_view(|| {
        let fired = Arc::clone(&click_fired);
        ToggleSwitch::standalone("Airplane Mode", false, SW_KEY, move |on| {
            *fired.lock().unwrap() = Some(on);
            Some(TestMsg::Switched(on))
        })
        .into()
    });

    let target = Point::new(15.0, 12.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(
        *click_fired.lock().unwrap(),
        Some(true),
        "pointer: a click on the track turns the switch on"
    );

    // Press and drag out does not toggle
    *click_fired.lock().unwrap() = None;
    harness.press(target);
    harness.hover(Point::new(400.0, 400.0));
    harness.release(Point::new(400.0, 400.0));
    harness.render();
    assert_eq!(
        *click_fired.lock().unwrap(),
        None,
        "pointer: a press that leaves the track before releasing toggles nothing"
    );

    // -- the controlled path answers the same click
    harness.set_view(|| {
        let fired = Arc::clone(&controlled_fired);
        ToggleSwitch::controlled(Controlled::new(&controlled_model), SW_KEY, move |commit| {
            *fired.lock().unwrap() = Some(*commit.value());
            Some(TestMsg::Switched(commit.into_value()))
        })
        .into()
    });

    let target = Point::new(15.0, 12.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(
        *controlled_fired.lock().unwrap(),
        Some(true),
        "controlled: a click commits the on state"
    );

    // -- Space toggles a focused switch
    harness.set_view(|| {
        let fired = Arc::clone(&space_fired);
        ToggleSwitch::standalone("Keyboard Switch", false, SW_KEY, move |on| {
            *fired.lock().unwrap() = Some(on);
            Some(TestMsg::Switched(on))
        })
        .into()
    });

    harness.unfocus();
    focus_one(&mut harness, SW_KEY);
    harness.render();

    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(msgs.len(), 1, "keyboard: Space publishes one message");
    assert_eq!(
        *space_fired.lock().unwrap(),
        Some(true),
        "keyboard: Space turns a focused switch on"
    );

    // -- a busy switch refuses both doors
    harness.set_view(|| {
        let fired = Arc::clone(&busy_fired);
        ToggleSwitch::standalone("Busy Switch", false, SW_KEY, move |on| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Switched(on))
        })
        .busy(true)
        .into()
    });

    harness.unfocus();
    let target = Point::new(15.0, 12.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(
        busy_fired.load(Ordering::SeqCst),
        0,
        "busy: the pointer toggles nothing while the switch is in flight"
    );

    // Keyboard Space when busy is also refused
    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert!(msgs.is_empty(), "busy: Space publishes nothing");
    assert_eq!(
        busy_fired.load(Ordering::SeqCst),
        0,
        "busy: Space toggles nothing"
    );

    // -- a disabled switch refuses the pointer
    harness.set_view(|| {
        let fired = Arc::clone(&disabled_fired);
        ToggleSwitch::standalone("Disabled Switch", false, SW_KEY, move |on| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Switched(on))
        })
        .disabled(true)
        .into()
    });

    harness.unfocus();
    let target = Point::new(15.0, 12.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(
        disabled_fired.load(Ordering::SeqCst),
        0,
        "disabled: the pointer toggles nothing"
    );
}

// ===========================================================================
// Draw: `Widget::draw` runs in every state (`UIF-ATOM-003`, §3.3)
//
// The claim is only that a real draw pass completes — in every model state,
// at rest, hovered, pressed and focused. One harness per control: the sweep
// swaps the state through `set_view` and drops focus between states so each
// state is drawn from a control as quiescent as a fresh tree's.
// ===========================================================================

#[test]
fn test_a_checkbox_draws_in_every_state() {
    let mut harness = Harness::headless(|| checkbox_tree(CB_DRAW, CheckState::Unchecked));

    for state in [
        CheckState::Unchecked,
        CheckState::Checked,
        CheckState::Indeterminate,
    ] {
        for disabled in [false, true] {
            harness.unfocus();
            harness.set_view(move || {
                Checkbox::<TestMsg>::standalone("Draw Test", state, CB_DRAW, |_| None)
                    .disabled(disabled)
                    .into()
            });

            let pt = Point::new(12.0, 12.0);

            // Rest render
            harness.render();

            // Hover render
            harness.hover(pt);
            harness.render();

            // Pressed render
            harness.press(pt);
            harness.render();
            harness.release(pt);

            // Focus render (if enabled)
            if !disabled {
                focus_one(&mut harness, CB_DRAW);
                harness.render();
            }
        }
    }
}

#[test]
fn test_a_switch_draws_in_every_state() {
    let mut harness = Harness::headless(|| switch_tree(SW_DRAW, false, false));

    for on in [false, true] {
        for busy in [false, true] {
            for disabled in [false, true] {
                harness.unfocus();
                harness.set_view(move || {
                    ToggleSwitch::<TestMsg>::standalone("Switch Draw", on, SW_DRAW, |_| None)
                        .with_texts("Active", "Inactive")
                        .busy(busy)
                        .disabled(disabled)
                        .into()
                });

                let pt = Point::new(18.0, 10.0);

                // Rest render
                harness.render();

                // Hover render
                harness.hover(pt);
                harness.render();

                // Press render
                harness.press(pt);
                harness.render();
                harness.release(pt);

                // Focus render (if enabled)
                if !disabled {
                    focus_one(&mut harness, SW_DRAW);
                    harness.render();
                }
            }
        }
    }
}

#[test]
fn test_a_radio_group_draws_in_every_state() {
    // The models are built up front: the harness holds the view closure, so a
    // model a state borrows has to outlive the harness, not just the iteration
    // that drew it.
    let mut cases: Vec<(bool, RadioGroupModel<&'static str>)> = Vec::new();
    for orientation in [RadioOrientation::Vertical, RadioOrientation::HorizontalWrap] {
        for disabled in [false, true] {
            let options = vec![
                RadioOption::new("1", "One"),
                RadioOption::new("2", "Two").disabled(),
                RadioOption::new("3", "Three"),
            ];
            let mut model = RadioGroupModel::new(options)
                .with_orientation(orientation)
                .with_selected("1");
            if disabled {
                model = model.disabled();
            }
            cases.push((disabled, model));
        }
    }

    let mut harness = Harness::headless(|| {
        RadioGroup::<TestMsg, &str>::controlled(Controlled::new(&cases[0].1), RG_DRAW, |_| None)
            .into()
    });

    for (disabled, model) in &cases {
        harness.unfocus();
        harness.set_view(move || {
            RadioGroup::<TestMsg, &str>::controlled(Controlled::new(model), RG_DRAW, |_| None)
                .into()
        });

        let pt0 = Point::new(10.0, 10.0);
        let pt1 = Point::new(10.0, 30.0);

        // Rest render
        harness.render();

        // Hover option 0
        harness.hover(pt0);
        harness.render();

        // Press option 0
        harness.press(pt0);
        harness.render();
        harness.release(pt0);

        // Hover option 1
        harness.hover(pt1);
        harness.render();

        // Focus group
        if !disabled {
            focus_one(&mut harness, RG_DRAW);
            harness.render();
        }
    }
}

// ===========================================================================
// Identity: a control publishes its own box in every state (`UIF-ATOM-003`)
//
// The rule all three controls share, stated once here because three
// separate `operate` impls have to agree on it:
//
//   * `container` fires **always** — enabled, disabled, busy — exactly
//     once per control. It answers *"where is this control?"*.
//   * `focusable` fires **only** when the control may take focus. It
//     answers *"can Tab stop here?"*.
//
// The two doors are orthogonal, and that is the whole point. Disabling a
// control takes its Tab stop away and leaves its box: a host can still
// see, measure and route to a disabled control, it simply cannot focus
// it. Before this rule a disabled control published its caption and
// nothing else, so the caption named a control the operation stream
// could not find — a label with nothing to bind to.
//
// Making the disabled control focusable would have "fixed" the count by
// breaking the accessibility contract. Publishing the box does it
// without touching focus at all.
//
// A probe only reads what the tree publishes, and publishing does not
// depend on widget state, so every probe in a test below runs through one
// harness by swapping the tree.
// ===========================================================================

/// The iced [`Id`] a control built under `focus_key` answers to.
fn id_of(focus_key: &str) -> Id {
    crate::bridge::widget_id(&FocusId::new(focus_key))
}

/// The boxes published under `id` through `container` — the control's
/// own record of where it sits.
fn containers_for(candidates: &[Candidate], id: &Id) -> Vec<Rectangle> {
    candidates
        .iter()
        .filter(|candidate| candidate.kind == KIND_CONTAINER && candidate.id.as_ref() == Some(id))
        .map(|candidate| candidate.bounds)
        .collect()
}

/// The boxes published under `id` through `focusable` — its Tab stops.
fn focusables_for(candidates: &[Candidate], id: &Id) -> Vec<Rectangle> {
    candidates
        .iter()
        .filter(|candidate| candidate.kind == KIND_FOCUSABLE && candidate.id.as_ref() == Some(id))
        .map(|candidate| candidate.bounds)
        .collect()
}

/// The caption a control publishes under its derived `label` id.
fn caption_for(candidates: &[Candidate], id: &Id) -> Option<Rectangle> {
    let caption_id = sub_part_id(id, role::LABEL)?;

    candidates
        .iter()
        .find(|candidate| candidate.kind == KIND_TEXT && candidate.id.as_ref() == Some(&caption_id))
        .map(|candidate| candidate.bounds)
}

/// Swap in a checkbox in `state` and read back what it publishes.
fn probe_checkbox<'a>(
    harness: &mut Harness<'a, TestMsg, HeadlessRenderer>,
    key: &'static str,
    disabled: bool,
    state: CheckState,
) -> Vec<Candidate> {
    harness.set_view(move || {
        Checkbox::<TestMsg>::standalone("Agree", state, key, |_| None)
            .disabled(disabled)
            .into()
    });

    harness.candidates()
}

/// Swap in a switch in `disabled` / `busy` and read back what it publishes.
fn probe_switch<'a>(
    harness: &mut Harness<'a, TestMsg, HeadlessRenderer>,
    key: &'static str,
    disabled: bool,
    busy: bool,
) -> Vec<Candidate> {
    harness.set_view(move || {
        ToggleSwitch::<TestMsg>::standalone("Sync", false, key, |_| None)
            .disabled(disabled)
            .busy(busy)
            .into()
    });

    harness.candidates()
}

/// The two-option vertical group every radio fixture in this file renders.
fn radio_model(selected: &'static str) -> RadioGroupModel<&'static str> {
    RadioGroupModel::new(vec![
        RadioOption::new("a", "Alpha"),
        RadioOption::new("b", "Beta"),
    ])
    .with_selected(selected)
    .with_orientation(RadioOrientation::Vertical)
}

/// Swap in a radio group over `model` and read back what it publishes.
///
/// `'m: 'a` because the tree the harness rebuilds borrows the model, and the
/// harness keeps that closure until the next probe replaces it.
fn probe_radio<'a, 'm: 'a>(
    harness: &mut Harness<'a, TestMsg, HeadlessRenderer>,
    key: &'static str,
    model: &'m RadioGroupModel<&'static str>,
) -> Vec<Candidate> {
    harness.set_view(move || radio_tree(key, model));

    harness.candidates()
}

#[test]
fn test_a_disabled_control_publishes_its_box_without_a_tab_stop() {
    let radio_disabled = radio_model("a").disabled();
    let mut harness = Harness::headless(|| checkbox_tree("cb_dis_box", CheckState::Unchecked));

    // -- the defect: a disabled checkbox used to be invisible --------------
    let id = id_of("cb_dis_box");
    let candidates = probe_checkbox(&mut harness, "cb_dis_box", true, CheckState::Unchecked);

    assert!(
        caption_for(&candidates, &id).is_some(),
        "precondition: the disabled checkbox's caption is published"
    );
    assert_eq!(
        containers_for(&candidates, &id).len(),
        1,
        "a disabled checkbox must still publish exactly one record of the box \
         it occupies — its caption names a control the host has to be able to find"
    );

    // …and the same claim for the switch, whose record is the track. The
    // disabled switch is published once and read twice: here for its box, and
    // below for the Tab stop it must not have.
    let id = id_of("sw_dis_box");
    let disabled_switch = probe_switch(&mut harness, "sw_dis_box", true, false);

    assert!(
        caption_for(&disabled_switch, &id).is_some(),
        "precondition: the disabled switch's caption is published"
    );
    assert_eq!(
        containers_for(&disabled_switch, &id).len(),
        1,
        "a disabled switch must still publish exactly one record of the track \
         it occupies"
    );

    // …and one record per option for a group, which is also the tree the
    // Tab-stop claim below is made against.
    let id = id_of("rg_dis_box");
    let disabled_radio = probe_radio(&mut harness, "rg_dis_box", &radio_disabled);

    assert_eq!(
        containers_for(&disabled_radio, &id).len(),
        2,
        "a disabled group of two options must publish one box per option — each \
         option is a control with a caption that has to bind to something"
    );

    // -- and it is still not focusable ------------------------------------
    let disabled_checkbox =
        probe_checkbox(&mut harness, "cb_dis_nofocus", true, CheckState::Checked);

    for (name, candidates, id) in [
        ("checkbox", &disabled_checkbox, id_of("cb_dis_nofocus")),
        ("switch", &disabled_switch, id_of("sw_dis_box")),
        ("radio group", &disabled_radio, id_of("rg_dis_box")),
    ] {
        assert!(
            focusables_for(candidates, &id).is_empty(),
            "a disabled {name} published a focusable record — publishing its box \
             must not make it a Tab stop"
        );
    }

    // -- busy locks the interaction, not the focus state ------------------
    let id = id_of("sw_busy_box");
    let busy_switch = probe_switch(&mut harness, "sw_busy_box", false, true);

    assert_eq!(
        containers_for(&busy_switch, &id).len(),
        1,
        "a busy switch still occupies a box and must publish it"
    );
    assert_eq!(
        focusables_for(&busy_switch, &id).len(),
        1,
        "busy locks the interaction, not the focus state — unchanged behaviour, \
         pinned so this fix cannot have altered it"
    );
}

#[test]
fn test_every_control_publishes_exactly_one_box_in_every_state() {
    let radio_enabled = radio_model("a");
    let radio_disabled = radio_model("a").disabled();
    let mut harness = Harness::headless(|| checkbox_tree("cb_rule", CheckState::Indeterminate));

    let cb_id = id_of("cb_rule");
    let sw_id = id_of("sw_rule");
    let rg_id = id_of("rg_rule");

    // One probe per state, read twice below — once for "at least one, and no
    // degenerate box", once for the exact count. The earlier shape re-ran each
    // probe a second time to ask the second question.
    let checkbox_enabled = containers_for(
        &probe_checkbox(&mut harness, "cb_rule", false, CheckState::Indeterminate),
        &cb_id,
    );
    let checkbox_disabled = containers_for(
        &probe_checkbox(&mut harness, "cb_rule", true, CheckState::Indeterminate),
        &cb_id,
    );
    let switch_enabled =
        containers_for(&probe_switch(&mut harness, "sw_rule", false, false), &sw_id);
    let switch_busy = containers_for(&probe_switch(&mut harness, "sw_rule", false, true), &sw_id);
    let switch_disabled =
        containers_for(&probe_switch(&mut harness, "sw_rule", true, false), &sw_id);
    let radio_enabled_boxes = containers_for(
        &probe_radio(&mut harness, "rg_rule", &radio_enabled),
        &rg_id,
    );
    let radio_disabled_boxes = containers_for(
        &probe_radio(&mut harness, "rg_rule", &radio_disabled),
        &rg_id,
    );

    for (label, boxes) in [
        ("checkbox enabled", &checkbox_enabled),
        ("checkbox disabled", &checkbox_disabled),
        ("switch enabled", &switch_enabled),
        ("switch busy", &switch_busy),
        ("switch disabled", &switch_disabled),
        ("radio enabled", &radio_enabled_boxes),
        ("radio disabled", &radio_disabled_boxes),
    ] {
        assert!(
            !boxes.is_empty(),
            "{label} published no record of itself at all"
        );
        assert!(
            boxes
                .iter()
                .all(|bounds| bounds.width > 0.0 && bounds.height > 0.0),
            "{label} published a degenerate box: {boxes:?}"
        );
    }

    // Exactly one per control: the checkbox and the switch are single
    // controls, so one record each, in every state.
    assert_eq!(
        checkbox_enabled.len(),
        1,
        "one enabled checkbox, one record"
    );
    assert_eq!(
        checkbox_disabled.len(),
        1,
        "one disabled checkbox, one record"
    );
    assert_eq!(switch_enabled.len(), 1, "one enabled switch, one record");
    assert_eq!(switch_disabled.len(), 1, "one disabled switch, one record");
}

// -- the published box is the painted box ------------------------------

/// The whole-canvas rectangle of a `canvas`-sized frame.
///
/// The 2-D ink reader this file used to hand-roll now lives on
/// [`Frame`] as `ink_bounds`, so the only thing left to supply is the
/// region to read — which for "the ink on this frame, wherever it is" is
/// the whole canvas.
fn whole(canvas: Size) -> Rectangle {
    Rectangle::with_size(canvas)
}

/// An unlabelled checkbox paints nothing but its indicator, so the ink on
/// the frame **is** the indicator box. Comparing that against what
/// `operate` published proves the published box is the drawn box rather
/// than a number the identity pass invented.
///
/// The switch claim rides along on the same capture session — the track is
/// the only toggle shape with an aspect ratio of its own — as does the claim
/// that a labelled control publishes the indicator and not the row.
#[test]
fn test_the_published_box_is_the_box_the_paint_pass_drew() {
    let mut harness = Harness::headless(|| checkbox_tree("cb_paint", CheckState::Unchecked));
    let mut capture = Capture::new();

    // -- the checkbox: ink extent against published box
    let key = "cb_paint";
    let id = id_of(key);
    let published = containers_for(
        &probe_checkbox(&mut harness, key, false, CheckState::Unchecked),
        &id,
    );
    assert_eq!(published.len(), 1, "precondition: one published box");

    let canvas = Size::new(size::HIT_TARGET_MIN, size::HIT_TARGET_MIN);
    let tree: iced::Element<'static, TestMsg> =
        Checkbox::<TestMsg>::standalone("", CheckState::Unchecked, key, |_| None).into();
    let frame = capture.frame(canvas, tree);

    let painted = frame
        .ink_bounds(whole(canvas))
        .expect("an unlabelled checkbox still paints its indicator");

    let tol = 1.0 / PIXEL_SCALE;
    let want = published[0];

    assert!(
        (want.x - painted.x).abs() <= tol
            && (want.y - painted.y).abs() <= tol
            && (want.width - painted.width).abs() <= tol
            && (want.height - painted.height).abs() <= tol,
        "the published box {want:?} is not the painted box {painted:?} — the \
         identity has drifted from the paint",
    );

    // -- the switch, on its own geometry
    let key = "sw_paint";
    let id = id_of(key);
    let published = containers_for(&probe_switch(&mut harness, key, false, false), &id);
    assert_eq!(
        published.len(),
        1,
        "precondition: one published box (switch)"
    );

    let canvas = Size::new(size::HIT_TARGET_MIN * 3.0, size::HIT_TARGET_MIN);
    let tree: iced::Element<'static, TestMsg> =
        ToggleSwitch::<TestMsg>::standalone("", false, key, |_| None).into();
    let frame = capture.frame(canvas, tree);

    let painted = frame
        .ink_bounds(whole(canvas))
        .expect("an unlabelled switch still paints its track");

    assert_eq!(
        published[0].width,
        size::SWITCH_TRACK_WIDTH,
        "the published box is the track's width, not the label-inclusive row"
    );
    assert!(
        (painted.width - size::SWITCH_TRACK_WIDTH).abs() <= 1.0 / PIXEL_SCALE,
        "the track painted {:?} wide against a published {:?}",
        painted.width,
        published[0].width,
    );
    assert!(
        (painted.x - published[0].x).abs() <= 1.0 / PIXEL_SCALE,
        "the track is drawn at x {:?} but the box was published at x {:?}",
        painted.x,
        published[0].x,
    );

    // -- and the published box is not the label-inclusive row: that would
    // tell a host the control is wider than the thing it can see.
    let key = "cb_vs_row";
    let id = id_of(key);
    let candidates = probe_checkbox(&mut harness, key, false, CheckState::Checked);

    let box_of = containers_for(&candidates, &id)[0];
    let caption = caption_for(&candidates, &id).expect("the caption is published");

    assert_eq!(
        box_of.width,
        size::CHECKBOX_BOX,
        "the control's own box is the indicator, not the row"
    );
    assert_eq!(
        box_of.height,
        size::CHECKBOX_BOX,
        "the control's own box is the indicator, not the row"
    );
    assert!(
        caption.x >= box_of.x + box_of.width,
        "the caption sits to the right of the indicator, so the two boxes must \
         not overlap: box {box_of:?}, caption {caption:?}"
    );
}

// ===========================================================================
// Repaint: the frame has to follow the model (`UIF-ATOM-003`, §3.3)
//
// Everything above proves the model moves and the identity is published.
// None of it proves the *pixels* followed. A control that latches its
// state and repaints byte-identical ink passes every assertion above —
// the application believes the box toggled and the user is looking at a
// box that never changed. That is the one failure mode a state check
// cannot see, so it gets its own layer: rasterise the same widget twice,
// in two model states, through one shared renderer, and diff.
//
// The rectangle is always the box the control published for itself
// through `operate`, never the canvas. A whole-canvas sweep credits a
// dead indicator with ink the caption drew, which is how a widget that
// paints nothing at all still passes.
//
// Every claim here is paired with the check that it can fail. A repaint
// test that passes when nothing changed is worse than no test at all: it
// reads as coverage. A guard re-reads the frame its claim was made on
// against itself — the same measurement, no repaint at all — which is why
// the guards live inside the test they guard instead of paying for a
// renderer of their own.
// ===========================================================================

/// The canvas a single toggle row is rasterised into.
///
/// One hit target tall — the row's own fixed height — and wide enough for
/// a labelled row, so the widget lands at the same origin the identity
/// probe laid it out at and the box being diffed is the box published.
fn toggle_canvas() -> Size {
    Size::new(240.0, size::HIT_TARGET_MIN)
}

/// The canvas a two-option vertical radio group is rasterised into.
fn radio_canvas() -> Size {
    Size::new(240.0, size::HIT_TARGET_MIN * 2.0)
}

/// Prove a repaint claim is load-bearing over `rect`.
///
/// Run against a frame compared with itself — no repaint whatsoever — the
/// assertion must go red, and the message must name the control and the
/// floor it missed. Returns the panic text so the caller can pin the
/// wording it fired on.
fn repaint_guard_panic(frame: &Frame, rect: Rectangle, what: &str) -> String {
    let text = assert_panics(
        &format!("{what} compared with itself over {rect:?}"),
        || {
            assert_repainted(frame, frame, rect, what);
        },
    );

    assert!(
        text.contains(what),
        "the panic should name {what:?}, got {text}"
    );
    assert!(
        text.contains(&MIN_CHANGED_PIXELS.to_string()),
        "the panic should state the {MIN_CHANGED_PIXELS}-pixel floor it missed, \
         got {text}"
    );
    assert!(
        text.contains("repaint"),
        "the panic should say the frame failed to repaint, got {text}"
    );

    text
}

/// Prove a "nothing changed" control is load-bearing.
///
/// The zero-diff assertion only means something if the same measurement
/// over a pair that *did* move reports movement. Otherwise it is reading
/// a blank canvas and calling it stability.
fn zero_diff_guard(moved_pixels: usize, what: &str) {
    assert!(
        moved_pixels > 0,
        "{what}: the comparison pair also shows zero movement — the \
         'nothing changed' control is measuring a dead canvas, not a stable \
         control"
    );
}

/// Assert the published box actually lies inside the canvas it is being
/// diffed in, so a mis-derived rect cannot silently read empty pixels.
fn assert_fits(rect: Rectangle, canvas: Size, what: &str) {
    assert!(
        rect.x >= 0.0
            && rect.y >= 0.0
            && rect.x + rect.width <= canvas.width
            && rect.y + rect.height <= canvas.height,
        "the {what} box {rect:?} lies outside the {canvas:?} canvas — the \
         repaint would be measured over pixels that were never rendered"
    );
}

/// The single box a one-box control publishes for itself.
fn published_box(key: &'static str, candidates: &[Candidate]) -> Rectangle {
    let boxes = containers_for(candidates, &id_of(key));
    assert_eq!(
        boxes.len(),
        1,
        "precondition: {key} publishes exactly one box, got {boxes:?}"
    );

    boxes[0]
}

fn checkbox_tree(key: &'static str, state: CheckState) -> iced::Element<'static, TestMsg> {
    Checkbox::<TestMsg>::standalone("Agree", state, key, |_| None).into()
}

fn switch_tree(key: &'static str, on: bool, disabled: bool) -> iced::Element<'static, TestMsg> {
    ToggleSwitch::<TestMsg>::standalone("Sync", on, key, |_| None)
        .disabled(disabled)
        .busy(false)
        .into()
}

/// A radio group tree over a model the caller owns and keeps alive.
fn radio_tree<'a>(
    key: &'static str,
    model: &'a RadioGroupModel<&'static str>,
) -> iced::Element<'a, TestMsg> {
    RadioGroup::<TestMsg, &str>::controlled(Controlled::new(model), key, |_| None)
        .with_palette(Palette::dark())
        .into()
}

/// Two frames of one checkbox in two model states, plus the indicator box
/// the control published.
///
/// The probe harness and the capture session belong to the caller: a sweep
/// over several state changes reuses both instead of minting a renderer per
/// pair.
fn checkbox_frames(
    harness: &mut Harness<'_, TestMsg, HeadlessRenderer>,
    capture: &mut Capture,
    key: &'static str,
    from: CheckState,
    to: CheckState,
) -> (Frame, Frame, Rectangle) {
    let rect = published_box(key, &probe_checkbox(harness, key, false, from));
    let canvas = toggle_canvas();

    let before = capture.frame(canvas, checkbox_tree(key, from));
    let after = capture.frame(canvas, checkbox_tree(key, to));

    assert_fits(rect, canvas, "checkbox indicator");

    (before, after, rect)
}

/// Two frames of one switch in two model states, plus the track box the
/// control published.
fn switch_frames(
    harness: &mut Harness<'_, TestMsg, HeadlessRenderer>,
    capture: &mut Capture,
    key: &'static str,
    from: bool,
    to: bool,
) -> (Frame, Frame, Rectangle) {
    let rect = published_box(key, &probe_switch(harness, key, false, false));
    let canvas = toggle_canvas();

    let before = capture.frame(canvas, switch_tree(key, from, false));
    let after = capture.frame(canvas, switch_tree(key, to, false));

    assert_fits(rect, canvas, "switch track");

    (before, after, rect)
}

/// Two frames of a disabled switch — at rest, then with the pointer driven
/// over it and pressed — plus the track box it published.
///
/// The negative control has to be **pointer**-driven rather than
/// state-driven: a disabled switch's thumb still follows its polarity, so
/// flipping the model would move pixels for a reason that has nothing to
/// do with whether the control answers the pointer.
fn disabled_switch_rest_and_pressed(
    harness: &mut Harness<'_, TestMsg, HeadlessRenderer>,
    capture: &mut Capture,
    key: &'static str,
) -> (Frame, Frame, Rectangle) {
    let rect = published_box(key, &probe_switch(harness, key, true, false));
    let canvas = toggle_canvas();
    let at = centre(rect);

    let rest = capture.frame(canvas, switch_tree(key, false, true));
    let pressed_frame = capture.frame_after_at(
        canvas,
        switch_tree(key, false, true),
        &[moved(at), pressed()],
        mouse::Cursor::Available(at),
    );

    assert_fits(rect, canvas, "disabled switch track");

    (rest, pressed_frame, rect)
}

/// Two frames of a radio group in two selections, plus the box each option
/// published.
///
/// The boxes are read from the `from` model, which is the selection every
/// caller starts from.
fn radio_frames<'a, 'm: 'a>(
    harness: &mut Harness<'a, TestMsg, HeadlessRenderer>,
    capture: &mut Capture,
    key: &'static str,
    from: &'m RadioGroupModel<&'static str>,
    to: &'m RadioGroupModel<&'static str>,
) -> (Frame, Frame, Vec<Rectangle>) {
    let boxes = containers_for(&probe_radio(harness, key, from), &id_of(key));
    assert_eq!(
        boxes.len(),
        2,
        "precondition: a two-option group publishes one box per option, got \
         {boxes:?}"
    );
    let canvas = radio_canvas();

    let before = capture.frame(canvas, radio_tree(key, from));
    let after = capture.frame(canvas, radio_tree(key, to));

    for rect in &boxes {
        assert_fits(*rect, canvas, "radio option indicator");
    }

    (before, after, boxes)
}

#[test]
fn test_a_checkbox_repaints_its_indicator_when_its_state_moves() {
    let mut harness = Harness::headless(|| checkbox_tree(CB_REPAINT, CheckState::Unchecked));
    let mut capture = Capture::new();

    // -- turning checked
    let (before, after, indicator) = checkbox_frames(
        &mut harness,
        &mut capture,
        CB_REPAINT,
        CheckState::Unchecked,
        CheckState::Checked,
    );
    assert_repainted(&before, &after, indicator, "a checkbox turning checked");
    repaint_guard_panic(&before, indicator, "a checkbox turning checked");

    // -- turning unchecked
    let (before, after, indicator) = checkbox_frames(
        &mut harness,
        &mut capture,
        CB_REPAINT,
        CheckState::Checked,
        CheckState::Unchecked,
    );
    assert_repainted(&before, &after, indicator, "a checkbox turning unchecked");
    repaint_guard_panic(&before, indicator, "a checkbox turning unchecked");

    // -- turning indeterminate
    let (before, after, indicator) = checkbox_frames(
        &mut harness,
        &mut capture,
        CB_REPAINT,
        CheckState::Unchecked,
        CheckState::Indeterminate,
    );
    assert_repainted(
        &before,
        &after,
        indicator,
        "a checkbox turning indeterminate",
    );

    // -- the two marks are not the same mark
    let (before, after, indicator) = checkbox_frames(
        &mut harness,
        &mut capture,
        CB_REPAINT,
        CheckState::Checked,
        CheckState::Indeterminate,
    );
    assert_repainted(
        &before,
        &after,
        indicator,
        "a checked checkbox swapping its tick for the indeterminate dash",
    );

    // -- a control that was never touched paints nothing new
    let (before, same, indicator) = checkbox_frames(
        &mut harness,
        &mut capture,
        CB_REPAINT,
        CheckState::Unchecked,
        CheckState::Unchecked,
    );
    assert_eq!(
        before.differing_pixels(&same, indicator),
        0,
        "the same checkbox state repainted pixels inside its own indicator box — \
         the paint pass is not a function of the state it was handed"
    );

    // …and the zero is a measurement, not a blank canvas. The pair that moved
    // is the unchecked-to-checked pair this test rasterised first.
    let (moved_before, moved_after, moved_indicator) = checkbox_frames(
        &mut harness,
        &mut capture,
        CB_REPAINT,
        CheckState::Unchecked,
        CheckState::Checked,
    );
    zero_diff_guard(
        moved_before.differing_pixels(&moved_after, moved_indicator),
        "the untouched-checkbox control",
    );

    // -- pointer-driven: the state never moves, the visual state does
    let (rest, hovered, indicator) =
        checkbox_rest_and_hover(&mut harness, &mut capture, CB_REPAINT);
    assert_repainted(
        &rest,
        &hovered,
        indicator,
        "bringing the pointer onto a checkbox",
    );
    repaint_guard_panic(&rest, indicator, "bringing the pointer onto a checkbox");
}

#[test]
fn test_a_switch_repaints_its_track_when_its_thumb_moves() {
    let mut harness = Harness::headless(|| switch_tree(SW_REPAINT, false, false));
    let mut capture = Capture::new();

    // -- turning on
    let (before, after, track) = switch_frames(&mut harness, &mut capture, SW_REPAINT, false, true);
    assert_repainted(&before, &after, track, "a switch turning on");
    repaint_guard_panic(&before, track, "a switch turning on");

    // -- turning off
    let (before, after, track) = switch_frames(&mut harness, &mut capture, SW_REPAINT, true, false);
    assert_repainted(&before, &after, track, "a switch turning off");
    repaint_guard_panic(&before, track, "a switch turning off");

    // -- the thumb has to *travel*, not reappear. Diffing each of its two
    // homes separately is what tells a slid thumb from a thumb that was simply
    // repainted the same colour in the same place.
    let (before, after, track) = switch_frames(&mut harness, &mut capture, SW_REPAINT, false, true);
    let inset = size::SWITCH_THUMB_INSET;
    let thumb = Size::new(size::SWITCH_THUMB, size::SWITCH_THUMB);

    let off_home = Rectangle::new(Point::new(track.x + inset, track.y + inset), thumb);
    let on_home = Rectangle::new(
        Point::new(track.x + inset + toggle::thumb_travel(), track.y + inset),
        thumb,
    );

    assert_repainted(
        &before,
        &after,
        off_home,
        "the switch thumb leaving its off home",
    );
    assert_repainted(
        &before,
        &after,
        on_home,
        "the switch thumb arriving at its on home",
    );

    // -- a disabled switch paints nothing under the pointer
    let (rest, pressed_frame, track) =
        disabled_switch_rest_and_pressed(&mut harness, &mut capture, SW_REPAINT);
    assert_eq!(
        rest.differing_pixels(&pressed_frame, track),
        0,
        "a disabled switch repainted its track under the pointer — it has no \
         hover skin and must not invent one"
    );

    // …and the same pointer sequence over an enabled switch does move
    // pixels, so the zero above is the control, not the harness.
    let canvas = toggle_canvas();
    let at = centre(track);
    let live_rest = capture.frame(canvas, switch_tree(SW_LIVE, false, false));
    let live_pressed = capture.frame_after_at(
        canvas,
        switch_tree(SW_LIVE, false, false),
        &[moved(at), pressed()],
        mouse::Cursor::Available(at),
    );

    zero_diff_guard(
        live_rest.differing_pixels(&live_pressed, track),
        "the disabled-switch pointer control",
    );
}

#[test]
fn test_a_radio_group_repaints_the_option_it_takes_and_the_one_it_leaves() {
    let model_a = radio_model("a");
    let model_b = radio_model("b");
    let mut harness = Harness::headless(|| radio_tree(RG_REPAINT, &model_a));
    let mut capture = Capture::new();

    // One "a" to "b" pair carries both halves of the claim — the option that
    // was chosen and the option that lost its selection — and each half is
    // paired with the guard that proves it can fail.
    let (before, after, boxes) =
        radio_frames(&mut harness, &mut capture, RG_REPAINT, &model_a, &model_b);

    assert_repainted(
        &before,
        &after,
        boxes[1],
        "the radio option that was just selected",
    );
    repaint_guard_panic(&before, boxes[1], "the radio option that was just selected");

    assert_repainted(
        &before,
        &after,
        boxes[0],
        "the radio option that lost its selection",
    );
    repaint_guard_panic(
        &before,
        boxes[0],
        "the radio option that lost its selection",
    );

    // -- re-choosing the selected option changes no pixels
    let (before, same, boxes) =
        radio_frames(&mut harness, &mut capture, RG_REPAINT, &model_a, &model_a);
    for (index, rect) in boxes.iter().enumerate() {
        assert_eq!(
            before.differing_pixels(&same, *rect),
            0,
            "re-selecting the already-selected option repainted option {index} at \
             {rect:?} — the group moved nothing and painted something"
        );
    }

    // …and the same measurement over a real change is non-zero.
    for rect in &boxes {
        zero_diff_guard(
            before.differing_pixels(&after, *rect),
            "the unchanged-selection radio control",
        );
    }

    // -- pointer-driven: the selection never changes
    let (rest, hovered, boxes) =
        radio_rest_and_hover(&mut harness, &mut capture, RG_REPAINT, &model_a);
    assert_repainted(
        &rest,
        &hovered,
        boxes[1],
        "bringing the pointer onto an unselected radio option",
    );
    repaint_guard_panic(
        &rest,
        boxes[1],
        "bringing the pointer onto an unselected radio option",
    );
}

/// Two frames of one checkbox — at rest, then with the pointer driven
/// onto its indicator — plus the indicator box it published.
///
/// The state never moves here. What moves is the visual state, which is
/// the half of the repaint contract a model check cannot reach at all.
fn checkbox_rest_and_hover(
    harness: &mut Harness<'_, TestMsg, HeadlessRenderer>,
    capture: &mut Capture,
    key: &'static str,
) -> (Frame, Frame, Rectangle) {
    let rect = published_box(
        key,
        &probe_checkbox(harness, key, false, CheckState::Unchecked),
    );
    let canvas = toggle_canvas();
    let at = centre(rect);

    let rest = capture.frame(canvas, checkbox_tree(key, CheckState::Unchecked));
    let hovered = capture.frame_after_at(
        canvas,
        checkbox_tree(key, CheckState::Unchecked),
        &[moved(at), pressed()],
        mouse::Cursor::Available(at),
    );

    assert_fits(rect, canvas, "checkbox indicator");

    (rest, hovered, rect)
}

/// Two frames of a radio group whose selection never changed, with the
/// pointer moved onto the *unselected* option, plus the option boxes.
fn radio_rest_and_hover<'a, 'm: 'a>(
    harness: &mut Harness<'a, TestMsg, HeadlessRenderer>,
    capture: &mut Capture,
    key: &'static str,
    model: &'m RadioGroupModel<&'static str>,
) -> (Frame, Frame, Vec<Rectangle>) {
    let boxes = containers_for(&probe_radio(harness, key, model), &id_of(key));
    assert_eq!(
        boxes.len(),
        2,
        "precondition: a two-option group publishes one box per option, got \
         {boxes:?}"
    );
    let canvas = radio_canvas();
    let at = centre(boxes[1]);

    let rest = capture.frame(canvas, radio_tree(key, model));
    let hovered = capture.frame_after_at(
        canvas,
        radio_tree(key, model),
        &[moved(at), pressed()],
        mouse::Cursor::Available(at),
    );

    (rest, hovered, boxes)
}
