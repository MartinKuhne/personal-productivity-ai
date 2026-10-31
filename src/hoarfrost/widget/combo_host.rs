//! The combobox host — one menu cascade, one snapshot slot, no second
//! flyout (§3.4, `UIF-ATOM-004`).
//!
//! §3.4 describes a control whose popup is a menu. The crate already owns
//! a menu engine, so the entire job of this module is to be *driven* by
//! that engine rather than to reproduce one: it holds exactly one
//! [`MenuCascade`] and one [`GestureSlot`], and every behaviour below is
//! those two existing types being asked to do what they already do.
//!
//! # The snapshot is taken before the flyout opens
//!
//! [`ComboHost::open`] captures [`ComboSnapshot`] — the selection *and*
//! the filter the user was looking at — before asking the cascade to open
//! anything. The order matters twice over. Taken afterwards, the snapshot
//! would be whatever the host last pushed rather than what the user saw;
//! and [`GestureSlot`]'s outermost-wins rule means a re-capture on a
//! nested open is refused anyway, so a late capture would silently govern
//! the revert. The filter is inside the snapshot because reverting only
//! the selection would hand the user back a value that the still-active
//! filter is hiding — a revert that cannot be seen is not a revert.
//!
//! # Focus goes to the control first, so Escape has somewhere to return
//!
//! Same mechanism as
//! [`SplitButtonHost::open_menu`](crate::widget::split_button_host::SplitButtonHost::open_menu):
//! the cascade's
//! `mount` pushes a focus trap recording whatever held focus at push
//! time, and `close_tree` pops it back. Focusing the trigger *before*
//! `open` makes the recorded focus the trigger itself, so Escape lands
//! the caret back on the control rather than in the void where it came
//! from.
//!
//! # A filter that matches nothing cannot open a flyout
//!
//! Not a taste call. The engine refuses any card with no focusable row
//! ([`IgnoredReason::NoFocusableRows`](crate::surface::menu::cascade::IgnoredReason))
//! because a focus trap over an
//! empty scope is what makes the focus model unsound. The combobox
//! inherits that refusal rather than working around it: with no pickable
//! option there is nothing the flyout could show that the keyboard could
//! reach, so `open` emits nothing and the trigger stays shut. The
//! editable variant still reports the keystroke through
//! [`FilterChanged`](ComboEmission::FilterChanged) — the user's text is
//! live whether or not a list appeared under it.
//!
//! # Home, End and Up-open without touching the engine
//!
//! The engine gives `move_focus(delta)` over the active card's navigable
//! roster, wrapping at both ends. That is enough to express the two keys
//! §3.4 wants that a menu does not already have: `Home` walks back by the
//! current roster position, `End` forward by the remainder, and `Up` on
//! a *closed* control opens the flyout and steps back once, which wraps
//! onto the last row. All three are arithmetic against
//! [`Menu::navigable_indices`]; none of them required a new engine
//! primitive, which is the point of reusing one.
//!
//! # The filter's working copy lives here, not in the caller
//!
//! [`ComboBoxModel`] is the application's mirror; while the user is
//! driving, the host's own [`ComboHost::filter`] is the truth the flyout
//! is built from. Without that, a [`sync`](ComboHost::sync) arriving
//! before the application has round-tripped the last keystroke would
//! rebuild the open card from a stale filter and yank the list back a
//! character. The working copy is seeded at open and cleared at settle.
//!
//! # Read-only never mutates
//!
//! A read-only combobox opens, browses and shows its `✓`, but activating
//! a row commits nothing — see the
//! [model's docs](super::combo_model#read-only-is-imported-not-specified).
//! The activation is not merely un-reported: it is not performed, so the
//! engine never flips a checkmark the application did not agree to.
//!
//! Unit tests live in the sibling `combo_host_tests.rs` sidecar.

use std::fmt;

use crate::binding::{Cancel, Commit, GestureSlot, Live};
use crate::focus::{FocusId, FocusState};
use crate::surface::menu::cascade::{CascadeOutcome, MenuCascade, MenuTreeKind};
use crate::surface::menu::model::Menu;
use crate::widget::combo_model::{ComboBoxModel, ComboSnapshot};

