//! Unit tests for `ColorPicker` and `ColorSwatchButton` (§3.12, `UIF-ATOM-012`).
//!
//! # Why the cases are grouped
//!
//! Every headless harness pays roughly 620 ms to build iced's renderer and
//! another ~100 ms to be spawned as its own test process, so the file used to
//! spend most of CI on scaffolding rather than on assertions. Cases that
//! observe the same fixture are therefore grouped into one `#[test]`, each case
//! labelled in its own failure message, and the read-only fixtures share a
//! single harness through [`Harness::set_view`].
//!
//! The grouping stops where widget state begins. `ColorPicker` and
//! `ColorSwatchButton` keep their interaction state in the iced `Tree`
//! (`ColorPickerState`, `ColorSwatchButtonState`), and `set_view` swaps the
//! element *underneath* that state — the harness cache carries it forward. So a
//! case that presses, drags, types or focuses still owns its own harness: two of
//! them in one tree would be one test whose second half only passes because of
//! what the first half left behind. The only harnesses shared across cases are
//! the ones that provably mutate nothing: the render sweeps, the Tab-stop probe
//! and the disabled controls, whose `update` returns before it reads the tree
//! and whose `operate` unfocuses instead of publishing.

use super::*;
use crate::test_helpers::{Harness, moved};

// ---------------------------------------------------------------------------
// Shared fixtures and helpers
// ---------------------------------------------------------------------------

/// A colour no default of the model can produce, so a test can tell "the widget
/// published this" apart from "the widget echoed what it was handed".
const DISTINCT: Rgba8 = Rgba8::new(17, 234, 51, 255);

/// Every emission either widget can make, so one harness can tell the tiers apart.
#[derive(Debug, Clone, PartialEq)]
enum PickerMsg {
    /// `on_change` — a live tick.
    Changed(Rgba8),
    /// `on_commit` — the interaction settled.
    Committed(Rgba8),
    /// `on_cancel` — the edit rolled back.
    Cancelled(Rgba8),
    /// `on_open_change` — the flyout opened or closed.
    Opened(bool),
    /// The [`ColorSwatchButton`]'s single `on_press` emission.
    Pressed,
}

/// A picker wired for all four tiers.
fn picker(model: ColorPickerModel) -> ColorPicker<'static, PickerMsg> {
    ColorPicker::new(model)
        .on_change(|live| PickerMsg::Changed(live.into_value()))
        .on_commit(|commit| PickerMsg::Committed(commit.into_value()))
        .on_cancel(|cancel| PickerMsg::Cancelled(cancel.into_restored()))
        .on_open_change(|live| PickerMsg::Opened(*live.value()))
}

/// The flyout geometry at the position the widget actually hit-tests it at.
fn flyout_at(model: &ColorPickerModel) -> ColorFlyoutLayout {
    ColorFlyoutLayout::compute(Point::new(0.0, TRIGGER_HEIGHT_MD + space::XS), model)
}

