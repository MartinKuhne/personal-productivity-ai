//! The `AutoSuggestBox` machine — the binding §3.2 (`UIF-ATOM-002d`)
//! asks for between a text field, a completion flyout and a provider that
//! answers late.
//!
//! Both halves the control joins already existed and are separately proven:
//! [`TextInputController`] owns the editing session, and
//! [`MenuCascade`] owns the card. What this file adds is the third thing
//! neither of them can own — the **request lifecycle**, because the
//! provider is not in the crate, answers on a schedule of its own, and
//! can therefore be wrong by the time it arrives.
//!
//! # Time enters as a value
//!
//! Every timed method takes `now: TimeMs`, the same clock-as-value idiom
//! [`HoverController`](crate::surface::tooltip::HoverController) uses,
//! and nothing here reads a clock. That is not stylistic. A debounce that
//! read the clock itself could not be tested for "exactly one pass per
//! burst" without sleeping, and a control whose timing behaviour depends
//! on wall-clock time is one whose tests are either slow or lying. The
//! host calls [`TimeMs::now`] once per frame and hands the same instant
//! to every control it drives that frame.
//!
//! # One request per burst, and every late answer refused
//!
//! Each keystroke re-arms the debounce window rather than extending a
//! queue, so ten characters typed inside the window produce **one**
//! [`QueryRequested`](AutoSuggestEmission::QueryRequested) carrying the
//! final text. That request carries a **generation** — a monotonic token
//! the host must echo back through
//! [`apply_suggestions`](AutoSuggestHost::apply_suggestions). A response
//! whose generation is not the one in flight is dropped and counted in
//! [`stale_dropped`](AutoSuggestHost::stale_dropped).
//!
//! The generation is what makes the out-of-order case safe, and it is the
//! whole reason the control cannot simply "set the suggestions". Type
//! `ab`, the request for `ab` goes out; type `abc`, that request goes
//! out too; the provider answers `abc` first and `ab` second. Without a
//! token the late `ab` answer overwrites the list the user is looking at
//! with completions for text they no longer have, and the highlighted row
//! they were about to press belongs to nothing.
//!
//! # `Escape` is two-stage, and the second stage clears rather than reverts
//!
//! | State at `Escape` | Result |
//! |---|---|
//! | Flyout open | Flyout closes; the text stays; the editing session stays open |
//! | Flyout closed, query non-empty | The query is cleared to empty |
//! | Flyout closed, query empty | Nothing — the key is not consumed and travels to the next rank |
//!
//! The first stage is the spec's "`Escape` closes flyout first"; the
//! second is its "second `Escape` clears query per `Escape Rank`", and
//! the third row is what keeps the control from swallowing a key that
//! belongs to a modal or the workspace above it.
//!
//! The second stage **clears** rather than reverting to the pre-edit
//! snapshot, which is a real difference from the plain field's `Escape`
//! and is deliberate: the spec writes it that way, and the reason holds
//! independently. A search term is not a value being edited toward a
//! final form — it is disposable text whose useful reset is "empty", not
//! "whatever was there when you focused". A user who clears `report`
//! down to `repo` and presses `Escape` wants the box empty; restoring
//! `report` would put back text they had already decided against. The
//! revert tier is not lost — it stays available on the inner
//! [`revert`](TextInputController::revert) for a host that wants it — it
//! is simply not what this control's `Escape` does.
//!
//! # A dismissed query stays dismissed
//!
//! `Escape` records the query it dismissed, and the flyout will not
//! reopen for that exact text. Without this the rule "the response opens
//! the flyout" and the rule "`Escape` closed the flyout" contradict each
//! other the moment a response is still in flight: the user presses
//! `Escape`, and two hundred milliseconds later the list they just
//! dismissed reappears. The dismissal is keyed on the **text**, not on a
//! flag, so typing anything else re-arms the control immediately, and
//! the dismissal is dropped by a commit, a clear, focus loss or a reset.
//!
//! # The flyout and the field's caret do not fight
//!
//! `Down`/`Up` rove the highlight without taking the caret away from the
//! input, because the two live in different systems.
//! [`FocusState`] is this framework's routing
//! model — it says which logical target the keyboard's arrows belong to —
//! while the caret is iced's editor state, reached through the widget's
//! own `operation::Focusable`. Roving the cascade moves the former and
//! issues no iced focus operation at all, so the caret stays exactly
//! where the user left it and `Left`/`Right` still edit the text they
//! typed while the highlight sits on the third suggestion.
//!
//! # A query that matches nothing opens nothing
//!
//! The engine will not host a card with no focusable row
//! ([`NoFocusableRows`](crate::surface::menu::cascade::IgnoredReason::NoFocusableRows)),
//! because a focus trap over an empty scope is unsound — the trap would
//! have nowhere to put the keyboard. The autosuggest does not work around
//! that and does not need to: an empty suggestion list builds a menu with
//! no navigable row, so the flyout simply does not open and the field
//! keeps the focus it already had. No empty-state card, no "no results"
//! row, no disabled placeholder. A completion list that has nothing to
//! complete is invisible, which is the honest rendering of nothing.
//!
//! # The three commit sources are not interchangeable
//!
//! [`Enter`](CommitSource::Enter), [`Click`](CommitSource::Click) and
//! [`Tab`](CommitSource::Tab) all settle the same candidate but arrive
//! from different intents, and the host is told which. `Tab` is the
//! subtle one: it commits *and* the focus is expected to move, so a host
//! that treats `Tab` as "stay and show the completion" will trap a
//! keyboard user in a field they were trying to leave.
//!
//! Unit tests live in the sibling `auto_suggest_host_tests.rs` sidecar.