/// The control-level key vocabulary of a combobox (§3.4, `UIF-ATOM-004`).
///
/// Seven keys, and deliberately no printable characters. The closed
/// select owns no text buffer, so a printable key is not something it
/// could claim a character from — see the
/// [model's docs](super::combo_model#type-ahead-on-the-closed-select-is-not-implemented--and-cannot-be).
/// The editable variant routes printable characters through
/// [`on_filter_typed`](ComboHost::on_filter_typed), which is a different
/// door, so the two variants can never contend for one keystroke.
///
/// There is also no `Tab`, for the same reason [`MenuKey`](crate::surface::menu::cascade::MenuKey)
/// has none: `Tab` is confined by the focus trap and must not be
/// reinterpreted by the control sitting inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComboKey {
    /// Move to the next row, or open the flyout from a closed control.
    Down,
    /// Move to the previous row, or open onto the *last* row.
    Up,
    /// Activate the focused row.
    Enter,
    /// Activate the focused row — the same action `Enter` performs.
    Space,
    /// Jump to the first navigable row.
    Home,
    /// Jump to the last navigable row.
    End,
    /// Abandon the gesture and restore the pre-open snapshot.
    Escape,
}

/// One piece of output the host produced for the application, already
/// sorted into the tier §3.4's callback belongs to.
///
/// The tiering is the reason this is a sum type rather than four optional
/// callbacks: a host wiring all of these is forced to write an arm for
/// each and cannot silently drop the one it did not demo.
#[derive(Clone)]
pub enum ComboEmission<Id> {
    /// The flyout opened (`true`) or closed (`false`) — the toggle
    /// channel a §3.4 control reports.
    OpenChanged(bool),
    /// A filter keystroke — `on_filter_change(String)`, the `Live` tier.
    ///
    /// Live rather than committed because the user is mid-word: the
    /// application narrows a list on each character and must not be told
    /// to *decide* anything until the gesture settles.
    FilterChanged(Live<String>),
    /// A row was picked — `on_select(Id)`, the `Commit` tier.
    Committed(Commit<Id>),
    /// The gesture was abandoned — `Escape`, the `Cancel` tier, carrying
    /// what the application should restore.
    Cancelled(Cancel<ComboSnapshot<Id>>),
}

impl<Id> ComboEmission<Id> {
    /// Whether this emission changes the control's value rather than only
    /// reporting where it stands.
    #[must_use]
    pub fn is_value_change(&self) -> bool {
        matches!(self, Self::Committed(_) | Self::Cancelled(_))
    }
}

impl<Id: PartialEq> PartialEq for ComboEmission<Id> {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] and [`Cancel`] are linear wrappers
    /// that deliberately do not implement `PartialEq`. Matching the tiers
    /// first means a commit can never compare equal to a cancel.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::OpenChanged(a), Self::OpenChanged(b)) => a == b,
            (Self::FilterChanged(a), Self::FilterChanged(b)) => a.value() == b.value(),
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            _ => false,
        }
    }
}

impl<Id: fmt::Debug> fmt::Debug for ComboEmission<Id> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenChanged(open) => f.debug_tuple("OpenChanged").field(open).finish(),
            Self::FilterChanged(live) => {
                f.debug_tuple("FilterChanged").field(live.value()).finish()
            }
            Self::Committed(commit) => f.debug_tuple("Committed").field(commit.value()).finish(),
            Self::Cancelled(cancel) => f.debug_tuple("Cancelled").field(cancel.restored()).finish(),
        }
    }
}

/// Everything one host input did.
///
/// The engine's own report travels alongside the emissions rather than
/// being flattened into them: the caller still needs `opened` and
/// `closed` to mount and tear down the flyout surface, and re-deriving
/// those from an [`OpenChanged`](ComboEmission::OpenChanged) bool would
/// lose which card moved.
#[derive(Clone, Debug)]
pub struct ComboOutcome<Id> {
    /// The engine's report for the cascade side of the input.
    pub cascade: CascadeOutcome,
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<ComboEmission<Id>>,
}

