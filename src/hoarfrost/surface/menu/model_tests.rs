//! Proofs about the menu tree data model: every row kind's predicates, the
//! navigable subset against §4.9.1's four fixtures, close policy, radio
//! selection, path-derived identity, and the walk agreement with
//! [`FocusState`] (`UIF-FMENU-003`, `UIF-CMENU-002`, `UIF-CMENU-003`).

use crate::focus::FocusId;
use crate::focus::FocusState;
use crate::surface::menu::model::ActivationEffect;
use crate::surface::menu::model::ClosePolicy;
use crate::surface::menu::model::LeadingSlot;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::MenuId;
use crate::surface::menu::model::MenuViolation;
use crate::surface::menu::model::RadioGroupId;
use crate::surface::menu::model::Row;
use crate::surface::menu::model::TrailingSlot;
use crate::surface::menu::model::submenu_id;

// ---------------------------------------------------------------------------
// Fixtures — the four §4.9.1 target shapes, plus a kitchen sink
// ---------------------------------------------------------------------------

/// §4.9.1 "Single File (Tree)".
///
/// Index map: 0 heading · 1-3 Open & Output · 4 sep · 5 heading ·
/// 6-8 AI & Automation · 9 sep · 10 heading · 11-14 File Management ·
/// 15 sep · 16 heading · 17 Destructive.
fn single_file_menu() -> Menu {
    Menu::new(
        "ctx-file",
        vec![
            Row::heading("Open & Output"),
            Row::action("edit", "Edit"),
            Row::action("open-editor", "Open in Editor"),
            Row::action("print", "Print"),
            Row::separator(),
            Row::heading("AI & Automation"),
            Row::action("run-prompt", "Run as prompt"),
            Row::action("format-markdown", "Format Markdown"),
            Row::submenu(
                "note-skills",
                "Note Skills",
                Menu::new("skills", vec![Row::action("summarise", "Summarise")]),
            ),
            Row::separator(),
            Row::heading("File Management"),
            Row::action("rename", "Rename...").with_shortcut("F2"),
            Row::action("move", "Move..."),
            Row::action("copy-path", "Copy path"),
            Row::action("show-in-explorer", "Show in File Explorer"),
            Row::separator(),
            Row::heading("Destructive"),
            Row::action("delete", "Delete...").destructive(),
        ],
    )
}

/// §4.9.1 "Directory (Tree)".
fn directory_menu() -> Menu {
    Menu::new(
        "ctx-dir",
        vec![
            Row::heading("Creation"),
            Row::action("new-document", "New document..."),
            Row::action("create-directory", "Create Directory..."),
            Row::separator(),
            Row::heading("Folder Automation & Management"),
            Row::submenu(
                "folder-skills",
                "Folder Skills",
                Menu::new("skills", vec![Row::action("tidy", "Tidy up")]),
            ),
            Row::action("rename", "Rename..."),
            Row::action("move", "Move..."),
            Row::action("copy-path", "Copy path"),
            Row::action("show-in-explorer", "Show in File Explorer"),
            Row::separator(),
            Row::heading("Destructive"),
            Row::action("delete", "Delete...").destructive(),
        ],
    )
}

/// §4.9.1 "Multi-File Selection (Tree)".
fn multi_file_menu() -> Menu {
    Menu::new(
        "ctx-multi",
        vec![
            Row::heading("Batch AI"),
            Row::action("merge", "Merge"),
            Row::separator(),
            Row::heading("Destructive"),
            Row::action("delete", "Delete...").destructive(),
        ],
    )
}

/// §4.9.1 "Document Tab (Tab Strip)".
fn document_tab_menu() -> Menu {
    Menu::new(
        "ctx-tab",
        vec![
            Row::heading("Tab Lifecycle"),
            Row::action("close", "Close"),
            Row::action("close-others", "Close Others"),
            Row::action("close-all", "Close All"),
            Row::separator(),
            Row::heading("Edit & Automation"),
            Row::action("edit", "Edit"),
            Row::action("open-editor", "Open in Editor"),
            Row::action("format-markdown", "Format Markdown"),
            Row::submenu(
                "note-skills",
                "Note Skills",
                Menu::new("skills", vec![Row::action("summarise", "Summarise")]),
            ),
            Row::separator(),
            Row::heading("Location"),
            Row::action("copy-path", "Copy Path"),
            Row::action("show-in-explorer", "Show in File Explorer"),
        ],
    )
}

/// One row of every kind, enabled then disabled, so a single pass covers the
/// whole predicate matrix.
///
/// Index map: 0 heading · 1 action · 2 check · 3 radio · 4 submenu ·
/// 5 sep · 6 disabled action · 7 disabled check · 8 disabled radio ·
/// 9 disabled submenu · 10 destructive heading · 11 sep.
fn kitchen_sink_menu() -> Menu {
    Menu::new(
        "sink",
        vec![
            Row::heading("Enabled"),
            Row::action("a", "Action"),
            Row::checkable("c", "Check", false),
            Row::radio("r", "Radio", "mode", false),
            Row::submenu("s", "Submenu", Menu::empty("sink-child")),
            Row::separator(),
            Row::action("da", "Disabled action").disabled(),
            Row::checkable("dc", "Disabled check", true).disabled(),
            Row::radio("dr", "Disabled radio", "mode", true).disabled(),
            Row::submenu("ds", "Disabled submenu", Menu::empty("sink-child-2")).disabled(),
            Row::heading("Danger zone").destructive(),
            Row::separator(),
        ],
    )
}

/// A menu carrying two independent radio groups.
fn radio_menu() -> Menu {
    Menu::new(
        "radio",
        vec![
            Row::heading("Wrap mode"),
            Row::radio("wrap-off", "Off", "wrap", true),
            Row::radio("wrap-soft", "Soft wrap", "wrap", false),
            Row::radio("wrap-hard", "Hard wrap", "wrap", false),
            Row::separator(),
            Row::heading("Tab size"),
            Row::radio("tab-2", "2 spaces", "tabs", false),
            Row::radio("tab-4", "4 spaces", "tabs", true),
        ],
    )
}

/// The navigable indices a fixture is expected to produce.
fn navigable(menu: &Menu) -> Vec<usize> {
    menu.navigable_indices()
}