/// The largest per-channel difference between two colours.
fn channel_delta(a: Rgba8, b: Rgba8) -> i16 {
    [
        (i16::from(a.r) - i16::from(b.r)).abs(),
        (i16::from(a.g) - i16::from(b.g)).abs(),
        (i16::from(a.b) - i16::from(b.b)).abs(),
        (i16::from(a.a) - i16::from(b.a)).abs(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
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

/// Deliver one named key with the pointer nowhere near the control.
fn press_key<Message: Send + 'static, Renderer: iced::advanced::renderer::Renderer>(
    harness: &mut Harness<'_, Message, Renderer>,
    key: iced::keyboard::key::Named,
) -> Vec<Message> {
    harness.pointer(&[key_press(key)], iced::mouse::Cursor::Unavailable)
}

/// Deliver one typed character with the pointer nowhere near the control.
fn type_char<Message: Send + 'static, Renderer: iced::advanced::renderer::Renderer>(
    harness: &mut Harness<'_, Message, Renderer>,
    ch: &'static str,
) -> Vec<Message> {
    harness.pointer(&[char_press(ch)], iced::mouse::Cursor::Unavailable)
}

/// The trigger's centre, in harness coordinates.
fn trigger_centre() -> Point {
    Point::new(COLOR_FLYOUT_WIDTH / 2.0, TRIGGER_HEIGHT_MD / 2.0)
}

/// A model mid-edit: the flyout is open and the value has moved off the snapshot.
fn mid_edit(original: Rgba8, edited: Rgba8) -> ColorPickerModel {
    let mut model = ColorPickerModel::new(original).with_open(true);
    model.set_rgba(edited);
    model
}

// ---------------------------------------------------------------------------
// Colour value types: construction
// ---------------------------------------------------------------------------

#[test]
fn test_rgba8_and_hsva_constructors_and_defaults() {
    // --- Rgba8: the three constructors and their accessors ---
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

    // --- Hsva: construction clamps every channel into its own range ---
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

// ---------------------------------------------------------------------------
// Colour value types: hex parsing and formatting
// ---------------------------------------------------------------------------

#[test]
fn test_hex_parsing_and_formatting_edges() {
    // --- the accepted shapes ---
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

    // --- every malformed shape is rejected, not panicked ---
    let rejected: [(&str, &str); 13] = [
        ("empty", ""),
        ("whitespace only", "   "),
        ("bare prefix", "#"),
        ("one digit", "#1"),
        ("two digits", "#12"),
        ("three digits", "#123"),
        ("four digits", "#1234"),
        ("five digits", "#12345"),
        ("seven digits", "#1234567"),
        ("nine digits", "#123456789"),
        ("non-hex digit", "#12g456"),
        ("interior whitespace", "#12 456"),
        ("a word", "invalid"),
    ];

    for (label, raw) in rejected {
        assert!(
            Rgba8::from_hex(raw).is_none(),
            "{label} ({raw:?}) must not parse"
        );
    }

    // --- the non-ASCII siblings, all rejected rather than panicked on ---
    //
    // `from_hex` now validates the whole run as ASCII hex digits before it
    // slices by byte index, so a non-ASCII string is a bad colour whatever its
    // byte length: `aé` is three bytes (no arm), `ééé` is six bytes and every
    // boundary lands on a char edge, and `aébcd` is six bytes that cut through
    // `é` — the case that used to panic, pinned by
    // `defect_02_from_hex_rejects_a_non_ascii_string_of_a_valid_byte_length`.
    assert!(Rgba8::from_hex("aé").is_none());
    assert!(Rgba8::from_hex("ééé").is_none());

    // --- formatting round-trips back through the parser ---
    let samples = [
        Rgba8::from_rgb(0, 0, 0),
        Rgba8::from_rgb(255, 255, 255),
        Rgba8::from_rgb(59, 130, 246),
        Rgba8::from_rgba(17, 34, 51, 0),
        Rgba8::from_rgba(200, 100, 50, 128),
    ];

    for colour in samples {
        let rgb_hex = colour.to_hex_rgb();
        let rgba_hex = colour.to_hex_rgba();

        // The 6-digit form drops alpha, so it parses back fully opaque.
        assert_eq!(
            Rgba8::from_hex(&rgb_hex),
            Some(Rgba8::new(colour.r, colour.g, colour.b, 255)),
            "{rgb_hex} must parse back to the same channels"
        );
        assert_eq!(
            Rgba8::from_hex(&rgba_hex),
            Some(colour),
            "{rgba_hex} must parse back losslessly"
        );
        assert_eq!(
            Rgba8::from_hex(&rgba_hex.to_lowercase()),
            Some(colour),
            "hex parsing is case insensitive"
        );

        // `Display` picks the compact form only when the alpha is a no-op.
        let expected = if colour.a == 255 { &rgb_hex } else { &rgba_hex };
        assert_eq!(colour.to_string(), *expected);
    }
}

/// **A defect fixed, and the test flipped to assert the correct behaviour.**
///
/// `Rgba8::from_hex` used to decide which shape it had by `str::len()` — a
/// **byte** count — and then slice `&hex[0..2]`, `[2..4]`, `[4..6]` by **byte
/// index**. Those two only agree while every character is one byte long.
///
/// `"aébcd"` is six bytes: `a` (1) + `é` (2) + `b` `c` `d` (3). It passed the
/// `len() == 6` arm, and `&hex[0..2]` then cut through the middle of `é`, so
/// `str` slicing panicked rather than returning `None`. The panic was
/// reachable from the public API — `ColorPickerModel::set_hex_input` calls
/// `from_hex` on whatever the hex field holds — so a hostile paste was enough
/// to take the process down.
///
/// `from_hex` now validates the whole run as ASCII hex digits before it slices,
/// so the answer to a string that is not one is `None`.
///
/// Kept as its own function rather than folded into the table below because it
/// is the case that used to abort its own process; the history reads better
/// next to the input that caused it.
#[test]
fn defect_02_from_hex_rejects_a_non_ascii_string_of_a_valid_byte_length() {
    assert_eq!(
        Rgba8::from_hex("aébcd"),
        None,
        "defect 02: six bytes of non-ASCII must be rejected, not sliced through"
    );
}

/// Every shape `from_hex` must answer with `None` — the byte-length traps that
/// used to slice through a character, the `+` sign `u8::from_str_radix` would
/// otherwise swallow, and the ordinary malformed shapes.
#[test]
fn defect_02_from_hex_answers_none_for_every_shape_it_cannot_parse() {
    const REJECTED: &[(&str, &str)] = &[
        ("non-ASCII across a byte boundary, 6 bytes", "aébcd"),
        ("non-ASCII across a byte boundary, 8 bytes", "aébcdef"),
        ("three-byte character inside 6 bytes", "世abc"),
        ("non-ASCII on aligned boundaries, 6 bytes", "ééé"),
        ("non-ASCII inside a prefixed 8", "#aébcdef"),
        ("leading plus, 6 bytes", "+A2B3C"),
        ("plus after the prefix", "#+A2B3C"),
        ("leading minus", "-A2B3C"),
        ("interior whitespace", "12 456"),
        ("interior whitespace with prefix", "#12 456"),
        ("empty", ""),
        ("bare prefix", "#"),
        ("one digit", "1"),
        ("five digits", "12345"),
        ("seven digits", "1234567"),
        ("nine digits", "123456789"),
        ("a word", "invalid"),
        ("double prefix", "##12345"),
    ];

    for (label, raw) in REJECTED {
        assert!(
            Rgba8::from_hex(raw).is_none(),
            "defect 02: {label} ({raw:?}) must not parse"
        );
    }
}

/// The shapes `from_hex` must still accept, so the new guard did not narrow the
/// language the colour field actually speaks.
#[test]
fn defect_02_from_hex_still_accepts_every_valid_hex_shape() {
    let upper = Rgba8::from_hex("#FF5722").expect("defect 02: uppercase with # must parse");
    assert_eq!(upper, Rgba8::from_rgb(255, 87, 34), "defect 02: #RRGGBB");

    let lower = Rgba8::from_hex("ff5722").expect("defect 02: lowercase without # must parse");
    assert_eq!(lower, Rgba8::from_rgb(255, 87, 34), "defect 02: RRGGBB");

    let mixed = Rgba8::from_hex("Ff5722").expect("defect 02: mixed case must parse");
    assert_eq!(mixed, Rgba8::from_rgb(255, 87, 34), "defect 02: mixed case");

    let padded =
        Rgba8::from_hex("  #FF5722  ").expect("defect 02: surrounding whitespace is trimmed");
    assert_eq!(padded, Rgba8::from_rgb(255, 87, 34), "defect 02: trimmed");

    let rgba = Rgba8::from_hex("#336699CC").expect("defect 02: #RRGGBBAA must parse");
    assert_eq!(
        rgba,
        Rgba8::from_rgba(51, 102, 153, 204),
        "defect 02: #RRGGBBAA"
    );

    let rgba_lower = Rgba8::from_hex("336699cc").expect("defect 02: RRGGBBAA must parse");
    assert_eq!(
        rgba_lower,
        Rgba8::from_rgba(51, 102, 153, 204),
        "defect 02: lowercase RRGGBBAA"
    );

    let black = Rgba8::from_hex("000000").expect("defect 02: all-zero must parse");
    assert_eq!(black, Rgba8::from_rgb(0, 0, 0), "defect 02: 000000");

    let white = Rgba8::from_hex("#FFFFFF").expect("defect 02: all-F must parse");
    assert_eq!(white, Rgba8::from_rgb(255, 255, 255), "defect 02: FFFFFF");
}

/// The reachable path to the panic: the hex field feeds `from_hex` whatever the
/// user pasted, so a rejected paste must be reported as a validation error and
/// leave the colour alone — not unwind the frame.
#[test]
fn defect_02_the_hex_field_reports_a_non_ascii_paste_instead_of_panicking() {
    let mut model = ColorPickerModel::new(Rgba8::from_rgb(1, 2, 3));
    let before = model.value;

    assert!(
        !model.set_hex_input("aébcd"),
        "defect 02: a non-ASCII paste must be reported as invalid, not panic"
    );
    assert_eq!(
        model.value, before,
        "defect 02: a rejected paste leaves the colour alone"
    );
    assert_eq!(
        model.hex_input, "aébcd",
        "defect 02: the field keeps what the user typed so they can correct it"
    );
    assert_eq!(
        model.validation_error.as_deref(),
        Some(ERR_INVALID_HEX),
        "defect 02: the rejection is reported through the model's own error channel"
    );

    // And a valid correction after the rejected paste still works.
    assert!(
        model.set_hex_input("#0A0B0C"),
        "defect 02: a valid paste after a rejected one still parses"
    );
    assert_eq!(
        model.value,
        Rgba8::from_rgb(10, 11, 12),
        "defect 02: the correction lands"
    );
    assert!(
        model.validation_error.is_none(),
        "defect 02: a good parse clears the error"
    );
}

// ---------------------------------------------------------------------------
// Colour value types: HSV <-> RGB conversion
// ---------------------------------------------------------------------------

#[test]
fn test_hsv_rgb_conversions_round_trip_clamp_and_wrap() {
    // --- the pure colours convert and come back unchanged ---
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

    // --- an achromatic colour has no hue and no saturation to report ---
    for grey in [
        Rgba8::from_rgb(0, 0, 0),
        Rgba8::from_rgb(128, 128, 128),
        Rgba8::from_rgb(255, 255, 255),
    ] {
        let hsv = Hsva::from_rgba8(grey);
        assert_eq!(hsv.h, 0.0, "{grey:?} has no hue to report");
        assert_eq!(hsv.s, 0.0, "{grey:?} has no saturation to report");
        assert_eq!(hsv.to_rgba8(), grey, "grey must survive the HSV round trip");
    }

    // --- hues outside the circle reduce before they are used ---
    let equivalences: [(f32, f32); 5] = [
        (400.0, 40.0),
        (-90.0, 270.0),
        (360.0, 0.0),
        (720.0, 0.0),
        (-360.0, 0.0),
    ];

    for (raw, equivalent) in equivalences {
        let full = Hsva {
            h: raw,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        };
        let reference = Hsva {
            h: equivalent,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        };
        assert_eq!(
            full.to_rgba8(),
            reference.to_rgba8(),
            "hue {raw} must reduce to hue {equivalent}"
        );
    }

    // --- and the other three channels clamp instead of wrapping ---
    let cases: [(&str, Hsva, Rgba8); 6] = [
        (
            "saturation above one",
            Hsva {
                h: 0.0,
                s: 2.0,
                v: 1.0,
                a: 1.0,
            },
            Rgba8::from_rgb(255, 0, 0),
        ),
        (
            "saturation below zero is grey",
            Hsva {
                h: 0.0,
                s: -1.0,
                v: 1.0,
                a: 1.0,
            },
            Rgba8::from_rgb(255, 255, 255),
        ),
        (
            "value below zero is black",
            Hsva {
                h: 120.0,
                s: 1.0,
                v: -2.0,
                a: 1.0,
            },
            Rgba8::from_rgb(0, 0, 0),
        ),
        (
            "value above one clamps to full brightness",
            Hsva {
                h: 120.0,
                s: 1.0,
                v: 5.0,
                a: 1.0,
            },
            Rgba8::from_rgb(0, 255, 0),
        ),
        (
            "alpha above one is opaque",
            Hsva {
                h: 0.0,
                s: 1.0,
                v: 1.0,
                a: 2.0,
            },
            Rgba8::from_rgba(255, 0, 0, 255),
        ),
        (
            "alpha below zero is invisible",
            Hsva {
                h: 0.0,
                s: 1.0,
                v: 1.0,
                a: -2.0,
            },
            Rgba8::from_rgba(255, 0, 0, 0),
        ),
    ];

    for (label, hsva, expected) in cases {
        assert_eq!(hsva.to_rgba8(), expected, "{label} is not clamped");
    }

    // --- the round trip is lossy, but only by an eighth of a step ---
    let samples = [
        Rgba8::from_rgb(255, 0, 0),
        Rgba8::from_rgb(0, 255, 0),
        Rgba8::from_rgb(0, 0, 255),
        Rgba8::from_rgb(249, 115, 22),
        Rgba8::from_rgb(234, 179, 8),
        Rgba8::from_rgb(34, 197, 94),
        Rgba8::from_rgb(6, 182, 212),
        Rgba8::from_rgb(59, 130, 246),
        Rgba8::from_rgb(168, 85, 247),
        Rgba8::from_rgb(236, 72, 153),
        Rgba8::from_rgb(128, 128, 128),
        Rgba8::from_rgba(17, 34, 51, 128),
    ];

    for colour in samples {
        let back = Hsva::from_rgba8(colour).to_rgba8();
        assert!(
            channel_delta(colour, back) <= 1,
            "{colour:?} came back as {back:?}: the round trip lost more than one step"
        );
    }
}

// ---------------------------------------------------------------------------
// Colour value types: the `iced::Color` boundary
// ---------------------------------------------------------------------------

#[test]
fn test_colour_conversions_across_the_iced_boundary() {
    // --- the round trip through `iced::Color` ---
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

    // --- `Rgba8::from(Color)` clamps, rounds and zeroes NaN ---
    // The three out-of-range cases are built with a struct literal, not with
    // `Color::from_rgb`: iced's own constructor `debug_assert!`s every channel
    // into `[0, 1]` (iced_core `color.rs`), so the builder would panic on the
    // very inputs this test exists to feed through the clamp. The literal is
    // how a host that never went through `from_rgb` — a deserialised theme, a
    // `Color` assembled by hand — hands the value to us anyway.
    let cases: [(&str, Color, Rgba8); 7] = [
        (
            "all zero",
            Color::from_rgb(0.0, 0.0, 0.0),
            Rgba8::new(0, 0, 0, 255),
        ),
        (
            "all one",
            Color::from_rgb(1.0, 1.0, 1.0),
            Rgba8::new(255, 255, 255, 255),
        ),
        (
            "below zero clamps up",
            Color {
                r: -1.0,
                g: -0.5,
                b: -0.001,
                a: 1.0,
            },
            Rgba8::new(0, 0, 0, 255),
        ),
        (
            "above one clamps down",
            Color {
                r: 2.0,
                g: 1.5,
                b: 10.0,
                a: 1.0,
            },
            Rgba8::new(255, 255, 255, 255),
        ),
        (
            "half steps round away from zero",
            Color::from_rgb(0.5, 0.25, 0.75),
            Rgba8::from_rgb(128, 64, 191),
        ),
        (
            "nan becomes zero",
            Color {
                r: f32::NAN,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            Rgba8::new(0, 255, 0, 255),
        ),
        (
            "eight-bit channels survive",
            Color::from_rgb8(10, 20, 30),
            Rgba8::new(10, 20, 30, 255),
        ),
    ];

    for (label, colour, expected) in cases {
        assert_eq!(
            Rgba8::from(colour),
            expected,
            "{label}: `Rgba8::from(Color)` disagrees"
        );
    }

    // The alpha channel is converted on exactly the same rule.
    let translucent = Color::from_rgba(0.0, 1.0, 0.0, 128.0 / 255.0);
    assert_eq!(Rgba8::from(translucent), Rgba8::new(0, 255, 0, 128));

    // --- and the same boundary feeds the live contrast readout ---
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

// ---------------------------------------------------------------------------
// Model: defaults, builders and state transitions
// ---------------------------------------------------------------------------

#[test]
fn test_model_defaults_and_builders() {
    // --- `new` plus the five builders ---
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

    // --- `Default` is opaque black with the standard presets ---
    let default = ColorPickerModel::default();

    assert_eq!(default, ColorPickerModel::new(Rgba8::from_rgb(0, 0, 0)));
    assert_eq!(default.value, Rgba8::from_rgb(0, 0, 0));
    assert_eq!(
        default.value.a, 255,
        "the model's default is opaque black, unlike `Rgba8::default()`"
    );
    assert_eq!(default.hex_input, "#000000");
    assert!(!default.alpha_enabled);
    assert!(!default.open);
    assert!(!default.disabled);
    assert!(default.pre_open_snapshot.is_none());
    assert!(default.validation_error.is_none());
    assert_eq!(default.preset_swatches, default_preset_swatches());

    // --- and `with_open(false)` is construction, not a close ---
    let model = ColorPickerModel::new(DISTINCT).with_open(false);
    assert!(!model.open);
    assert!(
        model.pre_open_snapshot.is_none(),
        "closing at construction time must not leave a rollback point behind"
    );
}

#[test]
fn test_model_state_transitions_and_rollback() {
    // --- every setter keeps `value`, `hsv` and `hex_input` in step ---
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

    // --- open snapshots, cancel rolls back, commit keeps ---
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

    // --- closing is not cancelling ---
    let mut model = ColorPickerModel::new(Rgba8::from_rgb(10, 20, 30)).with_open(true);
    model.set_rgba(DISTINCT);

    model.close_flyout();

    assert!(!model.open);
    assert!(model.pre_open_snapshot.is_none());
    assert_eq!(
        model.value, DISTINCT,
        "closing is not cancelling: the colour stays where the user left it"
    );

    // --- and a cancel with nothing to roll back to is a no-op, whether the
    // flyout was never opened or was opened without a snapshot ---
    let mut model = ColorPickerModel::new(DISTINCT);
    assert!(model.pre_open_snapshot.is_none());
    assert_eq!(model.cancel(), DISTINCT);
    assert_eq!(model.value, DISTINCT);
    assert!(!model.open);

    // A flyout that was opened without a snapshot has nothing to roll back to.
    let mut forced = ColorPickerModel::new(DISTINCT);
    forced.open = true;
    assert_eq!(forced.cancel(), DISTINCT);
    assert!(!forced.open);
    assert!(forced.pre_open_snapshot.is_none());

    // --- a disabled model refuses to open at all ---
    let mut model = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0)).with_disabled(true);
    assert!(model.disabled);

    // open_flyout is refused when disabled
    model.open_flyout();
    assert!(!model.open);
    assert!(model.pre_open_snapshot.is_none());

    // --- the widget's own accessors reach the very model they expose ---
    let mut picker: ColorPicker<PickerMsg> =
        ColorPicker::new(ColorPickerModel::new(Rgba8::from_rgb(1, 2, 3)))
            .with_trigger_height(TRIGGER_HEIGHT_SM);

    assert_eq!(picker.model().value, Rgba8::from_rgb(1, 2, 3));

    // `model_mut()` hands back the very model the read-only borrow exposes,
    // so a write through it is visible through `model()`.
    picker.model_mut().set_rgba(Rgba8::from_rgb(4, 5, 6));
    assert_eq!(picker.model().value, Rgba8::from_rgb(4, 5, 6));

    // Opening snapshots the value the card was opened with, and `cancel()`
    // walks back to it — both through the same borrowed model.
    picker.model_mut().open_flyout();
    picker.model_mut().set_rgba(Rgba8::from_rgb(7, 8, 9));
    assert_eq!(picker.model().value, Rgba8::from_rgb(7, 8, 9));
    assert_eq!(picker.model_mut().cancel(), Rgba8::from_rgb(4, 5, 6));
    assert_eq!(picker.model().value, Rgba8::from_rgb(4, 5, 6));
}

#[test]
fn test_hex_field_follows_the_alpha_mode_and_its_own_validation() {
    // --- `Display` for the model follows the alpha mode ---
    let colour = Rgba8::new(0x11, 0x22, 0x33, 0x44);

    assert_eq!(
        ColorPickerModel::new(colour).format_display(),
        "#112233",
        "an opaque model shows six digits"
    );
    assert_eq!(
        ColorPickerModel::new(colour)
            .with_alpha(true)
            .format_display(),
        "#11223344",
        "an alpha model shows eight"
    );

    // --- typed hex keeps its alpha only when the field can express it ---
    let mut alpha = ColorPickerModel::new(Rgba8::from_rgb(0, 0, 0)).with_alpha(true);
    assert!(alpha.set_hex_input("#11223344"));
    assert_eq!(alpha.value, Rgba8::new(0x11, 0x22, 0x33, 0x44));
    assert_eq!(alpha.hex_input, "#11223344");

    let mut opaque = ColorPickerModel::new(Rgba8::from_rgb(0, 0, 0));
    assert!(opaque.set_hex_input("#11223344"));
    assert_eq!(
        opaque.value,
        Rgba8::from_rgb(0x11, 0x22, 0x33),
        "a six-digit model forces whatever alpha it parsed to opaque"
    );
    assert_eq!(opaque.value.a, 255);

    // --- and the same rule holds when the alpha arrives through the SV well ---
    let mut alpha = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0)).with_alpha(true);
    alpha.set_hsva(Hsva::from_hsva(0.0, 1.0, 1.0, 0.5));
    assert_eq!(alpha.value, Rgba8::new(255, 0, 0, 128));
    assert_eq!(alpha.hex_input, "#FF000080");

    let mut opaque = ColorPickerModel::new(Rgba8::from_rgb(0, 0, 0));
    opaque.set_hsva(Hsva::from_hsva(0.0, 1.0, 1.0, 0.5));
    assert_eq!(
        (opaque.value.r, opaque.value.g, opaque.value.b),
        (255, 0, 0)
    );
    assert_eq!(
        opaque.hex_input, "#FF0000",
        "the six-digit field cannot express the alpha the model was handed"
    );

    // --- a rejected edit clears only itself ---
    let mut model = ColorPickerModel::new(Rgba8::from_rgb(0, 0, 255));
    assert!(model.set_hex_input("#00FF00"));
    assert!(!model.set_hex_input("#12345"));
    assert_eq!(
        model.value,
        Rgba8::from_rgb(0, 255, 0),
        "a rejected edit must not move the committed colour"
    );
    assert_eq!(
        model.hex_input, "#12345",
        "the field keeps what the user typed"
    );
    assert_eq!(model.validation_error, Some(ERR_INVALID_HEX.to_string()));

    assert!(model.set_hex_input("#0000FF"));
    assert_eq!(
        model.validation_error, None,
        "a valid edit must clear the error the invalid one left"
    );
}

