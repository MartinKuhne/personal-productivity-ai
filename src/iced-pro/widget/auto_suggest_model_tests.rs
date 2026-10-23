//! Tests for the `AutoSuggestBox` data model — `UIF-ATOM-002d`, §3.2.
//!
//! Four groups, in order:
//!
//! 1. **Shape.** The five fields the spec names, their defaults, and the
//!    queries that count as blank.
//! 2. **Reading the list back.** Highlight lookup, stale-index survival,
//!    and mapping a candidate id to a position.
//! 3. **The flyout's identity.** That the card id is derived from the
//!    control and cannot move with the text — the single property the
//!    whole keystroke-by-keystroke `sync` story rests on.
//! 4. **Geometry and the commit vocabulary.** The numbers §3.2 fixes,
//!    and the three commit sources it names.

use super::*;
use crate::focus::FocusId;
use crate::surface::menu::model::RowKind;
use crate::token::shape::RADIUS_WIDGET;
use crate::token::size::CONTROL_MD;
use crate::token::space::XS;

/// The control identity every test in this file is built against.
fn control() -> FocusId {
    FocusId::new("suggest-control")
}

/// The three candidates every list-shaped test is driven against.
fn items() -> Vec<SuggestionItem<String>> {
    vec![
        SuggestionItem::new("doc-readme".to_string(), "README.md"),
        SuggestionItem::new("doc-notes".to_string(), "notes.md"),
        SuggestionItem::new("doc-todo".to_string(), "todo.md"),
    ]
}

// ---------------------------------------------------------------------------
// 1 — Shape
// ---------------------------------------------------------------------------

#[test]
fn a_default_model_is_empty_enabled_and_idle() {
    let model: AutoSuggestModel<String> = AutoSuggestModel::new();

    assert_eq!(model.query, "");
    assert!(model.suggestions.is_empty());
    assert_eq!(model.highlighted_index, None);
    assert!(!model.busy);
    assert!(!model.disabled);
}

#[test]
fn the_builders_set_the_field_they_are_named_for() {
    let model = AutoSuggestModel::new()
        .with_query("read")
        .with_suggestions(items());

    assert_eq!(model.query, "read");
    assert_eq!(model.suggestions.len(), 3);
    assert!(!model.disabled);

    let off = AutoSuggestModel::<String>::new().disabled();
    assert!(off.disabled);
}

#[test]
fn whitespace_only_queries_count_as_blank() {
    assert!(AutoSuggestModel::<String>::new().query_is_blank());
    assert!(
        AutoSuggestModel::<String>::new()
            .with_query("   ")
            .query_is_blank()
    );
    assert!(
        AutoSuggestModel::<String>::new()
            .with_query("\t\n")
            .query_is_blank()
    );
    assert!(
        !AutoSuggestModel::<String>::new()
            .with_query(" r ")
            .query_is_blank()
    );
}

#[test]
fn an_empty_list_offers_nothing_and_a_non_empty_one_does() {
    assert!(!AutoSuggestModel::<String>::new().has_suggestions());
    assert!(
        AutoSuggestModel::<String>::new()
            .with_suggestions(items())
            .has_suggestions()
    );
}

// ---------------------------------------------------------------------------
// 2 — Reading the list back
// ---------------------------------------------------------------------------

#[test]
fn the_highlight_resolves_to_the_candidate_it_points_at() {
    let mut model = AutoSuggestModel::new().with_suggestions(items());

    model.highlighted_index = Some(1);
    assert_eq!(
        model.highlighted().map(|item| item.id.as_str()),
        Some("doc-notes")
    );

    model.highlighted_index = None;
    assert_eq!(model.highlighted(), None);
}

#[test]
fn a_highlight_past_the_end_of_a_replaced_list_reads_as_nothing_not_a_panic() {
    // The provider returned a shorter list while the keyboard was on the
    // last row. The model must survive that rather than index out of
    // bounds, because a shrinking result set is ordinary.
    let mut model = AutoSuggestModel::new().with_suggestions(items());
    model.highlighted_index = Some(2);

    model.suggestions = vec![SuggestionItem::new("only".to_string(), "only.md")];

    assert_eq!(model.highlighted(), None);
}

