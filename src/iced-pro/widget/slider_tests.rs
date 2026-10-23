//! Tests for the slider resolver and geometry — `UIF-ATOM-005`, §3.5.
//!
//! Five groups, in order:
//!
//! 1. **The matrix.** Each base state's resolved channels, and the two
//!    states §3.5 deliberately collapses into one cell.
//! 2. **Contrast.** Every pair recomputed from raw sRGB through
//!    [`contrast::contrast_ratio`], including the fill-on-fill measurement
//!    the spec never prints and the disabled pair that forced the rail to
//!    lose its accent.
//! 3. **Geometry.** Track, thumb, hit box, and the value↔pixel mapping a
//!    drag depends on.
//! 4. **Layers.** What is painted, where, and what is deliberately absent.
//! 5. **Chrome classification.** Which perimeters the audit judges and
//!    which it waves through.

use super::*;
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::contrast;
use iced::{Color, Point, Rectangle};

/// The snapshot world every ratio in this file is measured against.
fn palette() -> Palette {
    Palette::dark()
}

/// A `VisualState` with no focus ring.
fn state(base: BaseState) -> VisualState {
    VisualState {
        base,
        focused: false,
    }
}

/// A `VisualState` carrying the keyboard ring.
fn focused(base: BaseState) -> VisualState {
    VisualState {
        base,
        focused: true,
    }
}

/// Assert a measured ratio against its true value to four places.
fn assert_ratio(actual: f32, expected: f32, what: &str) {
    let measured = f64::from(actual);
    let wanted = f64::from(expected);
    assert!(
        (measured - wanted).abs() < 0.000_5,
        "{what}: expected {wanted:.4}, measured {measured:.4}"
    );
}

/// A 200 × 24 slider box — wide enough to travel, tall enough to hold
/// the hit target.
fn bounds() -> Rectangle {
    Rectangle::new(Point::new(10.0, 100.0), iced::Size::new(200.0, 24.0))
}

// ---------------------------------------------------------------------------
// 1. The matrix
// ---------------------------------------------------------------------------

#[test]
fn rest_paints_the_white_thumb_with_the_accent_ring() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    assert_eq!(s.rail_unfilled, p.surface_active, "groove");
    assert_eq!(
        s.rail_border, p.border_subtle,
        "groove outline is a divider"
    );
    assert_eq!(s.rail_filled, p.accent_fill, "filled rail");
    assert_eq!(s.thumb_fill, p.text_strong, "thumb face");
    assert_eq!(s.thumb_border, p.accent_border, "thumb ring");
    assert_eq!(s.thumb_border_width, THUMB_RING_WIDTH);
    assert_eq!(s.tick, p.border_strong, "ticks");
}

#[test]
fn hovered_and_pressed_share_one_cell_because_the_spec_gives_them_one() {
    // §3.5 writes "(`Hovered`/`Pressed` thumb scales optical ring to 2px
    // text.strong on accent.fill)" as a single rule and never spends
    // `accent.fill_active`. A third step would be invented.
    let p = palette();
    let hover = resolve_style(state(BaseState::Hovered), p.surface_overlay, &p);
    let press = resolve_style(state(BaseState::Pressed), p.surface_overlay, &p);
    assert_eq!(hover, press, "the two states resolve identically");
    assert_eq!(hover.thumb_fill, p.accent_fill, "face becomes the accent");
    assert_eq!(
        hover.thumb_border, p.text_strong,
        "ring becomes the strongest text"
    );
    // The hovered face *is* the filled rail's colour. That is not a
    // mistake to assert away — it is the reason the ring has to invert.
    // With an accent face on an accent rail the ring is the only channel
    // left, and it is the one §3.5 spends `text.strong` on.
    assert_eq!(
        hover.thumb_fill, hover.rail_filled,
        "the face matches the fill, so the ring must carry the handle"
    );
    assert_eq!(
        thumb_carrier(&hover, hover.rail_filled),
        ThumbCarrier::Ring,
        "and it does: {:.4} on the ring against the filled rail",
        contrast::contrast_ratio(hover.thumb_border, hover.rail_filled)
    );
}

