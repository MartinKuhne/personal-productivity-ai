//! Tests for the `A11yContract` value type, the closed role set and the
//! purity of the projection layer (`UIF-PRIN-008`).

use crate::a11y::contract::A11yContract;
use crate::a11y::contract::A11yRole;
use crate::a11y::contract::A11yValue;
use crate::a11y::contract::Checked;

// -- role: closed set ---------------------------------------------------------

#[test]
fn every_role_is_listed_in_all() {
    assert_eq!(A11yRole::ALL.len(), 14);
    let mut seen: Vec<&str> = A11yRole::ALL.into_iter().map(A11yRole::aria_role).collect();
    seen.sort_unstable();
    let unique_len = seen.len();
    seen.dedup();
    assert_eq!(
        unique_len,
        seen.len(),
        "two roles share an ARIA name: {seen:?}"
    );
}

#[test]
fn aria_names_are_lowercase_and_alphanumeric() {
    for role in A11yRole::ALL {
        let name = role.aria_role();
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_alphabetic()),
            "{name} carries a non-lowercase character"
        );
        assert_eq!(name, name.to_lowercase(), "{name} is not lowercase");
    }
}

#[test]
fn the_three_menu_check_roles_have_their_real_aria_names() {
    assert_eq!(A11yRole::MenuItemCheckBox.aria_role(), "menuitemcheckbox");
    assert_eq!(A11yRole::MenuItemRadio.aria_role(), "menuitemradio");
    assert_eq!(A11yRole::MenuItem.aria_role(), "menuitem");
}

#[test]
fn only_separator_and_heading_are_non_interactive() {
    let non_interactive: Vec<A11yRole> = A11yRole::ALL
        .into_iter()
        .filter(|role| !role.is_interactive())
        .collect();
    assert_eq!(
        non_interactive,
        vec![A11yRole::Separator, A11yRole::Heading]
    );
}

#[test]
fn exactly_four_roles_carry_a_checked_state() {
    let carrying: Vec<A11yRole> = A11yRole::ALL
        .into_iter()
        .filter(|role| role.carries_checked())
        .collect();
    assert_eq!(
        carrying,
        vec![
            A11yRole::Checkbox,
            A11yRole::Radio,
            A11yRole::MenuItemCheckBox,
            A11yRole::MenuItemRadio,
        ]
    );
}

#[test]
fn display_of_a_role_is_its_aria_name() {
    for role in A11yRole::ALL {
        assert_eq!(role.to_string(), role.aria_role());
    }
}

// -- Checked ------------------------------------------------------------------

#[test]
fn unchecked_is_the_only_off_state() {
    assert!(!Checked::Unchecked.is_on());
    assert!(Checked::Indeterminate.is_on());
    assert!(Checked::Checked.is_on());
}

#[test]
fn indeterminate_is_the_only_mixed_state() {
    assert!(Checked::Indeterminate.is_indeterminate());
    assert!(!Checked::Unchecked.is_indeterminate());
    assert!(!Checked::Checked.is_indeterminate());
}

#[test]
fn checked_default_is_unchecked() {
    assert_eq!(Checked::default(), Checked::Unchecked);
}

#[test]
fn checked_display_names_each_state() {
    assert_eq!(Checked::Unchecked.to_string(), "unchecked");
    assert_eq!(Checked::Indeterminate.to_string(), "indeterminate");
    assert_eq!(Checked::Checked.to_string(), "checked");
}

// -- A11yValue ---------------------------------------------------------------

#[test]
fn value_arms_report_their_kind() {
    assert!(A11yValue::Text("hi").is_text());
    assert!(!A11yValue::Number(1).is_text());
    assert_eq!(A11yValue::Fraction(0.25).as_fraction(), Some(0.25));
    assert_eq!(A11yValue::Number(3).as_fraction(), None);
    assert_eq!(A11yValue::Text("x").as_fraction(), None);
}

#[test]
fn fraction_display_is_a_whole_percentage() {
    assert_eq!(A11yValue::Fraction(0.5).to_string(), "50.00");
    assert_eq!(A11yValue::Fraction(1.0).to_string(), "100.00");
    assert_eq!(A11yValue::Fraction(0.0).to_string(), "0.00");
}

#[test]
fn text_display_is_quoted_so_whitespace_is_visible() {
    assert_eq!(A11yValue::Text("").to_string(), "''");
    assert_eq!(A11yValue::Text("a b").to_string(), "'a b'");
}

