//! Tests for the combobox data model — `UIF-ATOM-004`, §3.4.
//!
//! Four groups, in order:
//!
//! 1. **The spec's field list.** Every name §3.4 prints is present, with
//!    the default the spec implies, and the four fields the spec does not
//!    have are off by default so the closed select stays the base case.
//! 2. **Filter semantics.** Each of the four spec silences is pinned
//!    both ways — the default and the override — so a default is visible
//!    as a choice rather than as the only thing the code could do.
//! 3. **The engine-side flyout.** Rows are `Check`, they close the tree,
//!    and the menu id never moves with the filter.
//! 4. **Degenerate input.** A stale selection, an empty list, a query
//!    that matches nothing — none of them panic, all of them read as
//!    "nothing pickable".

use super::*;

/// The three options every model in this file is built over.
fn options() -> Vec<SelectOption<String>> {
    vec![
        SelectOption::new("wrap".to_string(), "Word Wrap"),
        SelectOption::new("tab".to_string(), "Tab Size"),
        SelectOption::new("font".to_string(), "Font Ligatures"),
    ]
}

/// A closed select over the standard options.
fn sample_model() -> ComboBoxModel<String> {
    ComboBoxModel::new(options(), "Choose a setting")
}

/// The labels of a filtered list, in order.
fn labels<'a>(list: &[&'a SelectOption<String>]) -> Vec<&'a str> {
    list.iter().map(|option| option.label.as_str()).collect()
}

// ---------------------------------------------------------------------------
// 1 — the spec's field list
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_model_carries_the_spec_field_defaults() {
    let model = sample_model();
    assert_eq!(model.items().len(), 3);
    assert_eq!(model.placeholder(), "Choose a setting");
    assert_eq!(model.selected(), None);
    assert_eq!(model.filter_query(), None);
    assert!(!model.is_open());
    assert_eq!(model.validation(), None);
    assert!(!model.disabled_flag());
}

#[test]
fn the_closed_select_is_the_base_case() {
    // §3.4 attaches `on_filter_change` with the parenthetical "when
    // searchable", so the filter is the conditional and the plain
    // click-to-pick select is what you get by default.
    assert!(!sample_model().is_editable());
    assert!(sample_model().editable().is_editable());
}

#[test]
fn read_only_is_off_by_default_and_independent_of_disabled() {
    let model = sample_model().read_only();
    assert!(model.read_only_flag());
    assert!(!model.disabled_flag());
    assert!(!sample_model().read_only_flag());
}

#[test]
fn validation_and_disabled_are_orthogonal_reads() {
    let model = sample_model().with_validation("Pick one").disabled();
    assert_eq!(model.validation(), Some("Pick one"));
    assert!(model.is_invalid());
    assert!(model.disabled_flag());
    assert!(!sample_model().is_invalid());
}

#[test]
fn the_open_flag_is_a_projection_the_host_sets() {
    assert!(!sample_model().is_open());
    assert!(sample_model().with_open(true).is_open());
    assert!(!sample_model().with_open(true).with_open(false).is_open());
}

#[test]
fn the_display_label_is_the_selection_or_the_placeholder() {
    assert_eq!(sample_model().display_label(), "Choose a setting");
    assert!(sample_model().shows_placeholder());

    let picked = sample_model().with_selected("wrap".to_string());
    assert_eq!(picked.display_label(), "Word Wrap");
    assert!(!picked.shows_placeholder());
}

#[test]
fn a_stale_selection_reads_as_no_selection_rather_than_panicking() {
    // The host owns the ids; a rebuild that dropped the selected one is
    // absorbed, not a crash.
    let stale = sample_model().with_selected("gone".to_string());
    assert_eq!(
        stale.selected_option().map(|option| option.label.as_str()),
        None
    );
    assert_eq!(stale.selected_index(), None);
    assert!(stale.shows_placeholder());
    assert_eq!(stale.display_label(), "Choose a setting");
}

// ---------------------------------------------------------------------------
// 2 — filter semantics: the four spec silences
// ---------------------------------------------------------------------------

#[test]
fn the_default_policy_is_insensitive_substring_keep_selected() {
    let policy = FilterPolicy::default();
    assert_eq!(policy.case, CaseHandling::Insensitive);
    assert_eq!(policy.mode, MatchMode::Contains);
    assert!(policy.keep_selected);
}

