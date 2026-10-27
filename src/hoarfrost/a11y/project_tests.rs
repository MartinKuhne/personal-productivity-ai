//! Tests for the model→contract projections (`UIF-PRIN-008`).
//!
//! The recurring assertion across every source model is the same one and
//! it is the reason the projections exist: a field the source does not
//! carry must be `None`, and the `filled_count` pins exactly how many
//! fields each projection is allowed to fill so a later edit that drops
//! or invents one shows up here.

use super::*;
use crate::a11y::contract::{A11yRole, A11yValue, Checked};
use crate::composite::expander::DisclosureToggle;
use crate::composite::tree_view::VisibleRow;
use crate::surface::menu::model::{Menu, MenuId, RadioGroupId, Row};
use crate::widget::button_model::ButtonModel;
use crate::widget::chip_model::ChipModel;
use crate::widget::progress_model::{ProgressModel, ProgressStatus};
use crate::widget::segmented_model::{SegmentItem, SegmentedModel};
use crate::widget::text_input_model::TextInputModel;
use crate::widget::toggle_model::{CheckState, CheckboxModel, RadioGroupModel, RadioOption};

// -- button -------------------------------------------------------------------

#[test]
fn a_labelled_button_projects_name_busy_and_disabled_only() {
    let model = ButtonModel::new("Save").busy();
    let contract = project_button(&model);
    assert_eq!(contract.role, A11yRole::Button);
    assert_eq!(contract.name, Some("Save"));
    assert_eq!(contract.busy, Some(true));
    assert_eq!(contract.disabled, Some(false));
    assert_eq!(contract.filled_count(), 3);
    assert_eq!(contract.expanded, None);
    assert_eq!(contract.selected, None);
    assert_eq!(contract.checked, None);
    assert_eq!(contract.invalid, None);
    assert_eq!(contract.value, None);
}

#[test]
fn an_icon_only_button_has_no_name_to_announce() {
    let model = ButtonModel::icon_only("×");
    let contract = project_button(&model);
    assert_eq!(contract.name, None, "an icon-only button has no text");
    assert_eq!(contract.filled_count(), 2);
}

#[test]
fn a_disabled_button_says_so() {
    let model = ButtonModel::new("Save").disabled();
    let contract = project_button(&model);
    assert_eq!(contract.disabled, Some(true));
    assert_eq!(contract.busy, Some(false));
}

// -- checkbox -----------------------------------------------------------------

#[test]
fn a_checkbox_projects_its_tri_state_verbatim() {
    for (state, expected) in [
        (CheckState::Unchecked, Checked::Unchecked),
        (CheckState::Indeterminate, Checked::Indeterminate),
        (CheckState::Checked, Checked::Checked),
    ] {
        let model = CheckboxModel::new("Agree").with_state(state);
        let contract = project_checkbox(&model);
        assert_eq!(contract.role, A11yRole::Checkbox);
        assert_eq!(contract.name, Some("Agree"));
        assert_eq!(contract.checked, Some(expected));
        assert_eq!(contract.filled_count(), 3);
        assert!(contract.checked_matches_role());
    }
}

#[test]
fn the_indeterminate_dash_survives_the_projection() {
    let model = CheckboxModel::new("All").with_state(CheckState::Indeterminate);
    let contract = project_checkbox(&model);
    assert_eq!(contract.checked, Some(Checked::Indeterminate));
    assert_ne!(contract.checked, Some(Checked::Checked));
    assert_ne!(contract.checked, Some(Checked::Unchecked));
}

// -- radio --------------------------------------------------------------------

#[test]
fn a_radio_lands_its_state_on_checked_not_selected() {
    let option = RadioOption::new("b", "Second");
    let selected = project_radio_option(&option, true);
    let unselected = project_radio_option(&option, false);
    assert_eq!(selected.role, A11yRole::Radio);
    assert_eq!(selected.checked, Some(Checked::Checked));
    assert_eq!(
        selected.selected, None,
        "a radio must not duplicate into selected"
    );
    assert_eq!(unselected.checked, Some(Checked::Unchecked));
    assert_eq!(selected.filled_count(), 3);
}