// ---------------------------------------------------------------------------
// Geometry constants and the preset grid
// ---------------------------------------------------------------------------

#[test]
fn test_geometry_constants_and_preset_swatches() {
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

    // --- the shipped preset grid is opaque and never repeats ---
    let presets = default_preset_swatches();
    assert_eq!(presets.len(), 10);

    for swatch in &presets {
        assert_eq!(swatch.a, 255, "{swatch:?} must be a fully opaque preset");
    }

    let mut sorted = presets.clone();
    sorted.sort();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        before,
        "the preset grid must not repeat a colour"
    );

    // --- and every preset survives the SV well round trip ---
    for swatch in default_preset_swatches() {
        let back = Hsva::from_rgba8(swatch).to_rgba8();
        assert!(
            channel_delta(swatch, back) <= 1,
            "preset {swatch:?} came back as {back:?} after the SV well round trip"
        );
    }
}

// ---------------------------------------------------------------------------
// Flyout geometry
// ---------------------------------------------------------------------------

#[test]
fn test_flyout_layout_regions_and_anchor() {
    // --- the default model lays out the regions it promises ---
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

    // --- and moving the anchor translates every region without resizing it ---
    let model = ColorPickerModel::new(DISTINCT).with_alpha(true);
    let origin = ColorFlyoutLayout::compute(Point::ORIGIN, &model);
    let shifted = ColorFlyoutLayout::compute(Point::new(100.0, 50.0), &model);

    let regions: [(&str, Rectangle, Rectangle); 7] = [
        ("card", origin.card_bounds, shifted.card_bounds),
        ("sv well", origin.sv_well_bounds, shifted.sv_well_bounds),
        (
            "hue track",
            origin.hue_track_bounds,
            shifted.hue_track_bounds,
        ),
        ("hex row", origin.hex_row_bounds, shifted.hex_row_bounds),
        (
            "hex swatch",
            origin.hex_swatch_bounds,
            shifted.hex_swatch_bounds,
        ),
        (
            "cancel",
            origin.cancel_button_bounds,
            shifted.cancel_button_bounds,
        ),
        (
            "apply",
            origin.apply_button_bounds,
            shifted.apply_button_bounds,
        ),
    ];

    for (label, base, moved) in regions {
        assert_eq!(
            moved.x,
            base.x + 100.0,
            "{label} did not follow the anchor in x"
        );
        assert_eq!(
            moved.y,
            base.y + 50.0,
            "{label} did not follow the anchor in y"
        );
        assert_eq!(moved.width, base.width, "{label} changed width");
        assert_eq!(moved.height, base.height, "{label} changed height");
    }

    let base_alpha = origin
        .alpha_track_bounds
        .expect("an alpha model lays out an alpha track");
    let moved_alpha = shifted
        .alpha_track_bounds
        .expect("an alpha model lays out an alpha track");
    assert_eq!(moved_alpha.x, base_alpha.x + 100.0);
    assert_eq!(moved_alpha.y, base_alpha.y + 50.0);

    assert_eq!(origin.swatches.len(), shifted.swatches.len());
    for (base, moved) in origin.swatches.iter().zip(&shifted.swatches) {
        assert_eq!(
            moved.x,
            base.x + 100.0,
            "a preset swatch did not follow the anchor"
        );
        assert_eq!(
            moved.y,
            base.y + 50.0,
            "a preset swatch did not follow the anchor"
        );
    }
}