// ---------------------------------------------------------------------------
// Row kind predicates
// ---------------------------------------------------------------------------

#[test]
fn action_row_predicates() {
    let row = Row::action("edit", "Edit");

    assert!(row.interactive());
    assert!(row.activatable());
    assert!(row.navigable());
    assert!(row.is_enabled());
    assert!(!row.is_disabled());
    assert!(!row.is_destructive());
    assert!(!row.is_heading());
    assert!(!row.is_separator());
    assert!(!row.is_checkable());
    assert!(!row.is_radio());
    assert!(!row.is_submenu_parent());
    assert_eq!(row.key(), "edit");
    assert_eq!(row.label(), "Edit");
    assert_eq!(row.radio_group(), None);
    assert_eq!(row.child_id(), None);
}

#[test]
fn checkable_row_predicates() {
    let row = Row::checkable("wrap", "Wrap lines", true);

    assert!(row.interactive());
    assert!(row.activatable());
    assert!(row.navigable());
    assert!(row.is_checkable());
    assert!(row.is_checked());
    assert!(!row.is_radio());
    assert!(!row.is_submenu_parent());
    assert_eq!(row.leading_slot(), LeadingSlot::Check);
}

#[test]
fn radio_row_predicates() {
    let row = Row::radio("soft", "Soft wrap", "wrap", true);

    assert!(row.interactive());
    assert!(row.activatable());
    assert!(row.navigable());
    assert!(row.is_radio());
    assert!(row.is_selected());
    assert!(!row.is_checkable());
    assert_eq!(row.radio_group(), Some(&RadioGroupId::new("wrap")));
    assert_eq!(row.leading_slot(), LeadingSlot::Radio);
}

#[test]
fn submenu_row_predicates() {
    let row = Row::submenu(
        "more",
        "More...",
        Menu::new("child", vec![Row::action("x", "X")]),
    );

    assert!(row.interactive());
    assert!(row.activatable());
    assert!(row.navigable());
    assert!(row.is_submenu_parent());
    assert!(!row.is_checkable());
    assert!(!row.is_radio());
    assert!(row.child().is_some());
    assert_eq!(row.trailing_slot(), TrailingSlot::Chevron);
}

#[test]
fn heading_row_predicates() {
    let row = Row::heading("Section");

    assert!(!row.interactive());
    assert!(!row.activatable());
    assert!(!row.navigable());
    assert!(row.is_heading());
    assert_eq!(row.key(), "");
    assert_eq!(row.leading_slot(), LeadingSlot::None);
    assert_eq!(row.trailing_slot(), TrailingSlot::None);
}

#[test]
fn separator_row_predicates() {
    let row = Row::separator();

    assert!(!row.interactive());
    assert!(!row.activatable());
    assert!(!row.navigable());
    assert!(row.is_separator());
    assert_eq!(row.leading_slot(), LeadingSlot::None);
    assert_eq!(row.trailing_slot(), TrailingSlot::None);
}

#[test]
fn disabled_rows_stay_interactive_but_lose_activation_and_navigability() {
    let rows = [
        Row::action("a", "Action").disabled(),
        Row::checkable("c", "Check", false).disabled(),
        Row::radio("r", "Radio", "g", false).disabled(),
        Row::submenu("s", "Sub", Menu::empty("kid")).disabled(),
    ];

    for row in &rows {
        assert!(
            row.interactive(),
            "a disabled {} row is still that kind of row",
            row.label()
        );
        assert!(!row.activatable(), "{} must not activate", row.label());
        assert!(!row.navigable(), "{} must not be navigable", row.label());
        assert!(row.is_disabled());
    }
}

#[test]
fn heading_and_separator_stay_inert_under_every_modifier() {
    for row in [
        Row::heading("H").disabled(),
        Row::heading("H").destructive(),
        Row::separator().disabled(),
        Row::separator().destructive(),
    ] {
        assert!(!row.interactive());
        assert!(!row.activatable());
        assert!(!row.navigable());
    }
}

#[test]
fn destructive_is_orthogonal_to_kind() {
    let destructive_action = Row::action("del", "Delete").destructive();
    let destructive_check = Row::checkable("quit", "Close all and quit", false).destructive();
    let destructive_radio = Row::radio("hard", "Hard delete", "mode", false).destructive();
    let destructive_submenu =
        Row::submenu("danger", "Dangerous...", Menu::empty("d")).destructive();

    assert!(destructive_action.is_destructive());
    assert!(!destructive_action.is_checkable());
    assert!(destructive_check.is_destructive() && destructive_check.is_checkable());
    assert!(destructive_radio.is_destructive() && destructive_radio.is_radio());
    assert!(destructive_submenu.is_destructive() && destructive_submenu.is_submenu_parent());

    for row in [
        &destructive_action,
        &destructive_check,
        &destructive_radio,
        &destructive_submenu,
    ] {
        assert!(row.interactive() && row.activatable() && row.navigable());
    }
}

#[test]
fn with_destructive_can_turn_the_flag_off_again() {
    let row = Row::action("del", "Delete")
        .destructive()
        .with_destructive(false);

    assert!(!row.is_destructive());
}

#[test]
fn with_enabled_controls_the_enabled_flag_directly() {
    assert!(Row::action("a", "A").with_enabled(true).is_enabled());
    assert!(Row::action("a", "A").with_enabled(false).is_disabled());
}

#[test]
fn leading_slot_is_the_icon_only_when_one_was_supplied() {
    assert_eq!(Row::action("a", "A").leading_slot(), LeadingSlot::None);
    assert_eq!(
        Row::action("a", "A").with_icon("edit").leading_slot(),
        LeadingSlot::Icon
    );
    assert_eq!(Row::heading("H").leading_slot(), LeadingSlot::None);
}

#[test]
fn trailing_slot_prefers_the_submenu_chevron_over_a_shortcut() {
    let row = Row::submenu("more", "More...", Menu::empty("kid")).with_shortcut("->");

    assert_eq!(row.trailing_slot(), TrailingSlot::Chevron);
    assert_eq!(row.shortcut(), Some("->"));
}

#[test]
fn trailing_slot_is_a_shortcut_hint_when_one_was_supplied() {
    assert_eq!(
        Row::action("rename", "Rename...")
            .with_shortcut("F2")
            .trailing_slot(),
        TrailingSlot::Shortcut
    );
    assert_eq!(
        Row::action("edit", "Edit").trailing_slot(),
        TrailingSlot::None
    );
}