impl<Id> ComboOutcome<Id> {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            cascade: CascadeOutcome::noop(),
            emissions: Vec::new(),
        }
    }

    /// An outcome carrying `emissions` and no cascade movement.
    #[must_use]
    pub fn emissions(emissions: Vec<ComboEmission<Id>>) -> Self {
        Self {
            cascade: CascadeOutcome::noop(),
            emissions,
        }
    }

    /// Whether this outcome changed nothing at all.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.emissions.is_empty()
            && self.cascade.opened().is_empty()
            && self.cascade.closed().is_empty()
            && self.cascade.activation().is_none()
    }
}

/// The per-control memory a combobox needs: the flyout's cascade, the
/// open gesture's snapshot, the live filter, and the pointer flags.
#[derive(Debug)]
pub struct ComboHost<Id> {
    /// The one menu engine this control is allowed to have.
    cascade: MenuCascade,
    /// The pre-open snapshot, held for the cancel tier.
    gesture: GestureSlot<ComboSnapshot<Id>>,
    /// The control's focus identity — what the trap is told to return to.
    control: FocusId,
    /// The host's working copy of the filter. See the module docs.
    filter: Option<String>,
    /// Whether the host has taken ownership of [`filter`](Self::filter).
    ///
    /// While this is `false` the model's own
    /// [`filter_query`](ComboBoxModel::filter_query) is what the flyout
    /// is built from; the first keystroke takes ownership and the model
    /// stops deciding. Without the flag, `open` would have to seed the
    /// working copy every time and would silently discard a filter the
    /// user had just typed but whose round-trip had not landed.
    filter_owned: bool,
    /// Whether the pointer is over the trigger.
    hovered: bool,
    /// Whether a press on the trigger is currently down.
    pressed: bool,
}

impl<Id: Clone + fmt::Display + PartialEq> ComboHost<Id> {
    /// A host for the control focusable as `control`.
    #[must_use]
    pub fn new(control: FocusId) -> Self {
        Self {
            cascade: MenuCascade::new(),
            gesture: GestureSlot::new(),
            control,
            filter: None,
            filter_owned: false,
            hovered: false,
            pressed: false,
        }
    }

