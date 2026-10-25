//! The pure semantic projection every widget model can expose
//! (`UIF-PRIN-008`).
//!
//! §1.1: *"Every widget model exposes a pure semantic descriptor
//! (`role`, `name`, `value`, `expanded`, `selected`, `checked`, `busy`,
//! `invalid`, `disabled`) so assistive technologies and headless test
//! selectors never rely on pixel scraping."*
//!
//! # Why `Option` on every field but `role`
//!
//! The whole value of this type is that a reader can tell **"this control
//! is off"** from **"this control has no on/off"**. A `bool` cannot make
//! that distinction, and a defaulted one is worse than useless: a
//! `ProgressBar` projected with `expanded: false` claims the bar
//! considered expansion and declined, and a headless selector reading
//! that cannot tell it from a collapsed tree row that happens to be
//! closed right now.
//!
//! So every field except [`role`](A11yContract::role) is an `Option`,
//! and the rule the projections in
//! [`project`](super::project) follow is absolute: **a field is `Some`
//! only when the source model genuinely carries that state.** Nothing
//! here defaults a field to something plausible because it "usually"
//! applies. A `ButtonModel` has no notion of being expanded, so
//! `expanded` is `None`, and that absence is information.
//!
//! # `role` is a closed enum, not a string
//!
//! A free `String` role means a typo compiles, and a typo in a role is
//! invisible: the widget renders identically and only the assistive
//! technology notices. [`A11yRole`] is closed, and every variant carries
//! its real ARIA name through [`aria_role`](A11yRole::aria_role) so the
//! mapping from this framework's vocabulary to the platform's is data a
//! test can check rather than a comment someone trusted.
//!
//! The set is exactly the roles this crate's widgets actually project —
//! no speculative members for controls that do not exist. When a Phase C
//! widget needs a role it will be added with its projection, not before.
//!
//! # Nothing here reads geometry
//!
//! A contract built from a model's declared state can be produced
//! headlessly, without a renderer, a font or a layout pass. The sidecar
//! scans this file and its sibling `project.rs` for geometry and clock
//! identifiers so a later edit cannot quietly break that.
//!
//! Unit tests live in the sibling `contract_tests.rs` sidecar.
//!
//! Unit tests live in the sibling `contract_tests.rs` sidecar.

/// The ARIA role a widget projects (`UIF-PRIN-008`).
///
/// Fourteen roles, each one a real ARIA role, each one assigned to a
/// widget that exists in this crate by a projection in
/// [`project`](super::project). There is no `Other` and no free-form
/// variant: an unassignable control is a reason to add a projection, not
/// a reason to widen the type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum A11yRole {
    /// `button` — an activatable control: `Button`, a `Chip`, an
    /// expander's disclosure header.
    Button,
    /// `checkbox` — an independently toggleable state, possibly
    /// tri-state: `CheckboxModel`, a menu `Check` row, a multi-select
    /// segment.
    Checkbox,
    /// `radio` — one member of a mutually exclusive choice:
    /// `RadioOption`, a menu `Radio` row, a single-select segment.
    Radio,
    /// `radiogroup` — the container of a mutually exclusive choice:
    /// `RadioGroupModel`, a single-select `SegmentedButton`.
    RadioGroup,
    /// `group` — a generic container with no single-value semantics: a
    /// multi-select `SegmentedButton`, which holds a set rather than a
    /// choice.
    Group,
    /// `menu` — a menu card.
    Menu,
    /// `menuitem` — a plain command row, and a submenu parent row.
    MenuItem,
    /// `menuitemcheckbox` — a menu row with an independent `✓` toggle.
    MenuItemCheckBox,
    /// `menuitemradio` — a menu row that is one of a mutually exclusive
    /// `•` choice.
    MenuItemRadio,
    /// `separator` — a non-interactive divider row.
    Separator,
    /// `heading` — a non-interactive section label row.
    Heading,
    /// `treeitem` — one row of a `TreeView`.
    TreeItem,
    /// `textbox` — an editable single-line field.
    TextBox,
    /// `progressbar` — a determinate or indeterminate progress bar.
    ProgressBar,
}

impl A11yRole {
    /// Every role.
    pub const ALL: [Self; 14] = [
        Self::Button,
        Self::Checkbox,
        Self::Radio,
        Self::RadioGroup,
        Self::Group,
        Self::Menu,
        Self::MenuItem,
        Self::MenuItemCheckBox,
        Self::MenuItemRadio,
        Self::Separator,
        Self::Heading,
        Self::TreeItem,
        Self::TextBox,
        Self::ProgressBar,
    ];

