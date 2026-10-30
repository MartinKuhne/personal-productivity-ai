//! The toggle-family data models and their transitions — `CheckboxModel`,
//! `RadioGroupModel` and `SwitchModel` (`UIF-ATOM-003`, §3.3).
//!
//! §3.3 specifies these three models and their callbacks in one block.
//! This module is that block as code: the fields the spec names, the
//! transitions its prose describes, and nothing else.
//!
//! # Every transition is a pure function
//!
//! §3.0.4.1 makes the value **application-owned**: the widget holds a
//! [`Controlled`] borrow it cannot write through, and the only path back
//! to the app is a tier emission handed *out*. So none of the methods
//! here mutate `self`. `CheckboxModel::toggle` returns
//! `Option<Commit<CheckState>>` — the new state the app is being asked
//! to adopt — rather than setting a field and returning nothing.
//!
//! That is not ceremony. A model that mutated itself would let the
//! widget layer change a value it does not own simply by calling a
//! method, which is the exact failure §3.0.4.1 was written to prevent;
//! and a pure transition is the only thing that can be exhaustively
//! tested, which is what makes the tri-state determinism below a checked
//! fact rather than a claim.
//!
//! # The tri-state transition is deterministic
//!
//! §3.3 asks that "clicking an `Indeterminate` parent checkbox
//! transitions deterministically to `Checked`, or `Unchecked` if all
//! enabled children are already checked". The rule needs to know
//! something the checkbox's own state does not say — what is under it —
//! so [`ChildSummary`] carries it in as an argument rather than the
//! model reaching for it.
//!
//! | From | Children | To |
//! |---|---|---|
//! | `Unchecked` | any | `Checked` |
//! | `Checked` | any | `Unchecked` |
//! | `Indeterminate` | all enabled children already checked | `Unchecked` |
//! | `Indeterminate` | otherwise | `Checked` |
//!
//! A leaf checkbox — one with nothing under it — passes
//! [`ChildSummary::LEAF`], whose `enabled == 0` fails the
//! "all checked" test, so a leaf's `Indeterminate` goes to `Checked`.
//! That is the right answer for the wrong-looking reason, so it is worth
//! stating: "all zero of my children are checked" is *vacuously* true in
//! ordinary logic, and the rule deliberately reads it as false, because
//! a control with nothing under it has no children to bring into line
//! and should simply become checked like any other.
//!
//! Determinism here means three things, each pinned by the sidecar: the
//! transition is **total** (every state has an answer), **pure** (the
//! same inputs give the same output, twice over), and **stateless** (it
//! consults nothing but its two arguments — no clock, no history).
//!
//! # Roving is over enabled options only, and wraps
//!
//! §3.3: "`Up`/`Down`/`Left`/`Right` roves focus and selects within
//! the group". Two consequences the short sentence hides:
//!
//! * a disabled option is **skipped**, not landed on and refused —
//!   landing on a control the user then cannot activate is worse than
//!   stepping past it;
//! * the rove **wraps**, because a group with no ends is the only
//!   behaviour that never leaves a keyboard user stuck at an edge with
//!   nothing left to try.
//!
//! `Down` and `Right` both move forward and `Up` and `Left` both move
//! backward, for every orientation. §3.3 does not split the axes by
//! `RadioOrientation`, and a group whose arrow mapping changed with its
//! layout would make muscle memory layout-dependent. The orientation is
//! carried on the model because the spec names it, and it governs
//! layout — not keys.
//!
//! # The switch's `busy` is a lock, not a cancellable action
//!
//! [`SwitchModel::toggle`] refuses while `busy`, which is what makes
//! `busy` mean something: an immediate live toggle that fires twice
//! during a round trip would put the control out of step with the host.
//! [`SwitchModel::pending`] projects the flag as an [`ActionPending`]
//! for a host that runs a gate, always with `cancellable` **off** —
//! §3.3 gives the switch no cancel affordance and no `on_cancel`, so a
//! cancellable projection here would advertise a cancellation nobody
//! implemented.
//!
//! [`Controlled`]: crate::binding::Controlled
//!
//! Unit tests live in the sibling `toggle_model_tests.rs` sidecar.

use crate::binding::{ActionPending, Commit};
use crate::widget::rove::rove_index;
use crate::widget::toggle::TogglePolarity;

/// What a tri-state checkbox has underneath it, as the transition rule
/// needs to know it.
///
/// Two counts rather than a slice of children, because the rule asks two
/// questions — *is there anything enabled down there*, and *is all of it
/// checked* — and both are answered by counters. A caller with real
/// children computes this once; a leaf passes [`ChildSummary::LEAF`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChildSummary {
    /// How many children are enabled.
    pub enabled: usize,
    /// How many of the enabled children are checked.
    pub checked: usize,
}