// ---------------------------------------------------------------------------
// Close policy (UIF-FMENU-003)
// ---------------------------------------------------------------------------

#[test]
fn every_kind_gets_its_documented_default_close_policy() {
    assert_eq!(
        Row::action("a", "A").close_policy(),
        ClosePolicy::Close,
        "an Action emits its command and closes the tree immediately"
    );
    assert_eq!(
        Row::checkable("c", "C", false).close_policy(),
        ClosePolicy::StayOpen,
        "a checkable stays open so several toggles fit in one visit"
    );
    assert_eq!(
        Row::radio("r", "R", "g", false).close_policy(),
        ClosePolicy::Close
    );
    assert_eq!(
        Row::submenu("s", "S", Menu::empty("k")).close_policy(),
        ClosePolicy::StayOpen
    );
    assert_eq!(Row::heading("H").close_policy(), ClosePolicy::StayOpen);
    assert_eq!(Row::separator().close_policy(), ClosePolicy::StayOpen);
}

#[test]
fn checkable_stays_open_by_default() {
    let mut menu = Menu::new("m", vec![Row::checkable("wrap", "Wrap", false)]);

    let activation = menu.activate(0).expect("row 0 exists");

    assert_eq!(
        activation.effect,
        ActivationEffect::Toggled { checked: true }
    );
    assert!(
        !activation.closes_menu,
        "UIF-FMENU-003: the checkmark updates in place and the menu stays open"
    );
    assert!(menu.row(0).is_some_and(|row| row.is_checked()));
}

#[test]
fn checkable_close_on_toggle_is_a_caller_option() {
    let mut menu = Menu::new(
        "m",
        vec![Row::checkable("wrap", "Wrap", false).close_on_toggle()],
    );

    let activation = menu.activate(0).expect("row 0 exists");

    assert_eq!(
        activation.effect,
        ActivationEffect::Toggled { checked: true }
    );
    assert!(
        activation.closes_menu,
        "the caller asked for close-on-toggle and the model must honour it"
    );
}

#[test]
fn toggling_a_checkable_twice_returns_it_to_its_original_state() {
    let mut menu = Menu::new("m", vec![Row::checkable("wrap", "Wrap", false)]);

    let first = menu.activate(0).unwrap();
    let second = menu.activate(0).unwrap();

    assert_eq!(first.effect, ActivationEffect::Toggled { checked: true });
    assert_eq!(second.effect, ActivationEffect::Toggled { checked: false });
    assert!(!menu.row(0).unwrap().is_checked());
}

#[test]
fn set_checked_syncs_a_toggle_without_activating_it() {
    let mut menu = Menu::new("m", vec![Row::checkable("wrap", "Wrap", false)]);

    assert_eq!(menu.set_checked(0, true), Some(false));
    assert_eq!(menu.set_checked(0, true), Some(true));
    assert_eq!(menu.set_checked(1, true), None, "out of range");
    assert_eq!(
        menu.set_checked(0, false),
        Some(true),
        "the previous value comes back so the caller can undo"
    );
}

#[test]
fn set_checked_refuses_a_row_that_is_not_a_toggle() {
    let mut menu = Menu::new("m", vec![Row::action("a", "A")]);

    assert_eq!(menu.set_checked(0, true), None);
}

#[test]
fn action_closes_by_default_and_can_be_told_to_stay() {
    let closing = Row::action("a", "A");
    let staying = Row::action("b", "B").stays_open();

    assert!(closing.closes_menu_on_activate());
    assert!(!staying.closes_menu_on_activate());
    assert_eq!(staying.close_policy(), ClosePolicy::StayOpen);
}

#[test]
fn radio_closes_by_default_and_submenu_never_does() {
    assert!(Row::radio("r", "R", "g", false).closes_menu_on_activate());
    assert!(!Row::submenu("s", "S", Menu::empty("k")).closes_menu_on_activate());
    assert!(
        !Row::submenu("s", "S", Menu::empty("k")).closes_menu_on_activate(),
        "opening a branch dismisses nothing"
    );
}

#[test]
fn with_close_policy_overrides_any_kind() {
    let row = Row::checkable("c", "C", false).with_close_policy(ClosePolicy::Close);

    assert_eq!(row.close_policy(), ClosePolicy::Close);
    assert!(row.closes_menu_on_activate());
}

#[test]
fn a_disabled_row_absorbs_the_click_and_does_not_close_the_menu() {
    let kinds = [
        Row::action("a", "Action").disabled(),
        Row::checkable("c", "Check", false).disabled(),
        Row::radio("r", "Radio", "g", false).disabled(),
        Row::submenu("s", "Sub", Menu::empty("k")).disabled(),
    ];

    for (offset, row) in kinds.into_iter().enumerate() {
        let mut menu = Menu::new("m", vec![row]);
        let activation = menu.activate(0).expect("the row exists");

        assert_eq!(
            activation.effect,
            ActivationEffect::Absorbed,
            "UIF-CMENU-002: a disabled row must absorb the click"
        );
        assert!(activation.is_absorbed());
        assert!(
            !activation.closes_menu,
            "a disabled row at offset {offset} must not dismiss the menu"
        );
    }
}

#[test]
fn a_disabled_checkable_does_not_flip_when_clicked() {
    let mut menu = Menu::new("m", vec![Row::checkable("c", "C", false).disabled()]);

    let _ = menu.activate(0);

    assert!(
        !menu.row(0).unwrap().is_checked(),
        "absorption means no state change, not a silent toggle"
    );
}

#[test]
fn heading_and_separator_activations_are_absorbed() {
    let mut menu = Menu::new("m", vec![Row::heading("H"), Row::separator()]);

    for index in 0..menu.len() {
        let activation = menu.activate(index).expect("the row exists");
        assert_eq!(activation.effect, ActivationEffect::Absorbed);
        assert!(!activation.closes_menu);
    }
}

#[test]
fn activating_an_out_of_range_row_reports_nothing() {
    let mut menu = Menu::new("m", vec![Row::action("a", "A")]);

    assert_eq!(menu.activate(1), None);
    assert_eq!(menu.activate(99), None);
}

