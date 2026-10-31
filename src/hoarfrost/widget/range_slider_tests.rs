//! Unit tests for `RangeSlider` paint resolver, geometry, and interactive widget (§3.5, `UIF-ATOM-005b`, `UIF-ATOM-005`).

use super::*;
use crate::state::BaseState;
use crate::test_helpers::{Harness, focus_one};
use iced::advanced::widget::Widget;
use iced::keyboard::key::{Key, Named};
use iced_test::renderer::Renderer as HeadlessRenderer;

fn palette() -> Palette {
    Palette::dark()
}

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Changed((f64, f64)),
    Committed((f64, f64)),
    Cancelled((f64, f64)),
}

fn key_event(key: Named, shift: bool) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(key),
        modified_key: Key::Named(key),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: if shift {
            iced::keyboard::Modifiers::SHIFT
        } else {
            iced::keyboard::Modifiers::NONE
        },
        text: None,
        repeat: false,
    })
}

// =============================================================================
// 1. Style Resolution (§3.5, `UIF-ATOM-005b`)
// =============================================================================

#[test]
fn test_style_resolution_enabled_and_disabled() {
    let p = palette();

    // Enabled style
    let style = resolve_style(&p, false);
    assert_eq!(style.rail_unselected, p.surface_active);
    assert_eq!(style.rail_border, p.border_subtle);
    assert_eq!(style.rail_selected, p.accent_fill);
    assert_eq!(style.thumb_resting_fill, p.text_strong);
    assert_eq!(style.thumb_resting_border, p.accent_border);
    assert_eq!(style.thumb_hovered_fill, p.accent_fill);
    assert_eq!(style.thumb_hovered_border, p.text_strong);
    assert_eq!(style.readout_bg, p.surface_overlay);
    assert_eq!(style.readout_border, p.border_strong);
    assert_eq!(style.readout_text, p.text_primary);
    assert_eq!(style.focus_ring_width, FOCUS_RING_WIDTH);

    // Disabled style drops accent fill on rail
    let disabled_style = resolve_style(&p, true);
    assert_eq!(disabled_style.rail_selected, p.border_strong);
    assert_eq!(disabled_style.thumb_disabled_fill, p.surface_raised);
    assert_eq!(disabled_style.thumb_disabled_border, p.text_disabled);
}

#[test]
fn test_thumb_paint_by_state() {
    let p = palette();
    let style = resolve_style(&p, false);

    // Rest
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    assert_eq!(
        thumb_paint(&style, rest),
        (style.thumb_resting_fill, style.thumb_resting_border)
    );

    // Hovered
    let hovered = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };
    assert_eq!(
        thumb_paint(&style, hovered),
        (style.thumb_hovered_fill, style.thumb_hovered_border)
    );

    // Pressed
    let pressed = VisualState {
        base: BaseState::Pressed,
        focused: false,
    };
    assert_eq!(
        thumb_paint(&style, pressed),
        (style.thumb_hovered_fill, style.thumb_hovered_border)
    );

    // Disabled
    let disabled = VisualState {
        base: BaseState::Disabled,
        focused: false,
    };
    assert_eq!(
        thumb_paint(&style, disabled),
        (style.thumb_disabled_fill, style.thumb_disabled_border)
    );
}

// =============================================================================
// 2. Geometry & Rectangles (§3.5, `UIF-ATOM-005b`)
// =============================================================================

#[test]
fn test_geometry_and_projections() {
    let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(200.0, 30.0));
    let track = track_rect(bounds);

    assert_eq!(track.x, 10.0);
    assert_eq!(track.y, 20.0 + (30.0 - TRACK_HEIGHT) / 2.0);
    assert_eq!(track.width, 200.0);
    assert_eq!(track.height, TRACK_HEIGHT);

    let travel = thumb_travel(track);
    assert_eq!(travel, 200.0 - THUMB_SIZE);

    // Fractions
    let start_x = thumb_center_x(track, 0.0);
    assert_eq!(start_x, 10.0 + THUMB_SIZE / 2.0);

    let end_x = thumb_center_x(track, 1.0);
    assert_eq!(end_x, 10.0 + 200.0 - THUMB_SIZE / 2.0);

    let mid_x = thumb_center_x(track, 0.5);
    assert_eq!(fraction_at_x(track, mid_x), 0.5);
    assert_eq!(fraction_at_x(track, 0.0), 0.0);
    assert_eq!(fraction_at_x(track, 500.0), 1.0);

    // Thumb and hit rects
    let t_rect = thumb_rect(bounds, 0.25);
    assert_eq!(t_rect.width, THUMB_SIZE);
    assert_eq!(t_rect.height, THUMB_SIZE);

    let h_rect = hit_rect(bounds, 0.25);
    assert_eq!(h_rect.width, THUMB_HIT_SIZE);
    assert_eq!(h_rect.height, THUMB_HIT_SIZE);
    assert!(h_rect.contains(Point::new(t_rect.x + 1.0, t_rect.y + 1.0)));

    // Active segment
    let seg = active_segment_rect(bounds, 0.2, 0.8);
    let lx = thumb_center_x(track, 0.2);
    let ux = thumb_center_x(track, 0.8);
    assert_eq!(seg.x, lx);
    assert_eq!(seg.width, ux - lx);

    // Readout positioning
    let text_size = Size::new(40.0, 16.0);
    let r_rect = readout_rect(t_rect, text_size);
    // Readout bottom sits exactly READOUT_GAP (4px) above thumb top
    assert_eq!(r_rect.y + r_rect.height, t_rect.y - READOUT_GAP);
}