#[test]
fn a_radio_group_projects_only_its_enabled_state() {
    let group = RadioGroupModel::new(vec![
        RadioOption::new("a", "One"),
        RadioOption::new("b", "Two"),
    ])
    .with_selected("b");
    let contract = project_radio_group(&group);
    assert_eq!(contract.role, A11yRole::RadioGroup);
    assert_eq!(contract.disabled, Some(false));
    assert_eq!(contract.filled_count(), 1);
    assert_eq!(contract.name, None, "the group model carries no label");
    assert_eq!(contract.checked, None);
    assert_eq!(contract.selected, None);
}

#[test]
fn a_disabled_radio_group_projects_disabled() {
    let group = RadioGroupModel::new(vec![RadioOption::new("a", "One")]).disabled();
    assert_eq!(project_radio_group(&group).disabled, Some(true));
}

// -- segmented ----------------------------------------------------------------

fn single_segmented() -> SegmentedModel<&'static str> {
    SegmentedModel::new(vec![
        SegmentItem::new("l", "Left"),
        SegmentItem::new("c", "Center"),
        SegmentItem::new("r", "Right").disabled(),
    ])
    .with_selected("c")
}

#[test]
fn a_single_select_segmented_control_is_a_radio_group() {
    let model = single_segmented();
    let contract = project_segmented(&model);
    assert_eq!(contract.role, A11yRole::RadioGroup);
    assert_eq!(contract.filled_count(), 1);
}

#[test]
fn a_multi_select_segmented_control_is_a_group_not_a_radio_group() {
    let model = SegmentedModel::multi(vec![
        SegmentItem::new("b", "Bold"),
        SegmentItem::new("i", "Italic"),
    ]);
    assert_eq!(project_segmented(&model).role, A11yRole::Group);
}

#[test]
fn segments_take_their_role_from_the_parent_mode() {
    let single = single_segmented();
    assert_eq!(
        project_segment(&single, 0).map(|c| c.role),
        Some(A11yRole::Radio)
    );
    let multi = SegmentedModel::multi(vec![SegmentItem::new("b", "Bold")]);
    assert_eq!(
        project_segment(&multi, 0).map(|c| c.role),
        Some(A11yRole::Checkbox)
    );
}

#[test]
fn a_selected_segment_reports_checked_and_a_disabled_one_reports_disabled() {
    let model = single_segmented();
    let selected = project_segment(&model, 1).expect("segment 1 exists");
    assert_eq!(selected.name, Some("Center"));
    assert_eq!(selected.checked, Some(Checked::Checked));
    assert_eq!(selected.disabled, Some(false));

    let disabled = project_segment(&model, 2).expect("segment 2 exists");
    assert_eq!(disabled.disabled, Some(true));
    assert_eq!(disabled.checked, Some(Checked::Unchecked));

    let unselected = project_segment(&model, 0).expect("segment 0 exists");
    assert_eq!(unselected.checked, Some(Checked::Unchecked));
    assert_eq!(unselected.disabled, Some(false));
}

#[test]
fn an_out_of_range_segment_is_missing_not_bare() {
    let model = single_segmented();
    assert!(project_segment(&model, 99).is_none());
}

// -- menu rows ----------------------------------------------------------------

fn leaf_menu() -> Menu {
    Menu::new("leaf", vec![Row::action("leaf-copy", "Copy")])
}

fn menu_with_rows() -> Menu {
    Menu::new(
        "root",
        vec![
            Row::action("copy", "Copy"),
            Row::checkable("wrap", "Table wrap", true),
            Row::radio("size", "Small", RadioGroupId::new("sz"), true),
            Row::submenu("win", "Windows...", leaf_menu()),
            Row::heading("Section"),
            Row::separator(),
        ],
    )
}

#[test]
fn each_menu_row_kind_maps_to_its_own_aria_role() {
    let menu = menu_with_rows();
    let expected = [
        (0, A11yRole::MenuItem),
        (1, A11yRole::MenuItemCheckBox),
        (2, A11yRole::MenuItemRadio),
        (3, A11yRole::MenuItem),
        (4, A11yRole::Heading),
        (5, A11yRole::Separator),
    ];
    for (index, role) in expected {
        let row = menu.row(index).expect("row exists");
        let contract = project_menu_row(row);
        assert_eq!(contract.role, role, "row {index} mapped wrong");
        assert!(
            contract.checked_matches_role(),
            "row {index} breaks the checked/role invariant"
        );
    }
}