#[test]
fn filtering_ignores_case_by_default() {
    let model = sample_model();
    assert_eq!(
        labels(&model.filtered_with(Some("wrap"))),
        vec!["Word Wrap"]
    );
    assert_eq!(
        labels(&model.filtered_with(Some("WRAP"))),
        vec!["Word Wrap"]
    );
    assert_eq!(
        labels(&model.filtered_with(Some("wRaP"))),
        vec!["Word Wrap"]
    );
}

#[test]
fn filtering_finds_a_substring_not_only_a_prefix() {
    // The ordinary reason to filter at all: "wrap" inside "Word Wrap".
    let model = sample_model();
    assert_eq!(labels(&model.filtered_with(Some("ord"))), vec!["Word Wrap"]);
    assert_eq!(labels(&model.filtered_with(Some("Si"))), vec!["Tab Size"]);
}

#[test]
fn a_prefix_policy_can_be_asked_for_and_changes_the_result() {
    let prefix = sample_model().with_filter_policy(FilterPolicy::new(
        CaseHandling::Insensitive,
        MatchMode::Prefix,
    ));
    assert!(prefix.filtered_with(Some("wrap")).is_empty());
    assert_eq!(
        labels(&prefix.filtered_with(Some("Word"))),
        vec!["Word Wrap"]
    );
}

#[test]
fn a_case_sensitive_policy_can_be_asked_for_and_changes_the_result() {
    let sensitive = sample_model().with_filter_policy(FilterPolicy::new(
        CaseHandling::Sensitive,
        MatchMode::Contains,
    ));
    assert!(
        sensitive.filtered_with(Some("wrap")).is_empty(),
        "case-sensitive matching cannot find a lower-case query in `Word Wrap`"
    );
    assert_eq!(
        labels(&sensitive.filtered_with(Some("Wrap"))),
        vec!["Word Wrap"]
    );
    assert!(sensitive.filtered_with(Some("WRAP")).is_empty());
}

#[test]
fn the_selected_row_stays_visible_while_filtering() {
    // §3.4 mandates a `✓` in the selected row's leading slot; hiding the
    // selected row makes that mandate unreachable.
    let model = sample_model().with_selected("wrap".to_string());
    assert_eq!(
        labels(&model.filtered_with(Some("Tab"))),
        vec!["Word Wrap", "Tab Size"],
        "the selected row survives a filter that would otherwise hide it"
    );
}

#[test]
fn keep_selected_can_be_turned_off_and_then_the_row_is_hidden() {
    let policy = FilterPolicy {
        keep_selected: false,
        ..FilterPolicy::default()
    };
    let model = sample_model()
        .with_selected("wrap".to_string())
        .with_filter_policy(policy);
    assert_eq!(labels(&model.filtered_with(Some("Tab"))), vec!["Tab Size"]);
}

#[test]
fn an_empty_query_is_no_filter_at_all() {
    let model = sample_model();
    assert_eq!(model.filtered_with(Some("")).len(), 3);
    assert_eq!(model.filtered_with(None).len(), 3);
}

#[test]
fn a_whitespace_only_query_is_no_filter() {
    // A lone space is "nothing typed yet", not a query that hides every
    // label without a space in it.
    let model = sample_model();
    assert_eq!(model.filtered_with(Some("   ")).len(), 3);
    assert!(FilterPolicy::default().matches("  ", "TabSize"));
}

#[test]
fn a_query_matching_only_disabled_options_leaves_nothing_pickable() {
    let mut items = options();
    items[0].enabled = false;
    let model = ComboBoxModel::new(items, "Choose a setting").with_filter_policy(FilterPolicy {
        keep_selected: false,
        ..FilterPolicy::default()
    });
    assert_eq!(
        labels(&model.filtered_with(Some("Word"))),
        vec!["Word Wrap"]
    );
    assert!(!model.has_pickable_with(Some("Word")));
    assert!(model.has_pickable_with(Some("Tab")));
}

#[test]
fn has_pickable_follows_the_models_own_filter_when_asked_with_no_argument() {
    let model = sample_model().with_filter("zzz".to_string());
    assert!(!model.has_pickable());
    assert!(model.with_filter("Tab".to_string()).has_pickable());
}

#[test]
fn a_disabled_option_stays_in_the_list_and_reports_itself_disabled() {
    let model = ComboBoxModel::new(
        vec![SelectOption::new("a".to_string(), "Alpha").disabled()],
        "p",
    );
    assert_eq!(model.items().len(), 1);
    assert!(!model.items()[0].enabled);
    assert!(!model.has_pickable());
}

#[test]
fn with_enabled_sets_the_flag_without_a_builder_chain_surprise() {
    let option = SelectOption::new("x".to_string(), "X").with_enabled(false);
    assert!(!option.enabled);
    assert!(option.with_enabled(true).enabled);
}

