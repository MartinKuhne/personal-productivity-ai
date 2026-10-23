//! The `MultiLineTextArea` host — the atom's tier engine applied to a
//! wrapping editor (`UIF-ATOM-002e`).
//!
//! # What is reused, and what could not be
//!
//! A text area is the `002a` atom with a taller row. Everything that
//! makes a text field a *controlled* text field is taken unchanged:
//!
//! * the **horizontal partition** —
//!   [`text_area_bounds`](super::text_input::text_area_bounds) and
//!   [`trailing_slot_bounds`](super::text_input::trailing_slot_bounds) at
//!   [`InputKind::TextArea`] give the same
//!   zero-shift slot, so the validation mark moves nothing;
//! * the **cell matrix** — `resolve_input_style` paints the same five
//!   §3.2 cells, because the kind selects slot content and never paint;
//! * the **editing engine** — [`TextInputController`] is composed here
//!   outright. The snapshot-at-first-edit rule, the coalesced `Live`
//!   tick, the exactly-once `Commit` on settle and the `Escape` revert
//!   are the same code that drives the single-line field, not a
//!   re-implementation of them.
//!
//! What could not be reused is the inner editor: iced's `text_input` is
//! single-line and does not wrap. That is the one genuine difference
//! between the two controls, and it is confined to what sits inside the
//! atom's text area.
//!
//! # `Enter` is a newline; `Ctrl+Enter` is a send
//!
//! The single-line atom settles on `Enter` because it has nothing else
//! `Enter` could mean. A multi-line field's `Enter` inserts a line break,
//! so settling there would make a three-line buffer impossible. The
//! settle moves to focus loss, and the send moves to `Ctrl+Enter`
//! (§3.2's `on_submit`). Both are distinct arms of
//! [`TextAreaEmission`]
//! because they are distinct events: a submit leaves the user in the
//! field with the session still open, so the `Escape` after a submit
//! still reverts what they typed.
//!
//! # Undo is a ledger, not an editor feature
//!
//! Iced's `text_editor` has no undo — its `Action` enum covers insert,
//! delete and movement and stops there — so
//! [`EditHistory`] is carried here
//! and stepped against the engine. An undo updates the engine's current
//! value so the snapshot and the eventual settle stay coherent, and
//! drains the coalesced live tick rather than forwarding it: the spec
//! reports an undo through `on_undo()`, and sending both would have the
//! host apply the same change twice.
//!
//! Unit tests live in the sibling `text_area_host_tests.rs` sidecar.

use crate::binding::Commit;
use crate::widget::text_area_model::EditHistory;
use crate::widget::text_area_model::LineCol;
use crate::widget::text_area_model::TextAreaEmission;
use crate::widget::text_area_model::TextAreaModel;
use crate::widget::text_input::InputKind;
use crate::widget::text_input_controller::TextInputController;
use crate::widget::text_input_controller::TextInputEmission;

/// The kind the multi-line well renders as.
///
/// Named so the call site reads as the decision rather than as a magic
/// variant.
pub const TEXT_AREA_KIND: InputKind = InputKind::TextArea;

/// Everything one host input to a text area did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextAreaOutcome {
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<TextAreaEmission>,
}

impl TextAreaOutcome {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            emissions: Vec::new(),
        }
    }

    /// An outcome carrying `emissions`.
    #[must_use]
    pub fn emissions(emissions: Vec<TextAreaEmission>) -> Self {
        Self { emissions }
    }

    /// Whether this outcome changed nothing at all.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.emissions.is_empty()
    }
}

/// The per-control memory a multi-line well needs: the editing engine,
/// the undo ledger, and the transcript.
#[derive(Debug)]
pub struct TextAreaController {
    engine: TextInputController,
    history: EditHistory,
    submits: usize,
    undos: usize,
    redos: usize,
}

impl Default for TextAreaController {
    fn default() -> Self {
        Self::new()
    }
}