#[test]
fn a_check_row_carries_its_tick_and_a_radio_row_its_dot() {
    let menu = menu_with_rows();
    assert_eq!(
        project_menu_row(menu.row(1).unwrap()).checked,
        Some(Checked::Checked)
    );
    assert_eq!(
        project_menu_row(menu.row(2).unwrap()).checked,
        Some(Checked::Checked)
    );
    assert_eq!(project_menu_row(menu.row(0).unwrap()).checked, None);
}

#[test]
fn a_plain_action_row_never_claims_to_be_expanded() {
    let menu = menu_with_rows();
    let action = project_menu_row(menu.row(0).unwrap());
    assert_eq!(action.expanded, None);
    // Even when a caller wrongly insists it is open.
    let forced = project_menu_row_open(menu.row(0).unwrap(), true);
    assert_eq!(
        forced.expanded, None,
        "a non-parent row must never gain an expanded field"
    );
}

#[test]
fn a_submenu_row_knows_it_has_a_child_but_not_whether_it_is_open() {
    let menu = menu_with_rows();
    let parent = menu.row(3).unwrap();
    assert!(parent.is_submenu_parent());
    assert_eq!(
        project_menu_row(parent).expanded,
        None,
        "the row cannot know openness; the cascade owns it"
    );
    assert_eq!(project_menu_row_open(parent, true).expanded, Some(true));
    assert_eq!(project_menu_row_open(parent, false).expanded, Some(false));
}

#[test]
fn heading_and_separator_rows_are_bare_apart_from_their_label() {
    let menu = menu_with_rows();
    let heading = project_menu_row(menu.row(4).unwrap());
    assert_eq!(heading.name, Some("Section"));
    assert_eq!(heading.filled_count(), 2, "name + disabled only");
    let separator = project_menu_row(menu.row(5).unwrap());
    assert_eq!(separator.filled_count(), 2);
    assert!(!separator.role.is_interactive());
}

#[test]
fn a_disabled_menu_row_projects_disabled() {
    let row = Row::action("x", "Nope").disabled();
    assert_eq!(project_menu_row(&row).disabled, Some(true));
}

#[test]
fn menu_row_labels_and_keys_survive_projection() {
    let menu = menu_with_rows();
    let row = menu.row(0).unwrap();
    assert_eq!(project_menu_row(row).name, Some("Copy"));
    assert_eq!(row.key(), "copy");
    let _ = MenuId::new("unused");
}

// -- tree rows ----------------------------------------------------------------

#[test]
fn a_tree_leaf_has_no_expansion_state_at_all() {
    let leaf = VisibleRow {
        id: "leaf",
        depth: 2,
        expandable: false,
        expanded: false,
    };
    let contract = project_tree_row(&leaf);
    assert_eq!(contract.role, A11yRole::TreeItem);
    assert_eq!(
        contract.expanded, None,
        "a leaf cannot be collapsed, so it must not report collapsed"
    );
    assert_eq!(contract.filled_count(), 0);
}

#[test]
fn an_expandable_tree_row_reports_its_expansion() {
    let open = VisibleRow {
        id: "dir",
        depth: 0,
        expandable: true,
        expanded: true,
    };
    let closed = VisibleRow {
        id: "dir",
        depth: 0,
        expandable: true,
        expanded: false,
    };
    assert_eq!(project_tree_row(&open).expanded, Some(true));
    assert_eq!(project_tree_row(&closed).expanded, Some(false));
    assert_eq!(project_tree_row(&open).filled_count(), 1);
}

// -- chip ---------------------------------------------------------------------

#[test]
fn a_chip_projects_its_label_selection_and_enabled_state() {
    let on = ChipModel::filter("f1", "Tagged", true);
    let off = ChipModel::filter("f2", "Plain", false);
    let contract = project_chip(&on);
    assert_eq!(contract.role, A11yRole::Button);
    assert_eq!(contract.name, Some("Tagged"));
    assert_eq!(contract.selected, Some(true));
    assert_eq!(contract.disabled, Some(false));
    assert_eq!(contract.filled_count(), 3);
    assert_eq!(project_chip(&off).selected, Some(false));
    assert_eq!(project_chip(&off).name, Some("Plain"));
}