#[test]
fn the_hover_inversion_moves_the_white_from_face_to_ring() {
    // The cue is that the resting face colour turns up as the hovered
    // ring. It is a half-swap, not a whole one: the resting ring is
    // `accent.border` and the hovered face is `accent.fill` — the same
    // hue one step deeper, two different tokens. §3.5 names both and
    // nothing makes them equal, so asserting a clean swap here would be
    // asserting a rule the section does not contain.
    let p = palette();
    let rest = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let hover = resolve_style(state(BaseState::Hovered), p.surface_overlay, &p);
    assert_eq!(
        rest.thumb_fill, hover.thumb_border,
        "the white moves from face to ring"
    );
    assert_ne!(
        rest.thumb_border, hover.thumb_fill,
        "and the ring's accent is replaced by the face's, not exchanged"
    );
    assert_eq!(rest.thumb_border, p.accent_border, "resting ring token");
    assert_eq!(hover.thumb_fill, p.accent_fill, "hovered face token");
}

#[test]
fn invalid_shares_the_rest_cell_because_the_spec_gives_the_slider_no_invalid_skin() {
    // §3B.1 renders a field's validation words in the caption slot, not
    // on the control, so there is nothing here to repaint.
    let p = palette();
    assert_eq!(
        resolve_style(state(BaseState::Invalid), p.surface_overlay, &p),
        resolve_style(state(BaseState::Rest), p.surface_overlay, &p),
        "an invalid slider looks like a resting one"
    );
}

#[test]
fn disabled_takes_the_accent_out_of_the_rail_as_well_as_the_thumb() {
    let p = palette();
    let s = resolve_style(state(BaseState::Disabled), p.surface_overlay, &p);
    assert_eq!(s.rail_filled, p.border_strong, "the rail loses its accent");
    assert_ne!(
        s.rail_filled, p.accent_fill,
        "and is provably not live colour"
    );
    assert_eq!(s.thumb_fill, p.surface_raised, "thumb goes flat");
    assert_eq!(s.thumb_border, p.text_disabled, "ring dims");
    assert_eq!(s.tick, p.border_strong, "ticks stay");
}

#[test]
fn the_groove_never_changes_state() {
    // The unfilled rail is the groove, not an indicator. Nothing in §3.5
    // repaints it, and a groove that brightened under the pointer would
    // compete with the thumb that is the actual feedback.
    let p = palette();
    let groove = resolve_style(state(BaseState::Rest), p.surface_overlay, &p).rail_unfilled;
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        let s = resolve_style(state(base), p.surface_overlay, &p);
        assert_eq!(
            s.rail_unfilled, groove,
            "{base:?} must not repaint the groove"
        );
        assert_eq!(s.rail_border, p.border_subtle, "{base:?} rail outline");
    }
}

#[test]
fn no_ring_is_due_without_focus_in_any_base_state() {
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        let s = resolve_style(state(base), p.surface_overlay, &p);
        assert_eq!(s.focus_ring_width, 0.0, "{base:?} draws no ring");
        assert_eq!(
            s.focus_ring_color,
            Color::TRANSPARENT,
            "{base:?} ring colour"
        );
    }
}

#[test]
fn a_focus_ring_is_due_in_every_enabled_state() {
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Invalid,
    ] {
        let s = resolve_style(focused(base), p.surface_overlay, &p);
        assert_eq!(
            s.focus_ring_width, FOCUS_RING_WIDTH,
            "{base:?} draws a ring"
        );
        assert_ne!(
            s.focus_ring_color,
            Color::TRANSPARENT,
            "{base:?} ring is painted"
        );
    }
}

#[test]
fn a_disabled_slider_never_draws_a_ring() {
    // The suppression lives in `state::resolve`, which owns the rule that
    // a dead control cannot hold focus — not in this resolver, which
    // trusts `ring_visible()` the same way
    // [`toggle::resolve_style`] does. So the test drives the real entry
    // point: hand `resolve` a disabled *and* focused control and show the
    // ring is gone before the slider ever sees the state.
    let p = palette();
    let resolved = crate::state::resolve(crate::state::InteractionInput {
        enabled: false,
        hovered: true,
        pressed: true,
        focused: true,
        invalid: false,
    });
    assert_eq!(resolved.base, BaseState::Disabled, "dead dominates");
    assert!(
        !resolved.ring_visible(),
        "resolve must not hand a dead control a ring"
    );

    let s = resolve_style(resolved, p.surface_overlay, &p);
    assert_eq!(
        s.focus_ring_width, 0.0,
        "a dead control advertises no focus"
    );
    assert_eq!(s.focus_ring_color, Color::TRANSPARENT);
    assert!(focus_ring_layer(track_rect(bounds()), &s, 0.5).is_none());
}

