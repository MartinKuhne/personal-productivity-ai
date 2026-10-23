//! The `ComboBox` / `Select` data model of §3.4 (`UIF-ATOM-004`).
//!
//! # The spec's field list, and the four fields it does not have
//!
//! §3.4 prints `ComboBoxModel<Id> { items, selected, placeholder,
//! filter_query, open, validation, disabled }`. Every one of those is
//! here with the same name. Four more are added, each because the spec's
//! own text needs a place to land that its field list does not provide:
//!
//! * [`editable`][ComboBoxModel::is_editable] — §3.4 attaches
//!   `on_filter_change` with the parenthetical "*when searchable*", which
//!   is a mode the model has to carry but never names. `editable` is that
//!   mode. It defaults to **off**, making the closed select (click → pick)
//!   the base case and the type-to-filter combobox the opt-in extension,
//!   because the spec's filter callback is the conditional one.
//! * `read_only` — **not in §3.4 at all.** Imported from
//!   [`TextInputModel`][crate::widget::text_input_model::TextInputModel]'s
//!   read-only input, where the control is live, focusable and browsable but
//!   cannot change its value. See the
//!   [read-only section](self#read-only-is-imported-not-specified) below.
//! * [`filter_policy`][ComboBoxModel::filter_policy] — §3.4 says a
//!   combobox can be filtered and never says *how*. Rather than hide a
//!   matching rule inside a method, the rule is a named, overridable value
//!   so every default in it is visible and auditable. See
//!   [Filter semantics](self#filter-semantics-four-spec-silences).
//! * `menu_id` — the flyout's identity. It is **derived from the control,
//!   never from the filter**, because
//!   [`MenuCascade::sync`](crate::surface::menu::cascade::MenuCascade) resolves an
//!   open card against its id: an id that changed with each keystroke would
//!   read as a different tree and tear the open flyout down between
//!   characters.
//!
//! `open` is a **projection**, not a source of truth, exactly as
//! [`SplitButtonModel::menu_open`](crate::widget::split_button_model::SplitButtonModel::menu_open)
//! is: the fact lives in the
//! [`MenuCascade`](crate::surface::menu::cascade::MenuCascade) the host drives, and the model mirrors it so a host
//! rendering from the model alone can still tell whether the chevron
//! reads as open. Nothing in this type opens or closes anything.
//!
//! # Filter semantics: four spec silences
//!
//! §3.4 gives `filter_query` and `on_filter_change(String)` and then
//! moves on. Four questions have to be answered to ship a filter, and the
//! spec answers none of them. Each is recorded here as a gap with the
//! decision that was made, rather than left implicit:
//!
//! | Question | Spec | Decision | Why |
//! |---|---|---|---|
//! | Case handling | silent | case-**insensitive** | a label's capitalisation is not something the user is asked to reproduce; a case-sensitive filter fails silently on the first mismatch |
//! | Substring vs prefix | silent | **substring** (`contains`) | prefix matching cannot find "Word Wrap" under `wrap`, which is the ordinary reason to filter a list at all |
//! | Is the selected row hidden while filtering? | silent | **kept** | this is the one case the spec *does* settle indirectly: §3.4 mandates a `✓` in the selected row's leading slot, and hiding the selected row makes that mandate unreachable while filtering |
//! | What shows when nothing matches? | silent | **the flyout cannot be shown** | not a taste call — the shared menu engine refuses any card with no focusable row ([`IgnoredReason::NoFocusableRows`][crate::surface::menu::cascade::IgnoredReason]) because a trap over an empty scope is what makes the focus model unsound. See the host's docs |
//!
//! The first two are overridable through [`FilterPolicy`]. The third is
//! fixed by the spec's own `✓` mandate. The fourth is fixed by the engine.
//!
//! # Type-ahead on the closed select is not implemented — and cannot be
//!
//! §3.4 does not mention type-ahead. None is implemented, and the reason
//! is structural rather than an omission: the closed select owns **no text
//! buffer**, so there is no keystroke it could claim a character from.
//! Its whole keyboard vocabulary is
//! [`ComboKey`][crate::widget::combo_host::ComboKey] — arrows, `Enter`,
//! `Space`, `Home`, `End`, `Escape` — and printable characters are not
//! in it. That is also what keeps the two variants from ever contending
//! for one key: the editable variant owns printable characters *because*
//! it owns the buffer they go into, and routes them through
//! [`on_filter_typed`][crate::widget::combo_host::ComboHost::on_filter_typed]
//! rather than through the control-level key vocabulary. If type-ahead is
//! ever specced for the closed select it must not be built by reusing the
//! filter channel, or the two become indistinguishable to the host.
//!
//! # `ActionPending` is not supported
//!
//! §3.4's model carries no `busy` and no `progress`, and §3.0.4.3 names
//! the controls that take `ActionPending` — `Button`, `ToggleSwitch`,
//! `MenuRow`, `Modal Primary CTA` — without naming the combobox. So this
//! model has no pending state, and a long-running option load is
//! unspecified. Recorded rather than invented: adding `busy` would imply a
//! suppression rule §3.0.4.3 does not extend to this control.
//!
//! # Read-only is imported, not specified
//!
//! §3.4 has no read-only state. The one here means: the control is
//! focusable, the flyout opens, the list is browsable and the `✓` still
//! marks the current value — but activating a row commits nothing. Browsing
//! is not mutation, and a read-only combobox that will not open would be
//! indistinguishable from a disabled one, which `UIF-PRIN-004` says must
//! not be the case.
//!
//! Unit tests live in the sibling `combo_model_tests.rs` sidecar.

