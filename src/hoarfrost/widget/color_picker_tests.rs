//! Unit tests for `ColorPicker` and `ColorSwatchButton` (§3.12, `UIF-ATOM-012`).

use super::*;
use crate::test_helpers::Harness;

#[test]
fn test_rgba8_constructors_and_accessors() {
    let black = Rgba8::from_rgb(0, 0, 0);
    assert_eq!(black.r, 0);
    assert_eq!(black.g, 0);
    assert_eq!(black.b, 0);
    assert_eq!(black.a, 255);

    let red = Rgba8::from_rgba(255, 0, 0, 128);
    assert_eq!(red.r, 255);
    assert_eq!(red.g, 0);
    assert_eq!(red.b, 0);
    assert_eq!(red.a, 128);

    let custom = Rgba8::new(10, 20, 30, 40);
    assert_eq!(custom.r, 10);
    assert_eq!(custom.g, 20);
    assert_eq!(custom.b, 30);
    assert_eq!(custom.a, 40);

    let default = Rgba8::default();
    assert_eq!(default, Rgba8::new(0, 0, 0, 0));
}

#[test]
fn test_rgba8_hex_parsing_and_formatting() {
    // 6-digit with #
    let parsed1 = Rgba8::from_hex("#FF5722").expect("valid 6-digit hex");
    assert_eq!(parsed1, Rgba8::from_rgb(255, 87, 34));
    assert_eq!(parsed1.to_hex_rgb(), "#FF5722");
    assert_eq!(format!("{parsed1}"), "#FF5722");

    // 6-digit without # and lowercase
    let parsed2 = Rgba8::from_hex("00ff00").expect("valid lowercase hex");
    assert_eq!(parsed2, Rgba8::from_rgb(0, 255, 0));
    assert_eq!(parsed2.to_hex_rgb(), "#00FF00");

    // 8-digit with #
    let parsed3 = Rgba8::from_hex("#336699CC").expect("valid 8-digit hex");
    assert_eq!(parsed3, Rgba8::from_rgba(51, 102, 153, 204));
    assert_eq!(parsed3.to_hex_rgba(), "#336699CC");
    assert_eq!(format!("{parsed3}"), "#336699CC");

    // Whitespace trimming
    let parsed4 = Rgba8::from_hex("  #112233  ").expect("trimmed hex");
    assert_eq!(parsed4, Rgba8::from_rgb(17, 34, 51));

    // Invalid hex strings
    assert!(Rgba8::from_hex("").is_none());
    assert!(Rgba8::from_hex("#12").is_none());
    assert!(Rgba8::from_hex("#12345").is_none());
    assert!(Rgba8::from_hex("#1234567").is_none());
    assert!(Rgba8::from_hex("#GG0000").is_none());
    assert!(Rgba8::from_hex("invalid").is_none());
}

#[test]
fn test_hsva_constructors_and_clamping() {
    let hsv = Hsva::new(400.0, 1.5, -0.5, 2.0);
    assert_eq!(hsv.h, 360.0);
    assert_eq!(hsv.s, 1.0);
    assert_eq!(hsv.v, 0.0);
    assert_eq!(hsv.a, 1.0);

    let hsv2 = Hsva::from_hsv(180.0, 0.5, 0.75);
    assert_eq!(hsv2.h, 180.0);
    assert_eq!(hsv2.s, 0.5);
    assert_eq!(hsv2.v, 0.75);
    assert_eq!(hsv2.a, 1.0);

    let hsv3 = Hsva::from_hsva(270.0, 0.8, 0.9, 0.4);
    assert_eq!(hsv3.h, 270.0);
    assert_eq!(hsv3.s, 0.8);
    assert_eq!(hsv3.v, 0.9);
    assert_eq!(hsv3.a, 0.4);
}