impl TextAreaController {
    /// A controller with no session and an empty history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: TextInputController::new(),
            history: EditHistory::new(),
            submits: 0,
            undos: 0,
            redos: 0,
        }
    }

    /// A controller whose engine starts holding `text`.
    #[must_use]
    pub fn with_value(text: impl Into<String>) -> Self {
        Self {
            engine: TextInputController::with_value(text),
            ..Self::new()
        }
    }

    /// Record the host's projection. Call once per frame, before any
    /// event handling.
    ///
    /// Goes through [`TextInputController::sync_value`], which carries
    /// the one rule that matters: the host may overwrite the engine only
    /// while no session is open. Pushing a new buffer at the user
    /// mid-keystroke would make a later `Escape` restore something they
    /// never saw.
    pub fn sync(&mut self, model: &TextAreaModel) {
        self.engine.sync_value(model.text());
    }

    /// Record an edit the editor produced.
    ///
    /// The pre-edit buffer goes into the undo ledger *before* the engine
    /// takes the new value, which is the same ordering the engine uses
    /// for its own snapshot: both the revert target and the undo target
    /// are captured before anything changes, never reconstructed after.
    /// An edit that changed nothing records nothing.
    pub fn note_edit(&mut self, model: &mut TextAreaModel, text: String) -> TextAreaOutcome {
        if text == model.text() {
            return TextAreaOutcome::noop();
        }

        self.history.record(model.text());
        if !self.engine.note_edit(text.clone()) {
            return TextAreaOutcome::noop();
        }

        model.set_text(text.clone());
        model.scroll_to_show(model.cursor());

        TextAreaOutcome::emissions(vec![TextAreaEmission::Changed(crate::binding::Live::new(
            text,
        ))])
    }

    /// `Ctrl+Enter` / `Cmd+Enter`: send the buffer.
    ///
    /// Deliberately does **not** touch the engine. The session stays
    /// open, the snapshot stays where it was, and the user stays in the
    /// field. Settling here would mean the `Escape` after a send had
    /// nothing to revert, and the blur after a send would send the same
    /// text a second time.
    #[must_use]
    pub fn submit(&mut self, model: &TextAreaModel) -> TextAreaOutcome {
        self.submits += 1;
        TextAreaOutcome::emissions(vec![TextAreaEmission::Submitted(Commit::new(
            model.text().to_string(),
        ))])
    }

    /// Settle the buffer on focus loss.
    ///
    /// Delegates to the engine, so a well that never opened a session
    /// settles the way the single-line atom does, and a second settle has
    /// nothing to settle.
    pub fn commit(&mut self) -> TextAreaOutcome {
        match self.engine.commit() {
            Some(TextInputEmission::Committed(commit)) => {
                TextAreaOutcome::emissions(vec![TextAreaEmission::Committed(commit)])
            }
            Some(other) => TextAreaOutcome::emissions(vec![map_settle(other)]),
            None => TextAreaOutcome::noop(),
        }
    }

    /// `Escape`: revert to the snapshot taken at the first edit after
    /// focus.
    ///
    /// The buffer is written back into the model as well as the engine,
    /// because the model is what the next frame renders — a revert that
    /// updated only the engine would show the reverted text for one frame
    /// and then paint the stale model over it.
    pub fn cancel(&mut self, model: &mut TextAreaModel) -> TextAreaOutcome {
        match self.engine.revert() {
            Some(TextInputEmission::Cancelled(cancel)) => {
                let restored = cancel.restored().to_string();
                model.set_text(restored.clone());
                model.mark_clean();
                TextAreaOutcome::emissions(vec![TextAreaEmission::Cancelled(cancel)])
            }
            _ => TextAreaOutcome::noop(),
        }
    }

    /// Step the buffer back one recorded edit.
    ///
    /// The engine is told the restored value so its `current`, its
    /// snapshot and whatever settle follows are all speaking about the
    /// same text; the live tick that produces is drained, because this
    /// gesture reports itself through `on_undo()` and forwarding both
    /// would have the host apply one change twice.
    pub fn undo(&mut self, model: &mut TextAreaModel) -> TextAreaOutcome {
        let Some(previous) = self.history.undo(model.text()) else {
            return TextAreaOutcome::noop();
        };

        self.engine.note_edit(previous.clone());
        self.engine.take_live();
        model.set_text(previous.clone());
        model.scroll_to_show(model.cursor());
        self.undos += 1;

        TextAreaOutcome::emissions(vec![TextAreaEmission::Undone(previous)])
    }

    /// Step the buffer forward one undone edit.
    pub fn redo(&mut self, model: &mut TextAreaModel) -> TextAreaOutcome {
        let Some(next) = self.history.redo(model.text()) else {
            return TextAreaOutcome::noop();
        };

        self.engine.note_edit(next.clone());
        self.engine.take_live();
        model.set_text(next.clone());
        model.scroll_to_show(model.cursor());
        self.redos += 1;

        TextAreaOutcome::emissions(vec![TextAreaEmission::Redone(next)])
    }

    /// Report a caret move.
    ///
    /// Emits `on_cursor_change(LineCol)` only on an actual change, and
    /// scrolls the minimum needed to keep the caret visible while it is
    /// at it — the two things a host would otherwise have to remember to
    /// do after every arrow key.
    pub fn note_cursor(&mut self, model: &mut TextAreaModel, at: LineCol) -> TextAreaOutcome {
        if model.cursor() == at {
            return TextAreaOutcome::noop();
        }

        model.set_cursor(at);
        model.scroll_to_show(at);
        TextAreaOutcome::emissions(vec![TextAreaEmission::CursorChanged(model.cursor())])
    }

    /// Record a focus edge.
    ///
    /// The engine owns what that means: losing focus with a session open
    /// settles it as a `Commit`, which is the multi-line field's only
    /// settle path now that `Enter` has been given a newline instead.
    pub fn note_focus(&mut self, model: &mut TextAreaModel, focused: bool) -> TextAreaOutcome {
        let emissions: Vec<TextAreaEmission> = self
            .engine
            .note_focus(focused)
            .into_iter()
            .map(map_settle)
            .collect();

        if focused {
            model.mark_clean();
        }

        TextAreaOutcome::emissions(emissions)
    }

    /// Forget the session, the history and the transcript.
    pub fn reset(&mut self) {
        self.engine.reset();
        self.history.clear();
        self.submits = 0;
        self.undos = 0;
        self.redos = 0;
    }

    // -- reads ---------------------------------------------------------

    /// The engine's current value.
    #[must_use]
    pub fn value(&self) -> &str {
        self.engine.value()
    }

    /// Whether an editing session is open.
    #[must_use]
    pub fn session_open(&self) -> bool {
        self.engine.session_open()
    }

    /// The value an `Escape` would restore.
    #[must_use]
    pub fn snapshot(&self) -> Option<&str> {
        self.engine.snapshot()
    }

    /// Whether the engine holds the keyboard.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.engine.is_focused()
    }

    /// The undo ledger.
    #[must_use]
    pub fn history(&self) -> &EditHistory {
        &self.history
    }

    /// How many sends have happened.
    #[must_use]
    pub const fn submits(&self) -> usize {
        self.submits
    }

    /// How many undo steps have been taken.
    #[must_use]
    pub const fn undos(&self) -> usize {
        self.undos
    }

    /// How many redo steps have been taken.
    #[must_use]
    pub const fn redos(&self) -> usize {
        self.redos
    }
}

/// Map a settling tier the engine produced onto its text-area arm.
fn map_settle(emission: TextInputEmission) -> TextAreaEmission {
    match emission {
        TextInputEmission::Committed(commit) => TextAreaEmission::Committed(commit),
        TextInputEmission::Cancelled(cancel) => TextAreaEmission::Cancelled(cancel),
        TextInputEmission::Cleared(commit) => TextAreaEmission::Committed(commit),
        TextInputEmission::Changed(live) => TextAreaEmission::Changed(live),
        TextInputEmission::FocusChanged(focused) => TextAreaEmission::FocusChanged(focused),
    }
}

#[cfg(test)]
#[path = "text_area_host_tests.rs"]
mod text_area_host_tests;