#[test]
fn a_candidate_id_maps_back_to_its_position_in_the_offered_list() {
    let model = AutoSuggestModel::new().with_suggestions(items());

    assert_eq!(model.index_of(&"doc-readme".to_string()), Some(0));
    assert_eq!(model.index_of(&"doc-todo".to_string()), Some(2));
    assert_eq!(model.index_of(&"gone".to_string()), None);
}

// ---------------------------------------------------------------------------
// 3 — The flyout's identity
// ---------------------------------------------------------------------------

#[test]
fn the_card_id_is_derived_from_the_control() {
    let id = menu_id_for(&control());
    assert_eq!(id.to_string(), "suggest:suggest-control");
}

#[test]
fn the_card_id_cannot_move_with_the_query() {
    // The load-bearing property. If the id tracked the text, every
    // keystroke would look like a different card to
    // `MenuCascade::sync`, and the flyout would close and reopen once
    // per character.
    let mut model = AutoSuggestModel::new().with_suggestions(items());

    let first = model.menu_for(&control()).id().clone();
    model.query = "r".to_string();
    let second = model.menu_for(&control()).id().clone();
    model.query = "readme".to_string();
    let third = model.menu_for(&control()).id().clone();

    assert_eq!(first, second, "the id moved with a one-character query");
    assert_eq!(second, third, "the id moved with the query");
}

#[test]
fn two_boxes_get_two_distinct_card_ids() {
    let a = menu_id_for(&FocusId::new("box-a"));
    let b = menu_id_for(&FocusId::new("box-b"));
    assert_ne!(a, b);
}

#[test]
fn the_card_is_stable_across_a_rebuild_from_the_same_suggestions() {
    let model = AutoSuggestModel::new().with_suggestions(items());
    let first = model.menu_for(&control());
    let second = model.menu_for(&control());

    assert_eq!(first, second, "an equivalent rebuild must be identical");
}

// ---------------------------------------------------------------------------
// 4 — The rows the flyout is built from
// ---------------------------------------------------------------------------

#[test]
fn every_candidate_becomes_one_action_row_in_list_order() {
    let model = AutoSuggestModel::new().with_suggestions(items());
    let menu = model.menu_for(&control());

    assert_eq!(menu.len(), 3);
    let keys: Vec<&str> = menu.rows().iter().map(|row| row.key()).collect();
    assert_eq!(keys, vec!["doc-readme", "doc-notes", "doc-todo"]);

    let labels: Vec<&str> = menu.rows().iter().map(|row| row.label()).collect();
    assert_eq!(labels, vec!["README.md", "notes.md", "todo.md"]);
}

#[test]
fn a_completion_row_is_an_action_not_a_checkable() {
    // §3.2 gives a completion row no checkmark. §3.4 gives the selected
    // combobox row one. The kinds are not interchangeable, and the
    // leading glyph the engine paints follows the kind.
    let model = AutoSuggestModel::new().with_suggestions(items());
    let menu = model.menu_for(&control());

    for row in menu.rows() {
        assert!(
            matches!(row.kind(), RowKind::Action),
            "completion row {:?} is not an Action",
            row.key()
        );
        assert!(!row.is_checkable());
    }
}

#[test]
fn picking_a_completion_row_closes_the_flyout() {
    let model = AutoSuggestModel::new().with_suggestions(items());
    let menu = model.menu_for(&control());

    for row in menu.rows() {
        assert!(
            row.closes_menu_on_activate(),
            "completion row {:?} must close the flyout when picked",
            row.key()
        );
    }
}

#[test]
fn every_row_is_navigable_so_the_card_can_hold_the_keyboard() {
    let model = AutoSuggestModel::new().with_suggestions(items());
    let menu = model.menu_for(&control());

    assert_eq!(menu.navigable_indices(), vec![0, 1, 2]);
    assert_eq!(menu.first_navigable(), Some(0));
    assert_eq!(menu.last_navigable(), Some(2));
}