use std::fmt;

use crate::surface::menu::model::{Menu, MenuId, Row};
use crate::widget::combo_model::CaseHandling::{Insensitive, Sensitive};
use crate::widget::combo_model::MatchMode::{Contains, Prefix};

/// The flyout's maximum height — the `max_height = 280px` §3.4 gives it.
///
/// A crate-local constant because §3.4 states the number inline and
/// `UIF-TOK-002` carries no token for a flyout ceiling. It is a
/// *ceiling*, not a height: a short list is as short as it is.
pub const FLYOUT_MAX_HEIGHT: f32 = 280.0;

/// The gap between the trigger and its flyout — `4px` (`space.xs`), per
/// §3.4.
pub const FLYOUT_GAP: f32 = crate::token::space::XS;

/// The flyout's minimum width, expressed as a function of the trigger:
/// §3.4 says `min_width = trigger.width`, so the list never reads as
/// narrower than the control that opened it.
#[must_use]
pub fn flyout_min_width(trigger_width: f32) -> f32 {
    trigger_width.max(0.0)
}

// ---------------------------------------------------------------------------
// The option
// ---------------------------------------------------------------------------

/// One choice in a [`ComboBoxModel`].
///
/// Deliberately narrower than a menu row: §3.4 gives an option an id, a
/// label and whether it can be picked. There is no destructive flag, no
/// shortcut hint and no icon, because a combobox option is a *value*, not
/// a command — and the menu engine's "destructive rows go last" rule
/// would otherwise start ordering a list the user expects to read in the
/// order the application chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectOption<Id> {
    /// The application's own identity for this choice.
    pub id: Id,
    /// The visible label.
    pub label: String,
    /// Whether the choice can be picked.
    pub enabled: bool,
}

impl<Id> SelectOption<Id> {
    /// A live option.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            enabled: true,
        }
    }

    /// Mark the option inert: it renders in the list and cannot be picked.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set the enabled flag explicitly.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

// ---------------------------------------------------------------------------
// Filter policy
// ---------------------------------------------------------------------------

/// How a filter query is folded against a label's case.
///
/// A gap-filling decision, not a spec rule — see the
/// [module docs](self#filter-semantics-four-spec-silences).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CaseHandling {
    /// `Wrap` matches `wrap`, `WRAP` and `wRaP`.
    #[default]
    Insensitive,
    /// The characters have to match as written.
    Sensitive,
}

/// Where in the label a query has to appear to count as a match.
///
/// A gap-filling decision, not a spec rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MatchMode {
    /// The query may appear anywhere in the label.
    #[default]
    Contains,
    /// The label has to begin with the query.
    Prefix,
}