#[test]
fn activation_reports_the_command_effect_for_an_action() {
    let mut menu = Menu::new("m", vec![Row::action("print", "Print")]);

    let activation = menu.activate(0).unwrap();

    assert_eq!(activation.row, 0);
    assert_eq!(activation.effect, ActivationEffect::Command);
    assert!(activation.closes_menu);
    assert!(!activation.is_absorbed());
}

#[test]
fn activating_a_submenu_parent_reports_the_child_id_it_owns() {
    let mut menu = Menu::new(
        "root",
        vec![Row::submenu("more", "More...", Menu::empty("given"))],
    );

    let activation = menu.activate(0).unwrap();

    assert_eq!(
        activation.effect,
        ActivationEffect::OpenSubmenu {
            child: MenuId::new("root/more"),
        }
    );
    assert!(
        !activation.closes_menu,
        "the parent card stays visible and interactive while the child opens"
    );
}

// ---------------------------------------------------------------------------
// Radio groups
// ---------------------------------------------------------------------------

#[test]
fn selecting_a_radio_clears_its_siblings() {
    let mut menu = radio_menu();
    let wrap = RadioGroupId::new("wrap");

    let cleared = menu.select_radio(3).expect("row 3 is a radio member");

    assert_eq!(
        cleared,
        vec![1],
        "the previously selected member must come back so the caller never diffs"
    );
    assert!(!menu.row(1).unwrap().is_selected());
    assert!(!menu.row(2).unwrap().is_selected());
    assert!(menu.row(3).unwrap().is_selected());
    assert_eq!(menu.radio_selection(&wrap), Some(3));
}

#[test]
fn exactly_one_radio_stays_selected_while_walking_the_group() {
    let mut menu = radio_menu();
    let wrap = RadioGroupId::new("wrap");

    for index in [1usize, 2, 3, 1, 3, 2] {
        let _ = menu.select_radio(index);

        let selected: Vec<usize> = menu
            .radio_members(&wrap)
            .into_iter()
            .filter(|member| menu.row(*member).is_some_and(|row| row.is_selected()))
            .collect();

        assert_eq!(
            selected,
            vec![index],
            "after selecting {index} exactly one member of `wrap` is selected"
        );
    }
}

#[test]
fn reselecting_the_selected_member_is_a_no_op_on_the_selection() {
    let mut menu = radio_menu();
    let wrap = RadioGroupId::new("wrap");

    let cleared = menu.select_radio(1).expect("row 1 is already selected");

    assert!(
        cleared.is_empty(),
        "re-selecting must not clear the group: the `•` always marks the current value"
    );
    assert_eq!(menu.radio_selection(&wrap), Some(1));
}

#[test]
fn reselecting_still_reports_a_selection_effect_and_closes() {
    let mut menu = radio_menu();

    let activation = menu.activate(1).expect("row 1 exists");

    assert_eq!(
        activation.effect,
        ActivationEffect::Selected {
            group: RadioGroupId::new("wrap"),
            cleared: vec![],
        },
        "the caller sees one uniform outcome whether or not anything changed"
    );
    assert!(
        activation.closes_menu,
        "UIF-FMENU-003: radio closes by default"
    );
}

#[test]
fn radio_groups_are_independent() {
    let mut menu = radio_menu();
    let wrap = RadioGroupId::new("wrap");
    let tabs = RadioGroupId::new("tabs");

    let cleared = menu.select_radio(2).unwrap();

    assert_eq!(cleared, vec![1]);
    assert_eq!(menu.radio_selection(&wrap), Some(2));
    assert_eq!(
        menu.radio_selection(&tabs),
        Some(7),
        "selecting in `wrap` must not touch `tabs`"
    );
}

#[test]
fn activating_a_radio_reports_the_cleared_siblings() {
    let mut menu = radio_menu();

    let activation = menu.activate(3).unwrap();

    assert_eq!(
        activation.effect,
        ActivationEffect::Selected {
            group: RadioGroupId::new("wrap"),
            cleared: vec![1],
        }
    );
    assert!(activation.closes_menu);
}

#[test]
fn select_radio_refuses_rows_that_are_not_radio_members() {
    let mut menu = radio_menu();

    assert_eq!(
        menu.select_radio(0),
        None,
        "a heading is not a radio member"
    );
    assert_eq!(menu.select_radio(99), None, "out of range");
}

#[test]
fn radio_group_info_lists_members_and_selection() {
    let menu = radio_menu();
    let tabs = RadioGroupId::new("tabs");

    let info = menu.radio_group(&tabs).expect("`tabs` exists");

    assert_eq!(info.id, tabs);
    assert_eq!(info.members, vec![6, 7]);
    assert_eq!(info.selected, Some(7));
    assert!(info.is_selected(7));
    assert!(!info.is_selected(6));
}

#[test]
fn an_unknown_radio_group_has_no_members_and_no_selection() {
    let menu = radio_menu();
    let unknown = RadioGroupId::new("nope");

    assert!(menu.radio_group(&unknown).is_none());
    assert_eq!(menu.radio_selection(&unknown), None);
    assert!(menu.radio_members(&unknown).is_empty());
}

#[test]
fn radio_group_ids_are_listed_once_in_first_appearance_order() {
    let menu = radio_menu();

    assert_eq!(
        menu.radio_group_ids(),
        vec![RadioGroupId::new("wrap"), RadioGroupId::new("tabs")]
    );
}

// ---------------------------------------------------------------------------
// The navigable subset — §4.9.1's four fixtures
// ---------------------------------------------------------------------------

#[test]
fn single_file_fixture_visits_exactly_its_eleven_live_rows() {
    let menu = single_file_menu();

    assert_eq!(menu.len(), 18);
    assert_eq!(
        navigable(&menu),
        vec![1, 2, 3, 6, 7, 8, 11, 12, 13, 14, 17],
        "headings (0, 5, 10, 16) and separators (4, 9, 15) are skipped"
    );
}

#[test]
fn directory_fixture_visits_exactly_its_eight_live_rows() {
    let menu = directory_menu();

    assert_eq!(menu.len(), 13);
    assert_eq!(navigable(&menu), vec![1, 2, 5, 6, 7, 8, 9, 12]);
}

