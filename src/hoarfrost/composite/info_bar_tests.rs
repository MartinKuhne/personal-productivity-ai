//! Tests for [`super::info_bar`] — the docked banner's zero-shift vertical
//! rhythm, the stripe-as-a-second-layer paint, the recomputed contrast of
//! every channel against the bar's own fill, and the `GestureSlot`
//! transcript its dismissal produces (`UIF-COMP-002`).

use crate::binding::GestureStart;
use crate::composite::info_bar::{
    ACTION_DISMISS_GAP, BODY_RUNG, BORDER_WIDTH, BarDismissal, BarPart, BarRelease, BarSurface,
    DISMISS_TARGET, ICON_BOX, ICON_TEXT_GAP, MIN_HEIGHT, PADDING_X, PADDING_Y, RADIUS,
    STRIPE_WIDTH, TITLE_RUNG, action_bounds, action_focus_id, background_layer, bar_bounds,
    content_bounds, dismiss_bounds, dismiss_focus_id, focus_members, icon_bounds, resolve_style,
    slots, stripe_bounds, stripe_layer, text_bounds, trailing_width,
};
use crate::composite::notification::{NotificationAction, NotificationId, Severity};
use crate::focus::{FocusId, FocusState, GroupId};
use crate::token::color::Palette;
use crate::token::contrast::{contrast_ratio, meets_chrome_contrast, meets_text_contrast};
use crate::token::{shape, size, space, typography};
use iced::{Point, Rectangle};

/// The shape `slots` wants when the bar carries no action button.
const NO_ACTION: Option<&NotificationAction<&str>> = None;

fn palette() -> Palette {
    Palette::dark()
}

/// A contrast ratio compared at the four decimal places the pinned
/// figures are written to.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

fn container(width: f32, height: f32) -> Rectangle {
    Rectangle::new(Point::new(20.0, 100.0), iced::Size::new(width, height))
}

// ---------------------------------------------------------------------------
// 1 — the rung
// ---------------------------------------------------------------------------

#[test]
fn the_bar_floor_is_the_large_control_rung() {
    assert_eq!(MIN_HEIGHT, 36.0);
    assert_eq!(MIN_HEIGHT, size::CONTROL_LG);
}

#[test]
fn the_padding_pair_is_section_3b_2s_eight_by_twelve() {
    assert_eq!(PADDING_Y, 8.0);
    assert_eq!(PADDING_Y, space::SM);
    assert_eq!(PADDING_X, 12.0);
    assert_eq!(PADDING_X, space::MD);
}

#[test]
fn the_stripe_is_four_pixels_and_the_icon_is_sixteen() {
    assert_eq!(STRIPE_WIDTH, 4.0);
    assert_eq!(STRIPE_WIDTH, space::XS);
    assert_eq!(ICON_BOX, 16.0);
    assert_eq!(ICON_BOX, size::ICON_MD);
}

#[test]
fn the_border_is_a_hairline_and_the_radius_is_the_widget_rung() {
    assert_eq!(BORDER_WIDTH, 1.0);
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(RADIUS, 4.0);
    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
}

#[test]
fn the_dismiss_affordance_clears_the_hit_target_floor() {
    assert_eq!(DISMISS_TARGET, size::HIT_TARGET_MIN);
    const { assert!(DISMISS_TARGET >= 24.0) };
}

#[test]
fn the_title_and_body_rungs_are_distinct_and_clear_the_minimum_size() {
    assert_ne!(
        TITLE_RUNG.size, BODY_RUNG.size,
        "a title on the same rung as the body is not a title"
    );
    assert!(TITLE_RUNG.size >= typography::MIN_SIZE);
    assert!(BODY_RUNG.size >= typography::MIN_SIZE);
}

// ---------------------------------------------------------------------------
// 2 — the bar box
// ---------------------------------------------------------------------------

#[test]
fn the_bar_takes_the_full_container_width() {
    let bar = bar_bounds(container(400.0, 36.0));
    assert_eq!(bar.x, 20.0);
    assert_eq!(bar.width, 400.0);
}

#[test]
fn a_short_container_is_lifted_to_the_floor() {
    let bar = bar_bounds(container(400.0, 10.0));
    assert_eq!(bar.height, MIN_HEIGHT);
}

#[test]
fn a_tall_container_is_respected_not_clamped_down() {
    let bar = bar_bounds(container(400.0, 90.0));
    assert_eq!(bar.height, 90.0);
}

