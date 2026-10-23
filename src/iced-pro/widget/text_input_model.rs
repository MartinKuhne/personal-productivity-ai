//! The declarative single-line input model — `UIF-ATOM-002a`'s
//! `TextInputModel` (spec §3.2).
//!
//! A [`TextInputModel`] is plain data: what the field holds, what it hints
//! at, whether the host's validator has rejected it, and what the user is
//! allowed to do with it. It carries no interaction state and no
//! callbacks — the host owns one of these per field and hands it to
//! [`TextInput::controlled`] every frame (spec §3.0.4.1: the model is the
//! truth, so the widget must not keep its own copy).
//!
//! The split this module enforces:
//!
//! - **Controlled** — the host owns the model, so the input reads it as
//!   [`Controlled`] (a shared reference, never a copy). See
//!   [`TextInputModel::controlled`].
//! - **Ephemeral** — what the *editing session* remembers (the
//!   pre-gesture snapshot, the coalesced live tick, whether the field has
//!   focus) lives in the widget tree under the [`Ephemeral`] tag, in
//!   [`TextInputController`], and a rebuild drops it along with every
//!   other piece of interaction state.
//!
//! # Why `validation` is a message and not a verdict
//!
//! §3.2's Invalid row is a *reported* state: the widget never inspects
//! [`value`](TextInputModel::value) to decide it is bad. [`validation`] carries the
//! host's own words, and [`is_invalid`](TextInputModel::is_invalid) is
//! simply "the host said something". That keeps the Invalid cell honest —
//! a second validator invented in the view layer would disagree with the
//! application's, and the field would flash between two opinions.
//!
//! # `clearable` is a capability, not a glyph
//!
//! [`slot_content`] already
//! decides *which* glyph the reserved trailing slot carries.
//! [`clearable`] decides whether that glyph is **actionable**: a model
//! with `clearable: false` still shows the search field's `×`, but the `×`
//! is inert and `Escape` will not clear the field either. The default is
//! `true`, which is exactly what the widget did before this model existed
//! — so adopting the model cannot quietly remove a clear affordance a host
//! was relying on.
//!
//! [`TextInput::controlled`]: crate::widget::text_input::TextInput::controlled
//! [`Controlled`]: crate::binding::Controlled
//! [`Ephemeral`]: crate::binding::Ephemeral
//! [`TextInputController`]: crate::widget::text_input_controller::TextInputController
//! [`validation`]: TextInputModel::validation
//! [`clearable`]: TextInputModel::clearable
//! [`slot_content`]: crate::widget::text_input::slot_content
//!
//! Unit tests live in the sibling `text_input_model_tests.rs` sidecar.

use crate::binding::Controlled;
use crate::widget::text_input::InputKind;
use crate::widget::text_input::SlotContent;
use crate::widget::text_input::slot_content;

/// A range inside a value, in the same index space iced's editor uses.
///
/// Byte offsets, not character counts, because that is what the underlying
/// editor addresses its caret and selection with — a model that counted
/// `char`s would be off by one for every multi-byte character the moment
/// the two were compared. [`clamped_to`](Self::clamped_to) is the guard: a
/// range that lands inside a character is moved to the boundary rather than
/// being handed to a slicing operation that would panic.
///
/// `start <= end` is the normal form. [`new`](Self::new) does not enforce
/// it — a caller passing a reversed range is a bug worth seeing — but
/// [`clamped_to`](Self::clamped_to) normalises it, so anything that has
/// been clamped is safe to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextRange {
    /// The first offset of the range.
    pub start: usize,
    /// The offset one past the end of the range.
    pub end: usize,
}