#[test]
fn test_flyout_layout_rows_and_footer() {
    // --- an empty preset list drops the rows and lifts the footer ---
    let with = ColorPickerModel::new(DISTINCT);
    let without = with.clone().with_preset_swatches(Vec::new());

    let a = ColorFlyoutLayout::compute(Point::ORIGIN, &with);
    let b = ColorFlyoutLayout::compute(Point::ORIGIN, &without);

    assert_eq!(a.swatches.len(), 10);
    assert!(
        b.swatches.is_empty(),
        "an empty preset list must not lay out any swatch"
    );

    // Two rows of swatches cost 2 * 24 + one 4px gap, plus the 8px section gap.
    let rows_cost = 2.0 * SWATCH_SIZE + SWATCH_GAP + SECTION_GAP;
    assert_eq!(a.card_bounds.height - b.card_bounds.height, rows_cost);
    assert_eq!(
        a.cancel_button_bounds.y - b.cancel_button_bounds.y,
        rows_cost,
        "the footer moves up by exactly what the preset rows occupied"
    );

    // --- presets wrap after the sixth column ---
    let presets: Vec<Rgba8> = (0..7u8).map(|i| Rgba8::from_rgb(i, i * 2, i * 3)).collect();
    let model = ColorPickerModel::new(DISTINCT).with_preset_swatches(presets);
    let layout = ColorFlyoutLayout::compute(Point::ORIGIN, &model);

    assert_eq!(layout.swatches.len(), 7);
    assert_eq!(
        layout.swatches[5].y, layout.swatches[0].y,
        "the sixth swatch still sits on the first row"
    );
    assert_eq!(
        layout.swatches[6].y,
        layout.swatches[0].y + SWATCH_SIZE + SWATCH_GAP,
        "the seventh wraps onto a second row"
    );
    assert_eq!(layout.swatches[6].x, layout.swatches[0].x);
    assert_eq!(
        layout.swatches[1].x,
        layout.swatches[0].x + SWATCH_SIZE + SWATCH_GAP,
        "columns step by a swatch plus the gap"
    );

    // --- the hex row splits between its swatch and its editor ---
    let layout = ColorFlyoutLayout::compute(Point::ORIGIN, &ColorPickerModel::new(DISTINCT));

    assert_eq!(layout.hex_swatch_bounds.x, layout.hex_row_bounds.x);
    assert_eq!(layout.hex_swatch_bounds.height, SWATCH_SIZE);
    assert_eq!(layout.hex_row_bounds.height, TRIGGER_HEIGHT_SM);
    assert_eq!(
        layout.hex_input_bounds.x,
        layout.hex_swatch_bounds.x + SWATCH_SIZE + space::SM
    );
    assert_eq!(layout.hex_input_bounds.height, TRIGGER_HEIGHT_SM);
    assert_eq!(
        layout.hex_input_bounds.x + layout.hex_input_bounds.width,
        layout.hex_row_bounds.x + COLOR_WELL_WIDTH,
        "the hex editor must end where the well ends"
    );

    // --- the footer splits its row evenly and closes the card ---
    assert_eq!(
        layout.apply_button_bounds.x,
        layout.cancel_button_bounds.x + layout.cancel_button_bounds.width + space::SM
    );
    assert_eq!(layout.apply_button_bounds.y, layout.cancel_button_bounds.y);
    assert_eq!(
        layout.apply_button_bounds.width, layout.cancel_button_bounds.width,
        "the two footer actions share the row evenly"
    );
    assert_eq!(
        layout.card_bounds.height,
        layout.cancel_button_bounds.y + TRIGGER_HEIGHT_SM + CARD_PADDING,
        "the card ends one padding below the footer"
    );

    // --- a contrast reference takes the row above the footer ---
    let plain = ColorFlyoutLayout::compute(Point::ORIGIN, &ColorPickerModel::new(DISTINCT));
    let model = ColorPickerModel::new(DISTINCT).with_contrast_reference(Rgba8::from_rgb(0, 0, 0));
    let with = ColorFlyoutLayout::compute(Point::ORIGIN, &model);

    let chip = with
        .contrast_bounds
        .expect("a contrast reference must lay out a chip");
    assert_eq!(chip.height, CONTRAST_CHIP_HEIGHT);
    assert_eq!(
        chip.y, plain.cancel_button_bounds.y,
        "the chip takes the row the footer used to hold"
    );
    assert_eq!(
        with.cancel_button_bounds.y - plain.cancel_button_bounds.y,
        CONTRAST_CHIP_HEIGHT + SECTION_GAP
    );
}