    /// Whether the flyout is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.cascade.is_open()
    }

    /// The host's working filter — the one the open flyout is built from.
    #[must_use]
    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    /// The filter the flyout is actually built from.
    ///
    /// The host's own working copy while it owns one, and the model's
    /// `filter_query` until it does. See
    /// [`filter_owned`](Self::filter_owned).
    fn effective_filter<'a>(&'a self, model: &'a ComboBoxModel<Id>) -> Option<&'a str> {
        if self.filter_owned {
            self.filter.as_deref()
        } else {
            model.filter_query()
        }
    }

    /// Release the working filter and hand ownership back to the model.
    ///
    /// Called at every settle point, so the next gesture starts from
    /// whatever the application believes the filter is.
    fn release_filter(&mut self) {
        self.filter = None;
        self.filter_owned = false;
    }

    /// Whether a pre-open snapshot is being held.
    #[must_use]
    pub fn gesture_open(&self) -> bool {
        self.gesture.is_open()
    }

    /// Whether the pointer is over the trigger.
    #[must_use]
    pub const fn hovered(&self) -> bool {
        self.hovered
    }

    /// Whether a trigger press is currently down.
    #[must_use]
    pub const fn pressed(&self) -> bool {
        self.pressed
    }

    /// The cascade, read-only.
    #[must_use]
    pub const fn cascade(&self) -> &MenuCascade {
        &self.cascade
    }

    /// The control's focus identity.
    #[must_use]
    pub const fn control(&self) -> &FocusId {
        &self.control
    }

    // -- opening and closing -------------------------------------------------

    /// Open the flyout (`UIF-ATOM-004`).
    ///
    /// Three guards run before the engine is asked for anything:
    ///
    /// * a disabled control opens nothing;
    /// * a list with nothing pickable cannot be shown at all, for the
    ///   reason given in the [module docs](self#a-filter-that-matches-nothing-cannot-open-a-flyout);
    /// * the snapshot is captured first, so the revert has something to
    ///   revert to that the user actually saw.
    ///
    /// `Up` and `Down` both arrive here from a closed control; the caller
    /// decides which row to land on afterwards via
    /// [`key`](Self::key), which is where the wrap-to-last lives.
    pub fn open(&mut self, model: &ComboBoxModel<Id>, focus: &mut FocusState) -> ComboOutcome<Id> {
        if model.disabled_flag() {
            return ComboOutcome::noop();
        }
        let menu = model.menu_with(self.effective_filter(model));
        if menu.first_navigable().is_none() {
            return ComboOutcome::noop();
        }
        let _ = self.gesture.begin(&model.snapshot());
        if focus.current() != Some(&self.control) {
            focus.focus(&self.control);
        }
        let cascade = self.cascade.open(&menu, MenuTreeKind::Fixed, focus);
        if cascade.ignored_reason().is_some() {
            self.gesture.abort();
            return ComboOutcome {
                cascade,
                emissions: Vec::new(),
            };
        }
        ComboOutcome {
            cascade,
            emissions: vec![ComboEmission::OpenChanged(true)],
        }
    }

    /// Close the flyout without settling the gesture either way.
    ///
    /// Closing by clicking the trigger again, or by clicking away, is not
    /// a cancellation: nothing changed, so nothing is owed back. The
    /// snapshot is dropped rather than replayed, and the working filter
    /// goes with it — the application keeps whatever it last heard
    /// through the live channel.
    pub fn close(&mut self, focus: &mut FocusState) -> ComboOutcome<Id> {
        if !self.cascade.is_open() {
            return ComboOutcome::noop();
        }
        let cascade = self.cascade.close_tree(focus);
        self.gesture.abort();
        self.release_filter();
        ComboOutcome {
            cascade,
            emissions: vec![ComboEmission::OpenChanged(false)],
        }
    }

    /// Toggle the flyout from the trigger.
    pub fn toggle(
        &mut self,
        model: &ComboBoxModel<Id>,
        focus: &mut FocusState,
    ) -> ComboOutcome<Id> {
        if self.cascade.is_open() {
            self.close(focus)
        } else {
            self.open(model, focus)
        }
    }

    // -- the filter ----------------------------------------------------------

    /// A printable character arrived in the editable variant's buffer
    /// (`on_filter_change`, the `Live` tier).
    ///
    /// The keystroke is reported whether or not a flyout can be shown
    /// for it: the user's text is live the moment they type it, and a
    /// filter that silently swallowed characters because the list went
    /// empty would be a filter that lies.
    ///
    /// A non-editable control reports nothing, because a non-editable
    /// control has no buffer for the character to have come from — see
    /// the [module docs](self#the-filter-s-working-copy-lives-here-not-in-the-caller).
    pub fn on_filter_typed(
        &mut self,
        model: &ComboBoxModel<Id>,
        query: String,
        focus: &mut FocusState,
    ) -> ComboOutcome<Id> {
        if !model.is_editable() || model.disabled_flag() || model.read_only_flag() {
            return ComboOutcome::noop();
        }
        let next = if query.is_empty() { None } else { Some(query) };
        self.filter = next.clone();
        self.filter_owned = true;
        let mut emissions = Vec::new();
        if let Some(query) = next {
            emissions.push(ComboEmission::FilterChanged(Live::new(query)));
        }

        if self.cascade.is_open() {
            let menu = model.menu_with(self.filter.as_deref());
            let cascade = self.cascade.sync(&menu, focus);
            if !self.cascade.is_open() {
                self.release_filter();
                emissions.push(ComboEmission::OpenChanged(false));
            }
            return ComboOutcome { cascade, emissions };
        }
        ComboOutcome::emissions(emissions)
    }

    // -- keyboard ------------------------------------------------------------

    /// Drive one control-level key (`UIF-ATOM-004`).
    ///
    /// A disabled control answers nothing. Everything else is routed by
    /// whether the flyout is already open, because that is the only
    /// thing that changes what a key means: `Down` on a closed control
    /// is "open", `Down` on an open one is "next row".
    pub fn key(
        &mut self,
        key: ComboKey,
        model: &ComboBoxModel<Id>,
        focus: &mut FocusState,
    ) -> ComboOutcome<Id> {
        if model.disabled_flag() {
            return ComboOutcome::noop();
        }
        match key {
            ComboKey::Escape => self.escape(focus),
            ComboKey::Down if self.cascade.is_open() => {
                let cascade = self.cascade.move_focus(1, focus);
                ComboOutcome {
                    cascade,
                    emissions: Vec::new(),
                }
            }
            ComboKey::Up if self.cascade.is_open() => {
                let cascade = self.cascade.move_focus(-1, focus);
                ComboOutcome {
                    cascade,
                    emissions: Vec::new(),
                }
            }
            ComboKey::Down => self.open(model, focus),
            ComboKey::Up => self.open_then_last(model, focus),
            ComboKey::Enter | ComboKey::Space if self.cascade.is_open() => {
                self.activate_focused(model, focus)
            }
            ComboKey::Enter | ComboKey::Space => self.open(model, focus),
            ComboKey::Home | ComboKey::End if self.cascade.is_open() => self.jump(key, focus),
            ComboKey::Home | ComboKey::End => ComboOutcome::noop(),
        }
    }

    /// `Escape` — abandon the gesture and hand back the pre-open snapshot.
    ///
    /// With no gesture held there is nothing to abandon, so a closed
    /// control's `Escape` emits nothing rather than cancelling a gesture
    /// that never started.
    pub fn escape(&mut self, focus: &mut FocusState) -> ComboOutcome<Id> {
        let was_open = self.cascade.is_open();
        let cascade = if was_open {
            self.cascade.close_tree(focus)
        } else {
            CascadeOutcome::noop()
        };
        let mut emissions = Vec::new();
        if let Some(cancel) = self.gesture.cancel() {
            emissions.push(ComboEmission::Cancelled(cancel));
        }
        if was_open && !self.cascade.is_open() {
            emissions.push(ComboEmission::OpenChanged(false));
        }
        self.release_filter();
        ComboOutcome { cascade, emissions }
    }

    /// Open the flyout and land on the **last** row (`Up` from closed).
    ///
    /// The engine focuses the first navigable row on mount, so reaching
    /// the last one is one wrapping step afterwards. The report that
    /// comes back is the *open's*, which is the sound one to return: the
    /// card list the caller mounts comes from the open, and
    /// [`CascadeOutcome::focus_task`] reads the live [`FocusState`] when
    /// it projects, so the wrap is already reflected in what the router
    /// is told. If the wrap step is refused the open is still the open —
    /// the flyout is showing, just at the top — so no rollback is
    /// attempted.
    fn open_then_last(
        &mut self,
        model: &ComboBoxModel<Id>,
        focus: &mut FocusState,
    ) -> ComboOutcome<Id> {
        let outcome = self.open(model, focus);
        if self.cascade.is_open() {
            let _ = self.cascade.move_focus(-1, focus);
        }
        outcome
    }

    /// `Home` / `End` — jump to an end of the active card's roster.
    ///
    /// Expressed as a wrapping walk whose length is computed from the
    /// roster position, so the engine needs no new primitive and
    /// headings, separators and disabled rows are skipped by absence
    /// exactly as they are for `Up`/`Down`.
    fn jump(&mut self, key: ComboKey, focus: &mut FocusState) -> ComboOutcome<Id> {
        let Some(roster) = self.cascade.active_menu().map(Menu::navigable_indices) else {
            return ComboOutcome::noop();
        };
        let Some(current) = self.cascade.focused_row() else {
            return ComboOutcome::noop();
        };
        let Some(position) = roster.iter().position(|row| *row == current) else {
            return ComboOutcome::noop();
        };
        let delta = match key {
            ComboKey::Home => -(position as i32),
            _ => (roster.len() - 1 - position) as i32,
        };
        if delta == 0 {
            return ComboOutcome::noop();
        }
        let cascade = self.cascade.move_focus(delta, focus);
        ComboOutcome {
            cascade,
            emissions: Vec::new(),
        }
    }

    /// Activate the row the keyboard is on (`UIF-ATOM-004`).
    ///
    /// The row's key is read from the card *before* the engine is asked
    /// to activate, because a row that closes the tree takes its own
    /// menu away with it and the key would no longer be reachable
    /// afterwards. Mapping key → application id rather than row index →
    /// id is what keeps the mapping honest across a list rebuild.
    fn activate_focused(
        &mut self,
        model: &ComboBoxModel<Id>,
        focus: &mut FocusState,
    ) -> ComboOutcome<Id> {
        if model.read_only_flag() {
            // Read-only never mutates: the activation is not performed,
            // so the engine never flips a checkmark the application did
            // not agree to. The flyout still closes.
            return self.close(focus);
        }
        let key = self.focused_row_key();
        let was_open = self.cascade.is_open();
        let cascade = self.cascade.activate_focused(focus);
        let mut emissions = Vec::new();
        if let Some(activation) = cascade.activation()
            && !activation.is_absorbed()
            && let Some(id) = key.as_deref().and_then(|key| model.id_by_key(key).cloned())
        {
            emissions.push(ComboEmission::Committed(Commit::new(id)));
        }
        // The gesture is over either way. A commit settles it forward; a
        // refused activation must not leave a snapshot held against a
        // gesture that has ended.
        self.gesture.abort();
        self.release_filter();
        if was_open && !self.cascade.is_open() {
            emissions.push(ComboEmission::OpenChanged(false));
        }
        ComboOutcome { cascade, emissions }
    }

    /// The `Display` key of the row the cascade currently focuses.
    fn focused_row_key(&self) -> Option<String> {
        let row = self.cascade.focused_row()?;
        self.cascade
            .active_menu()
            .and_then(|menu| menu.row(row))
            .map(|row| row.key().to_owned())
    }

    // -- pointer -------------------------------------------------------------

    /// The pointer moved onto (`true`) or off (`false`) the trigger.
    ///
    /// Inert by design: hover paints, it does not emit.
    pub fn hover(&mut self, over: bool) {
        self.hovered = over;
    }

    /// A press started on the trigger.
    ///
    /// Opens nothing. Under §3.0.3.3 a click is press-and-release inside
    /// the same target, so a press that slides off must be able to abort
    /// the flyout — which it can only do if the press itself did not
    /// already open it.
    pub fn press(&mut self, model: &ComboBoxModel<Id>) {
        if model.disabled_flag() {
            return;
        }
        self.pressed = true;
    }

    /// A press ended, `inside` saying whether the pointer is still over
    /// the trigger.
    ///
    /// Released outside, the gesture is dropped and nothing toggles.
    pub fn release(
        &mut self,
        inside: bool,
        model: &ComboBoxModel<Id>,
        focus: &mut FocusState,
    ) -> ComboOutcome<Id> {
        if !self.pressed {
            return ComboOutcome::noop();
        }
        self.pressed = false;
        if !inside || model.disabled_flag() {
            return ComboOutcome::noop();
        }
        self.toggle(model, focus)
    }

    /// Drop pointer state and any held snapshot without emitting, leaving
    /// the flyout alone.
    ///
    /// For teardown. A live cascade is deliberately *not* closed here —
    /// closing a flyout is a visible event the host has to be able to
    /// report, and silently tearing one down from a reset would drop that
    /// report.
    pub fn reset(&mut self) {
        self.hovered = false;
        self.pressed = false;
        self.gesture.abort();
        self.release_filter();
    }
    /// Re-resolve the open card against the model (`UIF-ATOM-004`).
    ///
    /// Call once per frame, after the application has applied whatever
    /// the last emission asked for. The card is rebuilt from
    /// `model.menu_with(self.filter)` — the model's options under the
    /// **host's** working filter, not the model's own — so a frame that
    /// arrives before the application has round-tripped the last
    /// keystroke still shows the list the user typed.
    ///
    /// A model whose filtered list has gone unopenable closes the card,
    /// which is reported as the flyout closing rather than swallowed.
    pub fn sync(&mut self, model: &ComboBoxModel<Id>, focus: &mut FocusState) -> ComboOutcome<Id> {
        if !self.cascade.is_open() {
            return ComboOutcome::noop();
        }
        let menu = model.menu_with(self.effective_filter(model));
        let cascade = self.cascade.sync(&menu, focus);
        let mut emissions = Vec::new();
        if !self.cascade.is_open() {
            self.release_filter();
            emissions.push(ComboEmission::OpenChanged(false));
        }
        ComboOutcome { cascade, emissions }
    }
}

#[cfg(test)]
#[path = "combo_host_tests.rs"]
mod combo_host_tests;