#[test]
fn multi_file_fixture_visits_only_merge_and_delete() {
    let menu = multi_file_menu();

    assert_eq!(menu.len(), 5);
    assert_eq!(navigable(&menu), vec![1, 4]);
}

#[test]
fn document_tab_fixture_visits_its_nine_live_rows() {
    let menu = document_tab_menu();

    assert_eq!(menu.len(), 14);
    assert_eq!(navigable(&menu), vec![1, 2, 3, 6, 7, 8, 9, 12, 13]);
}

#[test]
fn kitchen_sink_skips_every_heading_separator_and_disabled_row() {
    let menu = kitchen_sink_menu();

    assert_eq!(
        navigable(&menu),
        vec![1, 2, 3, 4],
        "rows 6-9 are disabled and 0, 5, 10, 11 are furniture"
    );
}

#[test]
fn the_navigable_subset_is_exactly_the_enabled_interactive_rows() {
    let menu = kitchen_sink_menu();

    let expected: Vec<usize> = menu
        .rows()
        .iter()
        .enumerate()
        .filter(|(_, row)| row.interactive() && row.is_enabled())
        .map(|(index, _)| index)
        .collect();

    assert_eq!(navigable(&menu), expected);
}

#[test]
fn focus_members_match_the_navigable_ids_and_are_all_enabled() {
    for menu in [
        single_file_menu(),
        directory_menu(),
        multi_file_menu(),
        document_tab_menu(),
        kitchen_sink_menu(),
    ] {
        let members = menu.focus_members();

        assert_eq!(
            members
                .iter()
                .map(|member| member.id.as_str())
                .collect::<Vec<_>>(),
            menu.navigable_focus_ids()
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>(),
            "the roster and the navigable id list must agree for `{}`",
            menu.id()
        );
        assert!(
            members.iter().all(|member| member.enabled),
            "every registered member is enabled"
        );
    }
}

#[test]
fn non_navigable_rows_are_absent_from_the_roster_not_registered_disabled() {
    let menu = kitchen_sink_menu();
    let roster: Vec<String> = menu
        .focus_members()
        .into_iter()
        .map(|member| member.id.to_string())
        .collect();

    for (index, row) in menu.rows().iter().enumerate() {
        let id = menu.focus_id(index).unwrap().to_string();
        if row.navigable() {
            assert!(roster.contains(&id), "row {index} must be registered");
        } else {
            assert!(
                !roster.contains(&id),
                "row {index} ({} / {}) must not appear in the roster at all",
                row.label(),
                "kind"
            );
        }
    }

    assert!(
        roster.len() == menu.rows().iter().filter(|row| row.navigable()).count(),
        "the roster holds nothing but navigable rows, so its length is the keyboard's cycle length"
    );
}

#[test]
fn a_menu_with_no_navigable_rows_produces_an_empty_roster() {
    let menu = Menu::new("furniture", vec![Row::heading("H"), Row::separator()]);

    assert!(menu.focus_members().is_empty());
    assert!(menu.navigable_indices().is_empty());
    assert_eq!(menu.first_navigable(), None);
    assert_eq!(menu.last_navigable(), None);
}

#[test]
fn first_and_last_navigable_bracket_the_subset() {
    let menu = single_file_menu();

    assert_eq!(menu.first_navigable(), Some(1));
    assert_eq!(menu.last_navigable(), Some(17));
}

// ---------------------------------------------------------------------------
// Stepping the navigable subset
// ---------------------------------------------------------------------------

#[test]
fn stepping_forward_from_outside_enters_at_the_first_row() {
    let menu = single_file_menu();

    assert_eq!(menu.step_navigable(None, 1), Some(1));
}

#[test]
fn stepping_backward_from_outside_enters_at_the_last_row() {
    let menu = single_file_menu();

    assert_eq!(menu.step_navigable(None, -1), Some(17));
}

#[test]
fn stepping_forward_visits_every_navigable_row_in_order() {
    let menu = single_file_menu();
    let order = navigable(&menu);

    let mut current = None;
    let mut visited = Vec::new();
    for _ in 0..order.len() {
        current = menu.step_navigable(current, 1);
        visited.push(current.expect("the menu has navigable rows"));
    }

    assert_eq!(visited, order);
}

#[test]
fn stepping_backward_visits_every_navigable_row_in_reverse() {
    let menu = single_file_menu();
    let order = navigable(&menu);

    let mut current = Some(*order.last().unwrap());
    let mut visited = Vec::new();
    for _ in 0..order.len() - 1 {
        current = menu.step_navigable(current, -1);
        visited.push(current.expect("the menu has navigable rows"));
    }

    let mut expected = order.clone();
    expected.pop();
    expected.reverse();
    assert_eq!(visited, expected);
}

#[test]
fn stepping_wraps_past_both_ends() {
    let menu = single_file_menu();

    assert_eq!(
        menu.step_navigable(Some(17), 1),
        Some(1),
        "Down wraps to the top"
    );
    assert_eq!(
        menu.step_navigable(Some(1), -1),
        Some(17),
        "Up wraps to the bottom"
    );
}

#[test]
fn stepping_skips_disabled_rows_in_both_directions() {
    let menu = Menu::new(
        "m",
        vec![
            Row::action("a", "A"),
            Row::action("b", "B").disabled(),
            Row::heading("H"),
            Row::separator(),
            Row::action("c", "C"),
        ],
    );

    assert_eq!(menu.step_navigable(Some(0), 1), Some(4));
    assert_eq!(menu.step_navigable(Some(4), -1), Some(0));
}

#[test]
fn a_zero_delta_never_moves_but_confirms_a_live_position() {
    let menu = single_file_menu();

    assert_eq!(menu.step_navigable(Some(6), 0), Some(6));
    assert_eq!(
        menu.step_navigable(Some(0), 0),
        None,
        "a heading is not a position the keyboard can hold"
    );
    assert_eq!(menu.step_navigable(None, 0), None);
}

#[test]
fn stepping_an_empty_subset_is_none() {
    let menu = Menu::new("furniture", vec![Row::heading("H")]);

    assert_eq!(menu.step_navigable(None, 1), None);
    assert_eq!(menu.step_navigable(Some(0), 1), None);
}

