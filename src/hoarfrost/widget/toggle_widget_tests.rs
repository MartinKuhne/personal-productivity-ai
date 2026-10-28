//! Unit tests for live Boolean & Choice Controls: `Checkbox`, `RadioGroup`,
//! and `ToggleSwitch` (`UIF-ATOM-003`, §3.3).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::binding::Controlled;
use crate::test_helpers::{Harness, focus_one};
use crate::token::color::Palette;
use crate::token::size;
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
use iced::{Event, Point, Rectangle, Size};

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
// Checkbox tests (`UIF-ATOM-003`, §3.3)
// ===========================================================================

#[test]
fn test_checkbox_standalone_creation_and_properties() {
    let cb = Checkbox::<TestMsg>::standalone("Agree", CheckState::Unchecked, "cb_agree", |s| {
        Some(TestMsg::Checked(s))
    });

    assert_eq!(cb.label_text(), "Agree");
    assert_eq!(cb.current_state(), CheckState::Unchecked);
    assert!(cb.is_enabled());

    let custom_palette = Palette::dark();
    let cb = cb
        .with_palette(custom_palette)
        .with_children(ChildSummary {
            enabled: 2,
            checked: 2,
        })
        .disabled(true);

    assert!(!cb.is_enabled());
}

#[test]
fn test_checkbox_controlled_creation() {
    let model = CheckboxModel::new("Controlled")
        .with_state(CheckState::Checked)
        .disabled();
    let cb = Checkbox::<TestMsg>::controlled(Controlled::new(&model), "cb_ctl", |c| {
        Some(TestMsg::Checked(c.into_value()))
    });

    assert_eq!(cb.label_text(), "Controlled");
    assert_eq!(cb.current_state(), CheckState::Checked);
    assert!(!cb.is_enabled());
}