impl ChildSummary {
    /// The summary of a checkbox with nothing under it.
    ///
    /// Zero enabled, zero checked. See the [module
    /// docs](self) for why this makes an `Indeterminate` leaf go to
    /// `Checked` rather than `Unchecked`.
    pub const LEAF: Self = Self {
        enabled: 0,
        checked: 0,
    };

    /// A summary where every enabled child is checked.
    ///
    /// Requires at least one enabled child — `all_checked(0, 0)` is
    /// false, which is the whole of the leaf case.
    #[must_use]
    pub const fn all_checked(self) -> bool {
        self.enabled > 0 && self.checked == self.enabled
    }

    /// A summary where no enabled child is checked.
    #[must_use]
    pub const fn none_checked(self) -> bool {
        self.checked == 0
    }
}

/// The three states of a tri-state checkbox (§3.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CheckState {
    /// Empty.
    #[default]
    Unchecked,
    /// The `−` dash: some but not all of what is underneath is checked.
    Indeterminate,
    /// The `✓` tick.
    Checked,
}

impl CheckState {
    /// The paint polarity this state resolves through (§3.3's two rows).
    ///
    /// `Indeterminate` is `On`, per the spec's own grouping: the dash
    /// is an accent-filled box, not a quiet empty one.
    #[must_use]
    pub const fn polarity(self) -> TogglePolarity {
        match self {
            Self::Unchecked => TogglePolarity::Off,
            Self::Indeterminate | Self::Checked => TogglePolarity::On,
        }
    }

    /// The next state after an activation, for a control with nothing
    /// underneath it.
    ///
    /// [`next_with`](Self::next_with) with [`ChildSummary::LEAF`], which
    /// is the common case: a checkbox that is not a parent.
    #[must_use]
    pub const fn next(self) -> Self {
        self.next_with(ChildSummary::LEAF)
    }

    /// The next state after an activation, given what is underneath
    /// (`UIF-ATOM-003`).
    ///
    /// The deterministic tri-state rule from the [module
    /// docs](self). Total and pure: every `(state, children)` pair has
    /// exactly one answer, computed from those two arguments alone.
    #[must_use]
    pub const fn next_with(self, children: ChildSummary) -> Self {
        match self {
            Self::Unchecked => Self::Checked,
            Self::Checked => Self::Unchecked,
            Self::Indeterminate => {
                if children.all_checked() {
                    Self::Unchecked
                } else {
                    Self::Checked
                }
            }
        }
    }
}

/// Which way the roving focus moves through a radio group.
///
/// The type lives in [`crate::widget::rove`], the shared home of the
/// single-focus walk: the radio group here and the `SegmentedModel` of
/// §3.1 walk the same ring, and one walk means one rule that cannot
/// drift between them. Re-exported so `toggle_model::RoveDirection`
/// keeps naming it.
pub use crate::widget::rove::RoveDirection;

/// The three kinds of key press a toggle-family control routes.
///
/// The widget maps iced's named keys onto these three so the models never
/// learn which physical key was pressed. That indirection is what lets
/// `Down` and `Right` share one behaviour in one place rather than in
/// two `match` arms that could drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToggleKey {
    /// `Space` — activate.
    Activate,
    /// `Down` or `Right`.
    Forward,
    /// `Up` or `Left`.
    Backward,
}

impl ToggleKey {
    /// The rove direction this key implies, or `None` for
    /// [`Activate`](Self::Activate).
    #[must_use]
    pub const fn rove_direction(self) -> Option<RoveDirection> {
        match self {
            Self::Activate => None,
            Self::Forward => Some(RoveDirection::Forward),
            Self::Backward => Some(RoveDirection::Backward),
        }
    }
}

/// The `CheckboxModel` of §3.3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckboxModel {
    /// The current tri-state.
    pub state: CheckState,
    /// The inline label.
    pub label: String,
    /// An optional description under the label.
    pub description: Option<String>,
    /// Whether the control is inert.
    pub disabled: bool,
}