/// The three filter questions §3.4 does not answer, as one value.
///
/// Making the policy a value rather than a private helper is the point:
/// a reader of the model can see that case folding and substring matching
/// are *choices*, can change them, and can test both ways. A hidden
/// `str::contains` inside a filter method would make the same behaviour
/// look like a fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FilterPolicy {
    /// Case folding.
    pub case: CaseHandling,
    /// Where the query has to match.
    pub mode: MatchMode,
    /// Whether the currently-selected option stays in the filtered list.
    ///
    /// `true` by default because §3.4 mandates a `✓` in the selected
    /// row's leading slot; a filter that hid that row would make the
    /// mandate unreachable.
    ///
    /// One consequence worth stating: while this is on and something *is*
    /// selected, no query can empty the list — the selected row always
    /// survives. A genuine "nothing matches" state therefore only arises
    /// with no selection, or with this flag off.
    pub keep_selected: bool,
}

impl Default for FilterPolicy {
    /// The defaults this increment settled on for a silent spec:
    /// case-insensitive, substring, selected row kept.
    fn default() -> Self {
        Self {
            case: Insensitive,
            mode: Contains,
            keep_selected: true,
        }
    }
}

impl FilterPolicy {
    /// A policy from a case rule and a match rule, keeping the selected
    /// row visible.
    #[must_use]
    pub const fn new(case: CaseHandling, mode: MatchMode) -> Self {
        Self {
            case,
            mode,
            keep_selected: true,
        }
    }

    /// Does `label` match `query` under this policy?
    ///
    /// An empty or whitespace-only query matches everything: the filter
    /// is *off*, and an "off" filter that hid rows would be worse than no
    /// filter at all. Trimming is what makes a lone space read as
    /// "nothing typed yet" rather than as a query that hides every label
    /// without a space in it.
    #[must_use]
    pub fn matches(&self, query: &str, label: &str) -> bool {
        let query = query.trim();
        if query.is_empty() {
            return true;
        }

        match (self.case, self.mode) {
            (Insensitive, Contains) => label.to_lowercase().contains(&query.to_lowercase()),
            (Insensitive, Prefix) => label.to_lowercase().starts_with(&query.to_lowercase()),
            (Sensitive, Contains) => label.contains(query),
            (Sensitive, Prefix) => label.starts_with(query),
        }
    }
}

// ---------------------------------------------------------------------------
// The pre-open snapshot
// ---------------------------------------------------------------------------

/// What a cancelled combobox gesture restores.
///
/// Two fields rather than one because the editable variant shows two
/// things the user can have changed by the time they press `Escape`: the
/// selection, and the text they typed. §3.4 only names the selection, and
/// reverting *only* the selection would leave the user looking at a
/// filter that hides the value they were just given back — a revert that
/// cannot be seen is not a revert. The filter is therefore part of the
/// snapshot, and the gap that forced it is recorded in the
/// [module docs](self#filter-semantics-four-spec-silences).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComboSnapshot<Id> {
    /// The selection as it stood when the flyout opened.
    pub selected: Option<Id>,
    /// The filter query as it stood when the flyout opened.
    pub filter: Option<String>,
}

impl<Id> ComboSnapshot<Id> {
    /// A snapshot of a selection and a filter.
    #[must_use]
    pub fn new(selected: Option<Id>, filter: Option<String>) -> Self {
        Self { selected, filter }
    }
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// The state of one `ComboBox` / `Select` control (§3.4, `UIF-ATOM-004`).
#[derive(Clone, Debug, PartialEq)]
pub struct ComboBoxModel<Id> {
    items: Vec<SelectOption<Id>>,
    selected: Option<Id>,
    placeholder: String,
    filter_query: Option<String>,
    open: bool,
    validation: Option<String>,
    disabled: bool,
    read_only: bool,
    editable: bool,
    filter_policy: FilterPolicy,
    menu_id: MenuId,
}

impl<Id: Clone + fmt::Display + PartialEq> ComboBoxModel<Id> {
    /// A closed (non-editable) select over `items` with `placeholder`.
    ///
    /// The base case: click, pick, done. Reach for
    /// [`editable`](Self::editable) for the type-to-filter variant.
    #[must_use]
    pub fn new(items: Vec<SelectOption<Id>>, placeholder: impl Into<String>) -> Self {
        let placeholder = placeholder.into();
        let menu_id = MenuId::new(format!("combo:{placeholder}"));
        Self {
            items,
            selected: None,
            placeholder,
            filter_query: None,
            open: false,
            validation: None,
            disabled: false,
            read_only: false,
            editable: false,
            filter_policy: FilterPolicy::default(),
            menu_id,
        }
    }