// ---------------------------------------------------------------------------
// Headless rendering
// ---------------------------------------------------------------------------

/// Every state the picker and the swatch button can be drawn in, through one
/// harness.
///
/// The nine states used to be nine harnesses, and each harness pays roughly
/// 620 ms to build iced's headless renderer. `set_view` is the harness's own
/// way of swapping the tree underneath a live session, so one renderer now
/// draws all nine frames for the price of one.
///
/// Sharing is safe here for a reason worth naming: `render` only lays out and
/// draws, and this test asserts nothing about state, so there is no second case
/// for a carried `Tree` to mislead.
#[test]
fn test_headless_render_picker_and_swatch_across_states() {
    // --- the picker's six drawable states ---
    let sample = Rgba8::from_rgb(255, 87, 34);
    let mut model_invalid = ColorPickerModel::new(sample).with_open(true);
    model_invalid.set_hex_input("bad-hex");

    let states: [(&str, ColorPickerModel); 6] = [
        // 1. Closed state
        ("closed", ColorPickerModel::new(sample)),
        // 2. Open state with alpha
        (
            "open with alpha",
            ColorPickerModel::new(sample)
                .with_alpha(true)
                .with_open(true),
        ),
        // 3. Open state with contrast reference (passing)
        (
            "open with a passing contrast reference",
            ColorPickerModel::new(Rgba8::from_rgb(255, 255, 255))
                .with_contrast_reference(Rgba8::from_rgb(0, 0, 0))
                .with_open(true),
        ),
        // 4. Open state with contrast reference (failing)
        (
            "open with a failing contrast reference",
            ColorPickerModel::new(Rgba8::from_rgb(30, 30, 30))
                .with_contrast_reference(Rgba8::from_rgb(0, 0, 0))
                .with_open(true),
        ),
        // 5. Disabled state
        (
            "disabled",
            ColorPickerModel::new(sample).with_disabled(true),
        ),
        // 6. Invalid hex input state
        ("open with an invalid hex buffer", model_invalid),
    ];

    let first = states[0].1.clone();
    let mut harness = Harness::<(), _>::headless(move || color_picker::<()>(first.clone()).into());

    for (_label, model) in &states {
        let model = model.clone();
        harness.set_view(move || color_picker::<()>(model.clone()).into());
        harness.render();
    }

    // --- and the swatch button's three ---
    let model = ColorPickerModel::new(Rgba8::from_rgb(59, 130, 246));

    let swatch_states: [(&str, ColorPickerModel, f32); 3] = [
        // 1. Standard 32px
        ("standard 32px", model.clone(), TRIGGER_HEIGHT_MD),
        // 2. Compact 28px
        ("compact 28px", model.clone(), TRIGGER_HEIGHT_SM),
        // 3. Disabled
        ("disabled", model.with_disabled(true), TRIGGER_HEIGHT_MD),
    ];

    for (_label, model, height) in &swatch_states {
        let model = model.clone();
        let height = *height;
        harness.set_view(move || color_swatch_button::<()>(&model).with_height(height).into());
        harness.render();
    }
}

// ---------------------------------------------------------------------------
// Widget behaviour
// ---------------------------------------------------------------------------

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

/// The picker's Tab-stop surface, and the disabled picker that publishes none.
///
/// One harness, two cases, because neither of them touches the widget's tree
/// state: the first only runs read-only operations, and a disabled picker's
/// `update` returns before it downcasts the state while its `operate` unfocuses
/// instead of publishing a stop. The second case therefore observes exactly what
/// a fresh tree would give it.
#[test]
fn test_the_picker_is_one_tab_stop_and_a_disabled_one_is_inert() {
    // --- an enabled picker is exactly one Tab stop, and it is the trigger ---
    let model = ColorPickerModel::new(DISTINCT);
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    let boxes = harness.recorded();
    assert_eq!(
        boxes.len(),
        1,
        "the picker is exactly one Tab stop, got {boxes:?}"
    );
    assert!(
        boxes[0].0.is_some(),
        "the trigger registers under its own id so focus can be routed to it"
    );
    assert_eq!(boxes[0].1.width, COLOR_FLYOUT_WIDTH);
    assert_eq!(
        boxes[0].1.height, TRIGGER_HEIGHT_MD,
        "the Tab stop is the trigger, not the trigger plus the flyout"
    );

    // --- a disabled picker answers nothing and is no Tab stop ---
    use iced::keyboard::key::Named;

    let disabled = ColorPickerModel::new(DISTINCT)
        .with_disabled(true)
        .with_open(true);
    let layout = flyout_at(&disabled);
    harness.set_view(move || picker(disabled.clone()).into());

    assert!(
        harness.press(trigger_centre()).is_empty(),
        "a disabled picker must not answer the trigger"
    );
    assert!(
        harness.press(layout.sv_well_bounds.center()).is_empty(),
        "a disabled picker must not answer the well"
    );
    assert!(
        harness
            .press(layout.apply_button_bounds.center())
            .is_empty(),
        "a disabled picker must not answer the footer"
    );
    assert!(
        press_key(&mut harness, Named::ArrowLeft).is_empty(),
        "a disabled picker must not answer the keyboard"
    );
    assert!(
        harness.registered().is_empty(),
        "a disabled picker must not be a Tab stop"
    );
}

#[test]
fn test_trigger_press_opens_and_closes_the_flyout() {
    let closed = ColorPickerModel::new(DISTINCT);
    let open = closed.clone().with_open(true);
    let trigger = trigger_centre();

    let mut harness = Harness::headless(move || picker(closed.clone()).into());
    assert_eq!(
        harness.press(trigger),
        vec![PickerMsg::Opened(true)],
        "pressing a closed trigger opens the flyout and nothing else"
    );
    assert_eq!(
        harness.release(trigger),
        Vec::new(),
        "releasing on the trigger settles no colour"
    );

    // The host answers `Opened(true)` by rebuilding the model with the flyout
    // open; the widget keeps no open flag of its own.
    harness.set_view(move || picker(open.clone()).into());
    assert_eq!(
        harness.press(trigger),
        vec![PickerMsg::Opened(false)],
        "pressing an open trigger closes it again, without cancelling"
    );
}