#[test]
fn the_ring_is_resolved_against_the_mount_not_the_thumb() {
    // The ring sits on the 24px hit box, over whatever the slider is
    // mounted on — not over the thumb's own fill.
    let p = palette();
    for mount in [p.surface_base, p.surface_overlay, p.surface_active] {
        let s = resolve_style(focused(BaseState::Rest), mount, &p);
        assert_eq!(
            s.focus_ring_color,
            focus::resolve_ring_color_for_background(mount, &p),
            "the ring must be the mount's answer"
        );
    }
}

#[test]
fn the_ring_displaces_on_a_surface_the_accent_cannot_survive_on() {
    // The adaptive rule, exercised against a mount that actually forces
    // it: `accent.border` on `accent.fill` is 1.6098:1, so the ring's
    // first choice fails and `text.strong` — 4.8020:1 there — takes over.
    let p = palette();
    let on_dark = resolve_style(focused(BaseState::Rest), p.surface_base, &p).focus_ring_color;
    let on_accent = resolve_style(focused(BaseState::Rest), p.accent_fill, &p).focus_ring_color;
    assert_eq!(
        on_dark, p.accent_border,
        "the ordinary mount keeps the accent ring"
    );
    assert_eq!(
        on_accent, p.text_strong,
        "a mount the accent cannot survive on displaces the ring by arithmetic"
    );
    assert_ne!(
        on_dark, on_accent,
        "the ring adapts rather than going invisible"
    );
    assert!(
        contrast::meets_chrome_contrast(on_accent, p.accent_fill),
        "the displaced ring still clears the chrome floor: {:.4}",
        contrast::contrast_ratio(on_accent, p.accent_fill)
    );
}

#[test]
fn a_mount_that_defeats_both_ring_candidates_returns_the_less_invisible_one() {
    // Recorded, not fixed. On a white mount `accent.border` measures
    // 2.9829:1 and `text.strong` measures 1.0:1 — neither reaches the
    // floor, and `focus::resolve_ring_color` returns the higher of the
    // two rather than waiving the rule. That is the documented contract:
    // a fill that defeats both candidates is a palette defect the audit
    // is supposed to surface, not something the resolver hides. The
    // slider inherits it unchanged, so the number is pinned here rather
    // than left to be rediscovered as a surprise in a light theme.
    let p = palette();
    let white = Color::from_rgb8(255, 255, 255);
    let ring = resolve_style(focused(BaseState::Rest), white, &p).focus_ring_color;
    assert_eq!(
        ring, p.accent_border,
        "the resolver picks the less invisible candidate, it does not give up"
    );
    assert!(
        !contrast::meets_chrome_contrast(ring, white),
        "and the shortfall is real: {:.4}",
        contrast::contrast_ratio(ring, white)
    );
    assert!(
        contrast::contrast_ratio(ring, white) > contrast::contrast_ratio(p.text_strong, white),
        "which is why the accent wins the tie against white"
    );
}