#[test]
fn a_disabled_chip_says_so() {
    let chip = ChipModel::assist("a", "Assist").disabled();
    assert_eq!(project_chip(&chip).disabled, Some(true));
}

#[test]
fn a_chip_never_claims_to_be_checked_or_expanded() {
    let chip = ChipModel::filter("f", "Tag", true);
    let contract = project_chip(&chip);
    assert_eq!(contract.checked, None);
    assert_eq!(contract.expanded, None);
    assert_eq!(contract.value, None);
}

// -- text input ---------------------------------------------------------------

#[test]
fn a_text_input_projects_value_invalid_and_disabled_but_never_a_name() {
    let model = TextInputModel::new("localhost").with_placeholder("Host name");
    let contract = project_text_input(&model);
    assert_eq!(contract.role, A11yRole::TextBox);
    assert_eq!(contract.value, Some(A11yValue::Text("localhost")));
    assert_eq!(contract.invalid, Some(false));
    assert_eq!(contract.disabled, Some(false));
    assert_eq!(
        contract.name, None,
        "a placeholder is hint text, not the field's identity"
    );
    assert_eq!(contract.filled_count(), 3);
}

#[test]
fn a_validation_message_makes_the_field_invalid() {
    let model = TextInputModel::new("nope").with_validation("Not a host name");
    assert_eq!(project_text_input(&model).invalid, Some(true));
}

#[test]
fn the_host_can_attach_the_accessible_name_the_model_lacks() {
    let model = TextInputModel::new("localhost");
    let contract = project_text_input(&model).named("Host");
    assert_eq!(contract.name, Some("Host"));
    assert_eq!(contract.filled_count(), 4);
}

#[test]
fn an_empty_field_still_projects_its_empty_value() {
    let model = TextInputModel::new("");
    let contract = project_text_input(&model);
    assert_eq!(contract.value, Some(A11yValue::Text("")));
}

// -- progress -----------------------------------------------------------------

#[test]
fn a_determinate_bar_projects_its_fraction() {
    let model = ProgressModel::determinate(1, 4);
    let contract = project_progress(&model);
    assert_eq!(contract.role, A11yRole::ProgressBar);
    assert_eq!(contract.busy, Some(true));
    assert_eq!(contract.value, Some(A11yValue::Fraction(0.25)));
    assert_eq!(contract.filled_count(), 2);
}

#[test]
fn an_indeterminate_bar_reports_no_value_rather_than_zero() {
    let model = ProgressModel::indeterminate();
    let contract = project_progress(&model);
    assert_eq!(
        contract.value, None,
        "an indeterminate bar has no completion to claim"
    );
    assert_eq!(contract.busy, Some(true));
    assert_eq!(contract.filled_count(), 1);
}

#[test]
fn a_paused_or_failed_bar_is_not_busy() {
    let paused_model = ProgressModel::determinate(1, 2).with_status(ProgressStatus::Paused);
    let paused = project_progress(&paused_model);
    assert_eq!(paused.busy, Some(false));
    let failed_model = ProgressModel::determinate(1, 2).with_status(ProgressStatus::Error);
    let failed = project_progress(&failed_model);
    assert_eq!(failed.busy, Some(false));
    assert_eq!(failed.value, Some(A11yValue::Fraction(0.5)));
}

// -- disclosure ---------------------------------------------------------------

#[test]
fn a_disclosure_header_projects_only_expansion() {
    let open_toggle = DisclosureToggle::new(true);
    let closed_toggle = DisclosureToggle::new(false);
    let open = project_disclosure(&open_toggle);
    let closed = project_disclosure(&closed_toggle);
    assert_eq!(open.role, A11yRole::Button);
    assert_eq!(open.expanded, Some(true));
    assert_eq!(closed.expanded, Some(false));
    assert_eq!(open.filled_count(), 1);
    assert_eq!(closed.filled_count(), 1);
}

// -- cross-cutting ------------------------------------------------------------