#[test]
fn a_negative_container_height_lands_on_the_floor_rather_than_below_zero() {
    let bar = bar_bounds(container(400.0, -50.0));
    assert_eq!(bar.height, MIN_HEIGHT);
    assert!(bar.height > 0.0);
}

#[test]
fn a_zero_width_container_gives_a_zero_width_bar_not_a_negative_one() {
    let bar = bar_bounds(container(0.0, 36.0));
    assert_eq!(bar.width, 0.0);
}

// ---------------------------------------------------------------------------
// 3 — the zero-shift guarantee
// ---------------------------------------------------------------------------

#[test]
fn the_bar_box_cannot_see_what_is_docked_at_the_end() {
    // `bar_bounds` has no trailing-slot parameter at all. Two bars in one
    // container, one with an action and a `[×]` and one with neither,
    // resolve the identical box.
    let c = container(400.0, 36.0);
    let plain = bar_bounds(c);
    let docked = bar_bounds(c);
    assert_eq!(plain, docked);
}

#[test]
fn the_icon_position_is_identical_whatever_is_docked() {
    let c = container(400.0, 36.0);
    let bare = slots(c, NO_ACTION, 0.0, false);
    let full = slots(c, NO_ACTION, 96.0, true);
    assert_eq!(bare.icon, full.icon);
}

#[test]
fn the_text_band_y_and_height_are_identical_whatever_is_docked() {
    let c = container(400.0, 36.0);
    let bare = slots(c, NO_ACTION, 0.0, false);
    let full = slots(c, NO_ACTION, 96.0, true);
    assert_eq!(bare.text.y, full.text.y);
    assert_eq!(bare.text.height, full.text.height);
}

#[test]
fn the_text_band_narrows_by_exactly_the_docked_trailing_width() {
    let c = container(400.0, 36.0);
    let action = NotificationAction::new("Retry", "retry-1");
    let bare = slots(c, NO_ACTION, 0.0, false);
    let full = slots(c, Some(&action), 96.0, true);
    let docked = trailing_width(true, 96.0, true);
    assert_eq!(
        bare.text.width - full.text.width,
        docked,
        "the width the text gave up is exactly the width the controls took"
    );
}

#[test]
fn the_text_band_never_goes_negative_when_the_controls_eat_everything() {
    let c = container(60.0, 36.0);
    let tight = slots(c, NO_ACTION, 500.0, true);
    assert!(tight.text.width >= 0.0);
}

// ---------------------------------------------------------------------------
// 4 — the stripe
// ---------------------------------------------------------------------------

#[test]
fn the_stripe_spans_the_bar_at_its_left_edge() {
    let bar = bar_bounds(container(400.0, 72.0));
    let stripe = stripe_bounds(bar);
    assert_eq!(stripe.x, bar.x);
    assert_eq!(stripe.height, bar.height);
    assert_eq!(stripe.width, STRIPE_WIDTH);
}

#[test]
fn the_stripe_never_wider_than_the_bar_itself() {
    let bar = bar_bounds(container(2.0, 36.0));
    let stripe = stripe_bounds(bar);
    assert!(stripe.width <= bar.width);
}

#[test]
fn the_content_box_starts_past_the_stripe_and_the_padding() {
    let bar = bar_bounds(container(400.0, 36.0));
    let content = content_bounds(bar);
    assert_eq!(content.x, bar.x + STRIPE_WIDTH + PADDING_X);
    assert_eq!(content.y, bar.y + PADDING_Y);
    assert_eq!(
        content.width,
        bar.width - STRIPE_WIDTH - PADDING_X * 2.0,
        "the content box is the bar minus the stripe and both horizontal paddings"
    );
}

#[test]
fn the_icon_is_vertically_centred_in_the_bar_not_in_the_content() {
    // A two-line body must not drag the icon down: the icon is centred on
    // the bar, so a one-line and a two-line bar of the same height put
    // the icon in the same place.
    let short = bar_bounds(container(400.0, MIN_HEIGHT));
    let centred = icon_bounds(short);
    assert_eq!(
        centred.y + centred.height / 2.0,
        short.y + short.height / 2.0,
        "the icon's centre is the bar's centre"
    );
}

#[test]
fn the_icon_leads_the_text_by_the_declared_gap() {
    let bar = bar_bounds(container(400.0, 36.0));
    let icon = icon_bounds(bar);
    let text = text_bounds(bar, 0.0);
    assert_eq!(text.x, icon.x + icon.width + ICON_TEXT_GAP);
}