#[test]
fn resolving_is_deterministic() {
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        for ring in [false, true] {
            let v = VisualState {
                base,
                focused: ring,
            };
            let a = resolve_style(v, p.surface_overlay, &p);
            let b = resolve_style(v, p.surface_overlay, &p);
            assert_eq!(a, b, "{v:?} must resolve the same way twice");
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Contrast — recomputed from raw sRGB, never transcribed
// ---------------------------------------------------------------------------

#[test]
fn the_filled_rail_clears_the_overlay_and_base_ratios_the_spec_prints() {
    let p = palette();
    assert_ratio(
        contrast::contrast_ratio(p.accent_fill, p.surface_overlay),
        3.6895,
        "accent.fill / surface.overlay (§3.5 prints 3.69:1)",
    );
    assert_ratio(
        contrast::contrast_ratio(p.accent_fill, p.surface_base),
        4.1432,
        "accent.fill / surface.base (§3.5 prints 4.14:1)",
    );
}

#[test]
fn the_filled_rail_clears_the_chrome_floor_on_both_mounts() {
    let p = palette();
    assert!(
        contrast::meets_chrome_contrast(p.accent_fill, p.surface_overlay),
        "filled rail on overlay"
    );
    assert!(
        contrast::meets_chrome_contrast(p.accent_fill, p.surface_base),
        "filled rail on base"
    );
}

/// Every surface the thumb can end up sitting on, read out of the
/// resolved style itself rather than a hand-written list: the mount the
/// slider was resolved against, the groove it travels in, and the filled
/// portion laid inside that groove.
fn surfaces_behind_thumb(style: &SliderStyle, mount: Color) -> [(&'static str, Color); 3] {
    [
        ("the mount", mount),
        ("the groove (rail_unfilled)", style.rail_unfilled),
        ("the filled rail (rail_filled)", style.rail_filled),
    ]
}

#[test]
fn the_thumb_is_distinguishable_on_every_surface_in_every_state() {
    // The requirement §3.5 leaves implicit and this control cannot skip:
    // the handle has to read wherever it travels, in every state, or a
    // drag becomes blind. Measured as the union of face and ring — see
    // the module docs for why the ring alone would be the wrong question.
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        let s = resolve_style(state(base), p.surface_overlay, &p);
        for (name, bg) in surfaces_behind_thumb(&s, p.surface_overlay) {
            assert!(
                thumb_is_distinguishable(&s, bg),
                "{base:?} thumb on {name}: {:.4} — invisible handle",
                thumb_distinguishable_ratio(&s, bg)
            );
        }
    }
}

#[test]
fn which_channel_carries_the_thumb_is_pinned_per_state() {
    // The union passing is necessary, not sufficient. If a palette edit
    // moved every thumb from face-carried to ring-carried the union would
    // still hold while the control changed appearance; pinning the
    // carrier per surface makes that a failing test instead of a silent
    // redesign.
    let p = palette();
    let cases: &[(BaseState, &str, ThumbCarrier)] = &[
        // Rest: white face, accent ring. Both clear the dark mount and
        // the groove; over the accent fill only the face survives.
        (BaseState::Rest, "the mount", ThumbCarrier::Both),
        (
            BaseState::Rest,
            "the groove (rail_unfilled)",
            ThumbCarrier::Both,
        ),
        (
            BaseState::Rest,
            "the filled rail (rail_filled)",
            ThumbCarrier::Face,
        ),
        // Hovered/Pressed: the inversion puts the accent on the face,
        // so over the fill the ring is the only thing left.
        (BaseState::Hovered, "the mount", ThumbCarrier::Both),
        (
            BaseState::Hovered,
            "the groove (rail_unfilled)",
            ThumbCarrier::Ring,
        ),
        (
            BaseState::Hovered,
            "the filled rail (rail_filled)",
            ThumbCarrier::Ring,
        ),
        (BaseState::Pressed, "the mount", ThumbCarrier::Both),
        (
            BaseState::Pressed,
            "the groove (rail_unfilled)",
            ThumbCarrier::Ring,
        ),
        (
            BaseState::Pressed,
            "the filled rail (rail_filled)",
            ThumbCarrier::Ring,
        ),
        // Disabled: the de-accented rail is the one background the ring
        // cannot cross, and the raised face is what carries it.
        (BaseState::Disabled, "the mount", ThumbCarrier::Ring),
        (
            BaseState::Disabled,
            "the groove (rail_unfilled)",
            ThumbCarrier::Ring,
        ),
        (
            BaseState::Disabled,
            "the filled rail (rail_filled)",
            ThumbCarrier::Face,
        ),
        // Invalid shares Rest, including where its visibility comes from.
        (BaseState::Invalid, "the mount", ThumbCarrier::Both),
        (
            BaseState::Invalid,
            "the groove (rail_unfilled)",
            ThumbCarrier::Both,
        ),
        (
            BaseState::Invalid,
            "the filled rail (rail_filled)",
            ThumbCarrier::Face,
        ),
    ];
    for (base, name, wanted) in cases {
        let s = resolve_style(state(*base), p.surface_overlay, &p);
        let (_, bg) = surfaces_behind_thumb(&s, p.surface_overlay)
            .into_iter()
            .find(|(label, _)| *label == *name)
            .unwrap_or_else(|| panic!("{base:?} has no surface named {name}"));
        assert_eq!(
            thumb_carrier(&s, bg),
            *wanted,
            "{base:?} thumb on {name} at {:.4}",
            thumb_distinguishable_ratio(&s, bg)
        );
    }
}

#[test]
fn the_rejected_disabled_pairing_still_measures_as_rejected() {
    // The pairing this design refuses, kept on the record: a disabled
    // thumb's ring over a rail that still wore the accent.
    let p = palette();
    assert_ratio(
        contrast::contrast_ratio(p.text_disabled, p.accent_fill),
        1.4280,
        "text.disabled on an accented rail — the rejected pairing",
    );
    assert!(
        !contrast::meets_chrome_contrast(p.text_disabled, p.accent_fill),
        "and it is below the floor, which is why the rail loses its accent"
    );
}

#[test]
fn the_disabled_thumb_is_distinguishable_on_every_polarity() {
    // The state the design had to be built around.
    let p = palette();
    let s = resolve_style(state(BaseState::Disabled), p.surface_overlay, &p);
    for (name, bg) in surfaces_behind_thumb(&s, p.surface_overlay) {
        assert!(
            thumb_is_distinguishable(&s, bg),
            "disabled thumb on {name}: {:.4}",
            thumb_distinguishable_ratio(&s, bg)
        );
    }
    assert_ratio(
        contrast::contrast_ratio(s.thumb_border, p.surface_overlay),
        5.2686,
        "disabled ring on the mount",
    );
    assert_ratio(
        contrast::contrast_ratio(s.thumb_border, p.surface_base),
        5.9165,
        "disabled ring on base",
    );
    assert_ratio(
        contrast::contrast_ratio(s.thumb_border, s.rail_unfilled),
        4.2510,
        "disabled ring on the de-accented groove",
    );
}

#[test]
fn the_disabled_thumb_face_separates_from_the_disabled_rail() {
    let p = palette();
    let s = resolve_style(state(BaseState::Disabled), p.surface_overlay, &p);
    assert!(
        contrast::meets_chrome_contrast(s.thumb_fill, s.rail_filled),
        "flat thumb face against the de-accented rail: {:.4}",
        contrast::contrast_ratio(s.thumb_fill, s.rail_filled)
    );
}

#[test]
fn the_fill_on_fill_measurement_is_recorded_below_the_floor_and_pinned_there() {
    // The pair §3.5 never prints. It measures below the chrome floor, and
    // the module docs explain why that is not a failure: both rails are
    // interior to one control, neither is a perimeter. The number is
    // pinned so a future palette retune that moved it further down shows
    // up here rather than silently.
    let p = palette();
    let measured = filled_on_unfilled(&p);
    assert_ratio(
        measured,
        FILLED_ON_UNFILLED_RATIO,
        "filled rail / unfilled rail",
    );
    assert_ratio(
        measured,
        2.9768,
        "filled rail / unfilled rail, from raw sRGB",
    );
    assert!(
        measured < contrast::MIN_CHROME_CONTRAST,
        "the recorded value is {measured:.4}, below the 3.0 floor — see the module docs"
    );
}

#[test]
fn the_value_position_is_carried_by_the_thumb_not_the_fill_boundary() {
    // The reason the sub-floor interior measurement is not load-bearing:
    // the resting thumb's ring clears the unfilled rail it sits against.
    let p = palette();
    let ring = resolve_style(state(BaseState::Rest), p.surface_overlay, &p).thumb_border;
    assert!(
        contrast::meets_chrome_contrast(ring, p.surface_active),
        "resting ring against the unfilled rail: {:.4}",
        contrast::contrast_ratio(ring, p.surface_active)
    );
    assert_ratio(
        contrast::contrast_ratio(p.accent_border, p.surface_active),
        4.7922,
        "the resting ring against the unfilled rail",
    );
}

#[test]
fn the_hovered_thumb_face_separates_from_the_filled_rail_by_its_ring_not_its_fill() {
    // The hovered face IS `accent.fill` on an `accent.fill` rail, so the
    // fill cannot carry it. The `text.strong` ring does, and that is what
    // the previous two tests measure.
    let p = palette();
    let s = resolve_style(state(BaseState::Hovered), p.surface_overlay, &p);
    assert_eq!(
        s.thumb_fill, s.rail_filled,
        "the face coincides with the rail"
    );
    assert!(
        contrast::meets_chrome_contrast(s.thumb_border, s.rail_filled),
        "and the ring carries the separation: {:.4}",
        contrast::contrast_ratio(s.thumb_border, s.rail_filled)
    );
}

// ---------------------------------------------------------------------------
// 3. Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_track_is_the_spec_thickness_centred_in_the_box() {
    let track = track_rect(bounds());
    assert_eq!(track.height, TRACK_HEIGHT, "4px rail");
    assert_eq!(track.width, 200.0, "full width");
    assert_eq!(track.x, 10.0, "flush left");
    assert!(
        (track.y - 110.0).abs() < 1e-6,
        "centred in a 24px box: {}",
        track.y
    );
}

#[test]
fn the_thumb_is_16_and_centred_on_the_rail() {
    let track = track_rect(bounds());
    let thumb = thumb_rect(track, 0.5);
    assert_eq!(thumb.width, THUMB_SIZE, "16px");
    assert_eq!(thumb.height, THUMB_SIZE, "16px square");
    let rail_centre = track.y + TRACK_HEIGHT / 2.0;
    let thumb_centre = thumb.y + thumb.height / 2.0;
    assert!(
        (rail_centre - thumb_centre).abs() < 1e-6,
        "the thumb straddles the rail"
    );
}

#[test]
fn the_hit_target_is_24_and_concentric_with_the_thumb() {
    let track = track_rect(bounds());
    for f in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let thumb = thumb_rect(track, f);
        let hit = hit_rect(track, f);
        assert_eq!(hit.width, THUMB_HIT_SIZE, "24px target");
        assert_eq!(hit.height, THUMB_HIT_SIZE, "24px target");
        assert!(
            (thumb.x + thumb.width / 2.0 - (hit.x + hit.width / 2.0)).abs() < 1e-4,
            "concentric at {f}"
        );
        assert!(
            (thumb.y + thumb.height / 2.0 - (hit.y + hit.height / 2.0)).abs() < 1e-4,
            "concentric at {f}"
        );
        assert!(
            hit.contains(Point::new(thumb.x, thumb.y)),
            "the target contains the face at {f}"
        );
    }
}

