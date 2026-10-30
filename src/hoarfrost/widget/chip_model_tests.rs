//! Tests for the declarative chip model and its gesture contract —
//! `UIF-ATOM-006`, §3.6.
//!
//! Four groups, in order:
//!
//! 1. **Variant facts.** Which variant can be selected, which can be
//!    removed, and that the `with_*` builders refuse to change a variant
//!    that has no such field.
//! 2. **The height rung.** That the two rungs are the two tokens §3.6
//!    names, and that the rung is carried by the model rather than
//!    derived from the variant.
//! 3. **Construction and identity.** The three named constructors, the
//!    builder chain, and the `Controlled` frame view.
//! 4. **The gesture → event table.** Every branch of §3.6's callback
//!    list, including the three ways a gesture produces nothing.
//!
//! # Why the event table is the centre of this file
//!
//! §3.6 lists three variants and three callbacks in adjacent bullets and
//! never says which pairs with which. The pairing is the whole decision
//! this model makes, so every cell of the cross product is asserted —
//! including the silences, which are as much a part of the contract as
//! the emissions.

use super::*;
use crate::binding::Controlled;

/// Every distinct variant the model can hold, in §3.6 order.
const VARIANTS: [ChipVariant; 5] = [
    ChipVariant::Assist,
    ChipVariant::Filter { selected: false },
    ChipVariant::Filter { selected: true },
    ChipVariant::Input { removable: false },
    ChipVariant::Input { removable: true },
];

/// Every gesture the model recognises.
const GESTURES: [ChipGesture; 2] = [ChipGesture::Activate, ChipGesture::Remove];

/// Every height rung.
const HEIGHTS: [ChipHeight; 2] = [ChipHeight::Compact, ChipHeight::Bar];

// ---------------------------------------------------------------------------
// 1. Variant facts
// ---------------------------------------------------------------------------

#[test]
fn only_the_filter_variant_is_selectable() {
    assert!(!ChipVariant::Assist.is_selectable());
    assert!(ChipVariant::Filter { selected: false }.is_selectable());
    assert!(!ChipVariant::Input { removable: true }.is_selectable());
}

#[test]
fn is_selected_is_true_only_for_a_selected_filter() {
    for variant in VARIANTS {
        let expected = matches!(variant, ChipVariant::Filter { selected: true });
        assert_eq!(
            variant.is_selected(),
            expected,
            "{variant:?} should report selected = {expected}",
        );
    }
}

#[test]
fn only_a_removable_input_variant_is_removable() {
    assert!(!ChipVariant::Assist.is_removable());
    assert!(!ChipVariant::Filter { selected: true }.is_removable());
    assert!(!ChipVariant::Input { removable: false }.is_removable());
    assert!(ChipVariant::Input { removable: true }.is_removable());
}

#[test]
fn with_selected_only_changes_a_filter() {
    assert_eq!(
        ChipVariant::Filter { selected: false }.with_selected(true),
        ChipVariant::Filter { selected: true },
    );
    assert_eq!(
        ChipVariant::Filter { selected: true }.with_selected(false),
        ChipVariant::Filter { selected: false },
    );

    for variant in [
        ChipVariant::Assist,
        ChipVariant::Input { removable: false },
        ChipVariant::Input { removable: true },
    ] {
        assert_eq!(
            variant.with_selected(true),
            variant,
            "{variant:?} has no selection to change",
        );
    }
}

#[test]
fn with_removable_only_changes_an_input() {
    assert_eq!(
        ChipVariant::Input { removable: false }.with_removable(true),
        ChipVariant::Input { removable: true },
    );
    assert_eq!(
        ChipVariant::Input { removable: true }.with_removable(false),
        ChipVariant::Input { removable: false },
    );

    for variant in [ChipVariant::Assist, ChipVariant::Filter { selected: true }] {
        assert_eq!(
            variant.with_removable(true),
            variant,
            "{variant:?} has no removability to change",
        );
    }
}

// ---------------------------------------------------------------------------
// 2. The height rung
// ---------------------------------------------------------------------------

#[test]
fn the_two_rungs_are_the_two_tokens_3_6_names() {
    assert_eq!(ChipHeight::Compact.px(), crate::token::size::BAR_STATUS);
    assert_eq!(ChipHeight::Compact.px(), 24.0);
    assert_eq!(ChipHeight::Bar.px(), crate::token::size::CONTROL_SM);
    assert_eq!(ChipHeight::Bar.px(), 28.0);
}

#[test]
fn compact_is_the_default_rung() {
    assert_eq!(ChipHeight::default(), ChipHeight::Compact);
    assert_eq!(
        ChipModel::assist("a", "A").height(),
        ChipHeight::Compact,
        "a chip built without saying a rung is the compact inline one",
    );
}

