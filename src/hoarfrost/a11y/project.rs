//! Projections from real widget models into an [`A11yContract`]
//! (`UIF-PRIN-008`).
//!
//! Each function here reads one model and answers the nine §1.1
//! questions about it. The rule governing every one of them is the same
//! and it is not a style preference:
//!
//! > **A field is `Some` only when the source model genuinely carries
//! > that state.**
//!
//! That is what makes the projection trustworthy instead of merely
//! tidy. A `ButtonModel` has no notion of being expanded, so
//! [`project_button`] leaves `expanded` `None` rather than writing
//! `false` — and a headless selector can therefore tell "this is a
//! button" from "this is a collapsed tree row", which is exactly the
//! distinction a defaulted field would destroy.
//!
//! # Three places the boundary is interesting
//!
//! **A menu row does not know whether its submenu is open.** `RowKind`
//! carries the child's *structure*; openness is
//! [`MenuCascade`](crate::surface::menu::cascade::MenuCascade) state.
//! So [`project_menu_row`] leaves `expanded` absent even for a submenu
//! parent, and [`project_menu_row_open`] takes the answer from the
//! cascade. A projection that guessed `false` would report an open
//! submenu as closed and no test could tell.
//!
//! **A tree leaf has no expansion state.** `VisibleRow` carries both
//! `expandable` and `expanded`, and for a leaf `expanded` is `false`
//! only because nothing was ever set. [`project_tree_row`] reports
//! `expanded: None` unless the row is `expandable`, because "cannot
//! expand" and "collapsed" are different answers to a screen reader.
//!
//! **A text input's accessible name is not its placeholder.**
//! `TextInputModel` has no label field — the label lives on the
//! surrounding `FormField` — so [`project_text_input`] leaves `name`
//! `None` and the host supplies it through
//! [`A11yContract::named`]. Inventing a name from the placeholder
//! would announce hint text as the field's identity.
//!
//! # What is deliberately not here
//!
//! No projection reads a `Rectangle`, a rendered string width, a clock
//! or anything else outside the model. The sidecar scans this file for
//! those identifiers and fails on sight, because a projection that
//! needed geometry could not run headlessly, and headless is the entire
//! reason [`A11yContract`] exists.
//!
//! Unit tests live in the sibling `project_tests.rs` sidecar.

use crate::a11y::contract::{A11yContract, A11yRole, A11yValue, Checked};
use crate::composite::expander::DisclosureToggle;
use crate::composite::tree_view::VisibleRow;
use crate::surface::menu::model::Row;
use crate::surface::menu::model::RowKind;
use crate::widget::button_model::ButtonModel;
use crate::widget::chip_model::ChipModel;
use crate::widget::progress_model::{ProgressModel, ProgressStatus};
use crate::widget::segmented_model::{SegmentedModel, SelectionMode};
use crate::widget::text_input_model::TextInputModel;
use crate::widget::toggle_model::{CheckState, CheckboxModel, RadioGroupModel, RadioOption};

impl From<CheckState> for Checked {
    /// Map the checkbox's own tri-state onto the projected one.
    ///
    /// A direct, total, one-to-one mapping: the two types have the same
    /// three states for the same three reasons, so anything cleverer
    /// would just be a place for them to drift apart.
    fn from(state: CheckState) -> Self {
        match state {
            CheckState::Unchecked => Self::Unchecked,
            CheckState::Indeterminate => Self::Indeterminate,
            CheckState::Checked => Self::Checked,
        }
    }
}

/// Project a [`ButtonModel`] (`UIF-PRIN-008`).
///
/// Carries `name` (absent for an icon-only button, which has no text to
/// announce), `busy` and `disabled`. Leaves `value`, `expanded`,
/// `selected`, `checked` and `invalid` absent: a button model has no
/// notion of any of them, and a button that reported `checked: false`
/// would be indistinguishable from a checkbox the user has not ticked.
#[must_use]
pub fn project_button(model: &ButtonModel) -> A11yContract<'_> {
    let contract = A11yContract::new(A11yRole::Button)
        .busy(model.busy)
        .disabled(model.disabled);
    match model.label.as_deref() {
        Some(label) => contract.named(label),
        None => contract,
    }
}