use std::fmt;

use crate::binding::{Commit, Live};
use crate::focus::{FocusId, FocusState};
use crate::surface::menu::cascade::{MenuCascade, MenuTreeKind};
use crate::surface::menu::model::MenuId;
use crate::surface::tooltip::TimeMs;
use crate::widget::auto_suggest_model::{
    AutoSuggestCommit, AutoSuggestModel, CommitSource, DEBOUNCE_MS, SuggestionItem, menu_id_for,
};
use crate::widget::text_input_controller::{TextInputController, TextInputEmission};

/// One key the autosuggest host understands.
///
/// The host is driven rather than driving: the application's key router
/// decides that a key belongs to this control and hands it over already
/// classified, exactly as [`ComboKey`](super::combo_host::ComboKey) is
/// handed to the combobox.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AutoSuggestKey {
    /// `Up` — rove the highlight toward the top, wrapping.
    Up,
    /// `Down` — rove the highlight toward the bottom, wrapping.
    Down,
    /// `Enter` — commit the highlighted candidate, or settle the query.
    Enter,
    /// `Tab` — commit the highlighted candidate and let focus move on.
    Tab,
    /// `Escape` — the two-stage unwind.
    Escape,
}

/// One thing the autosuggest control tells its host.
///
/// The set is the spec's four callbacks plus the three tiers
/// §3.0.4.2 requires and the one event the async contract cannot do
/// without. Each arm means exactly one thing:
///
/// | Arm | Spec origin | Tier |
/// |---|---|---|
/// | [`QueryChanged`](Self::QueryChanged) | `on_query_change(String)` | Live |
/// | [`QueryRequested`](Self::QueryRequested) | the debounce contract | Live |
/// | [`HighlightChanged`](Self::HighlightChanged) | `on_suggestion_highlight(usize)` | Live |
/// | [`Committed`](Self::Committed) | `on_suggestion_commit(Id, CommitSource)` | Commit |
/// | [`QueryCommitted`](Self::QueryCommitted) | the query's own settle | Commit |
/// | [`QueryCleared`](Self::QueryCleared) | `Escape` stage two | Commit |
/// | [`Dismissed`](Self::Dismissed) | `on_dismiss()` | Dismiss |
/// | [`OpenChanged`](Self::OpenChanged) | the flyout's visibility | Toggle |
/// | [`FocusChanged`](Self::FocusChanged) | focus edges | Focus |
#[derive(Clone, Debug)]
pub enum AutoSuggestEmission<Id> {
    /// The query text changed. Fires per coalesced edit, not per
    /// keystroke — the newest value wins.
    QueryChanged(Live<String>),
    /// The debounce window closed on `query`: ask the provider, and echo
    /// `generation` back through
    /// [`apply_suggestions`](AutoSuggestHost::apply_suggestions).
    QueryRequested {
        /// The token identifying this request.
        generation: u64,
        /// The text the request is about.
        query: String,
    },
    /// The keyboard's highlighted row moved to `index` of the offered
    /// list.
    HighlightChanged(usize),
    /// A candidate was committed, carrying its id, its source and the
    /// label the field settled to.
    Committed(AutoSuggestCommit<Id>),
    /// The query itself settled without a candidate being chosen — `Enter`
    /// on a closed flyout, or focus loss with edits outstanding.
    QueryCommitted(Commit<String>),
    /// `Escape` stage two: the query was cleared. Carries the empty
    /// string it cleared to.
    QueryCleared(Commit<String>),
    /// `Escape` stage one: the flyout was dismissed.
    Dismissed,
    /// The flyout appeared or went away.
    OpenChanged(bool),
    /// The control gained (`true`) or lost (`false`) focus.
    FocusChanged(bool),
}