impl TextRange {
    /// A range from `start` to `end`.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// A zero-width range at `offset` — a caret, not a selection.
    #[must_use]
    pub const fn caret(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    /// How much of the value the range covers.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range covers nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the range is a caret rather than a selection.
    ///
    /// Distinct from [`is_empty`](Self::is_empty): a caret is *at* a place,
    /// an empty range is *nowhere*. They coincide for a normalised range,
    /// but the names describe different intentions and a caller should pick
    /// the one it means.
    #[must_use]
    pub const fn is_caret(&self) -> bool {
        self.start == self.end
    }

    /// This range moved inside `value_len`, normalised and never panicking.
    ///
    /// Both ends are capped at `value_len` and reordered so `start <= end`.
    /// A range pointing past the end of a value is not an error worth
    /// crashing a frame over — the host changed the value under a selection
    /// it set earlier, which is exactly what a controlled field makes
    /// possible.
    #[must_use]
    pub const fn clamped_to(&self, value_len: usize) -> Self {
        let start = if self.start > value_len {
            value_len
        } else {
            self.start
        };
        let end = if self.end > value_len {
            value_len
        } else {
            self.end
        };

        if start <= end {
            Self { start, end }
        } else {
            Self {
                start: end,
                end: start,
            }
        }
    }

    /// The slice this range selects out of `value`, if it lands on
    /// character boundaries.
    ///
    /// `None` rather than a panic for a range that splits a character: the
    /// caller is reporting a selection, and "there is no such slice" is
    /// the truthful answer.
    #[must_use]
    pub fn text_of<'a>(&self, value: &'a str) -> Option<&'a str> {
        value.get(self.start..self.end)
    }
}

/// The declarative state of one single-line text field (`UIF-ATOM-002a`).
///
/// ```text
/// value ──────────────────────────── what the field holds
/// placeholder ────────────────────── what it hints at while empty
/// selection ──────────────────────── where the caret or highlight is
/// validation ─────────────────────── the host's own words about the value
/// read_only ──────────────────────── viewable and selectable, not editable
/// disabled ───────────────────────── the host refuses interaction entirely
/// clearable ──────────────────────── whether the clear affordance can act
/// ```
///
/// ## The Controlled / Ephemeral split
///
/// Everything here is host-owned, so the input takes it as
/// [`Controlled<'_, TextInputModel>`](Controlled) and reads it without
/// copying. What is *not* here — the pre-gesture snapshot, the coalesced
/// live tick, whether the field currently has focus — is the input's
/// [`Ephemeral`](crate::binding::Ephemeral) half, held by
/// [`TextInputController`](crate::widget::text_input_controller::TextInputController)
/// in the widget tree and rebuilt from zero whenever the tree is.
///
/// ## Why there is no `focused` field
///
/// Focus is not host-owned. The router decides it, the editor observes it,
/// and `on_focus_change(bool)` reports it outward. A `focused` field on a
/// host-owned model would be a second copy of a fact the framework already
/// holds, and the two would disagree for exactly as long as it takes the
/// host to notice — which is the desynchronisation §3.0.4.1 exists to
/// prevent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInputModel {
    /// The field's current value.
    pub value: String,
    /// The hint shown while [`value`](Self::value) is empty.
    pub placeholder: String,
    /// The caret or selection the host wants, if it has an opinion.
    ///
    /// `None` means "wherever the editor left it". A `Some` range is the
    /// host asking for a specific caret — the rename-stem selection
    /// `UIF-INP-004` describes is the named case.
    pub selection: Option<TextRange>,
    /// The host's validation message. `Some` is what makes the field
    /// invalid; the widget never derives invalidity from the value.
    pub validation: Option<String>,
    /// Viewable and selectable, but not editable.
    pub read_only: bool,
    /// The host refuses interaction entirely.
    pub disabled: bool,
    /// Whether the trailing clear affordance can act.
    ///
    /// `true` by default, matching what the widget did before this model
    /// existed. See the [module docs](self) for why the default matters.
    pub clearable: bool,
}

impl Default for TextInputModel {
    /// An empty, live, clearable field.
    ///
    /// Hand-written rather than derived because the derived default for
    /// `clearable` would be `false`, and a model built with
    /// `..Default::default()` would then silently strip the search
    /// field's clear affordance. The default is the behaviour the widget
    /// had before the model existed, so adopting the model is a no-op for
    /// a host that does not set the flag.
    fn default() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            selection: None,
            validation: None,
            read_only: false,
            disabled: false,
            clearable: true,
        }
    }
}