    /// The canonical ARIA role string this maps to.
    ///
    /// This is the value a host writes into an actual `role=` attribute,
    /// so the framework-to-platform mapping is checkable data rather
    /// than an assumption in whoever wires the adapter.
    #[must_use]
    pub const fn aria_role(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::RadioGroup => "radiogroup",
            Self::Group => "group",
            Self::Menu => "menu",
            Self::MenuItem => "menuitem",
            Self::MenuItemCheckBox => "menuitemcheckbox",
            Self::MenuItemRadio => "menuitemradio",
            Self::Separator => "separator",
            Self::Heading => "heading",
            Self::TreeItem => "treeitem",
            Self::TextBox => "textbox",
            Self::ProgressBar => "progressbar",
        }
    }

    /// Whether this role is one a user can interact with.
    ///
    /// `separator` and `heading` are the two roles the framework paints
    /// but never makes focusable or clickable, so a selector looking for
    /// live controls can filter on this instead of keeping its own list
    /// of which roles are decorative.
    #[must_use]
    pub const fn is_interactive(self) -> bool {
        !matches!(self, Self::Separator | Self::Heading)
    }

    /// Whether this role carries a `checked` state at all.
    ///
    /// The three checkbox-shaped roles and nothing else. A projection
    /// that sets `checked` on a role outside this set is a bug, and the
    /// sidecar asserts the two agree.
    #[must_use]
    pub const fn carries_checked(self) -> bool {
        matches!(
            self,
            Self::Checkbox | Self::Radio | Self::MenuItemCheckBox | Self::MenuItemRadio
        )
    }
}

impl std::fmt::Display for A11yRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.aria_role())
    }
}

/// The three check states a projected `checked` field can carry
/// (`UIF-PRIN-008`).
///
/// Tri-state rather than `bool` because the framework's checkbox is
/// genuinely tri-state — §3.3's `−` dash is a first-class state, not a
/// rendering of "checked with caveats" — and a `bool` projection would
/// have to collapse it into one of the two other answers.
///
/// Menu check rows are only ever `Unchecked` or `Checked`; the
/// indeterminate arm exists for the checkbox, and a menu row that
/// reported it would be a projection bug rather than a supported shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Checked {
    /// Off.
    #[default]
    Unchecked,
    /// The `−` dash: some but not all of what is underneath is on.
    Indeterminate,
    /// The `✓` tick.
    Checked,
}

impl Checked {
    /// Every state.
    pub const ALL: [Self; 3] = [Self::Unchecked, Self::Indeterminate, Self::Checked];

    /// Whether the control reads as "on".
    ///
    /// `Indeterminate` counts as on, matching
    /// [`TogglePolarity::On`](crate::widget::toggle::TogglePolarity),
    /// because the dash is an accent-filled box. A selector asking
    /// "is this thing on" wants the same answer the paint gives.
    #[must_use]
    pub const fn is_on(self) -> bool {
        !matches!(self, Self::Unchecked)
    }

    /// Whether the state is the mixed one.
    #[must_use]
    pub const fn is_indeterminate(self) -> bool {
        matches!(self, Self::Indeterminate)
    }
}

impl std::fmt::Display for Checked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unchecked => "unchecked",
            Self::Indeterminate => "indeterminate",
            Self::Checked => "checked",
        })
    }
}

/// The `value` field's payload (`UIF-PRIN-008`).
///
/// Three arms because the controls that carry a value carry three
/// different *kinds* of one: a text field's contents, a count, and a
/// determinate bar's fraction of completion. Collapsing them into a
/// string would make a selector compare `"0.5"` against `"50%"` and
/// call it a match.
#[derive(Clone, Debug, PartialEq)]
pub enum A11yValue<'a> {
    /// Free text — an editable field's contents.
    Text(&'a str),
    /// A whole number — a count, an index, a step position.
    Number(i64),
    /// A normalised fraction, `0.0` to `1.0` inclusive — a determinate
    /// progress bar's completion.
    ///
    /// Normalised rather than a percentage so the projection does not
    /// bake in a display decision, and bounded rather than free so a
    /// caller cannot hand a bar `3.0` and call it progress.
    Fraction(f32),
}

impl A11yValue<'_> {
    /// Whether the value is the text arm.
    #[must_use]
    pub const fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// The fraction, if this is one.
    #[must_use]
    pub const fn as_fraction(&self) -> Option<f32> {
        match self {
            Self::Fraction(value) => Some(*value),
            _ => None,
        }
    }
}

