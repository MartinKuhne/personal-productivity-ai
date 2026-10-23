//! Tests for `widget/button_model.rs` — the `UIF-ATOM-001a` declarative model.

use super::*;
use crate::binding::CancelRefusal;
use crate::token::space;

/// The layout one rung resolves to, through the same model path a real button uses.
fn layout_for(density: ButtonDensity) -> ResponsiveLayout {
    ButtonModel::new("Save").with_density(density).responsive()
}

// ---------------------------------------------------------------------------
// Construction and defaults
// ---------------------------------------------------------------------------

#[test]
fn a_new_model_is_a_live_standard_secondary_labelled_button() {
    let model = ButtonModel::new("Save");

    assert_eq!(model.label.as_deref(), Some("Save"));
    assert_eq!(model.leading_icon, None);
    assert_eq!(model.trailing_icon, None);
    assert_eq!(model.variant, ButtonVariant::Secondary);
    assert_eq!(model.density, ButtonDensity::Standard);
    assert!(!model.busy);
    assert!(!model.disabled);
    assert!(!model.cancellable);
    assert_eq!(model.tooltip, None);
}

#[test]
fn an_icon_only_model_has_no_label_and_carries_the_glyph_leading() {
    let model = ButtonModel::icon_only("X");

    assert_eq!(model.label, None);
    assert_eq!(
        model.leading_icon,
        Some(ButtonIcon::Glyph("X")),
        "the glyph goes in the leading slot, which is the one an icon-only button reads as itself"
    );
    assert_eq!(model.trailing_icon, None);
    assert_eq!(model.variant, ButtonVariant::Ghost);
}

#[test]
fn every_builder_returns_a_modified_copy_and_leaves_the_original_alone() {
    let original = ButtonModel::new("Save");

    assert_eq!(
        original.clone().with_variant(ButtonVariant::Danger).variant,
        ButtonVariant::Danger
    );
    assert_eq!(
        original
            .clone()
            .with_density(ButtonDensity::Compact)
            .density,
        ButtonDensity::Compact
    );
    assert_eq!(
        original
            .clone()
            .with_tooltip("Save this")
            .tooltip
            .as_deref(),
        Some("Save this")
    );
    assert!(original.clone().busy().busy);
    assert!(original.clone().cancellable().cancellable);
    assert!(original.clone().disabled().disabled);

    assert_eq!(
        original,
        ButtonModel::new("Save"),
        "the builders are pure: none of them reached back and changed the model they started from"
    );
}

// ---------------------------------------------------------------------------
// The Controlled half of the split
// ---------------------------------------------------------------------------

#[test]
fn controlled_wraps_a_shared_reference_rather_than_a_copy() {
    let model = ButtonModel::new("Save").busy();
    let controlled = model.controlled();

    // Reading through the wrapper sees the *same* value the host holds, and a
    // mutation the host makes is visible without rebuilding anything.
    assert_eq!(controlled.get(), &model);
    assert!(controlled.get().busy);

    // Projection stays inside the wrapper: `map` hands back a `Controlled` over
    // a field, never a bare reference the widget could hold past the model.
    let label = controlled.map(|model| &model.label);
    assert_eq!(label.get().as_deref(), Some("Save"));
}

// ---------------------------------------------------------------------------
// `tracks_action` — which models the gate is armed for
// ---------------------------------------------------------------------------

#[test]
fn tracks_action_is_true_for_busy_or_cancellable_and_false_for_neither() {
    assert!(
        ButtonModel::new("S").busy().tracks_action(),
        "a busy model has an action the gate must police"
    );
    assert!(
        ButtonModel::new("S").cancellable().tracks_action(),
        "a cancellable model will report back on what it dispatches"
    );
    assert!(ButtonModel::new("S").busy().cancellable().tracks_action());
    assert!(
        !ButtonModel::new("S").tracks_action(),
        "a synchronous command tracks nothing — arming a gate for it would latch forever \
         and the button would fire exactly once"
    );
}

// ---------------------------------------------------------------------------
// `pending` — the busy projection
// ---------------------------------------------------------------------------

#[test]
fn pending_projects_idle_and_indeterminate_without_reading_a_clock() {
    let idle = ButtonModel::new("S").pending();
    assert!(!idle.is_busy());
    assert_eq!(idle.progress(), None);
    assert_eq!(idle, ActionPending::idle());

    let busy = ButtonModel::new("S").busy().pending();
    assert!(busy.is_busy());
    assert_eq!(
        busy.progress(),
        None,
        "`busy` is a boolean, so the only honest projection is indeterminate"
    );
    assert_eq!(busy, ActionPending::indeterminate());
}