#[test]
fn test_footer_buttons_commit_or_roll_back_the_edit() {
    let original = Rgba8::from_rgb(10, 20, 30);
    let edited = Rgba8::from_rgb(200, 100, 50);
    // No presets, so the footer stays inside the harness viewport.
    let model = mid_edit(original, edited).with_preset_swatches(Vec::new());
    let layout = flyout_at(&model);
    let cancel = layout.cancel_button_bounds.center();
    let apply = layout.apply_button_bounds.center();

    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        harness.press(apply),
        vec![PickerMsg::Committed(edited), PickerMsg::Opened(false)],
        "Apply keeps the edited colour and closes the card"
    );
    assert_eq!(
        harness.press(cancel),
        vec![PickerMsg::Cancelled(original), PickerMsg::Opened(false)],
        "Cancel restores the pre-open snapshot and closes the card"
    );
}

#[test]
fn test_a_press_outside_the_card_cancels_the_edit() {
    let original = Rgba8::from_rgb(10, 20, 30);
    let edited = Rgba8::from_rgb(200, 100, 50);
    let model = mid_edit(original, edited);
    let layout = flyout_at(&model);

    let mut harness = Harness::headless(move || picker(model.clone()).into());

    let outside = Point::new(
        layout.card_bounds.x + layout.card_bounds.width + 60.0,
        layout.card_bounds.y + 164.0,
    );
    assert_eq!(
        harness.press(outside),
        vec![PickerMsg::Cancelled(original), PickerMsg::Opened(false)],
        "a press outside an open card rolls the edit back"
    );
}

#[test]
fn test_sv_well_press_maps_the_pointer_to_saturation_and_value() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0)).with_open(true);
    let opened_on = model.value;
    let layout = flyout_at(&model);
    let well = layout.sv_well_bounds;
    let top_left = Point::new(well.x, well.y);
    let middle = well.center();

    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        harness.press(top_left),
        vec![PickerMsg::Changed(Rgba8::from_rgb(255, 255, 255))],
        "the well's top-left corner is full value and no saturation"
    );
    // The commit carries the colour the *model* holds, not the one the drag
    // just published — see
    // `the_release_commits_the_models_colour_not_the_one_the_drag_just_published`.
    assert_eq!(
        harness.release(top_left),
        vec![PickerMsg::Committed(opened_on)],
        "with the host silent, the release settles exactly one commit of the \
         colour the picker opened on"
    );

    assert_eq!(
        harness.press(middle),
        vec![PickerMsg::Changed(Rgba8::new(128, 64, 64, 255))],
        "the well's centre halves both channels"
    );
    harness.release(middle);
}

#[test]
fn test_a_drag_that_leaves_the_well_clamps_to_its_edges() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0)).with_open(true);
    let opened_on = model.value;
    let layout = flyout_at(&model);
    let middle = layout.sv_well_bounds.center();

    let mut harness = Harness::headless(move || picker(model.clone()).into());
    harness.press(middle);

    assert_eq!(
        harness.pointer(
            &[moved(Point::new(-100.0, -100.0))],
            iced::mouse::Cursor::Available(Point::new(-100.0, -100.0)),
        ),
        vec![PickerMsg::Changed(Rgba8::from_rgb(255, 255, 255))],
        "dragging past the top-left must clamp to white, not wrap"
    );
    assert_eq!(
        harness.pointer(
            &[moved(Point::new(500.0, 500.0))],
            iced::mouse::Cursor::Available(Point::new(500.0, 500.0)),
        ),
        vec![PickerMsg::Changed(Rgba8::from_rgb(0, 0, 0))],
        "dragging past the bottom-right must clamp to black, not wrap"
    );

    assert_eq!(
        harness.release(Point::new(500.0, 500.0)),
        vec![PickerMsg::Committed(opened_on)],
        "the release ends the drag with exactly one commit — of the colour the \
         model holds, not the black the clamped bottom-right published"
    );
    assert!(
        harness
            .pointer(
                &[moved(Point::new(-100.0, -100.0))],
                iced::mouse::Cursor::Available(Point::new(-100.0, -100.0)),
            )
            .is_empty(),
        "a move after the release must not scrub anything: the drag is over"
    );
}

/// The commit carries the colour the **model** holds, not the one the drag just
/// published.
///
/// `ColorPicker` is a controlled component: a well drag publishes `Changed` and
/// writes nothing back into the model it was built with, and the release
/// publishes `Commit::new(self.model.value)` (the `ButtonReleased` arm of
/// `ColorPicker::update`). What a host therefore gets on the release depends on
/// whether it answered the live tick:
///
/// * a host that rebuilds the picker with the reported colour commits that
///   colour — the normal path, and the one the contract is written for;
/// * a host that does not, commits the colour the picker opened on, even though
///   the drag published something else a moment earlier.
///
/// Both legs are pinned, because the gap between them is the contract: the
/// widget reports the state it was given, never a state it took ownership of.
#[test]
fn the_release_commits_the_models_colour_not_the_one_the_drag_just_published() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0)).with_open(true);
    let well = flyout_at(&model).sv_well_bounds;
    let top_left = Point::new(well.x, well.y);
    let white = Rgba8::from_rgb(255, 255, 255);

    // Leg one: the host never answers the live tick.
    let silent = model.clone();
    let mut harness = Harness::headless(move || picker(silent.clone()).into());
    assert_eq!(
        harness.press(top_left),
        vec![PickerMsg::Changed(white)],
        "the drag publishes white"
    );
    assert_eq!(
        harness.release(top_left),
        vec![PickerMsg::Committed(model.value)],
        "and the commit reports the model back, unchanged"
    );

    // Leg two: the host does its job and rebuilds with the reported colour.
    let fed_back = ColorPickerModel::new(white).with_open(true);
    harness.set_view(move || picker(fed_back.clone()).into());
    assert_eq!(
        harness.press(top_left),
        vec![PickerMsg::Changed(white)],
        "the press re-arms the drag on the rebuilt view"
    );
    assert_eq!(
        harness.release(top_left),
        vec![PickerMsg::Committed(white)],
        "with the live tick fed back, the release commits the dragged colour"
    );
}

#[test]
fn test_hue_track_maps_x_to_hue_and_clamps_past_its_ends() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(0, 255, 0)).with_open(true);
    let layout = flyout_at(&model);
    let track = layout.hue_track_bounds;
    let middle = track.center();
    let left_edge = Point::new(track.x, middle.y);

    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        harness.press(middle),
        vec![PickerMsg::Changed(Rgba8::from_rgb(0, 255, 255))],
        "the middle of the track is 180 degrees"
    );
    harness.release(middle);

    assert_eq!(
        harness.press(left_edge),
        vec![PickerMsg::Changed(Rgba8::from_rgb(255, 0, 0))],
        "the track's left edge is 0 degrees"
    );
    harness.release(left_edge);

    let beyond = Point::new(600.0, middle.y);
    harness.press(middle);
    assert_eq!(
        harness.pointer(&[moved(beyond)], iced::mouse::Cursor::Available(beyond),),
        vec![PickerMsg::Changed(Rgba8::from_rgb(255, 0, 0))],
        "dragging past the right end clamps to 360 degrees, which is red again"
    );
    harness.release(beyond);
}

#[test]
fn test_alpha_track_maps_x_to_alpha_and_clamps_past_its_ends() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(0, 255, 0))
        .with_alpha(true)
        .with_open(true);
    let layout = flyout_at(&model);
    let track = layout
        .alpha_track_bounds
        .expect("an alpha model lays out an alpha track");
    let middle = track.center();
    let left_edge = Point::new(track.x, middle.y);

    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        harness.press(middle),
        vec![PickerMsg::Changed(Rgba8::new(0, 255, 0, 128))],
        "the middle of the alpha track is half transparent"
    );
    harness.release(middle);

    assert_eq!(
        harness.press(left_edge),
        vec![PickerMsg::Changed(Rgba8::new(0, 255, 0, 0))],
        "the track's left edge is fully transparent"
    );
    harness.release(left_edge);

    let beyond = Point::new(600.0, middle.y);
    harness.press(middle);
    assert_eq!(
        harness.pointer(&[moved(beyond)], iced::mouse::Cursor::Available(beyond)),
        vec![PickerMsg::Changed(Rgba8::new(0, 255, 0, 255))],
        "dragging past the right end clamps to fully opaque"
    );
    harness.release(beyond);
}

