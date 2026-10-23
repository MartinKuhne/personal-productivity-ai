//! The `AutoSuggestBox` data model — the query, the suggestions offered
//! against it, the keyboard's place among them, and the payloads
//! §3.2 (`UIF-ATOM-002d`) moves between a host and the control.
//!
//! This is the *declarative* half. The machine that turns keystrokes into
//! debounced provider requests and a highlighted row into a commit lives
//! in [`auto_suggest_host`](super::auto_suggest_host); this file owns the
//! shape a host holds and the geometry the flyout is specified in.
//!
//! # The five fields are the spec's five fields
//!
//! §3.2 writes `AutoSuggestModel<Id> { query, suggestions,
//! highlighted_index, busy, disabled }` and this is that struct, field for
//! field. Two absences are deliberate rather than overlooked:
//!
//! * **No `placeholder`.** A placeholder belongs to the field, and the
//!   field's model is
//!   [`TextInputModel`](crate::widget::text_input_model::TextInputModel),
//!   which already carries one along with the rest of the `002a` chrome.
//!   A second placeholder here would be a second truth about the same
//!   pixel.
//! * **No `min_chars`.** The item does not name one, and the threshold
//!   that actually decides whether a request is worth making is a policy
//!   the host applies to
//!   [`QueryRequested`](crate::widget::auto_suggest_host::AutoSuggestEmission::QueryRequested),
//!   not a property of the box.
//!
//! # Ownership is split by field, not by struct
//!
//! The host owns this struct, but three of its five fields are written by
//! the host's machine and only *read* by the application:
//!
//! | Field | Written by | Read by |
//! |---|---|---|
//! | `query` | the host, mirroring the editor | both |
//! | `suggestions` | the application, from its provider | both |
//! | `highlighted_index` | the host, mirroring the menu cascade | both |
//! | `busy` | the host, mirroring the in-flight request | both |
//! | `disabled` | the application | both |
//!
//! That split is why the host takes `&mut AutoSuggestModel<Id>` rather
//! than a shared borrow: `highlighted_index` and `busy` are projections
//! of machine state onto the model, and a control that could not write
//! them would leave the model describing a state that no longer exists.
//! The application must treat those two as read-only; the host rewrites
//! them on every pass that can change them.
//!
//! # The flyout's geometry is the spec's, and the gap is a token
//!
//! §3.2 fixes the completion flyout at `max_height: 240px`, `32px` rows
//! and an anchor `4px` below the input. The height is a Surface Geometry
//! Constant under `UIF-PRIN-001` §1.1 — a composite bound the spec sets
//! outright — while the row height and the gap are read from
//! [`size::CONTROL_MD`] and [`space::XS`], which is where those two
//! numbers live anyway. The row height is the same `32px` the menu
//! engine's [`ROW_HEIGHT`](crate::surface::menu::view::ROW_HEIGHT) uses,
//! so a completion row and a menu row are the same row.
//!
//! # The debounce window is not in the spec
//!
//! §3.2 says the box completes; it never says how long the box waits
//! before asking. A window is unavoidable — the alternative is one
//! provider call per keystroke — so [`DEBOUNCE_MS`] is a module-level
//! named constant, the same treatment `token::time`'s absence forced on
//! [`HOVER_DELAY`](crate::surface::tooltip::HOVER_DELAY). A host that
//! wants a different window passes its own to
//! [`with_debounce`](crate::widget::auto_suggest_host::AutoSuggestHost::with_debounce);
//! what it must not do is discover the default by timing the UI.
//!
//! Unit tests live in the sibling `auto_suggest_model_tests.rs` sidecar.

use crate::focus::FocusId;
use crate::surface::menu::model::{Menu, MenuId, Row};
use crate::token::{shape, size, space};

/// Maximum height of the completion flyout — `240px` (§3.2,
/// `UIF-ATOM-002d`).
///
/// A Surface Geometry Constant under `UIF-PRIN-001` §1.1: the spec sets
/// the figure outright and the registry in
/// [`token_lint`](crate::token_lint) carries the §-reference that
/// authorises it.
pub const FLYOUT_MAX_HEIGHT: f32 = 240.0;

/// Height of one completion row — `size.control.md`, the `32px` §3.2
/// names for its flyout rows.
pub const FLYOUT_ROW_HEIGHT: f32 = size::CONTROL_MD;

/// Gap between the input's bottom edge and the flyout's top edge —
/// `space.xs`, the `4px` §3.2 anchors it at.
pub const FLYOUT_GAP: f32 = space::XS;