#[test]
fn test_checkbox_mouse_click_toggle_standalone() {
    let fired = Arc::new(AtomicUsize::new(0));
    let fired_clone = Arc::clone(&fired);

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        Checkbox::standalone("Click me", CheckState::Unchecked, "cb_click", move |s| {
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
    assert_eq!(fired.load(Ordering::SeqCst), 0);

    harness.press(target);
    harness.render();
    assert_eq!(fired.load(Ordering::SeqCst), 0);

    harness.release(target);
    harness.render();
    assert_eq!(fired.load(Ordering::SeqCst), 1);

    // Press and drag out should not fire
    harness.press(target);
    harness.hover(Point::new(500.0, 500.0));
    harness.release(Point::new(500.0, 500.0));
    harness.render();
    assert_eq!(fired.load(Ordering::SeqCst), 1);
}

#[test]
fn test_checkbox_controlled_toggle() {
    let fired = Arc::new(AtomicUsize::new(0));
    let fired_clone = Arc::clone(&fired);

    let model = CheckboxModel::new("Controlled Checkbox");

    let mut harness = Harness::headless(|| {
        let fired = Arc::clone(&fired_clone);
        Checkbox::controlled(Controlled::new(&model), "cb_ctl_click", move |commit| {
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
    assert_eq!(fired.load(Ordering::SeqCst), 1);
}

#[test]
fn test_checkbox_keyboard_space_toggle() {
    let fired = Arc::new(AtomicUsize::new(0));
    let fired_clone = Arc::clone(&fired);

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        Checkbox::standalone("Space me", CheckState::Unchecked, "cb_space", move |s| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Checked(s))
        })
        .into()
    });

    focus_one(&mut harness, "cb_space");
    harness.render();

    // Space key when focused triggers toggle
    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(msgs.len(), 1);
    assert_eq!(fired.load(Ordering::SeqCst), 1);

    // Other keys (e.g. Enter) do not toggle
    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert!(enter_msgs.is_empty());
    assert_eq!(fired.load(Ordering::SeqCst), 1);
}

#[test]
fn test_checkbox_tri_state_determinism() {
    // 1. Leaf Indeterminate -> Checked
    let fired_leaf = Arc::new(AtomicUsize::new(0));
    let fl_clone = Arc::clone(&fired_leaf);
    let mut harness_leaf = Harness::headless(move || {
        let fl = Arc::clone(&fl_clone);
        Checkbox::standalone("Leaf", CheckState::Indeterminate, "cb_tri_leaf", move |s| {
            if s == CheckState::Checked {
                fl.fetch_add(1, Ordering::SeqCst);
            }
            Some(TestMsg::Checked(s))
        })
        .with_children(ChildSummary::LEAF)
        .into()
    });
    harness_leaf.hover(Point::new(10.0, 10.0));
    harness_leaf.press(Point::new(10.0, 10.0));
    harness_leaf.release(Point::new(10.0, 10.0));
    harness_leaf.render();
    assert_eq!(fired_leaf.load(Ordering::SeqCst), 1);

    // 2. Parent Indeterminate with all enabled children checked -> Unchecked
    let fired_all = Arc::new(AtomicUsize::new(0));
    let fa_clone = Arc::clone(&fired_all);
    let mut harness_all = Harness::headless(move || {
        let fa = Arc::clone(&fa_clone);
        Checkbox::standalone(
            "Parent All",
            CheckState::Indeterminate,
            "cb_tri_all",
            move |s| {
                if s == CheckState::Unchecked {
                    fa.fetch_add(1, Ordering::SeqCst);
                }
                Some(TestMsg::Checked(s))
            },
        )
        .with_children(ChildSummary {
            enabled: 3,
            checked: 3,
        })
        .into()
    });
    harness_all.hover(Point::new(10.0, 10.0));
    harness_all.press(Point::new(10.0, 10.0));
    harness_all.release(Point::new(10.0, 10.0));
    harness_all.render();
    assert_eq!(fired_all.load(Ordering::SeqCst), 1);

    // 3. Parent Indeterminate with partial children checked -> Checked
    let fired_partial = Arc::new(AtomicUsize::new(0));
    let fp_clone = Arc::clone(&fired_partial);
    let mut harness_partial = Harness::headless(move || {
        let fp = Arc::clone(&fp_clone);
        Checkbox::standalone(
            "Parent Partial",
            CheckState::Indeterminate,
            "cb_tri_part",
            move |s| {
                if s == CheckState::Checked {
                    fp.fetch_add(1, Ordering::SeqCst);
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
    harness_partial.hover(Point::new(10.0, 10.0));
    harness_partial.press(Point::new(10.0, 10.0));
    harness_partial.release(Point::new(10.0, 10.0));
    harness_partial.render();
    assert_eq!(fired_partial.load(Ordering::SeqCst), 1);
}

#[test]
fn test_checkbox_disabled_gating() {
    let fired = Arc::new(AtomicUsize::new(0));
    let fired_clone = Arc::clone(&fired);

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        Checkbox::standalone("Disabled", CheckState::Unchecked, "cb_dis", move |s| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Checked(s))
        })
        .disabled(true)
        .into()
    });

    let target = Point::new(10.0, 10.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(fired.load(Ordering::SeqCst), 0);

    // Space key when disabled does nothing
    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert!(msgs.is_empty());
    assert_eq!(fired.load(Ordering::SeqCst), 0);
}

#[test]
fn test_checkbox_headless_draw_across_states() {
    for state in [
        CheckState::Unchecked,
        CheckState::Checked,
        CheckState::Indeterminate,
    ] {
        for disabled in [false, true] {
            let mut harness = Harness::headless(move || {
                Checkbox::<TestMsg>::standalone("Draw Test", state, "cb_draw", |_| None)
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
                focus_one(&mut harness, "cb_draw");
                harness.render();
            }
        }
    }
}

// ===========================================================================
// RadioGroup tests (`UIF-ATOM-003`, §3.3)
// ===========================================================================

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

    assert_eq!(group.model.get().selected, Some("opt1"));
}

#[test]
fn test_radio_group_mouse_selection() {
    let selected = Arc::new(std::sync::Mutex::new(None));
    let sel_clone = Arc::clone(&selected);

    let options = vec![
        RadioOption::new("first".to_string(), "First"),
        RadioOption::new("second".to_string(), "Second"),
        RadioOption::new("third".to_string(), "Third").disabled(),
    ];
    let model = RadioGroupModel::new(options);

    let mut harness = Harness::headless(|| {
        let sel = Arc::clone(&sel_clone);
        RadioGroup::controlled(Controlled::new(&model), "rg_click", move |commit| {
            *sel.lock().unwrap() = Some(commit.value().clone());
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
    assert_eq!(selected.lock().unwrap().as_deref(), Some("first"));

    // Click option 1 ("second") -> y in [24, 48)
    let pt1 = Point::new(10.0, 34.0);
    harness.hover(pt1);
    harness.press(pt1);
    harness.release(pt1);
    harness.render();
    assert_eq!(selected.lock().unwrap().as_deref(), Some("second"));

    // Click option 2 ("third", disabled) -> y in [48, 72) - must be ignored!
    *selected.lock().unwrap() = None;
    let pt2 = Point::new(10.0, 58.0);
    harness.hover(pt2);
    harness.press(pt2);
    harness.release(pt2);
    harness.render();
    assert_eq!(*selected.lock().unwrap(), None);
}

#[test]
fn test_radio_group_keyboard_arrow_roving() {
    let selected = Arc::new(std::sync::Mutex::new(None));
    let sel_clone = Arc::clone(&selected);

    let options = vec![
        RadioOption::new("a".to_string(), "Option A"),
        RadioOption::new("b".to_string(), "Option B").disabled(), // disabled!
        RadioOption::new("c".to_string(), "Option C"),
    ];
    let model = RadioGroupModel::new(options).with_selected("a".to_string());

    let mut harness = Harness::headless(|| {
        let sel = Arc::clone(&sel_clone);
        RadioGroup::controlled(Controlled::new(&model), "rg_rove", move |commit| {
            *sel.lock().unwrap() = Some(commit.value().clone());
            Some(TestMsg::Radio(commit.into_value()))
        })
        .into()
    });

    focus_one(&mut harness, "rg_rove");
    harness.render();

    // ArrowDown roves forward: skips disabled 'b' and lands on 'c'!
    let d1 = harness.pointer(&[key_press(Named::ArrowDown)], mouse::Cursor::Unavailable);
    assert_eq!(d1.len(), 1);
    assert_eq!(selected.lock().unwrap().as_deref(), Some("c"));

    // ArrowDown wraps around to 'a'!
    let d2 = harness.pointer(&[key_press(Named::ArrowDown)], mouse::Cursor::Unavailable);
    assert_eq!(d2.len(), 1);
    assert_eq!(selected.lock().unwrap().as_deref(), Some("a"));

    // ArrowUp roves backward: wraps to 'c'!
    let u1 = harness.pointer(&[key_press(Named::ArrowUp)], mouse::Cursor::Unavailable);
    assert_eq!(u1.len(), 1);
    assert_eq!(selected.lock().unwrap().as_deref(), Some("c"));
}

#[test]
fn test_radio_group_disabled_group_gating() {
    let selected = Arc::new(std::sync::Mutex::new(None));
    let sel_clone = Arc::clone(&selected);

    let options = vec![
        RadioOption::new("x".to_string(), "X"),
        RadioOption::new("y".to_string(), "Y"),
    ];
    let model = RadioGroupModel::new(options).disabled();

    let mut harness = Harness::headless(|| {
        let sel = Arc::clone(&sel_clone);
        RadioGroup::controlled(Controlled::new(&model), "rg_dis", move |commit| {
            *sel.lock().unwrap() = Some(commit.value().clone());
            Some(TestMsg::Radio(commit.into_value()))
        })
        .into()
    });

    let pt = Point::new(10.0, 10.0);
    harness.hover(pt);
    harness.press(pt);
    harness.release(pt);
    harness.render();
    assert_eq!(*selected.lock().unwrap(), None);

    // Arrow key does nothing when group is disabled
    let msgs = harness.pointer(&[key_press(Named::ArrowDown)], mouse::Cursor::Unavailable);
    assert!(msgs.is_empty());
    assert_eq!(*selected.lock().unwrap(), None);
}

#[test]
fn test_radio_group_headless_draw_across_states() {
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

            let mut harness = Harness::headless(|| {
                RadioGroup::<TestMsg, &str>::controlled(Controlled::new(&model), "rg_draw", |_| {
                    None
                })
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
                focus_one(&mut harness, "rg_draw");
                harness.render();
            }
        }
    }
}

#[test]
fn test_option_rectangles_geometry() {
    let options = vec![RadioOption::new("a", "A"), RadioOption::new("b", "B")];
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 48.0));
    let label_widths = [10.0, 10.0];

    let vertical = option_rectangles(bounds, RadioOrientation::Vertical, &options, &label_widths);
    assert_eq!(vertical.len(), 2);
    assert_eq!(vertical[0].y, 0.0);
    assert_eq!(vertical[0].height, size::HIT_TARGET_MIN);
    assert_eq!(vertical[1].y, size::HIT_TARGET_MIN);

    let horizontal = option_rectangles(
        bounds,
        RadioOrientation::HorizontalWrap,
        &options,
        &label_widths,
    );
    assert_eq!(horizontal.len(), 2);
    assert_eq!(horizontal[0].y, 0.0);
    assert_eq!(horizontal[0].y, 0.0);
    assert!(horizontal[1].x > horizontal[0].x);
}

// ===========================================================================
// ToggleSwitch tests (`UIF-ATOM-003`, §3.3)
// ===========================================================================

#[test]
fn test_switch_standalone_creation_and_properties() {
    let switch = ToggleSwitch::<TestMsg>::standalone("WiFi", false, "sw_wifi", |on| {
        Some(TestMsg::Switched(on))
    });

    assert_eq!(switch.label_text(), "WiFi");
    assert!(!switch.is_on());
    assert!(switch.is_enabled());
    assert!(!switch.is_busy());

    let switch = switch
        .with_texts("Enabled", "Disabled")
        .with_palette(Palette::dark())
        .busy(true)
        .disabled(true);

    assert!(switch.is_busy());
    assert!(!switch.is_enabled());
    assert_eq!(switch.state_text(), Some("Disabled"));
}

#[test]
fn test_switch_controlled_creation() {
    let model = SwitchModel::new("Bluetooth")
        .with_on(true)
        .with_texts("On", "Off")
        .busy();
    let switch = ToggleSwitch::<TestMsg>::controlled(Controlled::new(&model), "sw_bt", |commit| {
        Some(TestMsg::Switched(commit.into_value()))
    });

    assert_eq!(switch.label_text(), "Bluetooth");
    assert!(switch.is_on());
    assert!(switch.is_busy());
    assert_eq!(switch.state_text(), Some("On"));
}

#[test]
fn test_switch_mouse_toggle_standalone() {
    let fired = Arc::new(std::sync::Mutex::new(None));
    let fired_clone = Arc::clone(&fired);

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        ToggleSwitch::standalone("Airplane Mode", false, "sw_click", move |on| {
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
    assert_eq!(*fired.lock().unwrap(), Some(true));

    // Press and drag out does not toggle
    *fired.lock().unwrap() = None;
    harness.press(target);
    harness.hover(Point::new(400.0, 400.0));
    harness.release(Point::new(400.0, 400.0));
    harness.render();
    assert_eq!(*fired.lock().unwrap(), None);
}

#[test]
fn test_switch_controlled_toggle() {
    let fired = Arc::new(std::sync::Mutex::new(None));
    let fired_clone = Arc::clone(&fired);

    let model = SwitchModel::new("Controlled Switch").with_on(false);

    let mut harness = Harness::headless(|| {
        let fired = Arc::clone(&fired_clone);
        ToggleSwitch::controlled(Controlled::new(&model), "sw_ctl", move |commit| {
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
    assert_eq!(*fired.lock().unwrap(), Some(true));
}

#[test]
fn test_switch_keyboard_space_toggle() {
    let fired = Arc::new(std::sync::Mutex::new(None));
    let fired_clone = Arc::clone(&fired);

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        ToggleSwitch::standalone("Keyboard Switch", false, "sw_kb", move |on| {
            *fired.lock().unwrap() = Some(on);
            Some(TestMsg::Switched(on))
        })
        .into()
    });

    focus_one(&mut harness, "sw_kb");
    harness.render();

    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(msgs.len(), 1);
    assert_eq!(*fired.lock().unwrap(), Some(true));
}

#[test]
fn test_switch_busy_lock_refusal() {
    let fired = Arc::new(AtomicUsize::new(0));
    let fired_clone = Arc::clone(&fired);

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        ToggleSwitch::standalone("Busy Switch", false, "sw_busy", move |on| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Switched(on))
        })
        .busy(true)
        .into()
    });

    let target = Point::new(15.0, 12.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(fired.load(Ordering::SeqCst), 0);

    // Keyboard Space when busy is also refused
    let msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert!(msgs.is_empty());
    assert_eq!(fired.load(Ordering::SeqCst), 0);
}

#[test]
fn test_switch_disabled_gating() {
    let fired = Arc::new(AtomicUsize::new(0));
    let fired_clone = Arc::clone(&fired);

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        ToggleSwitch::standalone("Disabled Switch", false, "sw_dis", move |on| {
            fired.fetch_add(1, Ordering::SeqCst);
            Some(TestMsg::Switched(on))
        })
        .disabled(true)
        .into()
    });

    let target = Point::new(15.0, 12.0);
    harness.hover(target);
    harness.press(target);
    harness.release(target);
    harness.render();
    assert_eq!(fired.load(Ordering::SeqCst), 0);
}

#[test]
fn test_switch_headless_draw_across_states() {
    for on in [false, true] {
        for busy in [false, true] {
            for disabled in [false, true] {
                let mut harness = Harness::headless(move || {
                    ToggleSwitch::<TestMsg>::standalone("Switch Draw", on, "sw_draw", |_| None)
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
                    focus_one(&mut harness, "sw_draw");
                    harness.render();
                }
            }
        }
    }
}

#[test]
fn test_constants_and_glyphs() {
    assert_eq!(CHECK_MARK, "✓");
    assert_eq!(INDETERMINATE_MARK, "\u{2212}");
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
// ===========================================================================

use crate::focus::FocusId;
use crate::identity::{role, sub_part_id};
use crate::test_helpers::{Candidate, KIND_CONTAINER, KIND_FOCUSABLE, KIND_TEXT};
use iced::widget::Id;

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

fn probe_checkbox(key: &'static str, disabled: bool, state: CheckState) -> Vec<Candidate> {
    let mut harness = Harness::headless(move || {
        Checkbox::<TestMsg>::standalone("Agree", state, key, |_| None)
            .disabled(disabled)
            .into()
    });

    harness.candidates()
}

fn probe_switch(key: &'static str, disabled: bool, busy: bool) -> Vec<Candidate> {
    let mut harness = Harness::headless(move || {
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

fn probe_radio(key: &'static str, group_disabled: bool) -> Vec<Candidate> {
    let base = radio_model("a");
    let model = if group_disabled {
        base.disabled()
    } else {
        base
    };

    let mut harness = Harness::headless(|| {
        RadioGroup::<TestMsg, &str>::controlled(Controlled::new(&model), key, |commit| {
            Some(TestMsg::Radio(commit.into_value().to_string()))
        })
        .with_palette(Palette::dark())
        .into()
    });

    harness.candidates()
}

// -- the defect: a disabled control used to be invisible -----------------

#[test]
fn test_a_disabled_checkbox_publishes_a_record_of_itself() {
    let id = id_of("cb_dis_box");
    let candidates = probe_checkbox("cb_dis_box", true, CheckState::Unchecked);

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
}

#[test]
fn test_a_disabled_switch_publishes_a_record_of_itself() {
    let id = id_of("sw_dis_box");
    let candidates = probe_switch("sw_dis_box", true, false);

    assert!(
        caption_for(&candidates, &id).is_some(),
        "precondition: the disabled switch's caption is published"
    );
    assert_eq!(
        containers_for(&candidates, &id).len(),
        1,
        "a disabled switch must still publish exactly one record of the track \
         it occupies"
    );
}

#[test]
fn test_a_disabled_radio_group_publishes_a_record_per_option() {
    let id = id_of("rg_dis_box");
    let candidates = probe_radio("rg_dis_box", true);

    assert_eq!(
        containers_for(&candidates, &id).len(),
        2,
        "a disabled group of two options must publish one box per option — each \
         option is a control with a caption that has to bind to something"
    );
}

// -- and it is still not focusable --------------------------------------

#[test]
fn test_publishing_the_box_does_not_make_a_disabled_control_focusable() {
    for (name, candidates, id) in [
        (
            "checkbox",
            probe_checkbox("cb_dis_nofocus", true, CheckState::Checked),
            id_of("cb_dis_nofocus"),
        ),
        (
            "switch",
            probe_switch("sw_dis_nofocus", true, false),
            id_of("sw_dis_nofocus"),
        ),
        (
            "radio group",
            probe_radio("rg_dis_nofocus", true),
            id_of("rg_dis_nofocus"),
        ),
    ] {
        assert!(
            focusables_for(&candidates, &id).is_empty(),
            "a disabled {name} published a focusable record — publishing its box \
             must not make it a Tab stop"
        );
    }
}

#[test]
fn test_a_busy_switch_publishes_its_box_and_keeps_its_focus_state() {
    let id = id_of("sw_busy_box");
    let candidates = probe_switch("sw_busy_box", false, true);

    assert_eq!(
        containers_for(&candidates, &id).len(),
        1,
        "a busy switch still occupies a box and must publish it"
    );
    assert_eq!(
        focusables_for(&candidates, &id).len(),
        1,
        "busy locks the interaction, not the focus state — unchanged behaviour, \
         pinned so this fix cannot have altered it"
    );
}

// -- the rule holds in every state, with exactly one record -------------

#[test]
fn test_every_control_publishes_exactly_one_box_in_every_state() {
    let checkbox = |disabled: bool| {
        containers_for(
            &probe_checkbox("cb_rule", disabled, CheckState::Indeterminate),
            &id_of("cb_rule"),
        )
    };
    let switch = |disabled: bool, busy: bool| {
        containers_for(&probe_switch("sw_rule", disabled, busy), &id_of("sw_rule"))
    };
    let radio = |group_disabled: bool| {
        containers_for(&probe_radio("rg_rule", group_disabled), &id_of("rg_rule"))
    };

    for (label, boxes) in [
        ("checkbox enabled", checkbox(false)),
        ("checkbox disabled", checkbox(true)),
        ("switch enabled", switch(false, false)),
        ("switch busy", switch(false, true)),
        ("switch disabled", switch(true, false)),
        ("radio enabled", radio(false)),
        ("radio disabled", radio(true)),
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
    assert_eq!(checkbox(false).len(), 1, "one enabled checkbox, one record");
    assert_eq!(checkbox(true).len(), 1, "one disabled checkbox, one record");
    assert_eq!(
        switch(false, false).len(),
        1,
        "one enabled switch, one record"
    );
    assert_eq!(
        switch(true, false).len(),
        1,
        "one disabled switch, one record"
    );
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
#[test]
fn test_the_published_checkbox_box_is_the_box_the_paint_pass_drew() {
    use crate::test_helpers::{Capture, PIXEL_SCALE};

    let key = "cb_paint";
    let id = id_of(key);
    let published = containers_for(&probe_checkbox(key, false, CheckState::Unchecked), &id);
    assert_eq!(published.len(), 1, "precondition: one published box");

    let canvas = Size::new(size::HIT_TARGET_MIN, size::HIT_TARGET_MIN);
    let tree: iced::Element<'static, TestMsg> =
        Checkbox::<TestMsg>::standalone("", CheckState::Unchecked, key, |_| None).into();
    let mut capture = Capture::new();
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
}

/// The same claim for the switch, whose track is the only toggle shape
/// with an aspect ratio of its own.
#[test]
fn test_the_published_switch_box_is_the_box_the_paint_pass_drew() {
    use crate::test_helpers::{Capture, PIXEL_SCALE};

    let key = "sw_paint";
    let id = id_of(key);
    let published = containers_for(&probe_switch(key, false, false), &id);
    assert_eq!(published.len(), 1, "precondition: one published box");

    let canvas = Size::new(size::HIT_TARGET_MIN * 3.0, size::HIT_TARGET_MIN);
    let tree: iced::Element<'static, TestMsg> =
        ToggleSwitch::<TestMsg>::standalone("", false, key, |_| None).into();
    let mut capture = Capture::new();
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
}

/// The published control box must not be the label-inclusive row: that
/// would tell a host the control is wider than the thing it can see.
#[test]
fn test_the_published_box_is_the_indicator_not_the_label_inclusive_row() {
    let key = "cb_vs_row";
    let id = id_of(key);
    let candidates = probe_checkbox(key, false, CheckState::Checked);

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
// reads as coverage.
// ===========================================================================

use crate::test_helpers::{
    Capture, Frame, MIN_CHANGED_PIXELS, assert_repainted, centre, moved, pressed,
};
use crate::widget::toggle;
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

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

/// The text a caught assertion produced.
///
/// `test_helpers` has an equivalent, but it is private to that sidecar,
/// so every file that wants to prove an assertion is load-bearing has to
/// write this down again.
fn panic_text(outcome: &Result<(), Box<dyn Any + Send>>) -> String {
    match outcome {
        Ok(()) => String::from("<no panic>"),
        Err(payload) => {
            if let Some(text) = payload.downcast_ref::<String>() {
                text.clone()
            } else if let Some(text) = payload.downcast_ref::<&str>() {
                (*text).to_owned()
            } else {
                String::from("<non-string panic payload>")
            }
        }
    }
}

/// Prove a repaint claim is load-bearing over `rect`.
///
/// Run against a frame compared with itself — no repaint whatsoever — the
/// assertion must go red, and the message must name the control and the
/// floor it missed. Returns the panic text so the caller can pin the
/// wording it fired on.
fn repaint_guard_panic(frame: &Frame, rect: Rectangle, what: &str) -> String {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        assert_repainted(frame, frame, rect, what);
    }));

    assert!(
        outcome.is_err(),
        "{what}: `assert_repainted` PASSED comparing a frame with itself over \
         {rect:?} — the assertion can never fail, so the repaint claim built on \
         it proves nothing"
    );

    let text = panic_text(&outcome);

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
fn checkbox_frames(
    key: &'static str,
    from: CheckState,
    to: CheckState,
) -> (Frame, Frame, Rectangle) {
    let rect = published_box(key, &probe_checkbox(key, false, from));
    let canvas = toggle_canvas();

    let mut capture = Capture::new();
    let (before, after) = {
        (
            capture.frame(canvas, checkbox_tree(key, from)),
            capture.frame(canvas, checkbox_tree(key, to)),
        )
    };

    assert_fits(rect, canvas, "checkbox indicator");

    (before, after, rect)
}

/// Two frames of one switch in two model states, plus the track box the
/// control published.
fn switch_frames(key: &'static str, from: bool, to: bool) -> (Frame, Frame, Rectangle) {
    let rect = published_box(key, &probe_switch(key, false, false));
    let canvas = toggle_canvas();

    let mut capture = Capture::new();
    let (before, after) = {
        (
            capture.frame(canvas, switch_tree(key, from, false)),
            capture.frame(canvas, switch_tree(key, to, false)),
        )
    };

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
fn disabled_switch_rest_and_pressed(key: &'static str) -> (Frame, Frame, Rectangle) {
    let rect = published_box(key, &probe_switch(key, true, false));
    let canvas = toggle_canvas();
    let at = centre(rect);

    let mut capture = Capture::new();
    let (rest, pressed_frame) = {
        let rest = capture.frame(canvas, switch_tree(key, false, true));
        let pressed_frame = capture.frame_after_at(
            canvas,
            switch_tree(key, false, true),
            &[moved(at), pressed()],
            mouse::Cursor::Available(at),
        );

        (rest, pressed_frame)
    };

    assert_fits(rect, canvas, "disabled switch track");

    (rest, pressed_frame, rect)
}

/// Two frames of a radio group in two selections, plus the box each option
/// published.
fn radio_frames(
    key: &'static str,
    from: &'static str,
    to: &'static str,
) -> (Frame, Frame, Vec<Rectangle>) {
    let boxes = containers_for(&probe_radio(key, false), &id_of(key));
    assert_eq!(
        boxes.len(),
        2,
        "precondition: a two-option group publishes one box per option, got \
         {boxes:?}"
    );
    let canvas = radio_canvas();
    let from_model = radio_model(from);
    let to_model = radio_model(to);

    let mut capture = Capture::new();
    let (before, after) = {
        (
            capture.frame(canvas, radio_tree(key, &from_model)),
            capture.frame(canvas, radio_tree(key, &to_model)),
        )
    };

    for rect in &boxes {
        assert_fits(*rect, canvas, "radio option indicator");
    }

    (before, after, boxes)
}

// -- Checkbox -----------------------------------------------------------

#[test]
fn test_a_checkbox_turning_checked_repaints_its_indicator() {
    let (before, after, indicator) =
        checkbox_frames("cb_rp_check", CheckState::Unchecked, CheckState::Checked);

    assert_repainted(&before, &after, indicator, "a checkbox turning checked");
}

#[test]
fn test_the_checkbox_checked_repaint_claim_fails_on_a_frame_that_never_moved() {
    let (before, _after, indicator) = checkbox_frames(
        "cb_rp_check_vac",
        CheckState::Unchecked,
        CheckState::Checked,
    );

    repaint_guard_panic(&before, indicator, "a checkbox turning checked");
}

#[test]
fn test_a_checkbox_turning_unchecked_repaints_its_indicator() {
    let (before, after, indicator) =
        checkbox_frames("cb_rp_uncheck", CheckState::Checked, CheckState::Unchecked);

    assert_repainted(&before, &after, indicator, "a checkbox turning unchecked");
}

#[test]
fn test_the_checkbox_unchecked_repaint_claim_fails_on_a_frame_that_never_moved() {
    let (before, _after, indicator) = checkbox_frames(
        "cb_rp_uncheck_vac",
        CheckState::Checked,
        CheckState::Unchecked,
    );

    repaint_guard_panic(&before, indicator, "a checkbox turning unchecked");
}

#[test]
fn test_a_checkbox_turning_indeterminate_repaints_its_indicator() {
    let (before, after, indicator) = checkbox_frames(
        "cb_rp_indet",
        CheckState::Unchecked,
        CheckState::Indeterminate,
    );

    assert_repainted(
        &before,
        &after,
        indicator,
        "a checkbox turning indeterminate",
    );
}

#[test]
fn test_the_indeterminate_mark_is_not_painted_the_checked_mark() {
    let (before, after, indicator) =
        checkbox_frames("cb_rp_mark", CheckState::Checked, CheckState::Indeterminate);

    assert_repainted(
        &before,
        &after,
        indicator,
        "a checked checkbox swapping its tick for the indeterminate dash",
    );
}

#[test]
fn test_a_checkbox_that_was_never_touched_paints_nothing_new() {
    let (before, same, indicator) =
        checkbox_frames("cb_rp_still", CheckState::Unchecked, CheckState::Unchecked);

    assert_eq!(
        before.differing_pixels(&same, indicator),
        0,
        "the same checkbox state repainted pixels inside its own indicator box — \
         the paint pass is not a function of the state it was handed"
    );

    // …and the zero is a measurement, not a blank canvas.
    let (moved_before, moved_after, moved_indicator) = checkbox_frames(
        "cb_rp_still_probe",
        CheckState::Unchecked,
        CheckState::Checked,
    );
    zero_diff_guard(
        moved_before.differing_pixels(&moved_after, moved_indicator),
        "the untouched-checkbox control",
    );
}

// -- Switch -------------------------------------------------------------

#[test]
fn test_a_switch_turning_on_repaints_its_track() {
    let (before, after, track) = switch_frames("sw_rp_on", false, true);

    assert_repainted(&before, &after, track, "a switch turning on");
}

#[test]
fn test_the_switch_on_repaint_claim_fails_on_a_frame_that_never_moved() {
    let (before, _after, track) = switch_frames("sw_rp_on_vac", false, true);

    repaint_guard_panic(&before, track, "a switch turning on");
}

#[test]
fn test_a_switch_turning_off_repaints_its_track() {
    let (before, after, track) = switch_frames("sw_rp_off", true, false);

    assert_repainted(&before, &after, track, "a switch turning off");
}

#[test]
fn test_the_switch_off_repaint_claim_fails_on_a_frame_that_never_moved() {
    let (before, _after, track) = switch_frames("sw_rp_off_vac", true, false);

    repaint_guard_panic(&before, track, "a switch turning off");
}

/// The thumb has to *travel*, not reappear. Diffing each of its two homes
/// separately is what tells a slid thumb from a thumb that was simply
/// repainted the same colour in the same place.
#[test]
fn test_a_switch_turning_on_moves_its_thumb_across_the_track() {
    let (before, after, track) = switch_frames("sw_rp_travel", false, true);
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
}

#[test]
fn test_a_disabled_switch_repaints_nothing_under_the_pointer() {
    let (rest, pressed_frame, track) = disabled_switch_rest_and_pressed("sw_rp_disabled");

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
    let mut capture = Capture::new();
    let (live_rest, live_pressed) = {
        let rest = capture.frame(canvas, switch_tree("sw_rp_disabled_live", false, false));
        let live_pressed = capture.frame_after_at(
            canvas,
            switch_tree("sw_rp_disabled_live", false, false),
            &[moved(at), pressed()],
            mouse::Cursor::Available(at),
        );

        (rest, live_pressed)
    };

    zero_diff_guard(
        live_rest.differing_pixels(&live_pressed, track),
        "the disabled-switch pointer control",
    );
}

// -- Radio --------------------------------------------------------------

#[test]
fn test_choosing_another_option_repaints_the_option_it_chose() {
    let (before, after, boxes) = radio_frames("rg_rp_take", "a", "b");

    assert_repainted(
        &before,
        &after,
        boxes[1],
        "the radio option that was just selected",
    );
}

#[test]
fn test_the_radio_selection_claim_fails_on_a_frame_that_never_moved() {
    let (before, _after, boxes) = radio_frames("rg_rp_take_vac", "a", "b");

    repaint_guard_panic(&before, boxes[1], "the radio option that was just selected");
}

#[test]
fn test_choosing_another_option_repaints_the_option_it_abandoned() {
    let (before, after, boxes) = radio_frames("rg_rp_leave", "a", "b");

    assert_repainted(
        &before,
        &after,
        boxes[0],
        "the radio option that lost its selection",
    );
}

#[test]
fn test_the_radio_abandonment_claim_fails_on_a_frame_that_never_moved() {
    let (before, _after, boxes) = radio_frames("rg_rp_leave_vac", "a", "b");

    repaint_guard_panic(
        &before,
        boxes[0],
        "the radio option that lost its selection",
    );
}

#[test]
fn test_re_choosing_the_selected_option_changes_no_pixels() {
    let (before, same, boxes) = radio_frames("rg_rp_still", "a", "a");

    for (index, rect) in boxes.iter().enumerate() {
        assert_eq!(
            before.differing_pixels(&same, *rect),
            0,
            "re-selecting the already-selected option repainted option {index} at \
             {rect:?} — the group moved nothing and painted something"
        );
    }

    // …and the same measurement over a real change is non-zero.
    let (moved_before, moved_after, moved_boxes) = radio_frames("rg_rp_still_probe", "a", "b");
    for rect in &moved_boxes {
        zero_diff_guard(
            moved_before.differing_pixels(&moved_after, *rect),
            "the unchanged-selection radio control",
        );
    }
}

// -- Pointer-driven repaint ---------------------------------------------

/// Two frames of one checkbox — at rest, then with the pointer driven
/// onto its indicator — plus the indicator box it published.
///
/// The state never moves here. What moves is the visual state, which is
/// the half of the repaint contract a model check cannot reach at all.
fn checkbox_rest_and_hover(key: &'static str) -> (Frame, Frame, Rectangle) {
    let rect = published_box(key, &probe_checkbox(key, false, CheckState::Unchecked));
    let canvas = toggle_canvas();
    let at = centre(rect);

    let mut capture = Capture::new();
    let (rest, hovered) = {
        let rest = capture.frame(canvas, checkbox_tree(key, CheckState::Unchecked));
        let hovered = capture.frame_after_at(
            canvas,
            checkbox_tree(key, CheckState::Unchecked),
            &[moved(at), pressed()],
            mouse::Cursor::Available(at),
        );

        (rest, hovered)
    };

    assert_fits(rect, canvas, "checkbox indicator");

    (rest, hovered, rect)
}

#[test]
fn test_a_checkbox_under_the_pointer_repaints_its_indicator() {
    let (rest, hovered, indicator) = checkbox_rest_and_hover("cb_rp_hover");

    assert_repainted(
        &rest,
        &hovered,
        indicator,
        "bringing the pointer onto a checkbox",
    );
}

#[test]
fn test_the_checkbox_hover_repaint_claim_fails_on_a_frame_that_never_moved() {
    let (rest, _hovered, indicator) = checkbox_rest_and_hover("cb_rp_hover_vac");

    repaint_guard_panic(&rest, indicator, "bringing the pointer onto a checkbox");
}

/// Two frames of a radio group whose selection never changed, with the
/// pointer moved onto the *unselected* option, plus the option boxes.
fn radio_rest_and_hover(key: &'static str) -> (Frame, Frame, Vec<Rectangle>) {
    let boxes = containers_for(&probe_radio(key, false), &id_of(key));
    assert_eq!(
        boxes.len(),
        2,
        "precondition: a two-option group publishes one box per option, got \
         {boxes:?}"
    );
    let canvas = radio_canvas();
    let at = centre(boxes[1]);
    let model = radio_model("a");

    let mut capture = Capture::new();
    let (rest, hovered) = {
        let rest = capture.frame(canvas, radio_tree(key, &model));
        let hovered = capture.frame_after_at(
            canvas,
            radio_tree(key, &model),
            &[moved(at), pressed()],
            mouse::Cursor::Available(at),
        );

        (rest, hovered)
    };

    (rest, hovered, boxes)
}

#[test]
fn test_a_radio_option_under_the_pointer_repaints_its_indicator() {
    let (rest, hovered, boxes) = radio_rest_and_hover("rg_rp_hover");

    assert_repainted(
        &rest,
        &hovered,
        boxes[1],
        "bringing the pointer onto an unselected radio option",
    );
}

#[test]
fn test_the_radio_hover_repaint_claim_fails_on_a_frame_that_never_moved() {
    let (rest, _hovered, boxes) = radio_rest_and_hover("rg_rp_hover_vac");

    repaint_guard_panic(
        &rest,
        boxes[1],
        "bringing the pointer onto an unselected radio option",
    );
}