#[test]
fn test_rgba8_hsva_bidirectional_conversions() {
    // Pure Red
    let red_rgb = Rgba8::from_rgb(255, 0, 0);
    let red_hsv = Hsva::from_rgba8(red_rgb);
    assert!((red_hsv.h - 0.0).abs() < 1e-4 || (red_hsv.h - 360.0).abs() < 1e-4);
    assert_eq!(red_hsv.s, 1.0);
    assert_eq!(red_hsv.v, 1.0);
    assert_eq!(red_hsv.to_rgba8(), red_rgb);

    // Pure Green
    let green_rgb = Rgba8::from_rgb(0, 255, 0);
    let green_hsv = Hsva::from_rgba8(green_rgb);
    assert!((green_hsv.h - 120.0).abs() < 1e-4);
    assert_eq!(green_hsv.s, 1.0);
    assert_eq!(green_hsv.v, 1.0);
    assert_eq!(green_hsv.to_rgba8(), green_rgb);

    // Pure Blue
    let blue_rgb = Rgba8::from_rgb(0, 0, 255);
    let blue_hsv = Hsva::from_rgba8(blue_rgb);
    assert!((blue_hsv.h - 240.0).abs() < 1e-4);
    assert_eq!(blue_hsv.s, 1.0);
    assert_eq!(blue_hsv.v, 1.0);
    assert_eq!(blue_hsv.to_rgba8(), blue_rgb);

    // Pure White
    let white_rgb = Rgba8::from_rgb(255, 255, 255);
    let white_hsv = Hsva::from_rgba8(white_rgb);
    assert_eq!(white_hsv.s, 0.0);
    assert_eq!(white_hsv.v, 1.0);
    assert_eq!(white_hsv.to_rgba8(), white_rgb);

    // Pure Black
    let black_rgb = Rgba8::from_rgb(0, 0, 0);
    let black_hsv = Hsva::from_rgba8(black_rgb);
    assert_eq!(black_hsv.s, 0.0);
    assert_eq!(black_hsv.v, 0.0);
    assert_eq!(black_hsv.to_rgba8(), black_rgb);

    // Yellow: #FFFF00
    let yellow_rgb = Rgba8::from_rgb(255, 255, 0);
    let yellow_hsv = Hsva::from_rgba8(yellow_rgb);
    assert!((yellow_hsv.h - 60.0).abs() < 1e-4);
    assert_eq!(yellow_hsv.to_rgba8(), yellow_rgb);

    // Cyan: #00FFFF
    let cyan_rgb = Rgba8::from_rgb(0, 255, 255);
    let cyan_hsv = Hsva::from_rgba8(cyan_rgb);
    assert!((cyan_hsv.h - 180.0).abs() < 1e-4);
    assert_eq!(cyan_hsv.to_rgba8(), cyan_rgb);

    // Magenta: #FF00FF
    let magenta_rgb = Rgba8::from_rgb(255, 0, 255);
    let magenta_hsv = Hsva::from_rgba8(magenta_rgb);
    assert!((magenta_hsv.h - 300.0).abs() < 1e-4);
    assert_eq!(magenta_hsv.to_rgba8(), magenta_rgb);

    // Alpha channel preserved
    let trans = Rgba8::from_rgba(100, 150, 200, 128);
    let trans_hsv = Hsva::from_rgba8(trans);
    assert!((trans_hsv.a - (128.0 / 255.0)).abs() < 1e-3);
    let trans_back = trans_hsv.to_rgba8();
    assert_eq!(trans_back.a, 128);
}

#[test]
fn test_iced_color_conversions() {
    let rgba = Rgba8::from_rgba(255, 128, 64, 200);
    let iced_color: Color = rgba.into();
    assert!((iced_color.r - 1.0).abs() < 1e-4);
    assert!((iced_color.g - (128.0 / 255.0)).abs() < 1e-4);
    assert!((iced_color.b - (64.0 / 255.0)).abs() < 1e-4);
    assert!((iced_color.a - (200.0 / 255.0)).abs() < 1e-4);

    let back: Rgba8 = iced_color.into();
    assert_eq!(back, rgba);

    let hsv = Hsva::from_hsv(180.0, 0.5, 0.8);
    let hsv_color: Color = hsv.into();
    let back_hsv: Hsva = hsv_color.into();
    assert!((back_hsv.h - hsv.h).abs() < 1.0);
    assert!((back_hsv.s - hsv.s).abs() < 0.05);
    assert!((back_hsv.v - hsv.v).abs() < 0.05);
}