#[test]
fn no_projection_ever_violates_the_checked_role_invariant() {
    // Every model is bound to a name first: a contract borrows the
    // state it was projected from, so the sources have to outlive the
    // list of contracts that reads them.
    let button = ButtonModel::new("x");
    let checkbox = CheckboxModel::new("c");
    let radio = RadioOption::new("r", "r");
    let group = RadioGroupModel::new(vec![RadioOption::new("r", "r")]);
    let segmented = single_segmented();
    let chip = ChipModel::filter("f", "f", true);
    let input = TextInputModel::new("t");
    let progress = ProgressModel::indeterminate();
    let toggle = DisclosureToggle::new(true);
    let leaf = VisibleRow {
        id: "leaf",
        depth: 0,
        expandable: false,
        expanded: false,
    };
    let branch = VisibleRow {
        id: "branch",
        depth: 0,
        expandable: true,
        expanded: true,
    };
    let menu = menu_with_rows();

    let contracts: Vec<A11yContract<'_>> = vec![
        project_button(&button),
        project_checkbox(&checkbox),
        project_radio_option(&radio, true),
        project_radio_group(&group),
        project_segmented(&segmented),
        project_segment(&segmented, 0).expect("segment 0 exists"),
        project_chip(&chip),
        project_text_input(&input),
        project_progress(&progress),
        project_disclosure(&toggle),
        project_tree_row(&leaf),
        project_tree_row(&branch),
    ];
    let mut all: Vec<A11yContract<'_>> = contracts;
    for index in 0..6 {
        let row = menu.row(index).expect("row exists");
        all.push(project_menu_row(row));
        all.push(project_menu_row_open(row, true));
    }

    assert_eq!(all.len(), 24);
    for contract in &all {
        assert!(
            contract.checked_matches_role(),
            "{contract} breaks the checked/role invariant"
        );
    }
}

#[test]
fn every_projection_is_pure_and_deterministic() {
    let menu = menu_with_rows();
    for index in 0..6 {
        let row = menu.row(index).unwrap();
        assert_eq!(project_menu_row(row), project_menu_row(row));
        assert_eq!(
            project_menu_row_open(row, true),
            project_menu_row_open(row, true)
        );
    }
    let chip = ChipModel::filter("f", "f", true);
    assert_eq!(project_chip(&chip), project_chip(&chip));
}

#[test]
fn projecting_does_not_mutate_the_source_model() {
    let model = ButtonModel::new("Save").busy();
    let before = model.clone();
    let _ = project_button(&model);
    assert_eq!(model, before);

    let segmented = single_segmented();
    let before_segmented = segmented.clone();
    let _ = project_segment(&segmented, 1);
    assert_eq!(segmented, before_segmented);
}

#[test]
fn test_project_new_collection_items() {
    // Tab item projection
    let tab = crate::composite::tab_strip::TabItem::new("t1", "Document 1");
    let c_tab = project_tab_item(&tab, true);
    assert_eq!(c_tab.role, A11yRole::Button);
    assert_eq!(c_tab.name, Some("Document 1"));
    assert_eq!(c_tab.selected, Some(true));

    // Nav rail destination projection
    let nav = crate::composite::navigation_rail::NavDestination::new("home", "Home", "⌂");
    let c_nav = project_nav_rail_destination(&nav, false);
    assert_eq!(c_nav.role, A11yRole::Button);
    assert_eq!(c_nav.name, Some("Home"));
    assert_eq!(c_nav.selected, Some(false));

    // Stepper step projection
    let step = crate::composite::stepper::StepItem::new(
        "step_1",
        "Account Setup",
        crate::composite::stepper::StepStatus::Current,
    );
    let c_step = project_stepper_step(&step, true);
    assert_eq!(c_step.role, A11yRole::Button);
    assert_eq!(c_step.name, Some("Account Setup"));
    assert_eq!(c_step.selected, Some(true));

    // Grid tile projection
    let tile = crate::composite::grid_view::GridTile::new("tile_1", "Project Alpha");
    let c_tile = project_grid_tile(&tile, false);
    assert_eq!(c_tile.role, A11yRole::Button);
    assert_eq!(c_tile.name, Some("Project Alpha"));
    assert_eq!(c_tile.selected, Some(false));

    // Command palette item projection
    let cmd = crate::composite::command_palette::PaletteItem::new("cmd_1", "Save File");
    let c_cmd = project_command_palette_item(&cmd, true);
    assert_eq!(c_cmd.role, A11yRole::MenuItem);
    assert_eq!(c_cmd.name, Some("Save File"));
    assert_eq!(c_cmd.selected, Some(true));
}