impl std::fmt::Display for A11yValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => write!(f, "'{text}'"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Fraction(value) => write!(f, "{:.2}", value * 100.0),
        }
    }
}

/// One field of the contract, rendered for a report.
///
/// Shared by [`A11yContract`]'s `Display` so a missing field and a
/// present-but-false field read differently on the page, which is the
/// whole distinction this type exists to preserve.
fn field<T: std::fmt::Display>(slot: &Option<T>) -> String {
    match slot {
        Some(value) => value.to_string(),
        None => "—".to_owned(),
    }
}

/// A widget's pure semantic descriptor (`UIF-PRIN-008`).
///
/// Nine fields, exactly the nine §1.1 names. `role` is always known;
/// every other field is `Option` so an absent one means "this control
/// has no such state" rather than a plausible-looking default. See the
/// [module docs](self) for why that distinction is the point.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yContract<'a> {
    /// What the control is.
    pub role: A11yRole,
    /// Its accessible name — the label a screen reader announces.
    pub name: Option<&'a str>,
    /// Its value, for the controls that have one.
    pub value: Option<A11yValue<'a>>,
    /// Whether an expandable thing is expanded.
    pub expanded: Option<bool>,
    /// Whether the control is selected within a group.
    pub selected: Option<bool>,
    /// Its check state, for the checkbox-shaped roles.
    pub checked: Option<Checked>,
    /// Whether work is in flight behind the control.
    pub busy: Option<bool>,
    /// Whether the control is in a validation error.
    pub invalid: Option<bool>,
    /// Whether the control refuses interaction.
    pub disabled: Option<bool>,
}

impl<'a> A11yContract<'a> {
    /// A contract that knows only its role.
    ///
    /// Every other field starts `None`, so a projection has to *add*
    /// each fact it actually read. That default is the safety property:
    /// forgetting a field leaves it absent rather than wrong.
    #[must_use]
    pub const fn new(role: A11yRole) -> Self {
        Self {
            role,
            name: None,
            value: None,
            expanded: None,
            selected: None,
            checked: None,
            busy: None,
            invalid: None,
            disabled: None,
        }
    }

    /// Attach the accessible name.
    #[must_use]
    pub const fn named(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// Attach a value.
    #[must_use]
    pub const fn with_value(mut self, value: A11yValue<'a>) -> Self {
        self.value = Some(value);
        self
    }

    /// Attach the expansion state.
    #[must_use]
    pub const fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Attach the selection state.
    #[must_use]
    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Attach the check state.
    #[must_use]
    pub const fn checked(mut self, checked: Checked) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Attach the busy state.
    #[must_use]
    pub const fn busy(mut self, busy: bool) -> Self {
        self.busy = Some(busy);
        self
    }

    /// Attach the validation state.
    #[must_use]
    pub const fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = Some(invalid);
        self
    }

    /// Attach the enabled state, inverted — the source models say
    /// `disabled`, and so does this field.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// How many of the eight optional slots this contract actually
    /// filled.
    ///
    /// A projection test asserts this number per source model, so
    /// silently dropping a field that *should* have been read shows up
    /// as a count that no longer matches.
    #[must_use]
    pub fn filled_count(&self) -> usize {
        usize::from(self.name.is_some())
            + usize::from(self.value.is_some())
            + usize::from(self.expanded.is_some())
            + usize::from(self.selected.is_some())
            + usize::from(self.checked.is_some())
            + usize::from(self.busy.is_some())
            + usize::from(self.invalid.is_some())
            + usize::from(self.disabled.is_some())
    }

    /// Whether every optional slot is empty.
    ///
    /// The shape a projection for a role with no state at all produces —
    /// a separator, a heading.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.filled_count() == 0
    }

    /// Whether the contract's `checked` field is legal for its role.
    ///
    /// Guards the one cross-field invariant the type has: only the three
    /// checkbox-shaped roles carry a check state, so a projection that
    /// sets `checked` on a `Button` or a `TreeItem` is wrong no matter
    /// how the rest of it reads.
    #[must_use]
    pub const fn checked_matches_role(&self) -> bool {
        self.checked.is_none() || self.role.carries_checked()
    }
}

impl std::fmt::Display for A11yContract<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} name={} value={} expanded={} selected={} checked={} busy={} invalid={} disabled={}",
            self.role.aria_role(),
            field(&self.name),
            field(&self.value),
            field(&self.expanded),
            field(&self.selected),
            field(&self.checked),
            field(&self.busy),
            field(&self.invalid),
            field(&self.disabled),
        )
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod contract_tests;