impl<Id> AutoSuggestEmission<Id> {
    /// The binding tier this emission belongs to.
    #[must_use]
    pub const fn tier(&self) -> &'static str {
        match self {
            Self::QueryChanged(_) | Self::QueryRequested { .. } | Self::HighlightChanged(_) => {
                "Live"
            }
            Self::Committed(_) | Self::QueryCommitted(_) | Self::QueryCleared(_) => "Commit",
            Self::Dismissed => "Dismiss",
            Self::OpenChanged(_) => "Toggle",
            Self::FocusChanged(_) => "Focus",
        }
    }

    /// Whether this emission settles the control rather than reporting
    /// motion toward a settlement.
    #[must_use]
    pub const fn settles(&self) -> bool {
        matches!(
            self,
            Self::Committed(_) | Self::QueryCommitted(_) | Self::QueryCleared(_)
        )
    }

    /// The emission's name, for recording.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::QueryChanged(_) => "QueryChanged",
            Self::QueryRequested { .. } => "QueryRequested",
            Self::HighlightChanged(_) => "HighlightChanged",
            Self::Committed(_) => "Committed",
            Self::QueryCommitted(_) => "QueryCommitted",
            Self::QueryCleared(_) => "QueryCleared",
            Self::Dismissed => "Dismissed",
            Self::OpenChanged(_) => "OpenChanged",
            Self::FocusChanged(_) => "FocusChanged",
        }
    }
}

impl<Id: PartialEq> PartialEq for AutoSuggestEmission<Id> {
    /// Compares the payload each arm carries.
    ///
    /// Hand-written because [`Live`] and [`Commit`] are linear wrappers
    /// that deliberately do not implement `PartialEq`: the same string
    /// arriving as a live tick and as a settlement are different events,
    /// and structural comparison at the wrapper level would blur them.
    /// The arms are matched first, so no arm can equal a different one.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::QueryChanged(a), Self::QueryChanged(b)) => a.value() == b.value(),
            (
                Self::QueryRequested {
                    generation: ga,
                    query: qa,
                },
                Self::QueryRequested {
                    generation: gb,
                    query: qb,
                },
            ) => ga == gb && qa == qb,
            (Self::HighlightChanged(a), Self::HighlightChanged(b)) => a == b,
            (Self::Committed(a), Self::Committed(b)) => a == b,
            (Self::QueryCommitted(a), Self::QueryCommitted(b)) => a.value() == b.value(),
            (Self::QueryCleared(a), Self::QueryCleared(b)) => a.value() == b.value(),
            (Self::Dismissed, Self::Dismissed) => true,
            (Self::OpenChanged(a), Self::OpenChanged(b)) => a == b,
            (Self::FocusChanged(a), Self::FocusChanged(b)) => a == b,
            _ => false,
        }
    }
}