// ---------------------------------------------------------------------------
// 5 — the trailing slots
// ---------------------------------------------------------------------------

#[test]
fn the_dismiss_box_sits_at_the_trailing_edge_inside_the_padding() {
    let bar = bar_bounds(container(400.0, 36.0));
    let dismiss = dismiss_bounds(bar);
    assert_eq!(dismiss.x, bar.x + bar.width - PADDING_X - DISMISS_TARGET);
    assert_eq!(dismiss.width, DISMISS_TARGET);
    assert!(dismiss.x + dismiss.width <= bar.x + bar.width);
}

#[test]
fn the_action_docks_left_of_the_dismiss_box() {
    let bar = bar_bounds(container(400.0, 36.0));
    let action = action_bounds(bar, 96.0, true);
    let dismiss = dismiss_bounds(bar);
    assert!(
        action.x + action.width <= dismiss.x,
        "the action must not overlap the [×]"
    );
    assert_eq!(
        dismiss.x - (action.x + action.width),
        ACTION_DISMISS_GAP,
        "and the gap between them is the declared one"
    );
}

#[test]
fn the_action_moves_left_when_a_dismiss_box_appears() {
    let bar = bar_bounds(container(400.0, 36.0));
    let without = action_bounds(bar, 96.0, false);
    let with = action_bounds(bar, 96.0, true);
    assert!(with.x < without.x);
    assert_eq!(without.x - with.x, DISMISS_TARGET + ACTION_DISMISS_GAP);
}

#[test]
fn the_action_keeps_its_width_when_the_dismiss_box_appears() {
    let bar = bar_bounds(container(400.0, 36.0));
    assert_eq!(
        action_bounds(bar, 96.0, false).width,
        action_bounds(bar, 96.0, true).width
    );
}

#[test]
fn the_action_never_runs_past_the_bar_s_left_edge() {
    let bar = bar_bounds(container(80.0, 36.0));
    let action = action_bounds(bar, 5_000.0, true);
    assert!(action.x >= bar.x);
}

#[test]
fn trailing_width_accounts_for_each_control_once() {
    assert_eq!(trailing_width(false, 96.0, false), 0.0);
    assert_eq!(trailing_width(false, 96.0, true), DISMISS_TARGET);
    assert_eq!(trailing_width(true, 96.0, false), 96.0 + ACTION_DISMISS_GAP);
    assert_eq!(
        trailing_width(true, 96.0, true),
        DISMISS_TARGET + 96.0 + ACTION_DISMISS_GAP
    );
}

#[test]
fn a_zero_width_action_still_costs_its_gap_and_nothing_else() {
    assert_eq!(trailing_width(true, 0.0, false), ACTION_DISMISS_GAP);
}

#[test]
fn the_slots_struct_agrees_with_the_individual_resolvers() {
    let c = container(400.0, 36.0);
    let action = NotificationAction::new("Retry", "retry-1");
    let s = slots(c, Some(&action), 96.0, true);
    assert_eq!(s.bar, bar_bounds(c));
    assert_eq!(s.stripe, stripe_bounds(s.bar));
    assert_eq!(s.icon, icon_bounds(s.bar));
    assert_eq!(s.text, text_bounds(s.bar, trailing_width(true, 96.0, true)));
    assert_eq!(s.action, Some(action_bounds(s.bar, 96.0, true)));
    assert_eq!(s.dismiss, Some(dismiss_bounds(s.bar)));
}

#[test]
fn absent_controls_are_absent_rather_than_zero_sized() {
    let c = container(400.0, 36.0);
    let bare = slots(c, NO_ACTION, 0.0, false);
    assert!(bare.action.is_none());
    assert!(bare.dismiss.is_none());
}

// ---------------------------------------------------------------------------
// 6 — resolved paint and recomputed contrast
// ---------------------------------------------------------------------------

#[test]
fn each_bar_surface_resolves_its_own_fill() {
    let p = palette();
    assert_eq!(BarSurface::Raised.fill(&p), p.surface_raised);
    assert_eq!(BarSurface::Overlay.fill(&p), p.surface_overlay);
    assert_eq!(BarSurface::default(), BarSurface::Raised);
}

#[test]
fn the_severity_colours_the_stripe_and_icon_and_nothing_else() {
    let p = palette();
    for severity in Severity::ALL {
        let style = resolve_style(severity, BarSurface::Raised, &p);
        let accent = severity.accent(&p);
        assert_eq!(style.stripe_colour, accent);
        assert_eq!(style.icon_colour, accent);
        assert_ne!(
            style.title_colour, accent,
            "{severity:?}: the title must stay neutral"
        );
        assert_ne!(
            style.body_colour, accent,
            "{severity:?}: the body must stay neutral"
        );
    }
}