/// Project a [`CheckboxModel`] (`UIF-PRIN-008`).
///
/// `checked` is tri-state here, so the `−` dash survives the projection
/// instead of being rounded to one of the two booleans.
#[must_use]
pub fn project_checkbox(model: &CheckboxModel) -> A11yContract<'_> {
    A11yContract::new(A11yRole::Checkbox)
        .named(&model.label)
        .checked(model.state.into())
        .disabled(model.disabled)
}

/// Project one [`RadioOption`] against the group's current selection
/// (`UIF-PRIN-008`).
///
/// The selection lands on `checked`, not `selected`, because that is
/// the field ARIA defines for a radio and the field a screen reader
/// listens to. `selected` stays absent rather than duplicating the same
/// fact into two slots that could later disagree.
#[must_use]
pub fn project_radio_option<Id>(option: &RadioOption<Id>, selected: bool) -> A11yContract<'_> {
    let checked = if selected {
        Checked::Checked
    } else {
        Checked::Unchecked
    };
    A11yContract::new(A11yRole::Radio)
        .named(&option.label)
        .checked(checked)
        .disabled(option.disabled)
}

/// Project a [`RadioGroupModel`] as its container role (`UIF-PRIN-008`).
///
/// Only `disabled` carries through. The group is not itself selectable,
/// checkable or expandable, and it has no label field of its own — the
/// host attaches one with [`A11yContract::named`] when it has one.
/// Member state comes from [`project_radio_option`].
#[must_use]
pub fn project_radio_group<Id>(model: &RadioGroupModel<Id>) -> A11yContract<'_> {
    A11yContract::new(A11yRole::RadioGroup).disabled(model.disabled)
}

/// Project a [`SegmentedModel`] as its container role (`UIF-PRIN-008`).
///
/// The role follows the selection mode rather than the widget's name,
/// because the mode is what the semantics actually are: a
/// single-select segmented control *is* a radio group — one of N,
/// picking one evicts the last — and a multi-select one is a `group`,
/// because it holds a set rather than a choice. Calling both of them
/// `radiogroup` would tell a screen reader that a multi-select toolbar
/// can only ever hold one value.
#[must_use]
pub fn project_segmented<Id>(model: &SegmentedModel<Id>) -> A11yContract<'_> {
    let role = match model.mode {
        SelectionMode::SingleSelect => A11yRole::RadioGroup,
        SelectionMode::MultiSelect => A11yRole::Group,
    };
    A11yContract::new(role).disabled(model.disabled)
}

/// Project one segment of a [`SegmentedModel`] (`UIF-PRIN-008`).
///
/// The segment's role follows the parent's mode for the same reason the
/// container's does. Returns `None` when `index` is out of range rather
/// than a bare contract, so a stale index in a host's loop surfaces as a
/// missing row instead of a phantom control with no name.
#[must_use]
pub fn project_segment<Id>(model: &SegmentedModel<Id>, index: usize) -> Option<A11yContract<'_>>
where
    Id: Clone + Ord,
{
    let segment = model.segments.get(index)?;
    let role = match model.mode {
        SelectionMode::SingleSelect => A11yRole::Radio,
        SelectionMode::MultiSelect => A11yRole::Checkbox,
    };
    Some(
        A11yContract::new(role)
            .named(&segment.label)
            .checked(if model.is_selected(&segment.id) {
                Checked::Checked
            } else {
                Checked::Unchecked
            })
            .disabled(!model.segment_enabled(index)),
    )
}