#[test]
fn test_color_picker_model_defaults_and_builders() {
    let initial = Rgba8::from_rgb(100, 150, 200);
    let model = ColorPickerModel::new(initial);

    assert_eq!(model.value, initial);
    assert_eq!(model.hex_input, "#6496C8");
    assert!(!model.alpha_enabled);
    assert!(!model.open);
    assert!(!model.disabled);
    assert_eq!(model.preset_swatches.len(), 10);
    assert!(model.contrast_reference.is_none());

    let custom_presets = vec![Rgba8::from_rgb(1, 2, 3), Rgba8::from_rgb(4, 5, 6)];
    let ref_bg = Rgba8::from_rgb(20, 20, 20);

    let model2 = model
        .with_alpha(true)
        .with_open(true)
        .with_disabled(true)
        .with_preset_swatches(custom_presets.clone())
        .with_contrast_reference(ref_bg);

    assert!(model2.alpha_enabled);
    assert_eq!(model2.hex_input, "#6496C8FF");
    assert!(model2.open);
    assert!(model2.disabled);
    assert_eq!(model2.preset_swatches, custom_presets);
    assert_eq!(model2.contrast_reference, Some(ref_bg));
    assert_eq!(model2.pre_open_snapshot, Some(initial));
}

#[test]
fn test_color_picker_model_synchronization() {
    let mut model = ColorPickerModel::new(Rgba8::from_rgb(0, 0, 0));

    // set_rgba
    model.set_rgba(Rgba8::from_rgb(255, 0, 0));
    assert_eq!(model.value, Rgba8::from_rgb(255, 0, 0));
    assert_eq!(model.hex_input, "#FF0000");
    assert!((model.hsv.h - 0.0).abs() < 1e-4 || (model.hsv.h - 360.0).abs() < 1e-4);
    assert_eq!(model.hsv.s, 1.0);
    assert_eq!(model.hsv.v, 1.0);

    // set_hsva
    model.set_hsva(Hsva::from_hsv(120.0, 1.0, 1.0));
    assert_eq!(model.value, Rgba8::from_rgb(0, 255, 0));
    assert_eq!(model.hex_input, "#00FF00");

    // set_hex_input valid
    let ok = model.set_hex_input("#0000FF");
    assert!(ok);
    assert_eq!(model.value, Rgba8::from_rgb(0, 0, 255));
    assert_eq!(model.hex_input, "#0000FF");
    assert!(model.validation_error.is_none());

    // set_hex_input invalid
    let fail = model.set_hex_input("not-a-color");
    assert!(!fail);
    assert_eq!(model.hex_input, "not-a-color");
    assert_eq!(model.value, Rgba8::from_rgb(0, 0, 255)); // Value left untouched
    assert_eq!(model.validation_error, Some(ERR_INVALID_HEX.to_string()));
}

#[test]
fn test_color_picker_snapshot_and_rollback() {
    let original = Rgba8::from_rgb(255, 128, 0);
    let mut model = ColorPickerModel::new(original);

    // Open creates snapshot
    model.open_flyout();
    assert!(model.open);
    assert_eq!(model.pre_open_snapshot, Some(original));

    // Mutate color while open
    model.set_rgba(Rgba8::from_rgb(0, 255, 255));
    assert_eq!(model.value, Rgba8::from_rgb(0, 255, 255));

    // Cancel rolls back to snapshot
    let reverted = model.cancel();
    assert_eq!(reverted, original);
    assert_eq!(model.value, original);
    assert_eq!(model.hex_input, "#FF8000");
    assert!(!model.open);
    assert!(model.pre_open_snapshot.is_none());

    // Open again, mutate, then commit
    model.open_flyout();
    let new_color = Rgba8::from_rgb(128, 0, 128);
    model.set_rgba(new_color);
    let committed = model.commit();
    assert_eq!(committed, new_color);
    assert_eq!(model.value, new_color);
    assert!(!model.open);
    assert!(model.pre_open_snapshot.is_none());
}