// ---------------------------------------------------------------------------
// 3 — the engine-side flyout
// ---------------------------------------------------------------------------

#[test]
fn the_menu_id_never_moves_with_the_filter() {
    // MenuCascade::sync resolves the open card by id. An id that changed
    // per keystroke would tear the flyout down between characters.
    let model = sample_model();
    assert_eq!(
        model.menu_with(None).id(),
        model.menu_with(Some("wrap")).id(),
        "the flyout's identity belongs to the control, not the query"
    );
    assert_eq!(model.menu().id(), model.menu_with(Some("zzz")).id());
}

#[test]
fn the_menu_id_can_be_overridden_when_two_controls_share_a_placeholder() {
    let a = sample_model().with_menu_id("combo:a");
    let b = sample_model().with_menu_id("combo:b");
    assert_ne!(a.menu_id(), b.menu_id());
    assert_eq!(a.menu().id().as_str(), "combo:a");
}

#[test]
fn every_flyout_row_is_a_check_that_closes_the_tree() {
    let menu = sample_model().menu();
    assert_eq!(menu.len(), 3);
    for row in menu.rows() {
        assert!(row.is_checkable(), "{} is not a `✓` row", row.key());
        assert!(
            row.closes_menu_on_activate(),
            "{} must close the popup when picked",
            row.key()
        );
    }
}

#[test]
fn the_selected_option_is_the_only_checked_row() {
    let menu = sample_model().with_selected("tab".to_string()).menu();
    let checked: Vec<&str> = menu
        .rows()
        .iter()
        .filter(|row| row.is_checked())
        .map(|row| row.key())
        .collect();
    assert_eq!(checked, vec!["tab"]);
}

#[test]
fn a_disabled_option_becomes_a_disabled_row_and_leaves_the_roster() {
    let menu = ComboBoxModel::new(
        vec![
            SelectOption::new("a".to_string(), "Alpha"),
            SelectOption::new("b".to_string(), "Beta").disabled(),
        ],
        "p",
    )
    .menu();
    assert_eq!(menu.len(), 2);
    assert!(menu.row(0).is_some_and(|row| row.navigable()));
    assert!(!menu.row(1).is_some_and(|row| row.navigable()));
    assert_eq!(menu.first_navigable(), Some(0));
}

#[test]
fn a_flyout_with_nothing_navigable_has_no_entry_point() {
    let menu = ComboBoxModel::new(
        vec![SelectOption::new("b".to_string(), "Beta").disabled()],
        "p",
    )
    .menu();
    assert_eq!(menu.first_navigable(), None);
}

#[test]
fn the_filtered_row_index_is_a_filtered_index_not_a_full_list_index() {
    let model = sample_model();
    // Full list index 2 is "Font Ligatures"; under "Tab" the filtered
    // index 0 is "Tab Size".
    assert_eq!(
        model
            .filtered_option_with(Some("Tab"), 0)
            .map(|option| option.label.as_str()),
        Some("Tab Size")
    );
    assert!(model.filtered_option_with(Some("Tab"), 1).is_none());
}

#[test]
fn id_by_key_round_trips_through_the_display_form() {
    let model = sample_model();
    assert_eq!(model.id_by_key("wrap"), Some(&"wrap".to_string()));
    assert_eq!(model.id_by_key("nope"), None);
}

// ---------------------------------------------------------------------------
// 4 — the pre-open snapshot
// ---------------------------------------------------------------------------

#[test]
fn the_snapshot_captures_both_the_selection_and_the_filter() {
    let model = sample_model()
        .with_selected("tab".to_string())
        .with_filter("wo".to_string());
    let snapshot = model.snapshot();
    assert_eq!(snapshot.selected, Some("tab".to_string()));
    assert_eq!(snapshot.filter, Some("wo".to_string()));
}

#[test]
fn the_snapshot_of_an_untouched_model_is_all_none() {
    let snapshot = sample_model().snapshot();
    assert_eq!(snapshot.selected, None);
    assert_eq!(snapshot.filter, None);
}

#[test]
fn a_snapshot_is_a_value_that_survives_the_model_moving_on() {
    let model = sample_model().with_selected("tab".to_string());
    let snapshot = model.snapshot();
    let later = model.with_selected("wrap".to_string());
    assert_ne!(snapshot.selected, later.selected);
    assert_eq!(snapshot.selected, Some("tab".to_string()));
}

#[test]
fn two_identical_models_snapshot_equal() {
    assert_eq!(sample_model().snapshot(), sample_model().snapshot());
}