/// Project one menu [`Row`] (`UIF-PRIN-008`).
///
/// The role follows [`RowKind`] exactly, including the two
/// checkbox-shaped menu roles ARIA defines for check and radio rows.
///
/// `expanded` is **always absent** here, even for a submenu parent: the
/// row knows it *has* a child, not whether that child is open. Use
/// [`project_menu_row_open`] when the cascade's answer is available.
#[must_use]
pub fn project_menu_row(row: &Row) -> A11yContract<'_> {
    let contract = A11yContract::new(role_of_row(row)).named(row.label());
    match row.kind() {
        RowKind::Check { checked } => contract.checked(if *checked {
            Checked::Checked
        } else {
            Checked::Unchecked
        }),
        RowKind::Radio { selected, .. } => contract.checked(if *selected {
            Checked::Checked
        } else {
            Checked::Unchecked
        }),
        // Action, Submenu, Heading and Separator rows have no check
        // state; leaving it absent is what keeps a `menuitem` from
        // reading as an unticked `menuitemcheckbox`.
        _ => contract,
    }
    .disabled(row.is_disabled())
}

/// Project a menu [`Row`] with the cascade's expansion answer attached
/// (`UIF-PRIN-008`).
///
/// `expanded` is set **only** for a submenu parent. A plain action row
/// stays `expanded: None` no matter what the caller passes, because
/// recording `expanded: false` against a row that cannot expand is the
/// fabrication this projection exists to prevent — and a cascade bug
/// that passed the wrong row index would otherwise be invisible.
#[must_use]
pub fn project_menu_row_open(row: &Row, is_open: bool) -> A11yContract<'_> {
    let contract = project_menu_row(row);
    if row.is_submenu_parent() {
        contract.expanded(is_open)
    } else {
        contract
    }
}

/// The ARIA role a menu row's kind maps to.
fn role_of_row(row: &Row) -> A11yRole {
    match row.kind() {
        RowKind::Action | RowKind::Submenu { .. } => A11yRole::MenuItem,
        RowKind::Check { .. } => A11yRole::MenuItemCheckBox,
        RowKind::Radio { .. } => A11yRole::MenuItemRadio,
        RowKind::Heading => A11yRole::Heading,
        RowKind::Separator => A11yRole::Separator,
    }
}

/// Project one visible tree [`VisibleRow`] (`UIF-PRIN-008`).
///
/// `expanded` is reported only when the row is `expandable`. A leaf's
/// `expanded` field is `false` purely because nothing ever set it, and
/// announcing "collapsed" for a row with nothing to collapse is a
/// different lie from announcing nothing at all.
#[must_use]
pub fn project_tree_row<Id>(row: &VisibleRow<Id>) -> A11yContract<'_> {
    let contract = A11yContract::new(A11yRole::TreeItem);
    if row.expandable {
        contract.expanded(row.expanded)
    } else {
        contract
    }
}

/// Project a [`ChipModel`] (`UIF-PRIN-008`).
///
/// `selected` carries the variant's own selection fact and `disabled`
/// carries the model's. The role is `button` because that is what a
/// chip *does*; an adapter mapping to a stricter ARIA shape (a filter
/// chip as an `option` in a `listbox`) reads `selected` from here
/// rather than re-deriving it from the paint.
#[must_use]
pub fn project_chip<Id>(model: &ChipModel<Id>) -> A11yContract<'_> {
    A11yContract::new(A11yRole::Button)
        .named(model.label())
        .selected(model.is_selected())
        .disabled(model.is_disabled())
}

/// Project a [`TextInputModel`] (`UIF-PRIN-008`).
///
/// Carries `value` (the live contents), `invalid` (read from the host's
/// own validation message, not inferred) and `disabled`.
///
/// `name` is **absent**: the model has no label field. The accessible
/// name belongs to the surrounding
/// [`form_field`](crate::composite::form_field) composite, and the host
/// attaches it with [`A11yContract::named`]. Falling back to
/// `placeholder` would announce hint text as the field's identity, and
/// a placeholder is by definition not what the field *is*.
#[must_use]
pub fn project_text_input(model: &TextInputModel) -> A11yContract<'_> {
    A11yContract::new(A11yRole::TextBox)
        .with_value(A11yValue::Text(&model.value))
        .invalid(model.is_invalid())
        .disabled(model.disabled)
}