impl CheckboxModel {
    /// A fresh unchecked box with the given label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            state: CheckState::Unchecked,
            label: label.into(),
            description: None,
            disabled: false,
        }
    }

    /// Set the starting state.
    #[must_use]
    pub fn with_state(mut self, state: CheckState) -> Self {
        self.state = state;
        self
    }

    /// Set the description line.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Make the control inert.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// The `on_toggle(CheckState)` emission an activation produces.
    ///
    /// `None` on a disabled control — an inert control emits nothing
    /// rather than emitting its own value back (`UIF-PRIN-004`: present
    /// but inert, not gone and not answering). Otherwise the next state
    /// from [`CheckState::next_with`], settled as a `Commit` because a
    /// checkbox change is one emission per intent rather than a live
    /// stream.
    ///
    /// `children` is what the app knows and the box does not: for a
    /// parent checkbox, how many of its children are enabled and
    /// checked. A leaf passes [`ChildSummary::LEAF`].
    #[must_use]
    pub fn toggle(&self, children: ChildSummary) -> Option<Commit<CheckState>> {
        if self.disabled {
            return None;
        }
        Some(Commit::new(self.state.next_with(children)))
    }
}

/// One option in a radio group (§3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioOption<Id> {
    /// The caller's option identity.
    pub id: Id,
    /// The inline label.
    pub label: String,
    /// Whether this option is inert.
    pub disabled: bool,
}

impl<Id> RadioOption<Id> {
    /// An enabled option.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            disabled: false,
        }
    }

    /// A disabled option — skippable by the rove, unselectable by click.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// How a radio group lays its options out (§3.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RadioOrientation {
    /// One option per row.
    #[default]
    Vertical,
    /// Options flow across rows and wrap.
    HorizontalWrap,
}

/// Where the roving focus landed and what that selects (§3.3).
///
/// The two travel together because §3.3's arrow contract is that each press
/// "roves and wraps focus across enabled options and selects" — one key press
/// produces both facts, and returning
/// only the index would make the caller re-derive the selection it was
/// told to make.
///
/// `Commit<Id>` carries no `PartialEq` of its own — the tier types
/// deliberately share no trait, so a `Commit` cannot be compared to a
/// `Live` — which is why this struct is `Debug` rather than `Eq`. Tests
/// read `selected.value()` instead of comparing the wrapper.
#[derive(Clone, Debug)]
pub struct Roving<Id> {
    /// The index the focus moved to.
    pub focus: usize,
    /// The `on_select(Id)` emission that goes with it.
    pub selected: Commit<Id>,
}

/// The `RadioGroupModel<Id>` of §3.3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioGroupModel<Id> {
    /// The options, in display order.
    pub options: Vec<RadioOption<Id>>,
    /// The currently selected option, if any.
    pub selected: Option<Id>,
    /// Layout orientation.
    pub orientation: RadioOrientation,
    /// Whether the whole group is inert.
    pub disabled: bool,
}

impl<Id> RadioGroupModel<Id> {
    /// A group of the given options, nothing selected, vertical.
    #[must_use]
    pub fn new(options: Vec<RadioOption<Id>>) -> Self {
        Self {
            options,
            selected: None,
            orientation: RadioOrientation::Vertical,
            disabled: false,
        }
    }

    /// Set the initial selection.
    #[must_use]
    pub fn with_selected(mut self, id: Id) -> Self {
        self.selected = Some(id);
        self
    }