impl TextInputModel {
    /// A live field holding `value`.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            ..Self::default()
        }
    }

    /// An empty live field.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Sets the placeholder.
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Reports the value invalid, carrying the host's own message.
    pub fn with_validation(mut self, message: impl Into<String>) -> Self {
        self.validation = Some(message.into());
        self
    }

    /// Asks for a specific caret or selection.
    #[must_use]
    pub fn with_selection(mut self, selection: TextRange) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Makes the field viewable and selectable but not editable.
    #[must_use]
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Makes the field entirely inert.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Declares whether the clear affordance can act.
    ///
    /// Takes a `bool` rather than being a `clear()` / `unclear()` pair
    /// because the flag is genuinely two-way state the host flips: a
    /// search field becomes clearable when a query arrives and stops being
    /// clearable while a result set is locked.
    #[must_use]
    pub fn with_clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    /// Wraps this model as host-owned state.
    #[must_use]
    pub fn controlled(&self) -> Controlled<'_, TextInputModel> {
        Controlled::new(self)
    }

    /// Whether the field holds anything.
    #[must_use]
    pub fn has_value(&self) -> bool {
        !self.value.is_empty()
    }

    /// Whether the host has reported this field's value invalid.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.validation.is_some()
    }

    /// Whether the user may change the value at all.
    ///
    /// `false` for a [`disabled`](Self::disabled) field (nothing happens)
    /// and for a [`read_only`](Self::read_only) one (the value can be read
    /// and selected but not edited). The two are different states that
    /// share the "no edits" consequence, which is why the model keeps them
    /// apart rather than collapsing them into one flag.
    #[must_use]
    pub fn accepts_input(&self) -> bool {
        !self.disabled && !self.read_only
    }

    /// What the reserved trailing slot carries for this model's state.
    ///
    /// [`slot_content`] decides the glyph; [`clearable`](Self::clearable)
    /// decides whether it may act. A non-clearable model downgrades an
    /// actionable glyph to a passive one — the slot still shows the `×`,
    /// it simply does nothing when pressed, which is what the zero-shift
    /// invariant requires: the glyph's *meaning* changed, its geometry did
    /// not.
    #[must_use]
    pub fn slot_for(&self, kind: InputKind) -> SlotContent {
        match slot_content(kind, self.has_value(), self.is_invalid()) {
            SlotContent::Actionable(glyph) if !self.clearable => SlotContent::Passive(glyph),
            other => other,
        }
    }

    /// Whether this model exposes a clear the user can actually trigger.
    ///
    /// The single predicate behind both of `002a`'s clear triggers: the
    /// trailing `×` and the `Escape`-clears-the-field path ask this same
    /// question, so the two can never disagree about whether clearing is
    /// available.
    #[must_use]
    pub fn clear_affordance(&self, kind: InputKind) -> bool {
        self.clearable && self.slot_for(kind).is_actionable()
    }

    /// [`selection`](Self::selection) moved inside the current value.
    ///
    /// A host that set a selection and then changed the value asks the
    /// model this rather than trusting its own range, because the value it
    /// shipped the range with may no longer be the value in the field.
    #[must_use]
    pub fn selection_in_value(&self) -> Option<TextRange> {
        self.selection
            .map(|range| range.clamped_to(self.value.len()))
    }
}

/// A text field model that is host-owned, i.e. [`Controlled`].
///
/// Re-exported here so a caller building an input reads
/// `Controlled<'_, TextInputModel>` without reaching for the binding
/// module. Mirrors `button_model::ControlledModel`.
pub type ControlledModel<'a> = Controlled<'a, TextInputModel>;

#[cfg(test)]
#[path = "text_input_model_tests.rs"]
mod text_input_model_tests;