#[test]
fn the_hit_target_is_at_least_the_minimum_target_size() {
    // Both sides are constants, so the guarantee is proved at compile time
    // rather than spent on a runtime branch that can never go the other way.
    const _: () = assert!(THUMB_HIT_SIZE >= size::HIT_TARGET_MIN);
    const _: () = assert!(THUMB_HIT_SIZE > THUMB_SIZE);
}

#[test]
fn the_thumb_stays_inside_the_track_at_both_ends() {
    // Contained travel: a slider flush against a panel edge never paints
    // outside itself.
    let track = track_rect(bounds());
    let left = thumb_rect(track, 0.0);
    let right = thumb_rect(track, 1.0);
    assert!(
        left.x >= track.x - 1e-4,
        "left end stays inside: {} vs {}",
        left.x,
        track.x
    );
    assert!(
        right.x + right.width <= track.x + track.width + 1e-4,
        "right end stays inside: {} vs {}",
        right.x + right.width,
        track.x + track.width
    );
}

#[test]
fn thumb_travel_is_the_track_width_less_the_thumb() {
    let track = track_rect(bounds());
    assert_eq!(thumb_travel(track), 200.0 - THUMB_SIZE);
    assert_eq!(
        thumb_center_x(track, 1.0) - thumb_center_x(track, 0.0),
        thumb_travel(track),
        "the centre travels exactly the travel"
    );
}