#[test]
fn a_large_delta_wraps_by_the_subset_length_not_the_row_count() {
    let menu = single_file_menu();
    let order = navigable(&menu);
    let len = order.len();

    assert_eq!(
        menu.step_navigable(Some(order[0]), i32::try_from(len).unwrap()),
        Some(order[0]),
        "a full cycle lands back where it started"
    );
    assert_eq!(
        menu.step_navigable(Some(order[0]), i32::try_from(len + 1).unwrap()),
        Some(order[1])
    );
}

// ---------------------------------------------------------------------------
// Integration with FocusState
// ---------------------------------------------------------------------------

#[test]
fn focus_state_walk_matches_the_model_navigable_subset() {
    let menu = single_file_menu();
    let group = menu.group_id();
    let expected = menu.navigable_focus_ids();

    let mut focus = FocusState::new();
    focus.register_group(group.clone(), menu.focus_members());

    let mut visited = Vec::new();
    for _ in 0..expected.len() {
        visited.push(
            focus
                .move_within(&group, 1)
                .expect("the group has enabled members"),
        );
    }

    assert_eq!(
        visited, expected,
        "the roving walk must visit exactly the rows the model calls navigable"
    );
}

#[test]
fn focus_state_wrap_agrees_with_step_navigable() {
    let menu = directory_menu();
    let group = menu.group_id();
    let order = navigable(&menu);

    let mut focus = FocusState::new();
    focus.register_group(group.clone(), menu.focus_members());

    let mut model_current = None;
    for _ in 0..order.len() + 2 {
        let model_next = menu.step_navigable(model_current, 1);
        let focus_next = focus.move_within(&group, 1);

        assert_eq!(
            focus_next
                .as_ref()
                .and_then(|id| menu.index_of_focus_id(id)),
            model_next,
            "model and `FocusState` must land on the same row every step"
        );

        model_current = model_next;
    }
}

#[test]
fn focus_state_wraps_at_the_bottom_the_same_way_the_model_does() {
    let menu = multi_file_menu();
    let group = menu.group_id();

    let mut focus = FocusState::new();
    focus.register_group(group.clone(), menu.focus_members());

    assert_eq!(focus.move_within(&group, 1), menu.focus_id(1));
    assert_eq!(focus.move_within(&group, 1), menu.focus_id(4));
    assert_eq!(
        focus.move_within(&group, 1),
        menu.focus_id(1),
        "UIF-CMENU-003 cycles: past the last row comes the first"
    );
}

#[test]
fn entering_a_freshly_opened_group_lands_first_going_up_lands_last() {
    let menu = single_file_menu();
    let group = menu.group_id();

    let mut down = FocusState::new();
    down.register_group(group.clone(), menu.focus_members());
    assert_eq!(down.move_within(&group, 1), menu.focus_id(1));

    let mut up = FocusState::new();
    up.register_group(group.clone(), menu.focus_members());
    assert_eq!(up.move_within(&group, -1), menu.focus_id(17));
}

#[test]
fn disabling_a_focused_row_falls_forward_onto_the_next_navigable_one() {
    let before = Menu::new(
        "m",
        vec![
            Row::heading("H"),
            Row::action("a", "A"),
            Row::action("b", "B"),
            Row::action("c", "C"),
        ],
    );
    let after = Menu::new(
        "m",
        vec![
            Row::heading("H"),
            Row::action("a", "A"),
            Row::action("b", "B").disabled(),
            Row::action("c", "C"),
        ],
    );
    let group = before.group_id();

    let mut focus = FocusState::new();
    focus.register_group(group.clone(), before.focus_members());
    assert!(focus.focus(&before.focus_id(2).unwrap()));

    focus.register_group(group.clone(), after.focus_members());

    assert_eq!(
        focus.current(),
        after.focus_id(3).as_ref(),
        "the slot survives: row 1 of the enabled list is now `C`"
    );
}

// ---------------------------------------------------------------------------
// Identity: path-derived, rebuild-stable
// ---------------------------------------------------------------------------

#[test]
fn focus_ids_are_stable_across_an_equivalent_rebuild() {
    let first = single_file_menu();
    let second = single_file_menu();

    assert_eq!(
        first.navigable_focus_ids(),
        second.navigable_focus_ids(),
        "an equivalent rebuild must claim the same ids or the focus model loses the position"
    );
    assert_eq!(first.group_id(), second.group_id());
}

#[test]
fn focus_id_is_the_group_id_plus_the_row_key() {
    let menu = single_file_menu();

    assert_eq!(menu.group_id().as_str(), "menu:ctx-file");
    assert_eq!(
        menu.focus_id(1).unwrap().as_str(),
        "menu:ctx-file#edit",
        "the id is readable from the path and the key alone"
    );
}

#[test]
fn index_of_focus_id_round_trips_every_navigable_row() {
    let menu = single_file_menu();

    for index in navigable(&menu) {
        let id = menu.focus_id(index).unwrap();
        assert_eq!(menu.index_of_focus_id(&id), Some(index));
    }
}

#[test]
fn a_furniture_row_resolves_to_no_index() {
    let menu = single_file_menu();

    let heading_id = menu.focus_id(0).unwrap();
    assert_eq!(
        menu.index_of_focus_id(&heading_id),
        None,
        "a heading carries no key, so its id names no row"
    );
}

#[test]
fn an_unknown_focus_id_resolves_to_no_index() {
    let menu = single_file_menu();

    assert_eq!(
        menu.index_of_focus_id(&FocusId::new("menu:ctx-file#nope")),
        None
    );
    assert_eq!(
        menu.index_of_focus_id(&FocusId::new("menu:other#edit")),
        None
    );
}

#[test]
fn two_menus_with_the_same_keys_have_disjoint_ids() {
    let a = Menu::new("a", vec![Row::action("edit", "Edit")]);
    let b = Menu::new("b", vec![Row::action("edit", "Edit")]);

    assert_ne!(a.focus_id(0), b.focus_id(0));
    assert_eq!(a.index_of_focus_id(&b.focus_id(0).unwrap()), None);
}

#[test]
fn attaching_a_child_rebases_the_id_it_was_built_with() {
    let child = Menu::empty("whatever-the-caller-called-it");
    let menu = Menu::new("root", vec![Row::submenu("more", "More...", child)]);

    assert_eq!(
        menu.row(0).unwrap().child_id().unwrap().as_str(),
        "root/more",
        "the child's own id is discarded so identity cannot collide"
    );
}