#[test]
fn pending_carries_the_cancellable_flag_whether_or_not_the_button_is_busy() {
    let cancellable_idle = ButtonModel::new("S").cancellable().pending();
    assert!(
        cancellable_idle.is_cancellable(),
        "the flag describes the action, not the moment, so it is set even at rest"
    );
    assert!(
        cancellable_idle.try_cancel().is_err(),
        "and a request against a resting action is still refused"
    );

    assert!(ButtonModel::new("S").busy().pending().try_cancel().is_err());
    assert_eq!(
        ButtonModel::new("S").busy().pending().try_cancel().err(),
        Some(CancelRefusal::NotCancellable),
        "busy without `cancellable` is the refusal that means 'this button has no stop affordance'"
    );
    assert!(
        ButtonModel::new("S")
            .busy()
            .cancellable()
            .pending()
            .try_cancel()
            .is_ok()
    );
}

// ---------------------------------------------------------------------------
// Icon-only and the tooltip obligation
// ---------------------------------------------------------------------------

#[test]
fn icon_only_tracks_the_rung_not_the_content() {
    let model = ButtonModel::new("Save");
    assert!(!model.is_icon_only(LabelMode::Full));
    assert!(
        model.is_icon_only(LabelMode::IconOnly),
        "a labelled button collapsed to icon-only is icon-only: the label is not on screen"
    );
}

#[test]
fn tooltip_is_required_on_the_collapsed_rung_and_suppressed_on_the_full_one() {
    let model = ButtonModel::new("Save").with_tooltip("Save the document");

    assert_eq!(
        model.tooltip_for(LabelMode::Full),
        None,
        "the label is already on screen — there is nothing to say twice"
    );
    assert_eq!(
        model.tooltip_for(LabelMode::IconOnly),
        Some("Save the document"),
        "the collapsed rung owes the label, and `UIF-ATOM-007` makes that mandatory"
    );
}

#[test]
fn a_collapsed_rung_with_no_tooltip_returns_none_so_the_caller_can_see_the_violation() {
    let model = ButtonModel::new("Save");
    assert_eq!(
        model.tooltip_for(LabelMode::IconOnly),
        None,
        "a collapsed rung with no tooltip must be distinguishable from one that has one — \
         `None` here is the violation, not an absence of interest"
    );
}

// ---------------------------------------------------------------------------
// Geometry the model reports
// ---------------------------------------------------------------------------

#[test]
fn the_spinner_slot_is_the_small_icon_rung() {
    assert_eq!(
        SPINNER_SLOT,
        size::ICON_SM,
        "the reserved slot is `size.icon.sm`; `size.spinner.sm` is the same rung and the \
         glyph that fills it is `UIF-ATOM-009`'s deliverable, not this increment's"
    );
    assert_eq!(ButtonIcon::Glyph("X").width(), SPINNER_SLOT);
    assert_eq!(ButtonIcon::Reserved.width(), SPINNER_SLOT);
}

#[test]
fn the_two_rung_floors_are_token_derived_and_strictly_ordered() {
    let model = ButtonModel::new("Save");

    assert_eq!(model.collapsed_min_width(), 2.0 * space::SM + SPINNER_SLOT);
    assert_eq!(model.expanded_min_width(), 2.0 * space::MD + SPINNER_SLOT);
    assert!(
        model.collapsed_min_width() < model.expanded_min_width(),
        "the collapsed rung must be strictly narrower, or the rung probe cannot reach it"
    );
}

#[test]
fn both_icon_slots_count_toward_the_floor() {
    let one = ButtonModel::new("S")
        .leading(ButtonIcon::Reserved)
        .trailing(ButtonIcon::Reserved);
    assert_eq!(
        one.collapsed_min_width(),
        2.0 * ButtonIcon::Reserved.width() + SPINNER_SLOT + 2.0 * space::SM,
        "two reserved icon slots *plus* the spinner slot — reserving is not free, and it should not be"
    );
}

#[test]
fn a_measured_label_widens_only_the_expanded_rung() {
    let model = ButtonModel::new("Save");
    let bare = model.measurement();
    let measured = model.measurement_with_label(80.0);

    assert_eq!(measured.compact_width(), bare.compact_width());
    assert_eq!(
        measured.expanded_width(),
        bare.expanded_width() + 80.0,
        "the label belongs to the full rung only; collapsing removes it, so the compact floor \
         must not move"
    );
}

