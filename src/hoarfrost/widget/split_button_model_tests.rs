//! Tests for the split-button data model — `UIF-ATOM-001c`, §3.1c.
//!
//! Five groups, in order:
//!
//! 1. **Composition.** The primary half *is* a [`ButtonModel`], and the
//!    `ActionPending` projection is that model's, unchanged.
//! 2. **Flyout construction.** Rows reach the engine's own `Menu` with
//!    their identity, flags and hints intact.
//! 3. **Navigability.** A flyout with nothing live in it says so, because
//!    the cascade would refuse it.
//! 4. **Identity.** The derived menu id and its override.
//! 5. **State read-through.** Busy, disabled and density land on the
//!    primary half rather than being tracked twice.

use super::*;

type Id = &'static str;

fn rows() -> Vec<SplitMenuRow<Id>> {
    vec![
        SplitMenuRow::new("copy", "Copy"),
        SplitMenuRow::new("paste", "Paste"),
        SplitMenuRow::new("delete", "Delete").destructive(),
    ]
}

fn model() -> SplitButtonModel<Id> {
    SplitButtonModel::new("Save", rows())
}

// ---------------------------------------------------------------------------
// 1. Composition
// ---------------------------------------------------------------------------

#[test]
fn the_primary_half_is_a_real_button_model() {
    let m = model();
    let button = m.primary_model();
    assert_eq!(button.label.as_deref(), Some("Save"));
    assert_eq!(button.variant, ButtonVariant::Secondary);
    assert_eq!(m.label(), "Save");
    assert_eq!(m.variant(), ButtonVariant::Secondary);
}

#[test]
fn the_primary_builder_moves_the_contained_button_not_a_copy_of_it() {
    let m = SplitButtonModel::primary("Save", rows());
    assert_eq!(m.variant(), ButtonVariant::Primary);
    assert_eq!(m.primary_model().variant, ButtonVariant::Primary);
}

#[test]
fn pending_is_the_contained_button_models_pending_verbatim() {
    for busy in [false, true] {
        for cancellable in [false, true] {
            let mut m = model();
            m.primary.busy = busy;
            m.primary.cancellable = cancellable;
            assert_eq!(
                m.pending(),
                m.primary_model().pending(),
                "pending drifted for busy={busy} cancellable={cancellable}"
            );
            assert_eq!(m.tracks_action(), busy || cancellable);
        }
    }
}

#[test]
fn a_resting_split_button_projects_idle_not_busy() {
    let m = model();
    assert!(!m.pending().is_busy());
    assert!(!m.tracks_action());
}

#[test]
fn a_busy_split_button_projects_busy() {
    let m = model().busy();
    assert!(m.pending().is_busy());
    assert!(m.tracks_action());
    assert!(m.busy_flag());
}

#[test]
fn a_cancellable_split_button_tracks_its_action_even_while_idle() {
    let m = model().cancellable();
    assert!(!m.pending().is_busy());
    assert!(m.tracks_action());
}

// ---------------------------------------------------------------------------
// 2. Flyout construction
// ---------------------------------------------------------------------------

#[test]
fn rows_reach_the_engine_keyed_by_the_action_id() {
    let m = model();
    let menu = m.menu();
    assert_eq!(menu.len(), 3);
    assert_eq!(menu.row(0).map(|row| row.key()), Some("copy"));
    assert_eq!(menu.row(1).map(|row| row.key()), Some("paste"));
    assert_eq!(menu.row(2).map(|row| row.key()), Some("delete"));
}

#[test]
fn labels_survive_the_trip_to_the_engine_row() {
    let m = model();
    let menu = m.menu();
    let labels: Vec<&str> = (0..menu.len())
        .map(|i| menu.row(i).map(|row| row.label()).unwrap_or_default())
        .collect();
    assert_eq!(labels, ["Copy", "Paste", "Delete"]);
}

#[test]
fn the_destructive_flag_survives_and_its_neighbours_do_not_catch_it() {
    let m = model();
    let menu = m.menu();
    assert!(menu.row(2).is_some_and(|row| row.is_destructive()));
    assert!(!menu.row(0).is_some_and(|row| row.is_destructive()));
    assert!(!menu.row(1).is_some_and(|row| row.is_destructive()));
}

#[test]
fn a_disabled_row_arrives_disabled_and_its_neighbours_stay_live() {
    let m = SplitButtonModel::new(
        "Save",
        vec![
            SplitMenuRow::new("a", "A"),
            SplitMenuRow::new("b", "B").disabled(),
        ],
    );
    let menu = m.menu();
    assert!(menu.row(0).is_some_and(|row| row.is_enabled()));
    assert!(!menu.row(1).is_some_and(|row| row.is_enabled()));
}