#[test]
fn test_a_preset_swatch_commits_immediately_and_the_gaps_are_inert() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(9, 9, 9)).with_open(true);
    let layout = flyout_at(&model);
    let first_swatch = layout.swatches[0];
    let black = default_preset_swatches()[0];
    assert_eq!(black, Rgba8::from_rgb(0, 0, 0));

    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        harness.press(first_swatch.center()),
        vec![PickerMsg::Changed(black), PickerMsg::Committed(black)],
        "a preset press is a finished intent: live tick and commit together"
    );
    assert_eq!(
        harness.release(first_swatch.center()),
        Vec::new(),
        "a preset press starts no drag, so its release settles nothing"
    );

    let gap = Point::new(
        first_swatch.x + SWATCH_SIZE + SWATCH_GAP / 2.0,
        first_swatch.center().y,
    );
    assert_eq!(
        harness.press(gap),
        Vec::new(),
        "the gap between two swatches must not select either of them"
    );
}

#[test]
fn test_arrow_keys_scrub_the_active_channel_and_clamp_at_its_ends() {
    use iced::keyboard::key::Named;

    // Green: saturation and value both already at their ceiling.
    let green = Rgba8::from_rgb(0, 255, 0);
    let model = ColorPickerModel::new(green).with_open(true);
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        press_key(&mut harness, Named::ArrowRight),
        vec![PickerMsg::Changed(green)],
        "saturation cannot go past 1.0"
    );
    assert_eq!(
        press_key(&mut harness, Named::ArrowUp),
        vec![PickerMsg::Changed(green)],
        "value cannot go past 1.0"
    );
    assert_eq!(
        press_key(&mut harness, Named::ArrowLeft),
        vec![PickerMsg::Changed(Rgba8::new(13, 255, 13, 255))],
        "ArrowLeft desaturates by one step"
    );
    assert_eq!(
        press_key(&mut harness, Named::ArrowDown),
        vec![PickerMsg::Changed(Rgba8::new(0, 242, 0, 255))],
        "ArrowDown darkens by one step"
    );
}

#[test]
fn test_escape_and_enter_close_an_open_flyout() {
    use iced::keyboard::key::Named;

    let original = Rgba8::from_rgb(10, 20, 30);
    let edited = Rgba8::from_rgb(200, 100, 50);
    let model = mid_edit(original, edited);
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        press_key(&mut harness, Named::Escape),
        vec![PickerMsg::Cancelled(original), PickerMsg::Opened(false)],
        "Escape rolls the edit back"
    );
    assert_eq!(
        press_key(&mut harness, Named::Enter),
        vec![PickerMsg::Committed(edited), PickerMsg::Opened(false)],
        "Enter keeps the edit"
    );
}

#[test]
fn test_enter_and_space_open_a_focused_closed_picker() {
    use iced::keyboard::key::Named;

    let model = ColorPickerModel::new(DISTINCT);
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    assert_eq!(
        harness.registered().len(),
        1,
        "a shut picker is exactly one Tab stop"
    );
    // Focus is taken by traversal here, though an id-addressed request would now
    // reach it too: the picker registers under a stable name, which
    // `defect_03_the_color_picker_trigger_keeps_one_id_across_rebuilds` pins.
    // Traversal focuses whatever the current tree actually holds, and the flag
    // lives in the widget's own tree state, which the cache carries forward.
    harness.iced_focus_next();
    assert!(harness.focused().is_some(), "the fixture must hold focus");

    assert_eq!(
        press_key(&mut harness, Named::Space),
        vec![PickerMsg::Opened(true)],
        "Space opens a closed picker"
    );
    assert_eq!(
        press_key(&mut harness, Named::Enter),
        vec![PickerMsg::Opened(true)],
        "Enter opens a closed picker"
    );
    assert!(
        press_key(&mut harness, Named::Escape).is_empty(),
        "Escape has nothing to cancel while the card is shut"
    );
    assert!(
        press_key(&mut harness, Named::ArrowLeft).is_empty(),
        "the arrow keys scrub the well, so they are silent while the card is shut"
    );
}

#[test]
fn test_backspacing_the_hex_field_stops_at_the_prefix() {
    use iced::keyboard::key::Named;

    let model = ColorPickerModel::new(Rgba8::from_rgb(0, 0, 0)).with_open(true);
    let hex = flyout_at(&model).hex_input_bounds.center();
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    harness.press(hex);

    for step in 0..7 {
        assert!(
            press_key(&mut harness, Named::Backspace).is_empty(),
            "backspace {step} shortens the field to an unparseable string and must stay quiet"
        );
    }

    // The buffer is now just "#": a second prefix is dropped, not appended.
    assert!(
        type_char(&mut harness, "#").is_empty(),
        "a second '#' must be dropped rather than pushed"
    );

    // Five more characters are still not a colour; the sixth completes it.
    for step in 0..5 {
        assert!(
            type_char(&mut harness, "F").is_empty(),
            "the field is not a colour yet after typing F {step} times"
        );
    }
    assert_eq!(
        type_char(&mut harness, "F"),
        vec![PickerMsg::Changed(Rgba8::from_rgb(255, 255, 255))],
        "the seventh character completes #FFFFFF"
    );

    // The buffer is full: further characters cannot make it longer.
    assert!(
        type_char(&mut harness, "A").is_empty(),
        "the buffer stops growing at the format's length"
    );
}

#[test]
fn test_a_non_hex_character_never_reaches_the_model() {
    let model = ColorPickerModel::new(Rgba8::from_rgb(0, 0, 0)).with_open(true);
    let hex = flyout_at(&model).hex_input_bounds.center();
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    harness.press(hex);

    for ch in ["z", "g", " ", "-", "é"] {
        assert!(
            type_char(&mut harness, ch).is_empty(),
            "'{ch}' is not a hex digit and must not move the colour"
        );
    }
}

#[test]
fn test_tab_hands_the_hex_field_back_to_the_trigger() {
    use iced::keyboard::key::Named;

    let model = ColorPickerModel::new(Rgba8::from_rgb(255, 0, 0)).with_open(true);
    let hex = flyout_at(&model).hex_input_bounds.center();
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    harness.press(hex);
    assert!(
        press_key(&mut harness, Named::Tab).is_empty(),
        "Tab only moves focus, it publishes nothing"
    );
    assert!(
        type_char(&mut harness, "F").is_empty(),
        "after Tab the field is no longer being edited, so a keystroke cannot change the colour"
    );
    assert_eq!(
        press_key(&mut harness, Named::ArrowLeft),
        vec![PickerMsg::Changed(Rgba8::new(255, 13, 13, 255))],
        "and the arrow keys are scrubbing the well again: ArrowLeft takes one \
         saturation step off the red the card opened on"
    );
}

#[test]
fn test_escape_and_enter_while_the_hex_field_is_focused_close_the_card() {
    use iced::keyboard::key::Named;

    let original = Rgba8::from_rgb(10, 20, 30);
    let edited = Rgba8::from_rgb(200, 100, 50);
    let model = mid_edit(original, edited);
    let hex = flyout_at(&model).hex_input_bounds.center();
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    harness.press(hex);
    assert_eq!(
        press_key(&mut harness, Named::Escape),
        vec![PickerMsg::Cancelled(original), PickerMsg::Opened(false)],
        "Escape from the hex field abandons the whole edit"
    );
    assert_eq!(
        press_key(&mut harness, Named::Enter),
        vec![PickerMsg::Committed(edited), PickerMsg::Opened(false)],
        "Enter from the hex field accepts the whole edit"
    );
}