// =============================================================================
// 3. Live Interactive Widget Events (§3.5, `UIF-ATOM-005b`)
// =============================================================================

#[test]
fn test_widget_creation_and_state() {
    let model = RangeSliderModel::new(0.0, 100.0, 20.0, 80.0);
    let slider = RangeSlider::<Msg>::new(model).with_focus_id("custom_range");

    assert_eq!(slider.focus_id().as_str(), "custom_range");
    assert_eq!(slider.model().lower(), 20.0);
    assert_eq!(slider.model().upper(), 80.0);

    let tag = Widget::<Msg, iced::Theme, HeadlessRenderer>::tag(&slider);
    assert_eq!(tag, tree::Tag::of::<RangeSliderState>());

    let state = Widget::<Msg, iced::Theme, HeadlessRenderer>::state(&slider);
    let downcast = state.downcast_ref::<RangeSliderState>();
    assert_eq!(downcast.hovered_thumb, None);
    assert_eq!(downcast.pressed_thumb, None);
    assert_eq!(downcast.captured_thumb, None);
    assert!(!downcast.is_capturing());
}

#[test]
fn test_mouse_drag_and_commit_settlement() {
    let model = RangeSliderModel::new(0.0, 100.0, 20.0, 80.0);
    let mut harness = Harness::headless(move || {
        range_slider(model.clone())
            .on_change(|live| Some(Msg::Changed(live.into_value())))
            .on_commit(|commit| Some(Msg::Committed(commit.into_value())))
            .on_cancel(|cancel| Some(Msg::Cancelled(cancel.into_restored())))
            .into()
    });

    let bounds = Rectangle::new(
        Point::ORIGIN,
        Size::new(crate::test_helpers::VIEWPORT.width, THUMB_HIT_SIZE),
    );
    let track = track_rect(bounds);

    // Click on Lower thumb (at fraction 0.2)
    let lower_center = Point::new(thumb_center_x(track, 0.2), track.y);
    harness.press(lower_center);

    // Drag lower thumb to 40% (x = thumb_center_x(track, 0.4))
    let target_drag = Point::new(thumb_center_x(track, 0.4), track.y);
    let move_msgs = harness.hover(target_drag);
    assert!(
        move_msgs.iter().any(|m| match m {
            Msg::Changed((l, _)) => (*l - 40.0).abs() < 1.0,
            _ => false,
        }),
        "mouse drag must emit on_change Live tick"
    );

    // Release mouse -> commits settled value
    let release_msgs = harness.release(target_drag);
    assert!(
        release_msgs.iter().any(|m| match m {
            Msg::Committed((l, _)) => (*l - 40.0).abs() < 1.0,
            _ => false,
        }),
        "mouse release must emit on_commit Commit settlement"
    );
}

#[test]
fn test_drag_non_crossing_invariant_and_no_swap() {
    let model = RangeSliderModel::new(0.0, 100.0, 20.0, 50.0).with_min_span(10.0);
    let mut harness = Harness::headless(move || {
        range_slider(model.clone())
            .on_change(|live| Some(Msg::Changed(live.into_value())))
            .on_commit(|commit| Some(Msg::Committed(commit.into_value())))
            .into()
    });

    let bounds = Rectangle::new(
        Point::ORIGIN,
        Size::new(crate::test_helpers::VIEWPORT.width, THUMB_HIT_SIZE),
    );
    let track = track_rect(bounds);

    // Click lower thumb
    let lower_center = Point::new(thumb_center_x(track, 0.2), track.y);
    harness.press(lower_center);

    // Attempt to drag lower thumb to 90% (far past upper at 50%)
    let past_upper = Point::new(thumb_center_x(track, 0.9), track.y);
    let move_msgs = harness.hover(past_upper);

    // Lower thumb must clamp to upper - min_span = 50 - 10 = 40.0; identity never swaps!
    if let Some(Msg::Changed((l, u))) = move_msgs.last() {
        assert!(
            *l <= 40.001,
            "lower thumb {l} must not exceed upper - min_span (40.0)"
        );
        assert_eq!(
            *u, 50.0,
            "upper thumb must remain stationary during lower drag"
        );
    }
}