#[test]
fn test_geometry_constants_and_checkerboard_invariants() {
    // Surface geometry constraints (§3.12, UIF-ATOM-012, UIF-PRIN-001 §1.1)
    assert_eq!(COLOR_FLYOUT_WIDTH, 240.0);
    assert_eq!(COLOR_WELL_WIDTH, 180.0);
    assert_eq!(COLOR_WELL_HEIGHT, 140.0);

    // Token-bound invariants (§3.12, UIF-TOK-002, UIF-TOK-006)
    assert_eq!(TRIGGER_HEIGHT_SM, size::CONTROL_SM);
    assert_eq!(TRIGGER_HEIGHT_MD, size::CONTROL_MD);
    assert_eq!(PREVIEW_WELL_SIZE, size::SLIDER_THUMB);
    assert_eq!(CHECKERBOARD_CELL_SIZE, space::XS); // 4px
    assert_eq!(SWATCH_SIZE, size::HIT_TARGET_MIN); // 24px
    assert_eq!(SLIDER_TRACK_HEIGHT, size::SLIDER_TRACK_HEIGHT); // 4px
    assert_eq!(THUMB_SIZE, size::SLIDER_THUMB); // 16px
    assert_eq!(CARD_PADDING, space::MD); // 12px
    assert_eq!(CARD_RADIUS, shape::RADIUS_CARD); // 6px
    assert_eq!(INNER_BORDER_WIDTH, shape::BORDER_HAIRLINE); // 1px
    assert_eq!(DUAL_RING_OUTER_WIDTH, shape::BORDER_FOCUS); // 2px
    assert_eq!(DUAL_RING_INNER_WIDTH, shape::BORDER_HAIRLINE); // 1px
    assert_eq!(SWATCH_GAP, space::XS); // 4px
    assert_eq!(SECTION_GAP, space::SM); // 8px
    assert_eq!(CONTRAST_CHIP_HEIGHT, size::BADGE_CHIP_HEIGHT); // 24px
}

#[test]
fn test_color_flyout_layout_calculation() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0));
    let layout = ColorFlyoutLayout::compute(Point::ORIGIN, &model);

    assert_eq!(layout.card_bounds.width, COLOR_FLYOUT_WIDTH);
    assert_eq!(layout.sv_well_bounds.width, COLOR_WELL_WIDTH);
    assert_eq!(layout.sv_well_bounds.height, COLOR_WELL_HEIGHT);
    assert_eq!(layout.hue_track_bounds.width, COLOR_WELL_WIDTH);
    assert_eq!(layout.hue_track_bounds.height, SLIDER_TRACK_HEIGHT);
    assert!(layout.alpha_track_bounds.is_none());
    assert_eq!(layout.swatches.len(), 10);
    assert!(layout.contrast_bounds.is_none());

    // With alpha enabled
    let model_alpha = model.clone().with_alpha(true);
    let layout_alpha = ColorFlyoutLayout::compute(Point::ORIGIN, &model_alpha);
    assert!(layout_alpha.alpha_track_bounds.is_some());
    assert!(layout_alpha.card_bounds.height > layout.card_bounds.height);

    // With contrast reference enabled
    let model_contrast = model
        .clone()
        .with_contrast_reference(Rgba8::from_rgb(0, 0, 0));
    let layout_contrast = ColorFlyoutLayout::compute(Point::ORIGIN, &model_contrast);
    assert!(layout_contrast.contrast_bounds.is_some());
    assert!(layout_contrast.card_bounds.height > layout.card_bounds.height);
}

#[test]
fn test_live_contrast_readout_with_reference() {
    let white = Rgba8::from_rgb(255, 255, 255);
    let black = Rgba8::from_rgb(0, 0, 0);

    // White text on black background: 21:1 -> AA & AAA pass
    let ratio1 = contrast_ratio(Color::from(white), Color::from(black));
    assert!(ratio1 >= 7.0);

    // Dark grey on black: < 4.5 -> Fail
    let dark_grey = Rgba8::from_rgb(40, 40, 40);
    let ratio2 = contrast_ratio(Color::from(dark_grey), Color::from(black));
    assert!(ratio2 < 4.5);
}

#[test]
fn test_disabled_gating() {
    let mut model = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0)).with_disabled(true);
    assert!(model.disabled);

    // open_flyout is refused when disabled
    model.open_flyout();
    assert!(!model.open);
    assert!(model.pre_open_snapshot.is_none());
}