#[test]
fn a_track_narrower_than_the_thumb_collapses_to_zero_travel() {
    let narrow = Rectangle::new(Point::ORIGIN, iced::Size::new(8.0, TRACK_HEIGHT));
    assert_eq!(thumb_travel(narrow), 0.0, "not negative");
    assert_eq!(
        thumb_center_x(narrow, 1.0),
        thumb_center_x(narrow, 0.0),
        "and nowhere to go"
    );
}

#[test]
fn fraction_at_x_inverts_thumb_center_x() {
    let track = track_rect(bounds());
    for f in [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let x = thumb_center_x(track, f);
        assert!(
            (fraction_at_x(track, x) - f).abs() < 1e-5,
            "centre at {f} must read back as {f}"
        );
    }
}

#[test]
fn fraction_at_x_clamps_a_pointer_that_left_the_track() {
    let track = track_rect(bounds());
    assert_eq!(fraction_at_x(track, -500.0), 0.0, "far left");
    assert_eq!(fraction_at_x(track, 5_000.0), 1.0, "far right");
    assert_eq!(
        fraction_at_x(track, f32::NAN),
        0.0,
        "a NaN x is not a position"
    );
    assert_eq!(fraction_at_x(track, f32::NEG_INFINITY), 0.0, "-inf");
    assert_eq!(fraction_at_x(track, f32::INFINITY), 1.0, "+inf");
}

#[test]
fn fraction_at_x_on_a_zero_travel_track_is_zero_not_a_division_by_zero() {
    let narrow = Rectangle::new(Point::ORIGIN, iced::Size::new(4.0, TRACK_HEIGHT));
    assert_eq!(fraction_at_x(narrow, 2.0), 0.0);
    assert!(fraction_at_x(narrow, 2.0).is_finite());
}

#[test]
fn thumb_center_x_clamps_an_out_of_range_fraction() {
    let track = track_rect(bounds());
    assert_eq!(
        thumb_center_x(track, -1.0),
        thumb_center_x(track, 0.0),
        "left of the start"
    );
    assert_eq!(
        thumb_center_x(track, 2.0),
        thumb_center_x(track, 1.0),
        "right of the end"
    );
    assert_eq!(
        thumb_center_x(track, f32::NAN),
        thumb_center_x(track, 0.0),
        "NaN rests at the start"
    );
}