#[test]
fn a_negative_label_width_cannot_narrow_the_expanded_rung() {
    let model = ButtonModel::new("Save");
    let measured = model.measurement_with_label(-500.0);
    assert_eq!(
        measured.expanded_width(),
        model.expanded_min_width(),
        "a negative measurement is clamped, not subtracted — an upstream bug must not be \
         able to make the full rung need less room than the control itself"
    );
}

// ---------------------------------------------------------------------------
// Rung resolution
// ---------------------------------------------------------------------------

#[test]
fn density_maps_onto_the_two_rungs_the_spec_names() {
    assert_eq!(ButtonDensity::Standard.breakpoint(), Breakpoint::Expanded);
    assert_eq!(ButtonDensity::Compact.breakpoint(), Breakpoint::Compact);
    assert!(!ButtonDensity::Standard.is_collapsed());
    assert!(ButtonDensity::Compact.is_collapsed());
}

#[test]
fn the_declared_density_resolves_through_the_real_solver() {
    let standard = ButtonModel::new("Save").responsive();
    let compact = ButtonModel::new("Save")
        .with_density(ButtonDensity::Compact)
        .responsive();

    assert_eq!(standard.breakpoint(), Breakpoint::Expanded);
    assert_eq!(compact.breakpoint(), Breakpoint::Compact);
    assert!(!standard.is_compact());
    assert!(compact.is_compact());
}

#[test]
fn the_rung_probe_lands_on_compact_even_for_a_model_with_no_label_at_all() {
    // The degenerate case: an icon-only model whose two floors are as close as
    // they can get. The probe must still reach Compact, or a collapsed toolbar
    // icon would render with expanded padding.
    let model = ButtonModel::icon_only("X").with_density(ButtonDensity::Compact);
    assert_eq!(
        model.responsive().breakpoint(),
        Breakpoint::Compact,
        "the probe is one step below the expanded floor, so it reaches Compact whatever \
         the content is"
    );
    assert_eq!(
        model.responsive().horizontal_padding(),
        space::SM,
        "and the padding really did collapse — this is the number a wrong rung would get wrong"
    );
}

#[test]
fn the_live_container_query_answers_for_a_width_the_host_actually_has() {
    let model = ButtonModel::new("Save");
    let needed = model.expanded_min_width();

    assert_eq!(
        model.responsive_to(needed).breakpoint(),
        Breakpoint::Expanded,
        "exactly enough is enough"
    );
    assert_eq!(
        model.responsive_to(needed - 1.0).breakpoint(),
        Breakpoint::Compact,
        "one pixel short is short"
    );
    assert_eq!(
        model.responsive_to(f32::NAN).breakpoint(),
        Breakpoint::Compact,
        "an unmeasurable allocation fails closed to the smaller presentation"
    );
    assert_eq!(
        model.responsive_to(-10.0).breakpoint(),
        Breakpoint::Compact,
        "a negative allocation collapses rather than oscillating"
    );
}

#[test]
fn no_rung_shifts_the_layout_vertically() {
    // §3.0.4.4 fixes this outright. The proof is that both rungs — and the
    // live query at any width — return the same zero, so there is no branch
    // that could produce a shift.
    assert_eq!(layout_for(ButtonDensity::Standard).vertical_shift(), 0.0);
    assert_eq!(layout_for(ButtonDensity::Compact).vertical_shift(), 0.0);
    for width in [0.0, 1.0, 96.0, 4096.0, -1.0] {
        assert_eq!(
            ButtonModel::new("Save")
                .responsive_to(width)
                .vertical_shift(),
            0.0,
            "a neighbour that moves when this one changes is a layout jump"
        );
    }
}

#[test]
fn horizontal_padding_compresses_md_to_sm_across_the_rung() {
    assert_eq!(
        layout_for(ButtonDensity::Standard).horizontal_padding(),
        space::MD
    );
    assert_eq!(
        layout_for(ButtonDensity::Compact).horizontal_padding(),
        space::SM
    );
}

#[test]
fn the_label_mode_a_collapsed_rung_chooses_is_the_one_that_owes_a_tooltip() {
    assert!(
        !layout_for(ButtonDensity::Standard)
            .label_mode()
            .requires_tooltip()
    );
    assert!(
        layout_for(ButtonDensity::Compact)
            .label_mode()
            .requires_tooltip(),
        "the collapse and the tooltip obligation are the same event, not two rules that \
         have to be kept in step"
    );
}