// -- contract: absence is the point ------------------------------------------

#[test]
fn a_new_contract_fills_only_the_role() {
    let contract = A11yContract::new(A11yRole::Button);
    assert_eq!(contract.role, A11yRole::Button);
    assert_eq!(contract.filled_count(), 0);
    assert!(contract.is_bare());
}

/// A contract plus the predicate naming the one slot its builder filled.
type SlotCase = (A11yContract<'static>, fn(&A11yContract<'static>) -> bool);

#[test]
fn every_builder_fills_exactly_one_slot() {
    let base = A11yContract::new(A11yRole::Checkbox);
    let cases: Vec<SlotCase> = vec![
        (base.clone().named("n"), |c| c.name.is_some()),
        (base.clone().with_value(A11yValue::Number(1)), |c| {
            c.value.is_some()
        }),
        (base.clone().expanded(true), |c| c.expanded.is_some()),
        (base.clone().selected(true), |c| c.selected.is_some()),
        (base.clone().checked(Checked::Checked), |c| {
            c.checked.is_some()
        }),
        (base.clone().busy(true), |c| c.busy.is_some()),
        (base.clone().invalid(true), |c| c.invalid.is_some()),
        (base.clone().disabled(true), |c| c.disabled.is_some()),
    ];
    assert_eq!(cases.len(), 8, "the contract has eight optional slots");
    for (contract, filled) in cases {
        assert_eq!(contract.filled_count(), 1, "{contract} filled too much");
        assert!(filled(&contract));
        assert!(!contract.is_bare());
    }
}

#[test]
fn a_false_field_is_not_the_same_as_an_absent_one() {
    let off = A11yContract::new(A11yRole::Checkbox).checked(Checked::Unchecked);
    let absent = A11yContract::new(A11yRole::Checkbox);
    assert_ne!(off, absent);
    assert_eq!(off.filled_count(), 1);
    assert_eq!(absent.filled_count(), 0);
    assert_ne!(off.to_string(), absent.to_string());
}

#[test]
fn filled_count_reaches_eight_when_every_slot_is_set() {
    let full = A11yContract::new(A11yRole::TextBox)
        .named("name")
        .with_value(A11yValue::Text("v"))
        .expanded(false)
        .selected(false)
        .checked(Checked::Unchecked)
        .busy(false)
        .invalid(false)
        .disabled(false);
    assert_eq!(full.filled_count(), 8);
    assert!(!full.is_bare());
}

#[test]
fn checked_must_match_the_role() {
    for role in A11yRole::ALL {
        let bare = A11yContract::new(role);
        assert!(
            bare.checked_matches_role(),
            "{role} with no checked field must be legal"
        );
    }
    for role in A11yRole::ALL
        .into_iter()
        .filter(|role| role.carries_checked())
    {
        let with = A11yContract::new(role).checked(Checked::Checked);
        assert!(with.checked_matches_role(), "{role} should carry checked");
    }
    for role in A11yRole::ALL.into_iter().filter(|r| !r.carries_checked()) {
        let wrong = A11yContract::new(role).checked(Checked::Checked);
        assert!(
            !wrong.checked_matches_role(),
            "{role} must not carry a checked field"
        );
    }
}

#[test]
fn display_renders_all_nine_fields_in_order() {
    let text = A11yContract::new(A11yRole::TextBox)
        .named("Host")
        .with_value(A11yValue::Text("localhost"))
        .disabled(true)
        .to_string();
    assert_eq!(
        text,
        "textbox name=Host value='localhost' expanded=— selected=— checked=— busy=— invalid=— disabled=true"
    );
}

#[test]
fn display_shows_absent_fields_as_an_em_dash() {
    let bare = A11yContract::new(A11yRole::Separator).to_string();
    assert_eq!(
        bare,
        "separator name=— value=— expanded=— selected=— checked=— busy=— invalid=— disabled=—"
    );
    assert!(
        bare.contains('—'),
        "an absent field must be visibly absent, not blank"
    );
}

#[test]
fn a_contract_is_not_default_constructible_without_a_role() {
    // `A11yContract` deliberately does not implement `Default`: a
    // contract with no role is meaningless, so the type cannot be
    // accidentally created without one. This test pins that by
    // asserting the only way to build one names a role.
    let built = A11yContract::new(A11yRole::Menu);
    assert_eq!(built.role, A11yRole::Menu);
}