#[test]
fn the_submenu_id_helper_matches_what_the_binding_produced() {
    let menu = Menu::new(
        "root",
        vec![Row::submenu("more", "More...", Menu::empty("x"))],
    );

    assert_eq!(
        menu.row(0).unwrap().child_id().unwrap(),
        &submenu_id(menu.id(), "more")
    );
}

#[test]
fn the_same_child_value_under_two_parents_gets_distinct_ids() {
    let shared = Menu::new("shared", vec![Row::action("go", "Go")]);

    let a = Menu::new("a", vec![Row::submenu("k", "K", shared.clone())]);
    let b = Menu::new("b", vec![Row::submenu("k", "K", shared.clone())]);

    assert_eq!(a.row(0).unwrap().child_id().unwrap().as_str(), "a/k");
    assert_eq!(b.row(0).unwrap().child_id().unwrap().as_str(), "b/k");
    assert_ne!(
        a.row(0).unwrap().child_id(),
        b.row(0).unwrap().child_id(),
        "one child value mounted twice must not produce one shared identity"
    );
}

#[test]
fn nested_children_carry_their_full_path() {
    let deep = Menu::new("deep-id", vec![Row::action("go", "Go")]);
    let mid = Menu::new("mid-id", vec![Row::submenu("d", "Deep", deep)]);
    let root = Menu::new("root", vec![Row::submenu("m", "Mid", mid)]);

    let mid_child = root.row(0).unwrap().child().unwrap();
    let deep_child = mid_child.row(0).unwrap().child().unwrap();

    assert_eq!(mid_child.id().as_str(), "root/m");
    assert_eq!(deep_child.id().as_str(), "root/m/d");
    assert_eq!(
        deep_child.focus_id(0).unwrap().as_str(),
        "menu:root/m/d#go",
        "a grandchild's focus id carries the whole path down to it"
    );
}

#[test]
fn child_rows_are_reachable_and_their_parent_identity_is_stable() {
    let build = || {
        Menu::new(
            "root",
            vec![
                Row::action("top", "Top"),
                Row::submenu(
                    "sub",
                    "Sub",
                    Menu::new("ignored", vec![Row::action("inner", "Inner")]),
                ),
            ],
        )
    };

    let first = build();
    let second = build();

    assert_eq!(first.id(), second.id());
    assert_eq!(
        first.row(1).unwrap().child_id(),
        second.row(1).unwrap().child_id()
    );
    assert_eq!(
        first.row(1).unwrap().child().unwrap().rows(),
        second.row(1).unwrap().child().unwrap().rows()
    );
    assert_eq!(
        first.row(1).unwrap().child().unwrap().first_navigable(),
        Some(0)
    );
}

#[test]
fn a_child_menu_has_its_own_group_id_once_rebased() {
    let menu = Menu::new(
        "root",
        vec![Row::submenu("more", "More...", Menu::empty("x"))],
    );
    let child = menu.row(0).unwrap().child().unwrap();

    assert_eq!(child.group_id().as_str(), "menu:root/more");
    assert_ne!(child.group_id(), menu.group_id());
    assert_eq!(
        child.focus_id(0),
        None,
        "the child is empty, so it registers nothing"
    );
}

// ---------------------------------------------------------------------------
// Semantic grouping (§4.9.1)
// ---------------------------------------------------------------------------

#[test]
fn separators_split_the_menu_into_its_functional_groups() {
    let menu = single_file_menu();
    let groups = menu.groups();

    assert_eq!(groups.len(), 4);
    assert_eq!(
        groups
            .iter()
            .map(|group| (group.start, group.end))
            .collect::<Vec<_>>(),
        vec![(0, 4), (5, 9), (10, 15), (16, 18)]
    );
}

#[test]
fn separators_themselves_belong_to_no_group() {
    let menu = single_file_menu();

    for group in menu.groups() {
        for index in group.start..group.end {
            assert!(
                !menu.row(index).unwrap().is_separator(),
                "separator at {index} leaked into a group"
            );
        }
    }
}

#[test]
fn a_group_counts_its_interactive_and_destructive_rows() {
    let menu = single_file_menu();
    let groups = menu.groups();

    assert_eq!(groups[0].interactive, vec![1, 2, 3]);
    assert!(groups[0].destructive.is_empty());
    assert!(groups[0].has_interactive());
    assert!(!groups[0].has_destructive());
    assert_eq!(groups[0].len(), 4);
    assert!(!groups[0].is_empty());

    let last = groups.last().unwrap();
    assert_eq!(last.interactive, vec![17]);
    assert_eq!(last.destructive, vec![17]);
    assert!(last.has_destructive());
    assert!(!last.mixes_destructive());
}

#[test]
fn a_heading_only_group_has_no_interactive_content() {
    let menu = Menu::new(
        "m",
        vec![
            Row::action("a", "A"),
            Row::separator(),
            Row::heading("Footer"),
        ],
    );
    let groups = menu.groups();

    assert_eq!(groups.len(), 2);
    assert!(!groups[1].has_interactive());
    assert_eq!(groups[1].len(), 1);
}

#[test]
fn groups_split_on_every_separator_the_menu_carries() {
    let menu = document_tab_menu();
    let separated = Menu::new("m", vec![Row::action("a", "A"), Row::separator()]);

    assert_eq!(
        menu.groups()
            .iter()
            .map(|group| (group.start, group.end))
            .collect::<Vec<_>>(),
        vec![(0, 4), (5, 10), (11, 14)],
        "two separators make three groups out of the tab fixture"
    );
    assert_eq!(separated.groups().len(), 1);
}

// ---------------------------------------------------------------------------
// Validation — catching a malformed menu
// ---------------------------------------------------------------------------

#[test]
fn the_four_spec_fixtures_are_well_formed() {
    for menu in [
        single_file_menu(),
        directory_menu(),
        multi_file_menu(),
        document_tab_menu(),
    ] {
        assert!(
            menu.validate().is_empty(),
            "`{}` should be well formed, got {:?}",
            menu.id(),
            menu.validate()
        );
        assert!(menu.is_valid());
    }
}