    /// Set the orientation.
    #[must_use]
    pub fn with_orientation(mut self, orientation: RadioOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Make the whole group inert.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Whether the option at `index` can be landed on or selected.
    ///
    /// Dead if the group is dead, or if that option is dead. The group
    /// flag dominates: a disabled group has no live options whatever its
    /// members declare.
    #[must_use]
    pub fn option_enabled(&self, index: usize) -> bool {
        !self.disabled && self.options.get(index).is_some_and(|o| !o.disabled)
    }

    /// The indices the rove is allowed to land on, in display order.
    #[must_use]
    pub fn enabled_indices(&self) -> Vec<usize> {
        (0..self.options.len())
            .filter(|&index| self.option_enabled(index))
            .collect()
    }

    /// The index of the currently selected option, if it is still in the
    /// list.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize>
    where
        Id: PartialEq,
    {
        let selected = self.selected.as_ref()?;
        self.options.iter().position(|o| &o.id == selected)
    }

    /// The index the roving focus lands on, given where it is now.
    ///
    /// Skips disabled options and wraps at both ends. `None` only when
    /// the group has no enabled option to land on — the one case where
    /// there is genuinely nowhere to go.
    ///
    /// `from` is the current focus, not the current *selection*: they
    /// coincide after a rove but not after a click on an already-focused
    /// group, and conflating them would make the first arrow key jump
    /// back to the selected row.
    ///
    /// The walk itself is [`rove_index`], shared with every other
    /// single-focus group in the crate; this method only supplies the
    /// roster.
    #[must_use]
    pub fn rove(&self, from: Option<usize>, direction: RoveDirection) -> Option<usize> {
        rove_index(&self.enabled_indices(), from, direction)
    }

    /// The rove-and-select a directional key produces (§3.3).
    ///
    /// [`rove`](Self::rove) plus the `on_select` emission for the option
    /// it landed on. `None` when there is nowhere to land, or when the
    /// key was [`Activate`](ToggleKey::Activate) — a radio group has no
    /// activation-only behaviour, because focus and selection are the
    /// same act.
    #[must_use]
    pub fn rove_and_select(&self, from: Option<usize>, key: ToggleKey) -> Option<Roving<Id>>
    where
        Id: Clone,
    {
        let direction = key.rove_direction()?;
        let focus = self.rove(from, direction)?;
        let option = self.options.get(focus)?;
        Some(Roving {
            focus,
            selected: Commit::new(option.id.clone()),
        })
    }

    /// The `on_select(Id)` emission a click on an option produces.
    ///
    /// `None` when the option is disabled, or when it is already
    /// selected — §3.3: "re-clicking the already-selected option is a
    /// no-op". Emitting a re-selection would look harmless but would
    /// give the host an event to react to for a value that did not
    /// change, which is how a radio group ends up re-running whatever
    /// expensive thing a selection triggers.
    #[must_use]
    pub fn select(&self, id: &Id) -> Option<Commit<Id>>
    where
        Id: PartialEq + Clone,
    {
        if self.disabled {
            return None;
        }
        if self.selected.as_ref() == Some(id) {
            return None;
        }
        if !self.options.iter().any(|o| &o.id == id && !o.disabled) {
            return None;
        }
        Some(Commit::new(id.clone()))
    }
}

/// The `SwitchModel` of §3.3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchModel {
    /// Whether the switch is on.
    pub on: bool,
    /// The inline label.
    pub label: String,
    /// Text shown beside the track when on.
    pub on_text: Option<String>,
    /// Text shown beside the track when off.
    pub off_text: Option<String>,
    /// Whether a toggle is still in flight with the host.
    pub busy: bool,
    /// Whether the control is inert.
    pub disabled: bool,
}

impl SwitchModel {
    /// A fresh off switch with the given label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            on: false,
            label: label.into(),
            on_text: None,
            off_text: None,
            busy: false,
            disabled: false,
        }
    }

    /// Set the starting position.
    #[must_use]
    pub fn with_on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// Set the text shown in each position.
    #[must_use]
    pub fn with_texts(mut self, on_text: impl Into<String>, off_text: impl Into<String>) -> Self {
        self.on_text = Some(on_text.into());
        self.off_text = Some(off_text.into());
        self
    }

    /// Mark a toggle as in flight with the host.
    #[must_use]
    pub fn busy(mut self) -> Self {
        self.busy = true;
        self
    }

    /// Make the control inert.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// The paint polarity the track resolves through.
    #[must_use]
    pub const fn polarity(&self) -> TogglePolarity {
        if self.on {
            TogglePolarity::On
        } else {
            TogglePolarity::Off
        }
    }

    /// The `on_toggle(bool)` emission an activation produces.
    ///
    /// `None` while the control is inert or busy. The busy refusal is
    /// what makes `busy` mean anything: §3.3 calls this an "immediate
    /// live toggle", so without the refusal a second press during a
    /// round trip would emit the opposite of what the host is about to
    /// report back, and the switch would visibly disagree with the thing
    /// it controls.
    #[must_use]
    pub fn toggle(&self) -> Option<Commit<bool>> {
        if self.disabled || self.busy {
            return None;
        }
        Some(Commit::new(!self.on))
    }

    /// The pending-action value this model projects, as a fresh value per
    /// frame.
    ///
    /// `busy` becomes an [`ActionPending::indeterminate`] so a host
    /// running an [`ActivationGate`](crate::binding::ActivationGate)
    /// sees the same fact the toggle refusal acts on, rather than two
    /// separate sources of truth about whether work is underway.
    ///
    /// `cancellable` is always **off**. §3.3 gives the switch no
    /// `on_cancel` and no stop affordance, so projecting a cancellable
    /// pending here would advertise a cancellation the framework cannot
    /// honour — and
    /// [`ActionPending::try_cancel`] would hand back a request nobody
    /// was going to act on.
    #[must_use]
    pub fn pending(&self) -> ActionPending {
        let pending = if self.busy {
            ActionPending::indeterminate()
        } else {
            ActionPending::idle()
        };
        pending.with_cancellable(false)
    }
}

#[cfg(test)]
#[path = "toggle_model_tests.rs"]
mod toggle_model_tests;