/// The swatch button's activation surface, and the disabled button that has none.
///
/// One harness, two cases. The disabled path is provably independent of whatever
/// the enabled case left in the tree: its `update` returns before it reads the
/// state, so it cannot publish, and its `operate` unfocuses instead of offering
/// a Tab stop.
#[test]
fn test_swatch_button_answers_the_press_and_the_keyboard_but_not_the_release() {
    use iced::keyboard::key::Named;

    let model = ColorPickerModel::new(DISTINCT);
    let centre = Point::new(COLOR_FLYOUT_WIDTH / 2.0, TRIGGER_HEIGHT_MD / 2.0);
    let mut harness = Harness::headless(move || {
        color_swatch_button::<PickerMsg>(&model)
            .on_press(|| PickerMsg::Pressed)
            .into()
    });

    assert_eq!(
        harness.press(centre),
        vec![PickerMsg::Pressed],
        "the button answers the press"
    );
    assert_eq!(harness.release(centre), Vec::new(), "and not the release");
    assert!(
        harness.press(Point::new(300.0, 200.0)).is_empty(),
        "a press outside the button is not a press on it"
    );
    assert!(
        harness.hover(centre).is_empty() && harness.hover(Point::new(390.0, 290.0)).is_empty(),
        "hover repaints, it does not publish"
    );

    // Focused by traversal, as in
    // `test_enter_and_space_open_a_focused_closed_picker`; the button's own id is
    // stable across rebuilds now, which
    // `defect_03_the_swatch_button_keeps_one_id_across_rebuilds` pins.
    harness.iced_focus_next();
    assert!(harness.focused().is_some(), "the button must hold focus");
    assert_eq!(
        press_key(&mut harness, Named::Enter),
        vec![PickerMsg::Pressed],
        "Enter activates the button"
    );
    assert_eq!(
        press_key(&mut harness, Named::Space),
        vec![PickerMsg::Pressed],
        "Space activates the button"
    );
    assert!(
        press_key(&mut harness, Named::Escape).is_empty(),
        "Escape is not an activation"
    );

    // --- a disabled button is inert and is no Tab stop ---
    let disabled = ColorPickerModel::new(DISTINCT).with_disabled(true);
    harness.set_view(move || {
        color_swatch_button::<PickerMsg>(&disabled)
            .on_press(|| PickerMsg::Pressed)
            .into()
    });

    assert!(
        harness.press(centre).is_empty(),
        "a disabled button publishes nothing"
    );
    assert!(
        harness.hover(centre).is_empty(),
        "a disabled button does not even track the hover"
    );
    assert!(
        harness.registered().is_empty(),
        "a disabled button must not be a Tab stop, got {:?}",
        harness.registered()
    );
}

// ---------------------------------------------------------------------------
// Defect 03 — a focus id the host can address across frames
// ---------------------------------------------------------------------------

/// **Defect 03, fixed.** The trigger registers under a name that survives a
/// rebuild, so an id-addressed focus request still names the same control on the
/// next frame.
///
/// `ColorPicker::new` used to mint `Id::unique()` on every construction, and a
/// host rebuilds its whole view on every update — so an id read in one frame
/// addressed nothing in the next, and `focus(id)` landed silently nowhere. The
/// harness rebuilds the tree on every step, which makes two reads from one
/// harness exactly two constructions of the widget.
#[test]
fn defect_03_the_color_picker_trigger_keeps_one_id_across_rebuilds() {
    let model = ColorPickerModel::new(DISTINCT);
    let mut harness = Harness::headless(move || picker(model.clone()).into());

    let first = harness.registered();
    let second = harness.registered();
    assert_eq!(
        first, second,
        "defect 03: two rebuilds of the same picker must register the same id"
    );
    assert_eq!(
        first,
        vec![Some(iced::advanced::widget::Id::new(
            DEFAULT_COLOR_PICKER_ID
        ))],
        "defect 03: the trigger registers under the documented default name"
    );

    // A second, unrelated tree — the same control built somewhere else — claims
    // the same name, which is what lets a bridge address it without a handle.
    let mut other = Harness::headless(move || picker(ColorPickerModel::new(DISTINCT)).into());
    assert_eq!(
        other.registered(),
        first,
        "defect 03: the id comes from the widget, not from the tree it was built into"
    );

    // And the widget reports the very name the router sees.
    assert_eq!(
        picker(ColorPickerModel::new(DISTINCT)).widget_id(),
        &iced::advanced::widget::Id::new(DEFAULT_COLOR_PICKER_ID),
        "defect 03: widget_id reports the id the router sees"
    );
}

/// **Defect 03, fixed.** `with_id` names the control, which is what a host with
/// more than one picker on a screen needs: two controls sharing one id cannot be
/// told apart by an id-addressed focus request.
#[test]
fn defect_03_with_id_names_the_color_picker_for_the_focus_router() {
    let mut foreground = Harness::headless(move || {
        picker(ColorPickerModel::new(DISTINCT))
            .with_id("field.foreground")
            .into()
    });
    assert_eq!(
        foreground.registered(),
        vec![Some(iced::advanced::widget::Id::new("field.foreground"))],
        "defect 03: the router must see the name the host chose"
    );

    // An owned name registers under the same id a borrowed one would, so a host
    // that composes its field names at run time can still name its controls.
    let owned = String::from("field.background");
    let mut background = Harness::headless(move || {
        picker(ColorPickerModel::new(DISTINCT))
            .with_id(owned.clone())
            .into()
    });
    assert_eq!(
        background.registered(),
        vec![Some(iced::advanced::widget::Id::new("field.background"))],
        "defect 03: an owned name and a borrowed one of the same text are one id"
    );

    assert_ne!(
        foreground.registered(),
        background.registered(),
        "defect 03: two differently named pickers are two distinct focus targets"
    );
    assert_eq!(
        picker(ColorPickerModel::new(DISTINCT))
            .with_id("field.foreground")
            .widget_id(),
        &iced::advanced::widget::Id::new("field.foreground"),
        "defect 03: widget_id reports the name `with_id` installed"
    );
}

/// **Defect 03, fixed.** The standalone swatch button keeps one id across
/// rebuilds, for the same reason and by the same mechanism as
/// [`defect_03_the_color_picker_trigger_keeps_one_id_across_rebuilds`].
#[test]
fn defect_03_the_swatch_button_keeps_one_id_across_rebuilds() {
    let model = ColorPickerModel::new(DISTINCT);
    let mut harness = Harness::headless(move || color_swatch_button::<PickerMsg>(&model).into());

    let first = harness.registered();
    let second = harness.registered();
    assert_eq!(
        first, second,
        "defect 03: two rebuilds of the same button must register the same id"
    );
    assert_eq!(
        first,
        vec![Some(iced::advanced::widget::Id::new(
            DEFAULT_COLOR_SWATCH_BUTTON_ID
        ))],
        "defect 03: the button registers under the documented default name"
    );
    assert_eq!(
        color_swatch_button::<()>(&ColorPickerModel::new(DISTINCT)).widget_id(),
        &iced::advanced::widget::Id::new(DEFAULT_COLOR_SWATCH_BUTTON_ID),
        "defect 03: widget_id reports the id the router sees"
    );
}

/// **Defect 03, fixed.** `with_id` names the swatch button, so a host with a
/// row of swatches can address each one on its own.
#[test]
fn defect_03_with_id_names_the_swatch_button_for_the_focus_router() {
    let accent_model = ColorPickerModel::new(DISTINCT);
    let mut accent = Harness::headless(move || {
        color_swatch_button::<PickerMsg>(&accent_model)
            .with_id("swatch.accent")
            .into()
    });
    assert_eq!(
        accent.registered(),
        vec![Some(iced::advanced::widget::Id::new("swatch.accent"))],
        "defect 03: the router must see the name the host chose"
    );

    let owned = String::from("swatch.highlight");
    let highlight_model = ColorPickerModel::new(DISTINCT);
    let mut highlight = Harness::headless(move || {
        color_swatch_button::<PickerMsg>(&highlight_model)
            .with_id(owned.clone())
            .into()
    });
    assert_eq!(
        highlight.registered(),
        vec![Some(iced::advanced::widget::Id::new("swatch.highlight"))],
        "defect 03: an owned name and a borrowed one of the same text are one id"
    );
    assert_ne!(
        accent.registered(),
        highlight.registered(),
        "defect 03: two differently named buttons are two distinct focus targets"
    );
    assert_eq!(
        color_swatch_button::<()>(&ColorPickerModel::new(DISTINCT))
            .with_id("swatch.accent")
            .widget_id(),
        &iced::advanced::widget::Id::new("swatch.accent"),
        "defect 03: widget_id reports the name `with_id` installed"
    );
}