#[test]
fn a_destructive_row_before_a_non_destructive_one_is_detected() {
    let menu = Menu::new(
        "bad",
        vec![
            Row::action("del", "Delete").destructive(),
            Row::separator(),
            Row::action("copy", "Copy"),
        ],
    );

    assert_eq!(
        menu.validate(),
        vec![MenuViolation::DestructiveNotLast { row: 0, group: 0 }]
    );
    assert!(!menu.is_valid());
}

#[test]
fn a_group_that_mixes_destructive_and_plain_rows_is_detected() {
    let menu = Menu::new(
        "bad",
        vec![
            Row::action("copy", "Copy"),
            Row::action("del", "Delete").destructive(),
        ],
    );

    assert_eq!(
        menu.validate(),
        vec![MenuViolation::MixedGroup { group: 0, row: 1 }]
    );
}

#[test]
fn a_trailing_heading_only_group_does_not_break_destructive_last() {
    let menu = Menu::new(
        "ok",
        vec![
            Row::action("copy", "Copy"),
            Row::separator(),
            Row::action("del", "Delete").destructive(),
            Row::separator(),
            Row::heading("Footer"),
        ],
    );

    assert!(
        menu.validate().is_empty(),
        "the last *interactive* group is the destructive one; a trailing heading is furniture"
    );
}

#[test]
fn a_destructive_heading_does_not_count_as_a_destructive_action() {
    let menu = Menu::new(
        "ok",
        vec![
            Row::heading("Danger zone").destructive(),
            Row::action("copy", "Copy"),
        ],
    );

    assert!(menu.validate().is_empty());
    assert!(!menu.groups()[0].has_destructive());
}

#[test]
fn an_interactive_row_without_a_key_is_detected() {
    let menu = Menu::new("bad", vec![Row::action("", "Nameless")]);

    assert_eq!(
        menu.validate(),
        vec![MenuViolation::MissingKey {
            row: 0,
            label: "Nameless".to_owned(),
        }]
    );
}

#[test]
fn furniture_rows_may_share_an_empty_key() {
    let menu = Menu::new(
        "ok",
        vec![Row::heading("A"), Row::heading("B"), Row::separator()],
    );

    assert!(menu.validate().is_empty());
}

#[test]
fn two_rows_sharing_a_key_are_detected_against_the_first_claimant() {
    let menu = Menu::new(
        "bad",
        vec![Row::action("dup", "First"), Row::action("dup", "Second")],
    );

    assert_eq!(
        menu.validate(),
        vec![MenuViolation::DuplicateKey {
            row: 1,
            key: "dup".to_owned(),
            first: 0,
        }]
    );
}

#[test]
fn the_same_key_in_two_different_menus_is_not_a_duplicate() {
    let a = Menu::new("a", vec![Row::action("dup", "A")]);
    let b = Menu::new("b", vec![Row::action("dup", "B")]);

    assert!(a.validate().is_empty() && b.validate().is_empty());
}

#[test]
fn a_radio_group_with_two_selected_members_is_detected() {
    let menu = Menu::new(
        "bad",
        vec![
            Row::radio("a", "A", "g", true),
            Row::radio("b", "B", "g", true),
            Row::radio("c", "C", "g", false),
        ],
    );

    assert_eq!(
        menu.validate(),
        vec![MenuViolation::RadioOverSelected {
            group: RadioGroupId::new("g"),
            rows: vec![0, 1],
        }]
    );
}

#[test]
fn two_selected_members_in_different_groups_is_fine() {
    let menu = Menu::new(
        "ok",
        vec![
            Row::radio("a", "A", "g1", true),
            Row::radio("b", "B", "g2", true),
        ],
    );

    assert!(menu.validate().is_empty());
}

#[test]
fn an_empty_menu_is_valid_and_holds_nothing() {
    let menu = Menu::empty("empty");

    assert!(menu.is_empty());
    assert_eq!(menu.len(), 0);
    assert!(menu.validate().is_empty());
    assert!(menu.groups().is_empty());
    assert!(menu.focus_members().is_empty());
    assert_eq!(menu.focus_id(0), None);
    assert_eq!(menu.row(0), None);
}

#[test]
fn is_valid_agrees_with_validate_being_empty() {
    let good = single_file_menu();
    let bad = Menu::new("bad", vec![Row::action("", "Nameless")]);

    assert_eq!(good.is_valid(), good.validate().is_empty());
    assert_eq!(bad.is_valid(), bad.validate().is_empty());
}

#[test]
fn every_violation_kind_surfaces_from_its_own_malformed_menu() {
    let menus = [
        Menu::new("m", vec![Row::action("", "x")]),
        Menu::new(
            "m",
            vec![
                Row::action("k", "a"),
                Row::separator(),
                Row::action("k", "b"),
            ],
        ),
        Menu::new(
            "m",
            vec![
                Row::action("d", "d").destructive(),
                Row::separator(),
                Row::action("p", "p"),
            ],
        ),
        Menu::new(
            "m",
            vec![Row::action("p", "p"), Row::action("d", "d").destructive()],
        ),
        Menu::new(
            "m",
            vec![
                Row::radio("a", "a", "g", true),
                Row::radio("b", "b", "g", true),
            ],
        ),
    ];

    let kinds: Vec<String> = menus
        .iter()
        .flat_map(|menu| menu.validate())
        .map(|violation| format!("{violation:?}"))
        .collect();

    assert_eq!(
        kinds.len(),
        5,
        "one violation per malformed menu: {kinds:?}"
    );
    assert!(kinds.iter().any(|v| v.starts_with("MissingKey")));
    assert!(kinds.iter().any(|v| v.starts_with("DuplicateKey")));
    assert!(kinds.iter().any(|v| v.starts_with("DestructiveNotLast")));
    assert!(kinds.iter().any(|v| v.starts_with("MixedGroup")));
    assert!(kinds.iter().any(|v| v.starts_with("RadioOverSelected")));
}

#[test]
fn a_menu_fixed_by_selecting_a_radio_stays_valid() {
    let mut menu = Menu::new(
        "m",
        vec![
            Row::radio("a", "A", "g", true),
            Row::radio("b", "B", "g", true),
        ],
    );
    assert!(!menu.is_valid());

    let _ = menu.select_radio(1);

    assert!(
        menu.is_valid(),
        "selecting one member cleared the other, which is the whole point of the rule"
    );
}