#[test]
fn every_variant_may_take_either_rung() {
    // §3.6 ties the height to *where the chip sits*, not to which
    // variant it is, so no variant may pin a rung to itself.
    for variant in VARIANTS {
        for height in HEIGHTS {
            let model = ChipModel::new("c", "Chip", variant).with_height(height);
            assert_eq!(
                model.height(),
                height,
                "{variant:?} must accept the {height:?} rung",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Construction, identity and the controlled view
// ---------------------------------------------------------------------------

#[test]
fn the_three_named_constructors_select_the_three_variants() {
    let assist = ChipModel::assist("id", "Assist");
    assert_eq!(assist.variant(), ChipVariant::Assist);
    assert_eq!(assist.id(), &"id");
    assert_eq!(assist.label(), "Assist");

    let filter = ChipModel::filter("f", "Filter", true);
    assert_eq!(filter.variant(), ChipVariant::Filter { selected: true });
    assert!(filter.is_selected());

    let input = ChipModel::input("i", "Input", true);
    assert_eq!(input.variant(), ChipVariant::Input { removable: true });
    assert!(input.is_removable());
}

#[test]
fn a_chip_starts_enabled_with_no_leading_icon() {
    let model = ChipModel::assist("a", "A");
    assert!(!model.is_disabled());
    assert_eq!(model.leading_icon(), None);
}

#[test]
fn builders_are_chainable_and_leave_the_receiver_alone() {
    let base = ChipModel::filter("f", "Filter", false);
    let built = base
        .clone()
        .with_height(ChipHeight::Bar)
        .with_leading(ChipIcon::Glyph("★"))
        .disabled();

    assert_eq!(built.height(), ChipHeight::Bar);
    assert_eq!(built.leading_icon(), Some(ChipIcon::Glyph("★")));
    assert!(built.is_disabled());

    assert_eq!(
        base.height(),
        ChipHeight::Compact,
        "the builder must not have mutated the model it was made from",
    );
    assert!(!base.is_disabled());
}

#[test]
fn the_controlled_view_shows_the_same_model() {
    let model = ChipModel::filter("f", "Filter", true);
    let view: Controlled<'_, ChipModel<&str>> = model.controlled();
    assert_eq!(view.get(), &model);
    assert!(view.get().is_selected());
}

#[test]
fn every_leading_icon_occupies_the_small_icon_rung() {
    use crate::token::size;
    assert_eq!(ChipIcon::Glyph("★").width(), size::ICON_SM);
    assert_eq!(ChipIcon::Reserved.width(), size::ICON_SM);
    assert_eq!(ChipIcon::Glyph("★").width(), 14.0);
}

// ---------------------------------------------------------------------------
// 4. The gesture → event table
// ---------------------------------------------------------------------------

#[test]
fn activating_an_assist_chip_emits_activated() {
    let model = ChipModel::assist("a", "Assist");
    assert_eq!(
        model.respond(ChipGesture::Activate),
        Some(ChipEvent::Activated("a")),
    );
}

#[test]
fn activating_an_input_chip_emits_activated_not_removed() {
    // The `×` is the only removal route on the body of the chip; a click
    // on the label activates the token.
    let model = ChipModel::input("i", "Input", true);
    assert_eq!(
        model.respond(ChipGesture::Activate),
        Some(ChipEvent::Activated("i")),
    );
}

#[test]
fn activating_a_filter_chip_emits_the_new_selection() {
    let off = ChipModel::filter("f", "Filter", false);
    assert_eq!(
        off.respond(ChipGesture::Activate),
        Some(ChipEvent::Toggled("f", true)),
        "a click on an unapplied filter reports it as now applied",
    );

    let on = ChipModel::filter("f", "Filter", true);
    assert_eq!(
        on.respond(ChipGesture::Activate),
        Some(ChipEvent::Toggled("f", false)),
        "a click on an applied filter reports it as now cleared",
    );
}

#[test]
fn removing_a_removable_input_chip_emits_removed() {
    let model = ChipModel::input("i", "Input", true);
    assert_eq!(
        model.respond(ChipGesture::Remove),
        Some(ChipEvent::Removed("i")),
    );
}

#[test]
fn removing_a_non_removable_chip_emits_nothing() {
    for variant in VARIANTS {
        let model = ChipModel::new("c", "Chip", variant);
        let got = model.respond(ChipGesture::Remove);
        if variant.is_removable() {
            assert_eq!(got, Some(ChipEvent::Removed("c")), "{variant:?}");
        } else {
            assert_eq!(got, None, "{variant:?} has no × to click",);
        }
    }
}

#[test]
fn a_disabled_chip_emits_nothing_for_either_gesture() {
    for variant in VARIANTS {
        let model = ChipModel::new("c", "Chip", variant).disabled();
        for gesture in GESTURES {
            assert_eq!(
                model.respond(gesture),
                None,
                "a disabled chip must not put {gesture:?} on the wire for {variant:?}",
            );
        }
    }
}

#[test]
fn one_gesture_produces_at_most_one_event() {
    // The anti-double-emission pin: a filter chip must not fire both
    // `Activated` and `Toggled` for the same press, because a host that
    // handled both would have to decide which one it trusted.
    let filter = ChipModel::filter("f", "Filter", false);
    let events: Vec<ChipEvent<&str>> = GESTURES
        .iter()
        .filter_map(|gesture| filter.respond(*gesture))
        .collect();
    assert_eq!(events.len(), 1, "only Activate is meaningful here");
    assert!(matches!(events[0], ChipEvent::Toggled(_, true)));
}

#[test]
fn the_event_carries_the_host_identifier_unchanged() {
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Key(u32);

    let model = ChipModel::filter(Key(42), "Filter", true);
    assert_eq!(
        model.respond(ChipGesture::Activate),
        Some(ChipEvent::Toggled(Key(42), false)),
    );
    assert_eq!(model.id(), &Key(42));
}

#[test]
fn the_model_round_trips_through_clone_and_equality() {
    let model = ChipModel::input("i", "Input", true)
        .with_height(ChipHeight::Bar)
        .with_leading(ChipIcon::Glyph("x"));
    assert_eq!(model.clone(), model);
    assert_ne!(model.clone().disabled(), model);
}
