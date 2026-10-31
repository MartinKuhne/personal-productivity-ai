//! What a rejected edit has to give back (`UIF-PRIN-009` clause 3).
//!
//! §1.1's third clause is a list, not a feeling: a validation failure or
//! a rejected background operation MUST preserve the **buffer**, the
//! **scroll offset** and the **selection range**. Three named things, so
//! the snapshot carries three named things — and says which of them it
//! actually moved on the way back.
//!
//! # Why the existing snapshot was not enough
//!
//! `binding::snapshot` already delivers a real pre-gesture revert, and
//! `TextInputController` uses it to restore the buffer on `Escape`. That
//! is one of the three. The other two had nowhere to live: `selection` sat
//! on the model and was never carried into the snapshot, and no scroll
//! offset was carried at all. A user deep in a long buffer who pressed
//! `Escape` got their text back and their viewport dumped somewhere else.
//!
//! This module widens the payload, not the machinery. [`EditSnapshot`]
//! is an ordinary `Clone` value, so it drops straight into the existing
//! `Snapshot<T>` / `Gesture<T>` / `GestureSlot<T>` guard: the linearity
//! that makes "commit twice" unwriteable is the same linearity, and
//! [`RecoverySession`] is that guard instantiated at the richer type
//! rather than a second implementation of it.
//!
//! # The honest limit
//!
//! Everything here is **model state**. Restoring a selection range puts
//! the range back on the model; it does not move a caret inside a live
//! iced editor, because this crate does not own iced's editor state and
//! `TextInputModel::selection` is documented as a request the widget may
//! or may not honour. The ledger says so plainly rather than implying the
//! caret is back on screen.
//!
//! Unit tests live in the sibling `recovery_tests.rs` sidecar.

use crate::binding::snapshot::{Cancel, GestureSlot, GestureStart};
use crate::widget::text_area_model::TextAreaModel;
use crate::widget::text_input_model::{TextInputModel, TextRange};

use std::fmt;

/// A scrolled viewport's distance from the top of its content.
///
/// A newtype rather than a bare `f32` because [`EditSnapshot`] holds
/// several numbers and the name is what stops a caller passing a height
/// where an offset was wanted. Negative input is clamped to zero: a
/// negatively scrolled field is not a thing this framework models, and
/// silently storing `-40.0` would come back as a restore that visibly
/// did nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollOffset(f32);

impl ScrollOffset {
    /// A viewport sitting at the top of its content.
    pub const ZERO: Self = Self(0.0);

    /// An offset of `pixels`, floored at zero.
    #[must_use]
    pub fn new(pixels: f32) -> Self {
        Self(if pixels < 0.0 { 0.0 } else { pixels })
    }

    /// The raw pixel offset.
    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }

    /// Whether the viewport is unscrolled.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0.abs() < f32::EPSILON
    }
}

impl fmt::Display for ScrollOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}px", self.0)
    }
}

/// Which of the three preserved fields a restore actually wrote back.
///
/// Reported rather than assumed because "the restore ran" and "the restore
/// changed anything" are different claims, and a caller that cannot tell
/// them apart cannot tell a working revert from a no-op.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RestoredFields {
    /// The text buffer was carried and differed.
    pub buffer: bool,
    /// The selection range was carried and differed.
    pub selection: bool,
    /// The scroll offset was carried and differed.
    pub scroll: bool,
}

impl RestoredFields {
    /// Nothing was written back.
    pub const NONE: Self = Self {
        buffer: false,
        selection: false,
        scroll: false,
    };

    /// Whether the restore moved anything at all.
    #[must_use]
    pub const fn any(self) -> bool {
        self.buffer || self.selection || self.scroll
    }

    /// How many of the three fields moved.
    #[must_use]
    pub fn count(self) -> usize {
        let buffer = if self.buffer { 1 } else { 0 };
        let selection = if self.selection { 1 } else { 0 };
        let scroll = if self.scroll { 1 } else { 0 };
        buffer + selection + scroll
    }
}

impl fmt::Display for RestoredFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "buffer:{} selection:{} scroll:{}",
            self.buffer, self.selection, self.scroll
        )
    }
}