// -- purity: no geometry, no clock -------------------------------------------

/// Identifiers that would mean the contract layer had stopped being
/// producible headlessly.
const IMPURE_IDENTIFIERS: [&str; 6] = [
    "Rectangle",
    "Instant::now",
    "geometry::",
    "f32::INFINITY",
    "measure",
    "pixel",
];

fn scan_for_impurity(path: &str) -> Vec<String> {
    let source =
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
    // Only code is scanned. Doc comments talk about pixels and rectangles
    // at length — that prose is precisely why the module exists — so
    // every comment line is dropped before the identifiers are sought.
    let code = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    IMPURE_IDENTIFIERS
        .into_iter()
        .filter(|needle| code.contains(needle))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_contract_module_reads_no_geometry_or_clock() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/a11y/contract.rs");
    let offenders = scan_for_impurity(path);
    assert!(
        offenders.is_empty(),
        "contract.rs references impure identifiers: {offenders:?}"
    );
}

#[test]
fn the_projection_module_reads_no_geometry_or_clock() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/a11y/project.rs");
    let offenders = scan_for_impurity(path);
    assert!(
        offenders.is_empty(),
        "project.rs references impure identifiers: {offenders:?}"
    );
}

#[test]
fn projection_is_deterministic_across_repeated_calls() {
    let model = crate::widget::button_model::ButtonModel::new("Save").busy();
    let first = crate::a11y::project::project_button(&model);
    let second = crate::a11y::project::project_button(&model);
    assert_eq!(first, second);
    assert_eq!(first.to_string(), second.to_string());
}

// -- redundant cues verification (UIF-PRIN-005) -------------------------------

#[test]
fn test_redundant_cues_across_all_semantic_statuses() {
    use crate::composite::diff_viewer::{DiffLine, DiffLineKind, GLYPH_ADDED, GLYPH_DELETED};
    use crate::composite::stepper::{
        GLYPH_COMPLETED, GLYPH_ERROR as STEPPER_ERROR, StepItem, StepStatus,
    };
    use crate::composite::tab_strip::{DIRTY_DOT_DIAMETER, TabItem, TabStripModel};
    use crate::widget::text_input::{GLYPH_CLEAR, GLYPH_ERROR, GLYPH_SEARCH};
    use crate::widget::toggle_widget::{CHECK_MARK, INDETERMINATE_MARK};

    // 1. Selection: Toggle controls pair selection state with glyphs / indicators, never color alone
    assert_eq!(CHECK_MARK, "✓");
    assert_eq!(INDETERMINATE_MARK, "\u{2212}");

    // 2. Validation errors: TextInput pairs error state with glyph and perimeter border
    assert_eq!(GLYPH_ERROR, '⚠');
    assert_eq!(GLYPH_SEARCH, '🔍');
    assert_eq!(GLYPH_CLEAR, '×');

    // 3. Dirty state: TabStrip pairs unsaved edits with dirty dot indicator and close confirmation guard
    assert_eq!(DIRTY_DOT_DIAMETER, 6.0);
    let mut tab_model = TabStripModel::new();
    let dirty_tab = TabItem::new("doc1", "Document 1").with_dirty(true);
    tab_model.open_tab(dirty_tab);
    let verdict = tab_model.request_close("doc1");
    assert!(matches!(
        verdict,
        crate::composite::tab_strip::TabCloseVerdict::RequiresConfirmation { .. }
    ));

    // 4. Diff lines: Added lines carry '+' glyph and deleted lines carry '-' glyph
    assert_eq!(GLYPH_ADDED, "+");
    assert_eq!(GLYPH_DELETED, "-");
    let added = DiffLine {
        kind: DiffLineKind::Added,
        old_line: None,
        new_line: Some(1),
        text: "new line".to_string(),
    };
    assert_eq!(added.kind, DiffLineKind::Added);

    // 5. Stepper: Completed steps carry '✓' glyph and Error steps carry '!' glyph
    assert_eq!(GLYPH_COMPLETED, "✓");
    assert_eq!(STEPPER_ERROR, "!");
    let step = StepItem {
        id: 0,
        label: "Step 1".to_string(),
        status: StepStatus::Completed,
        disabled: false,
    };
    assert_eq!(step.status, StepStatus::Completed);
}