/// Project a [`ProgressModel`] (`UIF-PRIN-008`).
///
/// `busy` is whether work is still moving — only `Active` counts, so a
/// paused or failed bar reports `false` rather than keeping the user
/// waiting on motion that stopped. `value` is the determinate fraction
/// and is **absent** for an indeterminate bar, which has no completion
/// to report; claiming `0.0` would tell a screen reader the job had not
/// started when it may be nearly done.
#[must_use]
pub fn project_progress(model: &ProgressModel) -> A11yContract<'_> {
    let contract =
        A11yContract::new(A11yRole::ProgressBar).busy(model.status() == ProgressStatus::Active);
    let contract = match model.label() {
        Some(label) => contract.named(label),
        None => contract,
    };
    match model.fraction() {
        Some(fraction) => contract.with_value(A11yValue::Fraction(fraction)),
        None => contract,
    }
}

/// Project an expander's [`DisclosureToggle`] header (`UIF-PRIN-008`).
///
/// `expanded` is the whole point of the control, so it is always
/// present. Nothing else is: the header is a button that opens or
/// closes, and is neither selected, checked, busy nor invalid.
#[must_use]
pub fn project_disclosure(toggle: &DisclosureToggle) -> A11yContract<'_> {
    A11yContract::new(A11yRole::Button).expanded(toggle.is_open())
}

/// Project a [`TabItem`](crate::composite::tab_strip::TabItem) from a tab strip (`UIF-PRIN-008`).
#[must_use]
pub fn project_tab_item(
    tab: &crate::composite::tab_strip::TabItem,
    is_active: bool,
) -> A11yContract<'_> {
    A11yContract::new(A11yRole::Button)
        .named(&tab.title)
        .selected(is_active)
}

/// Project a [`NavDestination`](crate::composite::navigation_rail::NavDestination) from a navigation rail (`UIF-PRIN-008`).
#[must_use]
pub fn project_nav_rail_destination<DestId>(
    destination: &crate::composite::navigation_rail::NavDestination<DestId>,
    is_selected: bool,
) -> A11yContract<'_> {
    A11yContract::new(A11yRole::Button)
        .named(&destination.label)
        .selected(is_selected)
        .disabled(destination.disabled)
}

/// Project a [`StepItem`](crate::composite::stepper::StepItem) from a stepper wizard (`UIF-PRIN-008`).
#[must_use]
pub fn project_stepper_step<StepId>(
    step: &crate::composite::stepper::StepItem<StepId>,
    is_active: bool,
) -> A11yContract<'_> {
    A11yContract::new(A11yRole::Button)
        .named(&step.label)
        .selected(is_active)
        .disabled(step.disabled)
}

/// Project a [`GridTile`](crate::composite::grid_view::GridTile) from a grid view (`UIF-PRIN-008`).
#[must_use]
pub fn project_grid_tile<Id>(
    tile: &crate::composite::grid_view::GridTile<Id>,
    is_selected: bool,
) -> A11yContract<'_> {
    A11yContract::new(A11yRole::Button)
        .named(&tile.title)
        .selected(is_selected)
        .disabled(tile.disabled)
}

/// Project a [`PaletteItem`](crate::composite::command_palette::PaletteItem) from a command palette (`UIF-PRIN-008`).
#[must_use]
pub fn project_command_palette_item<ItemId>(
    item: &crate::composite::command_palette::PaletteItem<ItemId>,
    is_selected: bool,
) -> A11yContract<'_> {
    A11yContract::new(A11yRole::MenuItem)
        .named(&item.title)
        .selected(is_selected)
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod project_tests;