/// Corner radius of the flyout card — `radius.widget`, the same radius
/// the input above it carries so the pair reads as one control.
pub const FLYOUT_RADIUS: f32 = shape::RADIUS_WIDGET;

/// How long the host waits after the last keystroke before asking the
/// provider for suggestions — `200ms`.
///
/// Trailing-edge: every keystroke restarts the window, so a burst of ten
/// typed characters produces **one** request carrying the final text
/// rather than ten carrying nine dead ones. The value is a framework
/// choice, not a spec figure — see the
/// [module docs](self#the-debounce-window-is-not-in-the-spec).
pub const DEBOUNCE_MS: u64 = 200;

/// One completion candidate: a stable id the application acts on and the
/// label the row shows (§3.2, `UIF-ATOM-002d`).
///
/// The id is what survives a list rebuild. Row indices do not — a
/// suggestion that was third before the provider trimmed the list may be
/// first or gone — so every commit maps through the id, never through a
/// position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestionItem<Id> {
    /// The application's own identity for this candidate.
    pub id: Id,
    /// The text the row shows, and what the field reads after a commit.
    pub label: String,
}

impl<Id> SuggestionItem<Id> {
    /// A candidate carrying `id` and showing `label`.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }
}

/// How a completion was committed (§3.2, `UIF-ATOM-002d`).
///
/// The spec names three sources and this is those three, unchanged. They
/// are a sum type rather than a boolean or a "was it the mouse" flag
/// because the three carry genuinely different meanings a host can act
/// on: `Enter` is an explicit "yes, this one", `Tab` is "this one, and
/// move on", and `Click` is a pointer choosing what the keyboard was
/// not on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommitSource {
    /// `Enter` with a row highlighted.
    Enter,
    /// A pointer press released over the row.
    Click,
    /// `Tab` with a row highlighted.
    Tab,
}

impl CommitSource {
    /// The slug this source is recorded under.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Click => "click",
            Self::Tab => "tab",
        }
    }
}

/// A committed completion: which candidate, how it was chosen, and what
/// the field now reads.
///
/// The spec's callback is `on_suggestion_commit(Id, CommitSource)`. The
/// third field is a recorded addition, and it is here for the same reason
/// [`SecretInput`](super::password_model::SecretInput) carries a value
/// where the spec's `on_commit()` carried none: a message that arrives
/// without the text it produced forces the receiver to go re-read a list
/// that may already have been rebuilt by the time the message is handled.
/// `label` is the suggestion's label at the instant of the commit, so the
/// host can set the field's text from the message alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoSuggestCommit<Id> {
    /// The candidate that was chosen.
    pub id: Id,
    /// Whether `Enter`, a click or `Tab` settled it.
    pub source: CommitSource,
    /// The chosen candidate's label — the text the field settles to.
    pub label: String,
}

impl<Id> AutoSuggestCommit<Id> {
    /// A commit of `id` via `source`, settling the field to `label`.
    #[must_use]
    pub fn new(id: Id, source: CommitSource, label: impl Into<String>) -> Self {
        Self {
            id,
            source,
            label: label.into(),
        }
    }
}

/// Everything the `AutoSuggestBox` shows and does, as one value the host
/// owns (§3.2, `UIF-ATOM-002d`).
///
/// See the [module docs](self#ownership-is-split-by-field-not-by-struct)
/// for who writes which field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoSuggestModel<Id> {
    /// The text in the field.
    pub query: String,
    /// The candidates currently offered for it.
    pub suggestions: Vec<SuggestionItem<Id>>,
    /// Index into [`suggestions`](Self::suggestions) that the keyboard is
    /// on, if any.
    pub highlighted_index: Option<usize>,
    /// A provider request for the current query is outstanding.
    pub busy: bool,
    /// The control is inert.
    pub disabled: bool,
}

/// An empty, enabled model.
///
/// Hand-written rather than derived so `Id` is not required to implement
/// [`Default`]: the model's default state holds no candidate at all, so
/// requiring the candidate type to have a default value would be asking
/// for something this state does not contain.
impl<Id> Default for AutoSuggestModel<Id> {
    fn default() -> Self {
        Self {
            query: String::new(),
            suggestions: Vec::new(),
            highlighted_index: None,
            busy: false,
            disabled: false,
        }
    }
}

impl<Id> AutoSuggestModel<Id> {
    /// An empty, enabled model with nothing offered and nothing in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the text the field reads.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// Offer `suggestions`.
    #[must_use]
    pub fn with_suggestions(mut self, suggestions: Vec<SuggestionItem<Id>>) -> Self {
        self.suggestions = suggestions;
        self
    }