#[test]
fn an_empty_suggestion_list_builds_a_card_the_engine_will_not_host() {
    // The no-match policy rests on this: no navigable row means
    // `MenuCascade::open` refuses with `NoFocusableRows`, so the
    // flyout never appears.
    let model: AutoSuggestModel<String> = AutoSuggestModel::new();
    let menu = model.menu_for(&control());

    assert!(menu.is_empty());
    assert_eq!(menu.first_navigable(), None);
    assert!(!model.has_pickable(&control()));
}

#[test]
fn a_non_empty_suggestion_list_is_pickable() {
    let model = AutoSuggestModel::new().with_suggestions(items());
    assert!(model.has_pickable(&control()));
}

#[test]
fn the_row_key_is_the_candidate_id_not_its_label() {
    // The commit maps candidate → id through the row key. If the key
    // were the label, two candidates sharing a display string would be
    // indistinguishable at commit time.
    let model = AutoSuggestModel::new().with_suggestions(vec![
        SuggestionItem::new("id-one".to_string(), "same label"),
        SuggestionItem::new("id-two".to_string(), "same label"),
    ]);
    let menu = model.menu_for(&control());

    assert_eq!(menu.row(0).map(|row| row.key()), Some("id-one"));
    assert_eq!(menu.row(1).map(|row| row.key()), Some("id-two"));
}

// ---------------------------------------------------------------------------
// 5 — Geometry and the commit vocabulary
// ---------------------------------------------------------------------------

#[test]
fn the_flyout_geometry_is_the_figures_the_spec_prints() {
    assert_eq!(FLYOUT_MAX_HEIGHT, 240.0);
    assert_eq!(FLYOUT_ROW_HEIGHT, 32.0);
    assert_eq!(FLYOUT_GAP, 4.0);
}

#[test]
fn the_flyout_geometry_is_read_from_tokens_where_tokens_own_the_number() {
    // Only the 240px cap is a spec-set surface bound. The row height and
    // the anchor gap are the framework's own existing measures, and a
    // completion row must be the same row every other 32px row is.
    assert_eq!(FLYOUT_ROW_HEIGHT, CONTROL_MD);
    assert_eq!(FLYOUT_GAP, XS);
    assert_eq!(FLYOUT_RADIUS, RADIUS_WIDGET);
}

#[test]
fn the_debounce_window_is_a_named_constant_not_an_inline_number() {
    assert_eq!(DEBOUNCE_MS, 200);
}

#[test]
fn the_three_commit_sources_are_the_three_the_spec_names() {
    assert_eq!(CommitSource::Enter.slug(), "enter");
    assert_eq!(CommitSource::Click.slug(), "click");
    assert_eq!(CommitSource::Tab.slug(), "tab");
}

#[test]
fn a_commit_carries_its_id_source_and_the_label_the_field_settled_to() {
    let commit = AutoSuggestCommit::new("doc-notes".to_string(), CommitSource::Tab, "notes.md");

    assert_eq!(commit.id, "doc-notes");
    assert_eq!(commit.source, CommitSource::Tab);
    assert_eq!(commit.label, "notes.md");
}

#[test]
fn two_commits_of_different_sources_are_not_equal() {
    // The source is part of the identity: a host that logged "the user
    // picked X" must be able to tell an Enter from a click, and a test
    // that could not tell them apart would not be proving the mapping.
    let enter = AutoSuggestCommit::new("x".to_string(), CommitSource::Enter, "X");
    let click = AutoSuggestCommit::new("x".to_string(), CommitSource::Click, "X");
    let tab = AutoSuggestCommit::new("x".to_string(), CommitSource::Tab, "X");

    assert_ne!(enter, click);
    assert_ne!(click, tab);
    assert_eq!(
        enter,
        AutoSuggestCommit::new("x".to_string(), CommitSource::Enter, "X")
    );
}