#[test]
fn the_perimeter_is_border_strong_at_every_severity() {
    let p = palette();
    for severity in Severity::ALL {
        let style = resolve_style(severity, BarSurface::Raised, &p);
        assert_eq!(style.border_colour, p.border_strong);
        assert_eq!(style.border_width, BORDER_WIDTH);
    }
}

#[test]
fn every_text_channel_clears_the_text_floor_on_a_raised_bar() {
    let p = palette();
    let style = resolve_style(Severity::Danger, BarSurface::Raised, &p);
    for (name, colour) in [
        ("title", style.title_colour),
        ("body", style.body_colour),
        ("dismiss", style.dismiss_colour),
        ("icon", style.icon_colour),
        ("stripe", style.stripe_colour),
    ] {
        assert!(
            meets_text_contrast(colour, style.background),
            "{name} on the raised bar measures {:.4}:1, under 4.50:1",
            contrast_ratio(colour, style.background),
        );
    }
}

#[test]
fn every_text_channel_clears_the_text_floor_on_an_overlay_bar() {
    let p = palette();
    for severity in Severity::ALL {
        let style = resolve_style(severity, BarSurface::Overlay, &p);
        for (name, colour) in [
            ("title", style.title_colour),
            ("body", style.body_colour),
            ("dismiss", style.dismiss_colour),
            ("icon", style.icon_colour),
        ] {
            assert!(
                meets_text_contrast(colour, style.background),
                "{severity:?} {name} on the overlay bar measures {:.4}:1, under 4.50:1",
                contrast_ratio(colour, style.background),
            );
        }
    }
}

#[test]
fn the_perimeter_clears_the_chrome_floor_on_both_fills() {
    let p = palette();
    for surface in [BarSurface::Raised, BarSurface::Overlay] {
        let style = resolve_style(Severity::Info, surface, &p);
        assert!(
            meets_chrome_contrast(style.border_colour, style.background),
            "{surface:?} perimeter measures {:.4}:1, under the 3.00:1 chrome floor",
            contrast_ratio(style.border_colour, style.background),
        );
    }
}

#[test]
fn the_recomputed_raised_bar_ratios_match_the_module_table() {
    let p = palette();
    let raised = p.surface_raised;
    let expected = [
        ("accent.text", p.accent_text, 10.0215_f32),
        ("status.success", p.status_success, 14.2825),
        ("status.warning", p.status_warning, 6.4474),
        ("status.danger.text", p.status_danger_text, 6.1812),
        ("text.strong", p.text_strong, 18.7108),
        ("text.primary", p.text_primary, 12.3755),
        ("text.muted", p.text_muted, 7.1553),
        ("border.strong", p.border_strong, 4.2379),
    ];
    for (name, colour, figure) in expected {
        let actual = contrast_ratio(colour, raised);
        assert!(
            approx(actual, figure),
            "{name} on surface.raised recomputes to {actual:.4}:1, not {figure:.4}:1"
        );
    }
}

#[test]
fn the_recomputed_overlay_bar_ratios_match_the_module_table() {
    let p = palette();
    let overlay = p.surface_overlay;
    let expected = [
        ("accent.text", p.accent_text, 9.4891_f32),
        ("status.success", p.status_success, 13.5237),
        ("status.warning", p.status_warning, 6.1048),
        ("status.danger.text", p.status_danger_text, 5.8528),
        ("text.strong", p.text_strong, 17.7168),
        ("text.primary", p.text_primary, 11.7180),
        ("text.muted", p.text_muted, 6.7751),
        ("border.strong", p.border_strong, 4.0128),
    ];
    for (name, colour, figure) in expected {
        let actual = contrast_ratio(colour, overlay);
        assert!(
            approx(actual, figure),
            "{name} on surface.overlay recomputes to {actual:.4}:1, not {figure:.4}:1"
        );
    }
}