#[test]
fn thumb_center_x_is_monotonic_in_the_fraction() {
    let track = track_rect(bounds());
    let mut last = f32::MIN;
    for i in 0..=200 {
        let f = i as f32 / 200.0;
        let x = thumb_center_x(track, f);
        assert!(x >= last, "the centre must not move backwards at {f}");
        last = x;
    }
}

// ---------------------------------------------------------------------------
// 4. Layers
// ---------------------------------------------------------------------------

#[test]
fn the_rail_layer_covers_the_track_with_the_pill_radius_and_a_hairline() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    let layer = rail_layer(track, &s);
    assert_eq!(layer.bounds, track, "the rail is the track");
    assert_eq!(layer.fill, p.surface_active);
    assert_eq!(layer.border.color, p.border_subtle);
    assert_eq!(layer.border.width, RAIL_BORDER_WIDTH, "1px");
    assert_eq!(layer.border.radius, TRACK_RADIUS.into(), "pill");
}

#[test]
fn there_is_no_filled_layer_at_the_very_start() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    assert!(
        filled_layer(track, &s, 0.0).is_none(),
        "nothing is filled at 0.0"
    );
    assert!(filled_layer(track, &s, -0.5).is_none(), "nor left of it");
    assert!(
        filled_layer(track, &s, f32::NAN).is_none(),
        "nor for a NaN fraction"
    );
}

#[test]
fn the_filled_layer_runs_to_the_thumb_centre() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    for f in [0.1f32, 0.5, 1.0] {
        let layer = filled_layer(track, &s, f).expect("a fill above zero");
        assert_eq!(layer.bounds.x, track.x, "starts at the track");
        assert!(
            (layer.bounds.width - (thumb_center_x(track, f) - track.x)).abs() < 1e-4,
            "ends at the thumb centre at {f}"
        );
        assert_eq!(layer.fill, p.accent_fill);
        assert_eq!(
            layer.border.width, 0.0,
            "the fill carries no stroke of its own"
        );
    }
}

#[test]
fn the_filled_layer_at_one_spans_the_whole_track_less_the_half_thumb() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    let full = filled_layer(track, &s, 1.0).unwrap();
    assert!(
        (full.bounds.width - (track.width - THUMB_SIZE / 2.0)).abs() < 1e-4,
        "full travel: {}",
        full.bounds.width
    );
}

#[test]
fn the_thumb_layer_carries_the_resolved_face_and_ring() {
    let p = palette();
    let s = resolve_style(state(BaseState::Hovered), p.surface_overlay, &p);
    let track = track_rect(bounds());
    let layer = thumb_layer(track, &s, 0.5);
    assert_eq!(layer.bounds, thumb_rect(track, 0.5));
    assert_eq!(layer.fill, p.accent_fill);
    assert_eq!(layer.border.color, p.text_strong);
    assert_eq!(layer.border.width, THUMB_RING_WIDTH);
}

#[test]
fn tick_marks_are_one_per_graduation_first_at_zero_last_at_one() {
    assert_eq!(tick_fractions(0), Vec::<f32>::new(), "none asked");
    assert_eq!(tick_fractions(1), Vec::<f32>::new(), "one is not a scale");
    assert_eq!(tick_fractions(2), vec![0.0, 1.0], "two are the ends");
    let five = tick_fractions(5);
    assert_eq!(five.len(), 5);
    assert_eq!(five[0], 0.0);
    assert_eq!(five[4], 1.0);
    assert!((five[2] - 0.5).abs() < 1e-6, "evenly spaced: {:?}", five);
}

#[test]
fn tick_layers_are_2_by_4_and_sit_on_the_rail() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    let ticks = tick_layers(track, &s, 5);
    assert_eq!(ticks.len(), 5);
    for tick in &ticks {
        assert_eq!(tick.bounds.width, TICK_WIDTH, "2px thick");
        assert_eq!(tick.bounds.height, TICK_HEIGHT, "4px tall");
        assert_eq!(tick.fill, p.border_strong);
        assert_eq!(tick.border.width, 0.0, "a tick is a fill, not a stroke");
        assert!(
            (tick.bounds.y - track.y).abs() < 1e-6,
            "flush with the rail top"
        );
    }
    assert!(
        ticks.windows(2).all(|w| w[0].bounds.x < w[1].bounds.x),
        "ticks read left to right"
    );
}