/// The pre-edit state a rejected operation must return.
///
/// Held by value, like every other snapshot in this crate: a revert that
/// aliased live model data would restore whatever the model had become,
/// which is not a revert.
#[derive(Clone, Debug, PartialEq)]
pub struct EditSnapshot {
    buffer: String,
    selection: Option<TextRange>,
    scroll: Option<ScrollOffset>,
    was_dirty: bool,
}

impl EditSnapshot {
    /// A snapshot of `buffer`, `selection` and `scroll`, clean.
    #[must_use]
    pub fn new(
        buffer: impl Into<String>,
        selection: Option<TextRange>,
        scroll: Option<ScrollOffset>,
    ) -> Self {
        Self {
            buffer: buffer.into(),
            selection,
            scroll,
            was_dirty: false,
        }
    }

    /// Record whether the buffer was already dirty when this was taken.
    ///
    /// The fourth field, and the one §1.1 does not name. It is carried
    /// because restoring a buffer is not free of consequence: writing
    /// text into a well marks it dirty, so a snapshot that did not remember
    /// the pre-edit dirtiness would turn a clean field dirty by reverting
    /// it — changing a fact the pillar says must stay intact. Without it,
    /// `Escape` from a rename dialog would leave a freshly-loaded,
    /// unmodified field claiming unsaved changes.
    #[must_use]
    pub const fn with_dirty(mut self, was_dirty: bool) -> Self {
        self.was_dirty = was_dirty;
        self
    }

    /// Capture a single-line field.
    ///
    /// `scroll` is `None` because `TextInputModel` carries no scroll
    /// position — iced owns the horizontal caret scroll of a single-line
    /// field and this crate never reads it. `None` rather than `Some(ZERO)`
    /// so a restore cannot claim to have put back something that was never
    /// there.
    #[must_use]
    pub fn from_text_input(model: &TextInputModel) -> Self {
        Self {
            buffer: model.value.clone(),
            selection: model.selection,
            scroll: None,
            was_dirty: false,
        }
    }

    /// Capture a multi-line well, including its dirtiness.
    #[must_use]
    pub fn from_text_area(model: &TextAreaModel) -> Self {
        Self {
            buffer: model.text().to_string(),
            selection: model.selection(),
            scroll: Some(ScrollOffset::new(model.scroll_offset())),
            was_dirty: model.is_dirty(),
        }
    }

    /// The preserved buffer.
    #[must_use]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// The preserved selection range, if one was carried.
    #[must_use]
    pub const fn selection(&self) -> Option<TextRange> {
        self.selection
    }

    /// The preserved scroll offset, if one was carried.
    #[must_use]
    pub const fn scroll(&self) -> Option<ScrollOffset> {
        self.scroll
    }

    /// Whether the buffer was already dirty at capture time.
    #[must_use]
    pub const fn was_dirty(&self) -> bool {
        self.was_dirty
    }

    /// How many of the three pillar-named fields this snapshot carries.
    ///
    /// The buffer always counts; selection and scroll count only when
    /// present. A test asserting this is `3` is asserting the snapshot is
    /// complete, which is the clause under review.
    #[must_use]
    pub fn carried_count(&self) -> usize {
        let selection = if self.selection.is_some() { 1 } else { 0 };
        let scroll = if self.scroll.is_some() { 1 } else { 0 };
        1 + selection + scroll
    }

    /// Restore a single-line field's buffer and selection.
    ///
    /// The selection is clamped to the restored buffer before it is
    /// written, so a snapshot taken against a long value cannot leave a
    /// range pointing past the end of a short one. Restoring into an empty
    /// or never-edited field is safe: every field simply compares equal
    /// and nothing is written.
    #[must_use]
    pub fn restore_text_input(&self, model: &mut TextInputModel) -> RestoredFields {
        let selection = self
            .selection
            .map(|range| range.clamped_to(self.buffer.len()));

        let buffer = model.value != self.buffer;
        let changed_selection = model.selection != selection;

        if buffer {
            model.value = self.buffer.clone();
        }
        if changed_selection {
            model.selection = selection;
        }

        RestoredFields {
            buffer,
            selection: changed_selection,
            scroll: false,
        }
    }