    /// A model that cannot be used.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Is there anything to offer?
    #[must_use]
    pub fn has_suggestions(&self) -> bool {
        !self.suggestions.is_empty()
    }

    /// Is the field empty of anything worth completing?
    ///
    /// Trimmed, because a field holding three spaces is empty for every
    /// purpose this control has: it completes to nothing, and asking a
    /// provider about it is a wasted call.
    #[must_use]
    pub fn query_is_blank(&self) -> bool {
        self.query.trim().is_empty()
    }

    /// The candidate at `index` of the offered list.
    #[must_use]
    pub fn suggestion_at(&self, index: usize) -> Option<&SuggestionItem<Id>> {
        self.suggestions.get(index)
    }

    /// The candidate the keyboard is on.
    ///
    /// `None` when nothing is highlighted, and also `None` when
    /// `highlighted_index` points past the end of a list that has since
    /// been replaced — a stale index reads as "nothing highlighted"
    /// rather than panicking, because a provider that returned a shorter
    /// list is ordinary and the model must survive it.
    #[must_use]
    pub fn highlighted(&self) -> Option<&SuggestionItem<Id>> {
        self.highlighted_index
            .and_then(|index| self.suggestions.get(index))
    }

    /// The index of the candidate carrying `id`, if the list still has it.
    #[must_use]
    pub fn index_of(&self, id: &Id) -> Option<usize>
    where
        Id: PartialEq,
    {
        self.suggestions.iter().position(|item| &item.id == id)
    }

    /// The menu the completion flyout is built from.
    ///
    /// `menu_id` is supplied by the caller rather than derived from the
    /// query, and that is the load-bearing decision in this method. The
    /// menu engine resolves an open card by identity
    /// ([`MenuCascade::sync`](crate::surface::menu::cascade::MenuCascade::sync));
    /// an id that moved with the text would make every keystroke look
    /// like a *different* card, so the open one would close and a new one
    /// open — a flyout that flickers once per character. The host mints
    /// the id from the **control** once, at construction, and this method
    /// cannot see the query at all.
    ///
    /// Each candidate becomes an [`action`](Row::action) row keyed by the
    /// candidate's `Display` form:
    ///
    /// * `action` rather than [`checkable`](Row::checkable) because a
    ///   completion row carries no checkmark — §3.2 gives it none, unlike
    ///   §3.4's selected combobox row.
    /// * The default close policy, which for an action row is *closes*:
    ///   picking a completion ends the interaction.
    #[must_use]
    pub fn menu_with(&self, menu_id: MenuId) -> Menu
    where
        Id: std::fmt::Display,
    {
        let rows = self
            .suggestions
            .iter()
            .map(|item| Row::action(item.id.to_string(), item.label.clone()))
            .collect();
        Menu::new(menu_id, rows)
    }

    /// The menu this model's current suggestions build, keyed off
    /// `control` so the identity is the control's and not the text's.
    #[must_use]
    pub fn menu_for(&self, control: &FocusId) -> Menu
    where
        Id: std::fmt::Display,
    {
        self.menu_with(menu_id_for(control))
    }

    /// Would these suggestions support a flyout the keyboard can enter?
    ///
    /// The question the no-match policy turns on. It is phrased as
    /// navigability rather than `!suggestions.is_empty()` so it stays
    /// correct if a future `SuggestionItem` ever grows an `enabled` flag:
    /// the engine will not host a card with no focusable row
    /// ([`NoFocusableRows`](crate::surface::menu::cascade::IgnoredReason::NoFocusableRows)),
    /// so "nothing pickable" and "nothing openable" are the same fact.
    #[must_use]
    pub fn has_pickable(&self, control: &FocusId) -> bool
    where
        Id: std::fmt::Display,
    {
        self.menu_for(control).first_navigable().is_some()
    }
}

/// The flyout identity for the completion list owned by `control`.
///
/// Derived from the control alone. Two different queries on the same box
/// produce the same id, which is exactly what keeps
/// [`MenuCascade::sync`](crate::surface::menu::cascade::MenuCascade::sync)
/// resolving the card that is already open.
#[must_use]
pub fn menu_id_for(control: &FocusId) -> MenuId {
    MenuId::new(format!("suggest:{control}"))
}

#[cfg(test)]
#[path = "auto_suggest_model_tests.rs"]
mod auto_suggest_model_tests;