#[test]
fn a_shortcut_hint_arrives_as_a_shortcut_hint() {
    let m = SplitButtonModel::new(
        "Save",
        vec![SplitMenuRow::new("save", "Save").with_shortcut("Ctrl+S")],
    );
    assert_eq!(
        m.menu().row(0).and_then(|row| row.shortcut()),
        Some("Ctrl+S")
    );
}

#[test]
fn a_row_without_a_shortcut_carries_no_shortcut() {
    let m = model();
    assert_eq!(m.menu().row(0).and_then(|row| row.shortcut()), None);
}

// ---------------------------------------------------------------------------
// 3. Navigability
// ---------------------------------------------------------------------------

#[test]
fn a_flyout_with_a_live_row_is_navigable() {
    assert!(model().has_navigable_menu());
}

#[test]
fn a_flyout_of_only_disabled_rows_is_not_navigable() {
    let m = SplitButtonModel::new(
        "Save",
        vec![
            SplitMenuRow::new("a", "A").disabled(),
            SplitMenuRow::new("b", "B").disabled(),
        ],
    );
    assert!(!m.has_navigable_menu());
}

#[test]
fn an_empty_flyout_is_not_navigable() {
    let m = SplitButtonModel::new("Save", Vec::<SplitMenuRow<Id>>::new());
    assert!(!m.has_navigable_menu());
    assert_eq!(m.menu().len(), 0);
    assert!(m.menu().first_navigable().is_none());
}

// ---------------------------------------------------------------------------
// 4. Identity
// ---------------------------------------------------------------------------

#[test]
fn the_menu_id_is_derived_from_the_prefix_and_the_label() {
    let m = model();
    assert_eq!(m.menu_id().as_str(), "split-button:Save");
}

#[test]
fn an_overridden_menu_id_replaces_the_derived_one() {
    let m = model().with_menu_id("doc-toolbar-save");
    assert_eq!(m.menu_id().as_str(), "doc-toolbar-save");
    assert_eq!(m.menu().id().as_str(), "doc-toolbar-save");
}

#[test]
fn two_same_labelled_controls_are_distinct_once_overridden() {
    let a = SplitButtonModel::new("Save", rows()).with_menu_id("left");
    let b = SplitButtonModel::new("Save", rows()).with_menu_id("right");
    assert_ne!(a.menu_id(), b.menu_id());
}

// ---------------------------------------------------------------------------
// 5. State read-through
// ---------------------------------------------------------------------------

#[test]
fn the_disabled_flag_lives_on_the_contained_button_only() {
    let m = model().disabled();
    assert!(m.disabled_flag());
    assert!(m.primary_model().disabled);
}

#[test]
fn density_set_on_the_split_button_set_it_on_the_primary_half() {
    let m = model().with_density(ButtonDensity::Compact);
    assert_eq!(m.density(), ButtonDensity::Compact);
    assert_eq!(m.primary_model().density, ButtonDensity::Compact);
}

#[test]
fn a_leading_icon_lands_on_the_primary_half() {
    let m = model().with_leading_icon(ButtonIcon::Glyph("💾"));
    assert!(m.primary_model().leading_icon.is_some());
}

#[test]
fn a_tooltip_lands_on_the_primary_half() {
    let m = model().with_tooltip("Save the document");
    assert_eq!(
        m.primary_model().tooltip.as_deref(),
        Some("Save the document")
    );
}

#[test]
fn action_at_round_trips_and_is_none_past_the_end() {
    let m = model();
    assert_eq!(m.action_at(0), Some(&"copy"));
    assert_eq!(m.action_at(2), Some(&"delete"));
    assert_eq!(m.action_at(3), None);
}

#[test]
fn replacing_the_rows_moves_the_action_ids_with_them() {
    let m = model().with_menu_rows(vec![SplitMenuRow::new("only", "Only")]);
    assert_eq!(m.action_at(0), Some(&"only"));
    assert_eq!(m.action_at(1), None);
}

#[test]
fn the_menu_open_flag_is_a_plain_mirror_and_no_operation_here_changes_it() {
    let mut m = model();
    assert!(!m.menu_open);
    m.menu_open = true;
    assert!(m.menu_open);
    // Nothing a model method does touches it: opening is the cascade's job.
    let _ = m.menu();
    assert!(m.menu_open);
}