    /// Make the control editable: typing filters the list.
    #[must_use]
    pub const fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Make the control read-only: browsable, never changed.
    ///
    /// See the [module docs](self#read-only-is-imported-not-specified).
    #[must_use]
    pub const fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Mark the whole control inert.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the current selection.
    #[must_use]
    pub fn with_selected(mut self, id: impl Into<Option<Id>>) -> Self {
        self.selected = id.into();
        self
    }

    /// Set the filter query.
    #[must_use]
    pub fn with_filter(mut self, query: impl Into<Option<String>>) -> Self {
        self.filter_query = query.into();
        self
    }

    /// Attach a validation message.
    #[must_use]
    pub fn with_validation(mut self, message: impl Into<String>) -> Self {
        self.validation = Some(message.into());
        self
    }

    /// Replace the filter policy.
    #[must_use]
    pub const fn with_filter_policy(mut self, policy: FilterPolicy) -> Self {
        self.filter_policy = policy;
        self
    }

    /// Override the flyout's identity.
    ///
    /// Needed when two comboboxes share a placeholder and would otherwise
    /// claim the same menu id — which the cascade would read as the same
    /// tree being re-opened.
    #[must_use]
    pub fn with_menu_id(mut self, id: impl Into<String>) -> Self {
        self.menu_id = MenuId::new(id);
        self
    }

    /// Project the cascade's openness onto the model. Call once per frame
    /// from the host.
    #[must_use]
    pub const fn with_open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    // -- reads -------------------------------------------------------------

    /// Every option, in the order the application supplied it.
    #[must_use]
    pub fn items(&self) -> &[SelectOption<Id>] {
        &self.items
    }

    /// The current selection.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// The placeholder shown when nothing is selected.
    #[must_use]
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// The current filter query.
    #[must_use]
    pub fn filter_query(&self) -> Option<&str> {
        self.filter_query.as_deref()
    }

    /// Whether the flyout is showing. A mirror of the cascade's state.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// The validation message, if the control is invalid.
    #[must_use]
    pub fn validation(&self) -> Option<&str> {
        self.validation.as_deref()
    }

    /// Whether the whole control is inert.
    #[must_use]
    pub const fn disabled_flag(&self) -> bool {
        self.disabled
    }

    /// Whether the control is browsable but cannot change its value.
    #[must_use]
    pub const fn read_only_flag(&self) -> bool {
        self.read_only
    }

    /// Whether typing filters the list.
    #[must_use]
    pub const fn is_editable(&self) -> bool {
        self.editable
    }

    /// The filter policy in force.
    #[must_use]
    pub const fn filter_policy(&self) -> FilterPolicy {
        self.filter_policy
    }

    /// The flyout's identity.
    #[must_use]
    pub const fn menu_id(&self) -> &MenuId {
        &self.menu_id
    }

    /// Whether the control is carrying a validation failure.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.validation.is_some()
    }

    /// The label the trigger shows: the selected option's label, or the
    /// placeholder when nothing is selected.
    #[must_use]
    pub fn display_label(&self) -> &str {
        self.selected_option()
            .map(|option| option.label.as_str())
            .unwrap_or(&self.placeholder)
    }

    /// Whether the trigger is showing its placeholder rather than a value.
    #[must_use]
    pub fn shows_placeholder(&self) -> bool {
        self.selected_option().is_none()
    }

    /// The selected option itself, if the selection names one.
    ///
    /// A selection that names no option reads as *no selection* rather
    /// than panicking: the host owns the id, and a stale id after the
    /// option list is rebuilt is a rebuild the model absorbs.
    #[must_use]
    pub fn selected_option(&self) -> Option<&SelectOption<Id>> {
        let selected = self.selected.as_ref()?;
        self.items.iter().find(|option| &option.id == selected)
    }