#[test]
fn test_headless_render_color_picker_across_states() {
    let sample = Rgba8::from_rgb(255, 87, 34);

    // 1. Closed state
    let model_closed = ColorPickerModel::new(sample);
    let mut h1 =
        Harness::<(), _>::headless(move || color_picker::<()>(model_closed.clone()).into());
    h1.render();

    // 2. Open state with alpha
    let model_open_alpha = ColorPickerModel::new(sample)
        .with_alpha(true)
        .with_open(true);
    let mut h2 =
        Harness::<(), _>::headless(move || color_picker::<()>(model_open_alpha.clone()).into());
    h2.render();

    // 3. Open state with contrast reference (passing)
    let model_contrast_pass = ColorPickerModel::new(Rgba8::from_rgb(255, 255, 255))
        .with_contrast_reference(Rgba8::from_rgb(0, 0, 0))
        .with_open(true);
    let mut h3 =
        Harness::<(), _>::headless(move || color_picker::<()>(model_contrast_pass.clone()).into());
    h3.render();

    // 4. Open state with contrast reference (failing)
    let model_contrast_fail = ColorPickerModel::new(Rgba8::from_rgb(30, 30, 30))
        .with_contrast_reference(Rgba8::from_rgb(0, 0, 0))
        .with_open(true);
    let mut h4 =
        Harness::<(), _>::headless(move || color_picker::<()>(model_contrast_fail.clone()).into());
    h4.render();

    // 5. Disabled state
    let model_disabled = ColorPickerModel::new(sample).with_disabled(true);
    let mut h5 =
        Harness::<(), _>::headless(move || color_picker::<()>(model_disabled.clone()).into());
    h5.render();

    // 6. Invalid hex input state
    let mut model_invalid = ColorPickerModel::new(sample).with_open(true);
    model_invalid.set_hex_input("bad-hex");
    let mut h6 =
        Harness::<(), _>::headless(move || color_picker::<()>(model_invalid.clone()).into());
    h6.render();
}

#[test]
fn test_headless_render_color_swatch_button_across_states() {
    let sample = Rgba8::from_rgb(59, 130, 246);
    let model = ColorPickerModel::new(sample);

    // 1. Standard 32px
    let m1 = model.clone();
    let mut h1 = Harness::<(), _>::headless(move || color_swatch_button::<()>(&m1).into());
    h1.render();

    // 2. Compact 28px
    let m2 = model.clone();
    let mut h2 = Harness::<(), _>::headless(move || {
        color_swatch_button::<()>(&m2)
            .with_height(TRIGGER_HEIGHT_SM)
            .into()
    });
    h2.render();

    // 3. Disabled
    let m3 = model.clone().with_disabled(true);
    let mut h3 = Harness::<(), _>::headless(move || color_swatch_button::<()>(&m3).into());
    h3.render();
}

fn key_press(name: iced::keyboard::key::Named) -> iced::Event {
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Named(name),
        modified_key: iced::keyboard::Key::Named(name),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

fn char_press(c: &'static str) -> iced::Event {
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Character(c.into()),
        modified_key: iced::keyboard::Key::Character(c.into()),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::NONE,
        text: Some(c.into()),
        repeat: false,
    })
}

#[test]
fn test_hex_input_focus_and_typing() {
    let initial = Rgba8::from_rgb(0, 0, 0);
    let model = ColorPickerModel::new(initial).with_open(true);
    let flyout_origin = Point::new(0.0, TRIGGER_HEIGHT_MD + space::XS);
    let flyout_layout = ColorFlyoutLayout::compute(flyout_origin, &model);
    let hex_center = flyout_layout.hex_input_bounds.center();

    let mut harness = Harness::headless(move || {
        ColorPicker::new(model.clone())
            .on_change(|live| live.into_value())
            .into()
    });

    // 1. Click on hex input bounds to focus it
    harness.press(hex_center);
    harness.release(hex_center);

    // 2. Clear out existing hex string with backspaces
    for _ in 0..7 {
        harness.pointer(
            &[key_press(iced::keyboard::key::Named::Backspace)],
            iced::mouse::Cursor::Unavailable,
        );
    }

    // 3. Type new valid hex "#FFFFFF"
    for ch in ["#", "F", "F", "F", "F", "F"] {
        harness.pointer(&[char_press(ch)], iced::mouse::Cursor::Unavailable);
    }
    let msgs = harness.pointer(&[char_press("F")], iced::mouse::Cursor::Unavailable);

    assert!(
        msgs.iter().any(|&c| c == Rgba8::from_rgb(255, 255, 255)),
        "typing #FFFFFF into focused hex input must emit Rgba8 white, got: {msgs:?}"
    );
}