/// A debounced query waiting for its window to close.
#[derive(Clone, Debug)]
struct Pending {
    /// The text the window was armed with.
    query: String,
    /// The instant of the keystroke that armed it.
    armed_at: TimeMs,
}

/// A request handed to the provider and not yet answered.
#[derive(Clone, Debug)]
struct InFlight {
    /// The generation the host must echo back.
    generation: u64,
    /// The text the request carries.
    query: String,
}

/// The `AutoSuggestBox` control machine.
///
/// Owns the editing session, the flyout cascade, the debounce timer, the
/// request generation and the dismissal record. The
/// [`AutoSuggestModel`] stays the application's; the host writes the
/// three projected fields through it, as the
/// [model's docs](super::auto_suggest_model#ownership-is-split-by-field-not-by-struct)
/// set out.
#[derive(Debug)]
pub struct AutoSuggestHost<Id> {
    /// The control's focus identity — the thing the flyout's identity is
    /// derived from, and never the query.
    control: FocusId,
    /// The stable card identity, minted once from
    /// [`control`](Self::control).
    menu_id: MenuId,
    /// The debounce window in force.
    debounce_ms: u64,
    /// The completion flyout's engine.
    cascade: MenuCascade,
    /// The field's editing session, live coalescing and focus edges.
    controller: TextInputController,
    /// A query awaiting its window.
    pending: Option<Pending>,
    /// A request the provider has not answered.
    in_flight: Option<InFlight>,
    /// Monotonic request counter.
    generation: u64,
    /// The query `Escape` dismissed. The flyout stays shut for exactly
    /// this text.
    dismissed_query: Option<String>,
    /// Responses refused because their generation was not the one in
    /// flight.
    stale_dropped: usize,
    /// Provider requests actually dispatched — the count that makes
    /// "one pass per burst" observable rather than asserted.
    passes: usize,
    /// The candidate id type this host is bound to.
    ///
    /// The host never stores a candidate — the model does — but the
    /// generic is what makes `AutoSuggestHost<DocId>` refuse an
    /// `AutoSuggestModel<UserId>` at the call site instead of quietly
    /// producing commits of the wrong type. The marker is what turns that
    /// into a compile error rather than a surprise in the host's message
    /// handler.
    marker: std::marker::PhantomData<Id>,
}

impl<Id: Clone + fmt::Display + PartialEq> AutoSuggestHost<Id> {
    /// A host for the control focusable as `control`, at the default
    /// [`DEBOUNCE_MS`] window.
    #[must_use]
    pub fn new(control: FocusId) -> Self {
        Self::with_debounce(control, DEBOUNCE_MS)
    }

    /// A host with an explicit debounce window, for a host that knows its
    /// provider's latency and wants a different trade.
    #[must_use]
    pub fn with_debounce(control: FocusId, debounce_ms: u64) -> Self {
        Self {
            menu_id: menu_id_for(&control),
            control,
            debounce_ms,
            cascade: MenuCascade::new(),
            controller: TextInputController::new(),
            pending: None,
            in_flight: None,
            generation: 0,
            dismissed_query: None,
            stale_dropped: 0,
            passes: 0,
            marker: std::marker::PhantomData,
        }
    }

    // -- what the machine looks like -----------------------------------------

    /// The control's focus identity.
    #[must_use]
    pub const fn control(&self) -> &FocusId {
        &self.control
    }

    /// The flyout's stable card identity.
    #[must_use]
    pub const fn menu_id(&self) -> &MenuId {
        &self.menu_id
    }

    /// The debounce window in force, in milliseconds.
    #[must_use]
    pub const fn debounce_ms(&self) -> u64 {
        self.debounce_ms
    }