    /// The index of the selected option in the full list.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        self.items.iter().position(|option| &option.id == selected)
    }

    // -- filtering ---------------------------------------------------------

    /// The options visible under `query`, in list order.
    ///
    /// Takes the query as an argument rather than reading
    /// [`filter_query`](Self::filter_query) so the host can ask about a
    /// *prospective* filter — what the list would look like after the
    /// keystroke it has not yet applied to the caller's model.
    #[must_use]
    pub fn filtered_with(&self, query: Option<&str>) -> Vec<&SelectOption<Id>> {
        let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
            return self.items.iter().collect();
        };
        let selected = self.selected.clone();
        self.items
            .iter()
            .filter(|option| {
                self.filter_policy.keep_selected && Some(&option.id) == selected.as_ref()
                    || self.filter_policy.matches(query, &option.label)
            })
            .collect()
    }

    /// The options visible under the model's current filter.
    #[must_use]
    pub fn filtered(&self) -> Vec<&SelectOption<Id>> {
        self.filtered_with(self.filter_query.as_deref())
    }

    /// Does `query` leave at least one **pickable** option on screen?
    ///
    /// This is the question the flyout's existence turns on: a list of
    /// only-disabled options is as unopenable as an empty one, because
    /// the cascade will not host a card the keyboard cannot enter.
    #[must_use]
    pub fn has_pickable_with(&self, query: Option<&str>) -> bool {
        self.filtered_with(query)
            .iter()
            .any(|option| option.enabled)
    }

    /// Does the current filter leave at least one pickable option?
    #[must_use]
    pub fn has_pickable(&self) -> bool {
        self.has_pickable_with(self.filter_query.as_deref())
    }

    /// The option at `index` **of the filtered list** for `query`.
    ///
    /// The flyout's row indices are filtered indices, not full-list
    /// indices, and every mapping back to the application's id goes
    /// through here or [`id_by_key`](Self::id_by_key).
    #[must_use]
    pub fn filtered_option_with(
        &self,
        query: Option<&str>,
        index: usize,
    ) -> Option<&SelectOption<Id>> {
        self.filtered_with(query).into_iter().nth(index)
    }

    /// The application id whose `Display` form is `key`.
    ///
    /// The menu engine keys its rows by string, so this is how a row the
    /// engine activated is turned back into the application's own id
    /// without trusting that a row index still means the same thing after
    /// a rebuild.
    #[must_use]
    pub fn id_by_key(&self, key: &str) -> Option<&Id> {
        self.items
            .iter()
            .find(|option| option.id.to_string() == key)
            .map(|option| &option.id)
    }

    /// Capture what `Escape` will restore.
    #[must_use]
    pub fn snapshot(&self) -> ComboSnapshot<Id>
    where
        Id: Clone,
    {
        ComboSnapshot::new(self.selected.clone(), self.filter_query.clone())
    }

    // -- the engine-side flyout --------------------------------------------

    /// Build the menu the cascade opens for `query`.
    ///
    /// Each visible option becomes a **checkable** row that closes the
    /// tree when activated:
    ///
    /// * `Check` because §3.4 mandates a `✓` in the selected row's
    ///   leading slot, and `Check` is the engine's `✓` kind — `Radio` is
    ///   the `•` kind. The engine already paints that glyph in
    ///   `accent.text`, which is the colour §3.4 names, so the mandate
    ///   is met by the reuse rather than by a second renderer.
    /// * `closes_menu` because the default for a checkable is to stay
    ///   open so several toggles fit in one visit, and a combobox is not
    ///   that: §3.4 says picking an option closes the popup.
    #[must_use]
    pub fn menu_with(&self, query: Option<&str>) -> Menu {
        let selected = self.selected.clone();
        let rows = self
            .filtered_with(query)
            .into_iter()
            .map(|option| {
                Row::checkable(
                    option.id.to_string(),
                    option.label.clone(),
                    Some(&option.id) == selected.as_ref(),
                )
                .with_enabled(option.enabled)
                .closes_menu()
            })
            .collect();
        Menu::new(self.menu_id.clone(), rows)
    }

    /// Build the flyout for the model's current filter.
    #[must_use]
    pub fn menu(&self) -> Menu {
        self.menu_with(self.filter_query.as_deref())
    }
}

#[cfg(test)]
#[path = "combo_model_tests.rs"]
mod combo_model_tests;