#[test]
fn test_escape_cancels_active_drag() {
    let model = RangeSliderModel::new(0.0, 100.0, 20.0, 80.0);
    let mut harness = Harness::headless(move || {
        range_slider(model.clone())
            .on_change(|live| Some(Msg::Changed(live.into_value())))
            .on_commit(|commit| Some(Msg::Committed(commit.into_value())))
            .on_cancel(|cancel| Some(Msg::Cancelled(cancel.into_restored())))
            .into()
    });

    let bounds = Rectangle::new(
        Point::ORIGIN,
        Size::new(crate::test_helpers::VIEWPORT.width, THUMB_HIT_SIZE),
    );
    let track = track_rect(bounds);

    // Start drag on lower thumb
    harness.press(Point::new(thumb_center_x(track, 0.2), track.y));
    harness.hover(Point::new(thumb_center_x(track, 0.4), track.y));

    // Press Escape
    let cancel_msgs = harness.pointer(
        &[key_event(Named::Escape, false)],
        mouse::Cursor::Unavailable,
    );
    assert_eq!(
        cancel_msgs,
        vec![Msg::Cancelled((20.0, 80.0))],
        "Escape during drag must emit on_cancel restoring pre-drag snapshot"
    );
}

#[test]
fn test_keyboard_stepping_on_focused_thumb() {
    let model = RangeSliderModel::new(0.0, 100.0, 20.0, 80.0).with_step(Some(1.0));
    let mut harness = Harness::headless(move || {
        range_slider(model.clone())
            .with_focus_id("slider_kb")
            .on_commit(|commit| Some(Msg::Committed(commit.into_value())))
            .into()
    });

    // Focus lower thumb ("slider_kb.lower")
    focus_one(&mut harness, "slider_kb.lower");

    // ArrowRight steps lower thumb +1
    let right_msgs = harness.pointer(
        &[key_event(Named::ArrowRight, false)],
        mouse::Cursor::Unavailable,
    );
    assert_eq!(right_msgs, vec![Msg::Committed((21.0, 80.0))]);

    // ArrowLeft steps lower thumb -1
    let left_msgs = harness.pointer(
        &[key_event(Named::ArrowLeft, false)],
        mouse::Cursor::Unavailable,
    );
    assert_eq!(left_msgs, vec![Msg::Committed((19.0, 80.0))]);

    // Tab moves focus from Lower to Upper thumb
    let tab_msgs = harness.pointer(&[key_event(Named::Tab, false)], mouse::Cursor::Unavailable);
    assert!(
        tab_msgs.is_empty(),
        "Tab within widget moves focus without committing"
    );

    // Stepping now affects Upper thumb: ArrowRight steps upper thumb +1
    let upper_right = harness.pointer(
        &[key_event(Named::ArrowRight, false)],
        mouse::Cursor::Unavailable,
    );
    assert_eq!(upper_right, vec![Msg::Committed((20.0, 81.0))]);

    // Shift+Tab moves focus back from Upper to Lower thumb
    harness.pointer(&[key_event(Named::Tab, true)], mouse::Cursor::Unavailable);

    // Stepping affects Lower thumb again: Home sets Lower to 0.0
    let home_msgs = harness.pointer(&[key_event(Named::Home, false)], mouse::Cursor::Unavailable);
    assert_eq!(home_msgs, vec![Msg::Committed((0.0, 80.0))]);
}

#[test]
fn test_headless_render_across_states() {
    let p = palette();
    for disabled in [false, true] {
        let model = RangeSliderModel::new(0.0, 100.0, 25.0, 75.0)
            .with_disabled(disabled)
            .with_unit_suffix("px");

        let mut harness = Harness::headless(move || {
            range_slider::<Msg>(model.clone())
                .with_focus_id("render_slider")
                .with_palette(p)
                .into()
        });

        let bounds = Rectangle::new(
            Point::ORIGIN,
            Size::new(crate::test_helpers::VIEWPORT.width, THUMB_HIT_SIZE),
        );
        let track = track_rect(bounds);

        // 1. Rest render
        harness.render();

        let lower_pt = Point::new(thumb_center_x(track, 0.25), track.y);
        let upper_pt = Point::new(thumb_center_x(track, 0.75), track.y);

        // 2. Hover lower thumb (triggers readout pill draw)
        harness.hover(lower_pt);
        harness.render();

        // 3. Press lower thumb
        harness.press(lower_pt);
        harness.render();
        harness.release(lower_pt);

        // 4. Hover upper thumb (triggers readout pill draw)
        harness.hover(upper_pt);
        harness.render();

        // 5. Focus test
        if !disabled {
            focus_one(&mut harness, "render_slider.lower");
            harness.render();
            focus_one(&mut harness, "render_slider.upper");
            harness.render();
        }
    }
}