    /// Restore a multi-line well's buffer, selection and scroll.
    ///
    /// Each field is reported by comparing the model before and after the
    /// write rather than by comparing against the snapshot, because
    /// `TextAreaModel::set_scroll` clamps: a snapshot taken when the well
    /// was tall, restored into a well that has since shrunk, cannot land
    /// at the same offset. Reporting the carried value there would be a
    /// lie about what the model holds.
    #[must_use]
    pub fn restore_text_area(&self, model: &mut TextAreaModel) -> RestoredFields {
        let buffer_before = model.text();
        let buffer = buffer_before != self.buffer;
        let selection = model.selection() != self.selection;

        if buffer {
            model.set_text(self.buffer.clone());
        }
        if selection {
            model.set_selection(self.selection);
        }

        let scroll_before = model.scroll_offset();
        if let Some(offset) = self.scroll {
            model.set_scroll(offset.get());
        }
        let scroll = model.scroll_offset() != scroll_before;

        if !self.was_dirty {
            model.mark_clean();
        }

        RestoredFields {
            buffer,
            selection,
            scroll,
        }
    }
}

/// The snapshot-at-first-edit path, widened to carry all three fields.
///
/// This is `GestureSlot<EditSnapshot>` with the two restore-shaped calls
/// added, which is the whole extension: the outermost-wins nesting rule,
/// the refusal to re-capture mid-session and the consume-on-settle
/// linearity all come from the existing guard unchanged.
///
/// A successful commit settles through [`settle`](Self::settle) rather than
/// the guard's `commit` tier, because a recovery snapshot is not a value
/// the host settles — it is a restore target, and accepting the edit means
/// it is no longer owed to anybody.
#[derive(Debug, Default)]
pub struct RecoverySession {
    slot: GestureSlot<EditSnapshot>,
}

impl RecoverySession {
    /// A session with nothing retained.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a revert target is being held.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.slot.is_open()
    }

    /// Take `snapshot` as the revert target.
    ///
    /// A second call while a session is open is refused and the **outer**
    /// snapshot keeps governing — the rule that stops a nested edit
    /// silently shortening the revert to an intermediate value the user
    /// never agreed to.
    #[must_use]
    pub fn begin(&mut self, snapshot: &EditSnapshot) -> GestureStart {
        self.slot.begin(snapshot)
    }

    /// Capture a single-line field and open the session in one step.
    #[must_use]
    pub fn begin_from_text_input(&mut self, model: &TextInputModel) -> GestureStart {
        self.begin(&EditSnapshot::from_text_input(model))
    }

    /// Capture a multi-line well and open the session in one step.
    #[must_use]
    pub fn begin_from_text_area(&mut self, model: &TextAreaModel) -> GestureStart {
        self.begin(&EditSnapshot::from_text_area(model))
    }

    /// `Escape`: hand back the revert target.
    ///
    /// `None` when no session is open, which is the honest answer — there
    /// is nothing to restore, and inventing an empty one would wipe a
    /// buffer the user never touched.
    #[must_use]
    pub fn cancel(&mut self) -> Option<Cancel<EditSnapshot>> {
        self.slot.cancel()
    }

    /// `Escape`, restoring a single-line field as it goes.
    #[must_use]
    pub fn cancel_into_text_input(&mut self, model: &mut TextInputModel) -> Option<RestoredFields> {
        let cancel = self.cancel()?;
        let restored = cancel.restored().restore_text_input(model);
        Some(restored)
    }

    /// `Escape`, restoring a multi-line well as it goes.
    #[must_use]
    pub fn cancel_into_text_area(&mut self, model: &mut TextAreaModel) -> Option<RestoredFields> {
        let cancel = self.cancel()?;
        let restored = cancel.restored().restore_text_area(model);
        Some(restored)
    }

    /// Discard the revert target without restoring anything.
    ///
    /// The accepted-edit path, and the teardown path. Returns whether a
    /// session was actually open, so a caller can tell it closed something
    /// rather than nothing.
    #[must_use]
    pub fn settle(&mut self) -> bool {
        let was_open = self.slot.is_open();
        self.slot.abort();
        was_open
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod recovery_tests;