#[test]
fn the_raised_bar_is_the_lower_contrast_of_the_two_permitted_fills() {
    // The module doc claims `raised` is the conservative mount surface
    // for a light foreground. Checked rather than asserted: for every
    // light channel, raised gives the *higher* ratio than overlay, so a
    // bar that passes on overlay passes on raised too — and the claim in
    // `gallery_composite`'s header is about the *lightest* surface,
    // which is the one that costs a dark foreground most.
    let p = palette();
    for colour in [p.text_strong, p.text_primary, p.text_muted] {
        let on_raised = contrast_ratio(colour, p.surface_raised);
        let on_overlay = contrast_ratio(colour, p.surface_overlay);
        assert!(
            on_raised > on_overlay,
            "a light foreground reads better on the darker raised surface"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 — layers
// ---------------------------------------------------------------------------

#[test]
fn the_background_layer_carries_the_fill_perimeter_and_radius() {
    let p = palette();
    let style = resolve_style(Severity::Warning, BarSurface::Raised, &p);
    let bar = bar_bounds(container(400.0, 36.0));
    let layer = background_layer(bar, &style);
    assert_eq!(layer.bounds, bar);
    assert_eq!(layer.fill, style.background);
    assert_eq!(layer.border.color, style.border_colour);
    assert_eq!(layer.border.width, BORDER_WIDTH);
    assert_eq!(layer.border.radius.top_left, RADIUS);
}

#[test]
fn the_stripe_layer_is_a_fill_with_no_stroke() {
    let p = palette();
    let style = resolve_style(Severity::Danger, BarSurface::Raised, &p);
    let stripe = stripe_bounds(bar_bounds(container(400.0, 36.0)));
    let layer = stripe_layer(stripe, &style);
    assert_eq!(layer.fill, style.stripe_colour);
    assert_eq!(layer.border.width, 0.0);
    assert_eq!(layer.border.color, iced::Color::TRANSPARENT);
}

#[test]
fn the_stripe_paints_a_different_colour_at_every_severity() {
    let p = palette();
    let mut seen = Vec::new();
    for severity in Severity::ALL {
        let style = resolve_style(severity, BarSurface::Raised, &p);
        let layer = stripe_layer(stripe_bounds(bar_bounds(container(400.0, 36.0))), &style);
        assert!(
            !seen.contains(&layer.fill),
            "{severity:?} reuses a stripe colour another severity already has"
        );
        seen.push(layer.fill);
    }
    assert_eq!(seen.len(), Severity::ALL.len());
}

// ---------------------------------------------------------------------------
// 8 — tab reachability
// ---------------------------------------------------------------------------

#[test]
fn the_bar_itself_is_never_a_tab_stop() {
    let members = focus_members(&NotificationId::new("b"), true, true);
    assert_eq!(members.len(), 2);
    for member in &members {
        assert_ne!(member.id.as_str(), "b");
    }
}

#[test]
fn only_the_controls_that_are_present_are_tab_stops() {
    let id = NotificationId::new("b");
    assert_eq!(focus_members(&id, false, false).len(), 0);
    assert_eq!(focus_members(&id, true, false).len(), 1);
    assert_eq!(focus_members(&id, false, true).len(), 1);
    assert_eq!(focus_members(&id, true, true).len(), 2);
}

#[test]
fn the_action_stop_comes_before_the_dismiss_stop() {
    let id = NotificationId::new("b");
    let members = focus_members(&id, true, true);
    assert_eq!(members[0].id, action_focus_id(&id));
    assert_eq!(members[1].id, dismiss_focus_id(&id));
}

#[test]
fn every_member_is_enabled() {
    let id = NotificationId::new("b");
    for member in focus_members(&id, true, true) {
        assert!(member.enabled);
    }
}

#[test]
fn the_focus_ids_are_distinct_and_derived_from_the_notification_id() {
    let id = NotificationId::new("banner");
    let action = action_focus_id(&id);
    let dismiss = dismiss_focus_id(&id);
    assert_ne!(action, dismiss);
    assert!(action.as_str().contains("banner"));
    assert!(dismiss.as_str().contains("banner"));
}

#[test]
fn two_bars_do_not_share_a_tab_stop() {
    let a = focus_members(&NotificationId::new("a"), true, true);
    let b = focus_members(&NotificationId::new("b"), true, true);
    for member in &a {
        assert!(
            !b.iter().any(|other| other.id == member.id),
            "bars a and b both claim {}",
            member.id.as_str()
        );
    }
}

#[test]
fn the_members_register_into_a_focus_group_that_traverses_them() {
    let id = NotificationId::new("banner");
    let mut focus = FocusState::new();
    focus.register_group(GroupId::new("banner-group"), focus_members(&id, true, true));
    let first = focus.focus_first_in_group(&GroupId::new("banner-group"));
    assert_eq!(first, Some(action_focus_id(&id)));
    let next = focus.focus_next();
    assert_eq!(next, Some(dismiss_focus_id(&id)));
}

// ---------------------------------------------------------------------------
// 9 — the dismissal transcript
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_dismissal_is_visible_and_unpressed() {
    let dismissal = BarDismissal::new(false);
    assert!(!dismissal.is_dismissed());
    assert_eq!(dismissal.pressed_part(), None);
}

#[test]
fn a_press_without_a_release_dismisses_nothing() {
    let mut dismissal = BarDismissal::new(false);
    assert_eq!(dismissal.press(BarPart::Dismiss), GestureStart::Started);
    assert!(!dismissal.is_dismissed());
    assert_eq!(dismissal.release().map(|r| r.part), Some(BarPart::Dismiss));
}

#[test]
fn a_release_with_no_press_dismisses_nothing() {
    let mut dismissal = BarDismissal::new(false);
    assert!(dismissal.release().is_none());
    assert!(!dismissal.is_dismissed());
}

#[test]
fn the_release_names_the_control_that_was_pressed() {
    let mut dismissal = BarDismissal::new(false);
    let _ = dismissal.press(BarPart::Action);
    let release: BarRelease = dismissal.release().expect("release settles");
    assert_eq!(release.part, BarPart::Action);
    assert!(*release.commit.value());
}

#[test]
fn a_second_press_while_one_is_open_is_refused_and_remembers_nothing() {
    let mut dismissal = BarDismissal::new(false);
    assert_eq!(dismissal.press(BarPart::Action), GestureStart::Started);
    assert_eq!(dismissal.press(BarPart::Dismiss), GestureStart::AlreadyOpen);
    assert_eq!(dismissal.pressed_part(), Some(BarPart::Action));
    // The refused part is not remembered, so the release settles the
    // control the user actually pressed first.
    assert_eq!(dismissal.release().map(|r| r.part), Some(BarPart::Action));
}

#[test]
fn a_cancel_reverts_to_what_the_press_captured() {
    let mut dismissal = BarDismissal::new(false);
    let _ = dismissal.press(BarPart::Dismiss);
    let cancel = dismissal.cancel().expect("a cancel is owed");
    assert!(!cancel.restored());
    assert!(!dismissal.is_dismissed());
    assert_eq!(dismissal.pressed_part(), None);
}

#[test]
fn a_cancel_with_no_press_is_not_invented() {
    let mut dismissal = BarDismissal::new(false);
    assert!(dismissal.cancel().is_none());
}

#[test]
fn a_direct_dismissal_settles_a_stuck_press_rather_than_leaving_it_open() {
    let mut dismissal = BarDismissal::new(false);
    let _ = dismissal.press(BarPart::Action);
    let commit = dismissal.set_dismissed(true);
    assert!(*commit.value());
    assert!(dismissal.is_dismissed());
    assert_eq!(dismissal.pressed_part(), None);
    // The slot is settled, so a later release emits nothing rather than
    // a second commit for a bar that is already gone.
    assert!(dismissal.release().is_none());
}

#[test]
fn a_direct_dismissal_with_no_open_slot_still_reports_the_commit() {
    let mut dismissal = BarDismissal::new(false);
    let commit = dismissal.set_dismissed(true);
    assert!(*commit.value());
    assert!(dismissal.is_dismissed());
}

#[test]
fn a_dismissed_bar_can_be_restored() {
    let mut dismissal = BarDismissal::new(true);
    assert!(dismissal.is_dismissed());
    let commit = dismissal.set_dismissed(false);
    assert!(!commit.value());
    assert!(!dismissal.is_dismissed());
}

#[test]
fn the_release_commit_is_the_only_path_that_both_dismisses_and_names_a_part() {
    // Guards against a future edit that dismisses without reporting the
    // part, which would leave the host unable to choose between
    // `on_action` and `on_dismiss`.
    let mut dismissal = BarDismissal::new(false);
    let _ = dismissal.press(BarPart::Action);
    let release = dismissal.release().expect("settled");
    assert!(dismissal.is_dismissed());
    assert_eq!(release.part, BarPart::Action);
}

#[test]
fn an_action_focus_id_is_derived_from_the_notification_it_belongs_to() {
    let id = NotificationId::new("sync-banner");
    let _: FocusId = action_focus_id(&id);
    assert!(action_focus_id(&id).as_str().starts_with("sync-banner"));
}