    /// Whether the completion flyout is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.cascade.is_open()
    }

    /// Whether a provider request is outstanding.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Whether a keystroke is waiting out its debounce window.
    #[must_use]
    pub fn is_debouncing(&self) -> bool {
        self.pending.is_some()
    }

    /// The row the keyboard is on, if any.
    #[must_use]
    pub fn highlighted(&self) -> Option<usize> {
        self.cascade.focused_row()
    }

    /// The next request's generation number.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// The text of the request the provider has not answered.
    #[must_use]
    pub fn in_flight_query(&self) -> Option<&str> {
        self.in_flight.as_ref().map(|flight| flight.query.as_str())
    }

    /// The query `Escape` dismissed, if one is still being held.
    #[must_use]
    pub fn dismissed_query(&self) -> Option<&str> {
        self.dismissed_query.as_deref()
    }

    /// How many responses arrived for a generation that was no longer in
    /// flight.
    #[must_use]
    pub const fn stale_dropped(&self) -> usize {
        self.stale_dropped
    }

    /// How many provider requests this host has dispatched.
    #[must_use]
    pub const fn provider_passes(&self) -> usize {
        self.passes
    }

    /// Whether `query` is the text the user dismissed.
    #[must_use]
    pub fn is_dismissed(&self, query: &str) -> bool {
        self.dismissed_query.as_deref() == Some(query)
    }

    // // -- the query ---------------------------------------------------------

    /// A keystroke arrived in the field.
    ///
    /// Records the edit on the editing session, mirrors it onto the
    /// model's `query`, and re-arms the debounce window at `now`. The
    /// window is *restarted* rather than extended: the request that
    /// eventually fires must carry the text the user stopped on, not the
    /// text they were on when the window first opened.
    ///
    /// Reports nothing when the text did not change, so a paste of what
    /// the field already held is not an event.
    pub fn note_edit(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        value: String,
        now: TimeMs,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if model.disabled || !self.controller.note_edit(value) {
            return Vec::new();
        }
        let query = self.controller.value().to_string();
        model.query = query.clone();
        self.pending = Some(Pending {
            query,
            armed_at: now,
        });
        vec![AutoSuggestEmission::QueryChanged(Live::new(
            model.query.clone(),
        ))]
    }

    /// Advance the debounce timer by one frame.
    ///
    /// Call once per frame with the frame's instant. When a pending query
    /// has been idle for the whole window, dispatch exactly one request
    /// and put the model in its busy state. A window that has not closed
    /// reports nothing, which is the state the user is watching: the
    /// text is final but the list has not arrived yet.
    pub fn tick(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        now: TimeMs,
    ) -> Vec<AutoSuggestEmission<Id>> {
        let Some(pending) = self.pending.clone() else {
            return Vec::new();
        };
        if model.disabled {
            // A control that cannot be used has no business holding a
            // request open against a query nobody can see.
            self.pending = None;
            return Vec::new();
        }
        if now.elapsed_since(pending.armed_at) < self.debounce_ms {
            return Vec::new();
        }
        self.pending = None;
        self.generation += 1;
        let generation = self.generation;
        self.in_flight = Some(InFlight {
            generation,
            query: pending.query.clone(),
        });
        model.busy = true;
        self.passes += 1;
        vec![AutoSuggestEmission::QueryRequested {
            generation,
            query: pending.query,
        }]
    }

    /// The provider answered.
    ///
    /// The `generation` is checked against the request still in flight
    /// before anything is applied. A response for any other generation —
    /// one superseded by later typing, one whose query has since been
    /// cleared, or one arriving after the field lost focus — is dropped
    /// and counted, never applied. This is the whole out-of-order defence,
    /// and it is why the host cannot simply assign `model.suggestions`.
    ///
    /// A fresh answer replaces the list, clears the busy flag, and then
    /// decides what the flyout does: it opens if the query was never
    /// dismissed and the list has a row the keyboard can enter, stays
    /// shut if the user dismissed this exact text, and re-resolves
    /// through the engine if it was already open.
    pub fn apply_suggestions(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        generation: u64,
        suggestions: Vec<SuggestionItem<Id>>,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if !self
            .in_flight
            .as_ref()
            .is_some_and(|f| f.generation == generation)
        {
            self.stale_dropped += 1;
            return Vec::new();
        }
        self.in_flight = None;
        model.busy = false;
        model.suggestions = suggestions;

        if model.disabled || self.is_dismissed(&model.query) {
            return self.close_flyout(model, focus);
        }
        if self.cascade.is_open() {
            return self.resync_flyout(model, focus);
        }
        self.open_flyout(model, focus)
    }

    // // -- the keyboard ------------------------------------------------------

    /// Drive one control-level key.
    ///
    /// A disabled control answers nothing. Everything else is routed by
    /// whether the flyout is open, because that is the only thing that
    /// changes what a key means: `Down` on an open flyout is "next row",
    /// `Down` on a closed one is "show me what you have", and `Enter` on
    /// a closed one is "settle the text I typed".
    pub fn key(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        key: AutoSuggestKey,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if model.disabled {
            return Vec::new();
        }
        match key {
            AutoSuggestKey::Escape => self.escape(model, focus),
            AutoSuggestKey::Up | AutoSuggestKey::Down if self.cascade.is_open() => {
                let delta = if key == AutoSuggestKey::Down { 1 } else { -1 };
                self.rove(delta, model, focus)
            }
            AutoSuggestKey::Up | AutoSuggestKey::Down => {
                if self.is_dismissed(&model.query) {
                    return Vec::new();
                }
                self.open_flyout(model, focus)
            }
            AutoSuggestKey::Enter if self.cascade.is_open() => {
                let index = self.cascade.focused_row();
                match index {
                    Some(index) => self.commit_at(model, index, CommitSource::Enter, focus),
                    None => self.close_flyout(model, focus),
                }
            }
            AutoSuggestKey::Enter => self.settle_query(model),
            AutoSuggestKey::Tab if self.cascade.is_open() => {
                let index = self.cascade.focused_row();
                match index {
                    Some(index) => self.commit_at(model, index, CommitSource::Tab, focus),
                    None => self.close_flyout(model, focus),
                }
            }
            AutoSuggestKey::Tab => Vec::new(),
        }
    }

    /// `Escape` — the two-stage unwind the spec's `on_dismiss` names.
    ///
    /// See the [module docs](self#escape-is-two-stage-and-the-second-stage-clears-rather-than-reverts)
    /// for the transition table and for why stage two clears rather than
    /// reverts.
    pub fn escape(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if model.disabled {
            return Vec::new();
        }
        if self.cascade.is_open() {
            let closed = self.close_flyout(model, focus);
            if closed.is_empty() {
                return closed;
            }
            self.dismissed_query = Some(model.query.clone());
            let mut out = vec![AutoSuggestEmission::Dismissed];
            out.extend(closed);
            return out;
        }
        if model.query.is_empty() {
            return Vec::new();
        }
        let Some(TextInputEmission::Cleared(commit)) = self.controller.clear() else {
            return Vec::new();
        };
        model.query = self.controller.value().to_string();
        self.pending = None;
        self.in_flight = None;
        self.dismissed_query = None;
        model.busy = false;
        model.suggestions.clear();
        model.highlighted_index = None;
        vec![AutoSuggestEmission::QueryCleared(commit)]
    }

    // // -- the pointer -------------------------------------------------------

    /// A pointer press was released over the candidate at `index`
    /// (`CommitSource::Click`).
    ///
    /// The index is the row the pointer hit, and it is resolved to the
    /// candidate through the open card's row key rather than trusted
    /// directly, so a list rebuilt between the frame that painted the row
    /// and the frame that received the click cannot commit the wrong
    /// candidate. A click on a row that no longer maps to a candidate
    /// commits nothing.
    pub fn click(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        index: usize,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if model.disabled || !self.cascade.is_open() {
            return Vec::new();
        }
        self.commit_at(model, index, CommitSource::Click, focus)
    }

    // // -- focus -------------------------------------------------------------

    /// Record a focus edge.
    ///
    /// Losing focus closes the flyout — a completion list belonging to a
    /// field the user has left must not stay on screen — and settles any
    /// edits outstanding through the query's own commit tier. The
    /// dismissal record is dropped rather than kept: the user left, they
    /// did not dismiss, and carrying a dismissal past the field's focus
    /// would suppress the next time they come back to the same text.
    ///
    /// Gaining focus reports the edge and opens nothing. There is nothing
    /// to complete until something is typed.
    pub fn note_focus(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        focused: bool,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        let mut out = Vec::new();
        if !focused {
            out.extend(self.close_flyout(model, focus));
            self.pending = None;
            self.in_flight = None;
            self.dismissed_query = None;
            model.busy = false;
        }
        for emission in self.controller.note_focus(focused) {
            match emission {
                TextInputEmission::Committed(commit) => {
                    out.push(AutoSuggestEmission::QueryCommitted(commit));
                }
                TextInputEmission::FocusChanged(focused) => {
                    out.push(AutoSuggestEmission::FocusChanged(focused));
                }
                _ => {}
            }
        }
        out
    }

    // // -- the frame ---------------------------------------------------------

    /// Re-resolve the open card against the model, once per frame.
    ///
    /// The card is rebuilt from the model's current suggestions under the
    /// **host's** stable identity, so a frame that arrives before the
    /// application has round-tripped the last response still shows the
    /// list the user was last shown rather than an empty one. A model
    /// that has become unopenable — emptied, disabled, or dismissed at
    /// exactly this query — closes the card, and the close is reported
    /// rather than swallowed.
    ///
    /// This method never *opens* a flyout. Opening is a decision with a
    /// reason behind it; a per-frame maintenance pass inventing one would
    /// be the flyout appearing on its own.
    pub fn sync(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        self.controller.sync_value(&model.query);
        if !self.cascade.is_open() {
            return Vec::new();
        }
        if model.disabled || self.is_dismissed(&model.query) {
            return self.close_flyout(model, focus);
        }
        self.resync_flyout(model, focus)
    }

    /// Drop the machine's timing, dismissal and editing state without
    /// emitting.
    ///
    /// A live flyout is deliberately *not* closed here, for the reason
    /// [`ComboHost::reset`](super::combo_host::ComboHost::reset) gives:
    /// closing a flyout is a visible event the host has to be able to
    /// report, and silently tearing one down from a reset would drop that
    /// report.
    pub fn reset(&mut self) {
        self.controller.reset();
        self.pending = None;
        self.in_flight = None;
        self.dismissed_query = None;
    }

    // -- internals ---------------------------------------------------------

    /// Open the flyout over the model's current suggestions.
    ///
    /// Refused silently when there is nothing the keyboard can enter,
    /// which is the no-match policy: the engine's
    /// [`NoFocusableRows`](crate::surface::menu::cascade::IgnoredReason::NoFocusableRows)
    /// guard is the reason, and the field keeps the focus it already had.
    fn open_flyout(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if self.cascade.is_open() || model.disabled {
            return Vec::new();
        }
        let menu = model.menu_with(self.menu_id.clone());
        if menu.first_navigable().is_none() {
            return Vec::new();
        }
        let outcome = self.cascade.open(&menu, MenuTreeKind::Fixed, focus);
        if outcome.ignored_reason().is_some() {
            return Vec::new();
        }
        model.highlighted_index = self.cascade.focused_row();
        let mut out = vec![AutoSuggestEmission::OpenChanged(true)];
        if let Some(row) = self.cascade.focused_row() {
            out.push(AutoSuggestEmission::HighlightChanged(row));
        }
        out
    }

    /// Close the open card, dropping the highlight with it.
    fn close_flyout(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if !self.cascade.is_open() {
            return Vec::new();
        }
        let _ = self.cascade.close_tree(focus);
        model.highlighted_index = None;
        vec![AutoSuggestEmission::OpenChanged(false)]
    }

    /// Re-resolve an already-open card and report what moved.
    fn resync_flyout(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        let before = self.cascade.focused_row();
        let menu = model.menu_with(self.menu_id.clone());
        let _ = self.cascade.sync(&menu, focus);
        let after = self.cascade.focused_row();
        model.highlighted_index = after;
        if !self.cascade.is_open() {
            return vec![AutoSuggestEmission::OpenChanged(false)];
        }
        match after {
            Some(after) if Some(after) != before => {
                vec![AutoSuggestEmission::HighlightChanged(after)]
            }
            _ => Vec::new(),
        }
    }

    /// Move the highlight by `delta`, wrapping at both ends.
    ///
    /// Wrapping comes from the engine's own roving walk, so headings,
    /// separators and disabled rows are skipped by absence rather than
    /// special-cased here — and the input's caret is untouched, because
    /// nothing in this path issues an iced focus operation.
    fn rove(
        &mut self,
        delta: i32,
        model: &mut AutoSuggestModel<Id>,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        let before = self.cascade.focused_row();
        let _ = self.cascade.move_focus(delta, focus);
        let after = self.cascade.focused_row();
        model.highlighted_index = after;
        match after {
            Some(after) if Some(after) != before => {
                vec![AutoSuggestEmission::HighlightChanged(after)]
            }
            _ => Vec::new(),
        }
    }

    /// Commit the candidate at `index` of the offered list.
    ///
    /// The row's key is read from the open card *before* anything settles
    /// and mapped back to the candidate by `Display` form, so the commit
    /// names the candidate the user was pointed at rather than whatever
    /// now sits at that index. The field settles to the candidate's label
    /// and the flyout closes.
    fn commit_at(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        index: usize,
        source: CommitSource,
        focus: &mut FocusState,
    ) -> Vec<AutoSuggestEmission<Id>> {
        let Some(key) = self
            .cascade
            .active_menu()
            .and_then(|menu| menu.row(index))
            .map(|row| row.key().to_owned())
        else {
            return Vec::new();
        };
        let Some(position) = model
            .suggestions
            .iter()
            .position(|item| item.id.to_string() == key)
        else {
            return Vec::new();
        };
        let Some(item) = model.suggestions.get(position) else {
            return Vec::new();
        };
        let commit = AutoSuggestCommit::new(item.id.clone(), source, item.label.clone());
        let label = item.label.clone();

        let mut out = vec![AutoSuggestEmission::Committed(commit)];
        out.extend(self.settle_query_to(model, label));
        out.extend(self.close_flyout(model, focus));
        self.pending = None;
        self.in_flight = None;
        self.dismissed_query = None;
        model.busy = false;
        out
    }

    /// Settle the query as it stands — `Enter` with no candidate chosen.
    fn settle_query(&mut self, model: &mut AutoSuggestModel<Id>) -> Vec<AutoSuggestEmission<Id>> {
        match self.controller.commit() {
            Some(TextInputEmission::Committed(commit)) => {
                model.query = commit.value().clone();
                vec![AutoSuggestEmission::QueryCommitted(commit)]
            }
            _ => Vec::new(),
        }
    }

    /// Settle the query to a picked candidate's label.
    ///
    /// The label is pushed through the editing session rather than
    /// assigned, so the field's own tier machinery sees the change: the
    /// snapshot is taken before the label lands, the live tick is
    /// superseded by the settle rather than delivered after it, and the
    /// session closes rather than being left holding text the user has
    /// already committed.
    fn settle_query_to(
        &mut self,
        model: &mut AutoSuggestModel<Id>,
        label: String,
    ) -> Vec<AutoSuggestEmission<Id>> {
        if self.controller.value() != label {
            self.controller.note_edit(label);
        }
        match self.controller.commit() {
            Some(TextInputEmission::Committed(commit)) => {
                model.query = commit.value().clone();
                vec![AutoSuggestEmission::QueryCommitted(commit)]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "auto_suggest_host_tests.rs"]
mod auto_suggest_host_tests;
