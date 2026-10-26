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

fn leaked<T>(v: T) -> &'static T {
    Box::leak(Box::new(v))
}

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

    let model = leaked(CheckboxModel::new("Controlled Checkbox"));

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        Checkbox::controlled(Controlled::new(model), "cb_ctl_click", move |commit| {
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
    let model = leaked(
        RadioGroupModel::new(options)
            .with_selected("opt1")
            .with_orientation(RadioOrientation::HorizontalWrap),
    );

    let group =
        RadioGroup::<TestMsg, &str>::controlled(Controlled::new(model), "rg_prop", |commit| {
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
    let model = leaked(RadioGroupModel::new(options));

    let mut harness = Harness::headless(move || {
        let sel = Arc::clone(&sel_clone);
        RadioGroup::controlled(Controlled::new(model), "rg_click", move |commit| {
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
    let model = leaked(RadioGroupModel::new(options).with_selected("a".to_string()));

    let mut harness = Harness::headless(move || {
        let sel = Arc::clone(&sel_clone);
        RadioGroup::controlled(Controlled::new(model), "rg_rove", move |commit| {
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
    let model = leaked(RadioGroupModel::new(options).disabled());

    let mut harness = Harness::headless(move || {
        let sel = Arc::clone(&sel_clone);
        RadioGroup::controlled(Controlled::new(model), "rg_dis", move |commit| {
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
            let mut m = RadioGroupModel::new(options)
                .with_orientation(orientation)
                .with_selected("1");
            if disabled {
                m = m.disabled();
            }
            let model = leaked(m);

            let mut harness = Harness::headless(move || {
                RadioGroup::<TestMsg, &str>::controlled(Controlled::new(model), "rg_draw", |_| None)
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

    let model = leaked(SwitchModel::new("Controlled Switch").with_on(false));

    let mut harness = Harness::headless(move || {
        let fired = Arc::clone(&fired_clone);
        ToggleSwitch::controlled(Controlled::new(model), "sw_ctl", move |commit| {
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