#[test]
fn a_single_tick_produces_no_layers() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    assert!(tick_layers(track, &s, 0).is_empty());
    assert!(
        tick_layers(track, &s, 1).is_empty(),
        "one graduation says nothing"
    );
}

#[test]
fn no_focus_ring_layer_is_due_without_focus() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    assert!(focus_ring_layer(track, &s, 0.5).is_none());
}

#[test]
fn the_focus_ring_is_drawn_on_the_hit_box_not_the_face() {
    let p = palette();
    let s = resolve_style(focused(BaseState::Rest), p.surface_overlay, &p);
    let track = track_rect(bounds());
    let ring = focus_ring_layer(track, &s, 0.5).expect("a focused slider draws a ring");
    assert_eq!(ring.bounds, hit_rect(track, 0.5), "on the 24px target");
    assert_eq!(
        ring.fill,
        Color::TRANSPARENT,
        "the ring frames, it does not fill"
    );
    assert_eq!(ring.border.width, FOCUS_RING_WIDTH);
}

#[test]
fn the_focus_ring_leaves_a_gap_clear_of_the_thumb() {
    // A ring drawn on the face would read as a second border on a thumb
    // that already has one. The 24px hit box puts its edge 4px out from
    // the face on each side, and the 2px stroke is drawn inward from
    // that edge, so the clear gap between the ring's inner edge and the
    // thumb's face is exactly `2px` — `(24 - 16) / 2 - 2`.
    //
    // That is the whole gap the tokens allow. Widening it would mean
    // either a bigger hit box than `size.hit_target_min` provides or a
    // thinner ring than `border.focus` is, and neither is this resolver's
    // to decide. Two pixels of mount showing between face and ring is
    // enough to read as a separate indicator, which is what the ring is
    // for.
    let track = track_rect(bounds());
    let thumb = thumb_rect(track, 0.5);
    let ring = hit_rect(track, 0.5);
    let gap = (thumb.x - ring.x) - THUMB_RING_WIDTH;
    assert_eq!(gap, 2.0, "the gap the two tokens produce");
    assert!(
        ring.contains(Point::new(thumb.x, thumb.y)),
        "and the ring still surrounds the face"
    );
    assert!(
        !thumb.contains(Point::new(ring.x, ring.y)),
        "without touching it"
    );
}

// ---------------------------------------------------------------------------
// 5. Chrome classification
// ---------------------------------------------------------------------------

#[test]
fn the_groove_outline_is_a_resting_divider_and_the_rest_is_judged() {
    let p = palette();
    assert_eq!(
        rail_border_chrome_class(p.border_subtle, &p),
        ChromeClass::RestingDivider,
        "the groove is not an indicator"
    );
    assert_eq!(
        rail_border_chrome_class(p.border_strong, &p),
        ChromeClass::Judged,
        "anything else is active chrome"
    );
    assert_eq!(
        rail_border_chrome_class(p.accent_border, &p),
        ChromeClass::Judged
    );
}

#[test]
fn the_thumb_ring_is_always_judged_including_when_disabled() {
    // The disabled ring is the only thing separating a dead thumb from
    // the rail, so it is held to the floor rather than waved through.
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        let s = resolve_style(state(base), p.surface_overlay, &p);
        assert_eq!(
            thumb_chrome_class(s.thumb_border, &p),
            ChromeClass::Judged,
            "{base:?} thumb ring is measured"
        );
    }
}

#[test]
fn a_resting_divider_classification_is_exempt_and_a_judged_one_is_not() {
    let p = palette();
    assert!(
        rail_border_chrome_class(p.border_subtle, &p).is_exempt(),
        "the divider owes no measurement"
    );
    assert!(
        !thumb_chrome_class(p.accent_border, &p).is_exempt(),
        "the ring does"
    );
}

#[test]
fn every_judged_slider_pair_clears_the_floor() {
    // The audit, run in miniature over the pairs the gallery will emit:
    // every non-exempt perimeter against every mount it can land on.
    let p = palette();
    let mounts = [
        ("base", p.surface_base),
        ("raised", p.surface_raised),
        ("overlay", p.surface_overlay),
        ("hover", p.surface_hover),
        ("active", p.surface_active),
    ];
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        let s = resolve_style(state(base), p.surface_overlay, &p);
        for (name, mount) in mounts {
            let class = thumb_chrome_class(s.thumb_border, &p);
            assert!(
                contrast::meets_chrome_contrast_as(class, s.thumb_border, mount),
                "{base:?} thumb ring on {name}: {:.4}",
                contrast::contrast_ratio(s.thumb_border, mount)
            );
        }
    }
}
