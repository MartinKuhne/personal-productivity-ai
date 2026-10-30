//! Inline Markdown editor (UI-037..UI-045).
//!
//! A **pure view over plain state**: [`editor_modal`] renders an
//! [`EditorViewModel`] into an [`Element`] of [`Message`] and never touches
//! the bus, the disk or `FastMdApp`. The reducer owns the file write
//! (UI-044) and the overlay lifecycle.
//!
//! # Why the undo stack is ours
//!
//! `iced::widget::text_editor` has **no native undo/redo**. In iced 0.14 the
//! widget does not even mutate its own buffer: `TextEditor` reports every
//! interaction as an [`Action`] through `on_action`
//! (`iced_widget-0.14.2/src/text_editor.rs:212-222`) and the *application*
//! applies it with `Content::perform`
//! (`iced_widget-0.14.2/src/text_editor.rs:411-416`, and the crate's own
//! example at `text_editor.rs:28-30`). That callback is the interception
//! point, and [`EditorHistory`] is the stack.
//!
//! [`classify_action`] is the interception rule. It delegates to
//! `Action::is_edit()` — verified at
//! `iced_core-0.14.0/src/text/editor.rs:94-99`:
//!
//! ```text
//! pub fn is_edit(&self) -> bool { matches!(self, Self::Edit(_)) }
//! ```
//!
//! Only `Action::Edit(..)` changes content. `Move`, `Select`, `SelectWord`,
//! `SelectLine`, `SelectAll`, `Click`, `Drag` and `Scroll` move the caret,
//! the selection or the viewport and must never touch history — otherwise a
//! few arrow keys would burn the whole undo budget.
//!
//! # UI-042 — the history cap
//!
//! [`MIN_UNDO_STEPS`] is UI-042's floor of 100 undoable steps.
//! [`UNDO_CAPACITY`] stores one more snapshot than that — the current state
//! *plus* 100 previous ones — so a full 100-step undo is available at all
//! times rather than 99. Overflow evicts the oldest snapshot
//! ([`EditorHistory::push`]), and identical consecutive snapshots are
//! coalesced so a stream of no-op edits cannot flood the stack.
//!
//! # What iced `text_editor` gives us for free (verified)
//!
//! | Requirement | Native support |
//! | --- | --- |
//! | UI-039 selection | `Action::Select(Motion)` for Shift+arrows, `SelectWord` (double-click), `SelectLine` (triple-click), `SelectAll`; drag via `Action::Drag(Point)` (`iced_core-0.14.0/src/text/editor.rs:68-92`) |
//! | UI-040 clipboard | Copy/Cut/Paste arrive as `Action::Edit(Edit::Paste(Arc<str>))` / delete edits over the selection (`iced_core-0.14.0/src/text/editor.rs:101-118`); the widget owns the OS clipboard binding |
//! | UI-041 navigation | `Motion::{Left, Right, WordLeft, WordRight, Home, End, PageUp, PageDown, DocumentStart, DocumentEnd}` (`iced_core-0.14.0/src/text/editor.rs:119-147`), with `Motion::widen()` mapping Ctrl+Left→WordLeft, Ctrl+Home→DocumentStart |
//! | UI-038 monospace | `Font::MONOSPACE` (`iced_core-0.14.0/src/font.rs:27`) |
//!
//! What iced does **not** provide, and this module adds: undo/redo,
//! the front-matter-preserving save model (UI-037/UI-044), and pre-save GFM
//! validation (UI-043).
//!
//! # Validation (UI-043)
//!
//! Reuses [`crate::markdown::validate_markdown`], which parses with
//! [`crate::markdown::gfm_options`] — the *same* option set
//! `parse_markdown_to_events` renders with, so the two cannot drift.
//!
//! # Front matter (UI-037 / UI-044)
//!
//! Reuses [`crate::markdown::DocumentContent`], the existing split/join
//! helper: [`EditorViewModel::open`] parses the raw file into
//! `front_matter` + `body` and shows only the body; [`EditorViewModel::document`]
//! re-joins them verbatim so the reducer's write preserves the original
//! YAML exactly.
//!
//! # Known limitations
//!
//! * **Undo/redo resets the caret to the document start.** [`EditorHistory`]
//!   snapshots the *text* only. Extending snapshots to carry
//!   `iced::widget::text_editor::Cursor` would restore the caret alongside
//!   the text.
//! * **The status-bar column is iced's glyph index, not a Unicode character
//!   count.** `Content::cursor()` maps cosmic-text's `cursor.index`
//!   (`iced_graphics-0.14.0/src/text/editor.rs:277-303`), which under
//!   advanced shaping can differ from the character count for composed
//!   scripts. ASCII columns are exact.
//! * **Validation runs on every content-changing action**, not only at save
//!   time, because the Save button's enabled state (UI-043's "abort") has to
//!   be visible while the user types.
//!
//! Unit tests live in the sibling `editor_tests.rs` sidecar.

//! # The framework owns the box, the app owns the answer
//!
//! As of integration increment **INT-5b** this module is a consumer of
//! [`hoarfrost::surface::modal`] rather than a re-implementation of §4.6.
//! Nothing here carries a card measurement of its own:
//!
//! | Concern | Owner |
//! |---|---|
//! | Card width, height, gutter clamp | [`ModalSizing::editor`] via [`editor_size`] |
//! | Card surface, border, radius, padding | [`ModalFrame::resolve`] through [`crate::ui_iced::overlay::modal_card`] |
//! | Header / footer divider colour and weight | [`crate::ui_iced::overlay::modal_divider`] (`color.border.subtle`) |
//! | Title, buffer and status type; section gaps | `hoarfrost::token::{typography, space}` |
//! | Footer button geometry and states | [`hoarfrost::widget::button_widget::Button`] |
//! | Inverted canvas colours | [`EditorColors::inverted`] → `hoarfrost::token::color::EditorPalette::inverted` |
//! | Which buffer is open, what dismisses it | `OverlayState::editor` + `OverlayState::close_topmost` |
//!
//! [`hoarfrost::surface::modal::ModalHost`] is deliberately **not** used,
//! for the same reason [`crate::ui_iced::about`] and
//! [`crate::ui_iced::dialogs`] did not take it: the host self-registers
//! its surface into a
//! [`SurfaceRegistry`](hoarfrost::surface::zorder::SurfaceRegistry) and
//! pushes its own focus trap, while this app already owns
//! `SURFACE_ID_EDITOR` in
//! [`crate::ui_iced::app::OverlayState::sync_surfaces`] and owns
//! dismissal in [`crate::ui_iced::app::OverlayState::close_topmost`].
//! Two owners of one surface id is the collision INT-4 settled. What the
//! host *would* have guarded for us is guarded here instead, by
//! [`guard_leave`] — see the next section.
//!
//! # The unsaved-edit guard (`UIF-MOD-002`) — the real value of this increment
//!
//! Before this increment [`EditorViewModel::is_dirty`] existed and **no
//! dismiss path consulted it**: `Escape`, a scrim click and the `Cancel`
//! button all reached `close_editor` and dropped the buffer silently. The
//! spec's rule is that every user-initiated close routes through one
//! `attempt_dismiss` that refuses while `dirty` is set unless the user
//! confirmed or explicitly cancelled.
//!
//! The rule is [`guard_leave`], and it reads the gesture, not just the
//! flag:
//!
//! | Gesture | Clean buffer | Dirty buffer |
//! |---|---|---|
//! | [`LeaveGesture::Escape`] | closes | **refused** → discard confirmation |
//! | [`LeaveGesture::Scrim`] | closes | **refused** → discard confirmation |
//! | [`LeaveGesture::Cancel`] | closes | closes — this *is* the explicit discard |
//!
//! `Save` is absent from that table because it is not a guarded gesture: it
//! commits the buffer, so there is no work for it to throw away.
//!
//! The confirmation is not a new surface. It reuses the INT-4 small dialog
//! as [`DiscardEditorChanges`](crate::ui_iced::dialogs::DialogKind::DiscardEditorChanges),
//! whose control row is the same
//! [`InputControl::Confirmation`](hoarfrost::surface::input_dialog::InputControl::Confirmation)
//! the delete confirmation uses and whose
//! primary button is [`ButtonVariant::Danger`] labelled
//! [`strings::DISCARD_ACTION`]. Composing the two surfaces works because
//! the dialog is registered *above* the editor in the modal band
//! (`sync_surfaces` registers `SURFACE_ID_EDITOR` before
//! `SURFACE_ID_DIALOG`), so:
//!
//! * a dirty `Escape` leaves the editor mounted, consumes the key
//!   ([`UnwindAnswer::Unwound`](hoarfrost::surface::zorder::UnwindAnswer::Unwound)) and opens the confirmation centred over
//!   it;
//! * `Discard` closes the dialog **and** the buffer;
//! * `Cancel` closes only the dialog — the editor, and the edits in it,
//!   stay exactly where they were;
//! * a second `Escape` closes the confirmation, and a third asks again.
//!   Re-asking is the correct failure mode; silently discarding is not.
//!
//! # What this increment does NOT wire (`UIF-MOD-001`)
//!
//! **`UIF-MOD-001` (strict keyboard focus trap + restoration) is NOT
//! provided**, for the same reason INT-5a refused to claim it: the app
//! captures only `Escape` off its keyboard stream
//! (`ui_iced/subscription.rs`, `escape_to_message`) and leaves `Tab` to
//! iced's built-in router, which knows nothing about a trap. Nothing
//! would feed `FocusState::push_trap` the `Tab` events it needs, and the
//! editor's focusable set is never enumerated by the app, so pushing a
//! trap here would be a trap on paper only. Focus can still reach the
//! panes under the scrim.
//!
//! Unit tests live in the sibling `editor_tests.rs` sidecar.

use std::fmt;
use std::path::{Path, PathBuf};

use iced::widget::{Column, Row, Space, container, text, text_editor};
use iced::{Alignment, Element, Length, Size};

use hoarfrost::surface::modal::{ErrorLocation, ModalError, ModalFrame, ModalSizing, Recovery};
use hoarfrost::token::{shape, space, typography};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;

use crate::markdown::{DocumentContent, MarkdownErrorKind, MarkdownParseError, validate_markdown};
use crate::ui::strings;
use crate::ui::theme::{EditorColors, Palette};
use crate::ui_iced::message::{EditorMessage, Message};
use crate::ui_iced::overlay::{modal_card, modal_centered, modal_divider};

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;

/// UI-042's floor: the editor must retain **at least** this many undo steps.
pub const MIN_UNDO_STEPS: usize = 100;

/// Snapshots [`EditorHistory`] keeps — the current state plus
/// [`MIN_UNDO_STEPS`] previous ones.
///
/// One more than the floor on purpose: with `N` stored snapshots only `N - 1`
/// undo steps exist, so storing exactly 100 would ship 99 and miss UI-042.
pub const UNDO_CAPACITY: usize = MIN_UNDO_STEPS + 1;

/// §4.6.2 — the Inline Document Editor's sizing policy: 80% of the
/// viewport on both axes, clamped to `520..=960` wide and `380..=760`
/// tall.
///
/// Held as the policy object rather than two numbers, so the card states
/// *which* modal it is and the framework does the clamping. This replaced
/// the app's fixed `800 x 600`, which was the same size on a 1024x768
/// laptop and a 3840x2160 monitor.
pub const EDITOR_SIZING: ModalSizing = ModalSizing::editor();

/// Stable id of the editor card box itself, so a layout test can address
/// the rendered card rather than only its contents (AGENTS §5).
pub const EDITOR_CARD_ID: &str = "editor.card";

/// Stable id of the editing surface.
///
/// AGENTS §6 names this id as the one that must survive every rebuild:
/// the `text_editor` widget is mounted under it unconditionally, so the
/// caret and selection are never dropped by a branch in the card. It is
/// also the control [`Recovery::FocusControl`] on a validation error
/// points at.
pub const EDITOR_TEXT_ID: &str = "editor.buffer";

/// Stable focus id of the footer `Save` button.
pub const EDITOR_SAVE_BUTTON_ID: &str = "editor.save";

/// Stable focus id of the footer `Cancel` button.
pub const EDITOR_CANCEL_BUTTON_ID: &str = "editor.cancel";

/// Stable focus id of the error banner's recovery button.
pub const EDITOR_RECOVERY_BUTTON_ID: &str = "editor.recovery";

/// `NFR-004` grouping code for a blocking Markdown validation failure.
pub const EDITOR_VALIDATION_ERROR_CODE: &str = "EDITOR-4001";

/// `NFR-004` grouping code for a save that could not be queued.
pub const EDITOR_SAVE_ERROR_CODE: &str = "EDITOR-5001";

/// The gesture the user used to try to leave the editor (`UIF-MOD-002`).
///
/// The guard has to read *which* gesture it is being asked about, not just
/// whether the buffer is dirty: `Cancel` is the user saying "throw it
/// away" in as many words, while `Escape` and a scrim click are reflexes
/// that must never be able to do that. Collapsing the three into one
/// "dismiss" message is exactly how the pre-INT-5b editor came to lose
/// work on a keypress.
///
/// `Save` is deliberately absent: it commits the buffer, so there is no
/// work for it to throw away and nothing for the guard to refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveGesture {
    /// The `Escape` key, arriving through
    /// [`crate::ui_iced::app::OverlayState::close_topmost`].
    Escape,
    /// A click on the scrim, outside the card.
    Scrim,
    /// The footer `Cancel` button — the explicit discard.
    Cancel,
}

impl LeaveGesture {
    /// The framework's own word for this gesture, so the app's guard and
    /// the framework's `attempt_dismiss` shape speak the same language.
    ///
    /// `Cancel` maps to
    /// [`DismissCause::Programmatic`](hoarfrost::surface::modal::DismissCause::Programmatic)
    /// because that is the cause the framework's `ModalHost` treats as
    /// "the caller decided, `UIF-MOD-002` does not apply" — which is
    /// precisely what an explicit `Cancel` is.
    #[must_use]
    pub const fn dismiss_cause(self) -> hoarfrost::surface::modal::DismissCause {
        match self {
            Self::Escape => hoarfrost::surface::modal::DismissCause::Escape,
            Self::Scrim => hoarfrost::surface::modal::DismissCause::Scrim,
            Self::Cancel => hoarfrost::surface::modal::DismissCause::Programmatic,
        }
    }
}

/// What [`guard_leave`] decided about one attempt to leave the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveDecision {
    /// The editor may close.
    Proceed,
    /// Unsaved edits block this gesture; the caller must ask the user first.
    ///
    /// The refusal is terminal for this gesture — the caller consumes it
    /// rather than letting it travel on and close something else.
    AskBeforeDiscarding,
}

/// The whole of `UIF-MOD-002` for the inline editor, as a pure function.
///
/// Pure so the rule can be tested against every gesture without a widget
/// tree, a runtime, or a file on disk — and so *both* dismiss paths (the
/// `Escape` walk in `OverlayState` and the scrim message in the reducer)
/// are provably answering the same question with the same function rather
/// than each re-deriving it.
///
/// * An explicit [`LeaveGesture::Cancel`] always proceeds: that button is
///   the escape hatch the requirement names.
/// * `Escape` and `Scrim` proceed only while the buffer is clean. With
///   unsaved edits they are refused, and the caller raises the discard
///   confirmation instead of closing.
#[must_use]
pub fn guard_leave(dirty: bool, gesture: LeaveGesture) -> LeaveDecision {
    match gesture {
        LeaveGesture::Cancel => LeaveDecision::Proceed,
        LeaveGesture::Escape | LeaveGesture::Scrim if dirty => LeaveDecision::AskBeforeDiscarding,
        LeaveGesture::Escape | LeaveGesture::Scrim => LeaveDecision::Proceed,
    }
}

/// Whether an iced [`Action`] changes buffer content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    /// A content-changing edit — [`EditorHistory::push`] a snapshot.
    ContentChange,
    /// A caret, selection or viewport action — history is untouched.
    CaretOnly,
}

/// Classifies an [`Action`] for the undo interception rule.
///
/// Delegates to `Action::is_edit()` (`iced_core-0.14.0/src/text/editor.rs:94-99`):
/// `Action::Edit(..)` is the only content-changing variant, so every other
/// variant — `Move`, `Select`, `SelectWord`, `SelectLine`, `SelectAll`,
/// `Click`, `Drag`, `Scroll` — is [`ActionKind::CaretOnly`].
#[must_use]
pub fn classify_action(action: &Action) -> ActionKind {
    if action.is_edit() {
        ActionKind::ContentChange
    } else {
        ActionKind::CaretOnly
    }
}

/// The iced text-editor action type the editor intercepts.
pub type Action = iced::widget::text_editor::Action;

/// The iced text-editor cursor type.
pub type EditorCursor = iced::widget::text_editor::Cursor;

/// Turns an iced cursor into the 1-based `(line, column)` the status bar shows
/// (UI-038).
///
/// iced reports both as 0-based (`iced_core-0.14.0/src/text/editor.rs:209-216`);
/// users count from 1. See the module docs for the glyph-index caveat.
#[must_use]
pub fn display_line_column(cursor: EditorCursor) -> (usize, usize) {
    (cursor.position.line + 1, cursor.position.column + 1)
}

/// A bounded linear undo / redo history of text snapshots ([UI-042]).
///
/// The pure core of the editor: no iced types, no I/O, no clock. `cursor`
/// indexes the snapshot that is the buffer's *current* state, so undo and
/// redo are index moves rather than a second stack.
///
/// Invariants:
/// * `snapshots.len() <= capacity`
/// * `cursor < snapshots.len()` whenever the history is non-empty
/// * pushing a snapshot equal to the current one changes nothing
/// * pushing after an undo drops the redo tail
#[derive(Clone, Debug)]
pub struct EditorHistory {
    snapshots: Vec<String>,
    cursor: usize,
    capacity: usize,
}

impl Default for EditorHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorHistory {
    /// An empty history with [`UNDO_CAPACITY`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(UNDO_CAPACITY)
    }

    /// An empty history holding at most `capacity` snapshots.
    ///
    /// A `capacity` of `0` is raised to `1`: a history that cannot hold a
    /// snapshot is not a history, and silently doing nothing would be worse.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            cursor: 0,
            capacity: capacity.max(1),
        }
    }

    /// Records `snapshot` as the new current state.
    ///
    /// * A snapshot equal to the current one is **coalesced** — no entry is
    ///   added and the redo tail survives, so no-op edits cannot flood the
    ///   stack or destroy redo capability.
    /// * Otherwise the redo tail is discarded and the snapshot is pushed.
    /// * Over capacity, the **oldest** snapshot is evicted, so the cap holds
    ///   while the most recent [`UNDO_CAPACITY`] states stay reachable.
    pub fn push(&mut self, snapshot: impl Into<String>) {
        let snapshot = snapshot.into();

        if self.current() == Some(snapshot.as_str()) {
            return;
        }

        // Drop the redo branch: a new edit invalidates everything ahead.
        self.snapshots.truncate(self.cursor + 1);
        self.snapshots.push(snapshot);
        self.cursor = self.snapshots.len() - 1;

        if self.snapshots.len() > self.capacity {
            let evicted = self.snapshots.len() - self.capacity;
            self.snapshots.drain(..evicted);
            self.cursor -= evicted;
        }
    }

    /// The snapshot the buffer currently holds, `None` when empty.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.snapshots.get(self.cursor).map(String::as_str)
    }

    /// Whether [`Self::undo`] would return a snapshot.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Whether [`Self::redo`] would return a snapshot.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.snapshots.len()
    }

    /// Step one state back and return the text to restore, or `None` at the
    /// oldest state.
    pub fn undo(&mut self) -> Option<&str> {
        if !self.can_undo() {
            return None;
        }
        self.cursor -= 1;
        self.current()
    }

    /// Step one state forward and return the text to restore, or `None` at
    /// the newest state.
    pub fn redo(&mut self) -> Option<&str> {
        if !self.can_redo() {
            return None;
        }
        self.cursor += 1;
        self.current()
    }

    /// How many snapshots are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether nothing has been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// The configured snapshot ceiling.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Forget everything and reset the position.
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.cursor = 0;
    }
}

/// The inline editor's view model: the buffer, its undo history, the file it
/// belongs to and the preserved front matter.
///
/// Owns the iced [`text_editor::Content`] because that *is* the text buffer
/// — keeping a second `String` beside it would be a second source of truth.
/// History snapshots are `String`s taken from `Content::text()` at each
/// content-changing action.
pub struct EditorViewModel {
    /// File being edited (UI-044's write target).
    path: PathBuf,
    /// The live iced editing surface holding the body (UI-037).
    content: text_editor::Content,
    /// Undo / redo snapshots of the body (UI-042).
    history: EditorHistory,
    /// The original YAML front matter, verbatim (UI-037).
    front_matter: Option<String>,
    /// The body as opened, for the dirty check.
    original_body: String,
    /// 1-based cursor line for the status bar (UI-038).
    line: usize,
    /// 1-based cursor column for the status bar (UI-038).
    column: usize,
    /// The blocking validation failure, if any (UI-043).
    validation_error: Option<MarkdownParseError>,
    /// A save that failed and is still being shown (`UIF-MOD-003`).
    ///
    /// Sticky rather than derived: unlike the validation error, which is
    /// recomputed from the buffer every keystroke, a refused save is a fact
    /// about the *world* that stays true until the user retries. It clears
    /// on a successful queue, on [`LeaveGesture::Cancel`], and when a new
    /// error supersedes it — never on its own.
    save_error: Option<ModalError>,
}

impl fmt::Debug for EditorViewModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EditorViewModel")
            .field("path", &self.path)
            .field("text_len", &self.content.text().len())
            .field("front_matter", &self.front_matter)
            .field("line", &self.line)
            .field("column", &self.column)
            .field("validation_error", &self.validation_error)
            .field("save_error", &self.save_error)
            .field("history", &self.history)
            .finish()
    }
}

impl EditorViewModel {
    /// Opens `raw_content` for editing `path` (UI-037).
    ///
    /// Splits the file with [`DocumentContent::parse`] so only the Markdown
    /// body reaches the text area and the YAML front matter is cached
    /// verbatim. The opened body is seeded into the history as the baseline,
    /// which is what makes the *first* keystroke undoable.
    #[must_use]
    pub fn open(path: impl Into<PathBuf>, raw_content: &str) -> Self {
        let document = DocumentContent::parse(raw_content);
        let body = document.body;

        let mut history = EditorHistory::new();
        history.push(body.clone());

        let mut vm = Self {
            path: path.into(),
            content: text_editor::Content::with_text(&body),
            history,
            front_matter: document.front_matter,
            original_body: body,
            line: 1,
            column: 1,
            validation_error: None,
            save_error: None,
        };
        vm.refresh_cursor();
        vm.revalidate();
        vm
    }

    /// The file this buffer writes back to (UI-044).
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The body currently in the text area (UI-037).
    #[must_use]
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// The preserved YAML front matter, verbatim (UI-037).
    #[must_use]
    pub fn front_matter(&self) -> Option<&str> {
        self.front_matter.as_deref()
    }

    /// The buffer re-joined with its original front matter, ready to write
    /// (UI-044). The reducer performs the actual write.
    #[must_use]
    pub fn document(&self) -> DocumentContent {
        DocumentContent {
            front_matter: self.front_matter.clone(),
            body: self.text(),
        }
    }

    /// 1-based cursor line (UI-038).
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// 1-based cursor column (UI-038).
    #[must_use]
    pub fn column(&self) -> usize {
        self.column
    }

    /// The status-bar text (UI-038).
    #[must_use]
    pub fn status_text(&self) -> String {
        strings::build_editor_status_text(self.line, self.column)
    }

    /// The validation failure blocking a save, if any (UI-043).
    #[must_use]
    pub fn validation_error(&self) -> Option<MarkdownParseError> {
        self.validation_error
    }

    /// The user-facing validation message, built from
    /// [`crate::ui::strings`], or `None` when the body is valid.
    #[must_use]
    pub fn validation_message(&self) -> Option<String> {
        let error = self.validation_error?;
        Some(strings::build_editor_validation_error(
            validation_reason(error.kind),
            error.line,
            error.column,
        ))
    }

    /// The save failure the banner is showing, if any (`UIF-MOD-003`).
    #[must_use]
    pub fn save_error(&self) -> Option<&ModalError> {
        self.save_error.as_ref()
    }

    /// Show a save failure, superseding whatever was showing.
    ///
    /// Returns what it replaced, mirroring
    /// [`ModalHost::set_error`](hoarfrost::surface::modal::ModalHost::set_error):
    /// the banner changes by exactly two operations, this and
    /// [`clear_save_error`](Self::clear_save_error).
    pub fn set_save_error(&mut self, error: ModalError) -> Option<ModalError> {
        self.save_error.replace(error)
    }

    /// Record that a save could not be queued, in the app's own words.
    ///
    /// Returns whether the banner **changed**: showing a refusal where none
    /// was showing is a change, re-noting the same refusal is not. That is
    /// the contract [`ModalHost::set_error`](hoarfrost::surface::modal::ModalHost::set_error)
    /// keeps, and it is what lets a reducer tell a refusal worth repainting
    /// from a repeat of one already on screen.
    pub fn note_save_failed(&mut self) -> bool {
        let was_showing = self.save_error.is_some();
        self.save_error = Some(save_queue_error());
        !was_showing
    }

    /// Clear the save banner; returns what it removed.
    pub fn clear_save_error(&mut self) -> Option<ModalError> {
        self.save_error.take()
    }

    /// The structured failure the card's error slot renders (`UIF-MOD-003`).
    ///
    /// One slot, two sources, with a stated precedence: a **save** failure
    /// outranks a validation failure, because the user has already tried
    /// to commit and been refused, and that is the more recent fact about
    /// what went wrong. In practice the two are mutually exclusive — a
    /// buffer that fails validation never reaches the save path — so the
    /// ordering is a tie-break rule, not a common case.
    ///
    /// The validation arm carries [`ErrorLocation::Position`] with the
    /// 1-based line and column the parser reported, and
    /// [`Recovery::FocusControl`] pointing at [`EDITOR_TEXT_ID`]: what the
    /// user must do is go fix that line in the buffer.
    #[must_use]
    pub fn modal_error(&self) -> Option<ModalError> {
        if let Some(error) = self.save_error.as_ref() {
            return Some(error.clone());
        }
        let error = self.validation_error?;
        Some(
            ModalError::new(
                EDITOR_VALIDATION_ERROR_CODE,
                strings::build_editor_validation_error(
                    validation_reason(error.kind),
                    error.line,
                    error.column,
                ),
            )
            .at(ErrorLocation::Position {
                line: error.line as u32,
                column: error.column as u32,
            })
            .with_recovery(Recovery::FocusControl(hoarfrost::focus::FocusId::new(
                EDITOR_TEXT_ID,
            ))),
        )
    }

    /// Whether a save may proceed — `false` while a validation failure is
    /// outstanding (UI-043 aborts the save).
    #[must_use]
    pub fn save_enabled(&self) -> bool {
        self.validation_error.is_none()
    }

    /// Whether the body differs from what was opened.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.text() != self.original_body
    }

    /// Whether [`Self::undo`] can do anything (UI-042).
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Whether [`Self::redo`] can do anything (UI-042).
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The buffer's undo history.
    #[must_use]
    pub fn history(&self) -> &EditorHistory {
        &self.history
    }

    /// Applies one intercepted [`Action`] to the buffer.
    ///
    /// iced never mutates `Content` itself, so `perform` is what actually
    /// edits the text. A content-changing action additionally records the
    /// new snapshot and re-runs UI-043 validation; a caret/selection/viewport
    /// action only refreshes the status-bar position.
    ///
    /// Returns how the action was classified, so the reducer can log it as a
    /// discrete interaction.
    pub fn apply_action(&mut self, action: Action) -> ActionKind {
        let kind = classify_action(&action);
        self.content.perform(action);
        self.refresh_cursor();

        if kind == ActionKind::ContentChange {
            let text = self.text();
            self.validation_error = validate_markdown(&text).err();
            self.history.push(text);
        }

        kind
    }

    /// Restores the previous snapshot (UI-042). Returns whether anything
    /// changed.
    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.history.undo() else {
            return false;
        };
        let snapshot = snapshot.to_owned();
        self.restore(&snapshot);
        true
    }

    /// Re-applies the next redo snapshot (UI-042). Returns whether anything
    /// changed.
    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.history.redo() else {
            return false;
        };
        let snapshot = snapshot.to_owned();
        self.restore(&snapshot);
        true
    }

    /// Replaces the editing surface with `text` and refreshes everything
    /// derived from it.
    ///
    /// `Content` has no in-place setter, so undo/redo rebuild it — which is
    /// also why the caret lands at the document start (see the module docs).
    fn restore(&mut self, text: &str) {
        self.content = text_editor::Content::with_text(text);
        self.refresh_cursor();
        self.revalidate();
    }

    /// Re-reads the cursor into the 1-based status-bar fields (UI-038).
    fn refresh_cursor(&mut self) {
        let (line, column) = display_line_column(self.content.cursor());
        self.line = line;
        self.column = column;
    }

    /// Re-runs UI-043 validation over the current body.
    fn revalidate(&mut self) {
        self.validation_error = validate_markdown(&self.text()).err();
    }
}

/// The user-facing label for a [`MarkdownErrorKind`].
#[must_use]
pub fn validation_reason(kind: MarkdownErrorKind) -> &'static str {
    match kind {
        MarkdownErrorKind::UnterminatedCodeFence => strings::EDITOR_ERR_UNTERMINATED_FENCE,
    }
}

/// Reduces an [`EditorMessage`] onto `vm` — the pure entry point the
/// application reducer delegates to.
///
/// Pure: no file I/O, no bus publish, no overlay mutation. Returns whether
/// the buffer changed, which is all the caller needs to decide on a
/// re-render.
///
/// * [`EditorMessage::Action`] — the undo interception; applied through
///   [`EditorViewModel::apply_action`].
/// * [`EditorMessage::Undo`] / [`EditorMessage::Redo`] — replay one snapshot
///   on the stack, but **only** when the message names this buffer's file.
///   The path travels with the intent, so a message aimed at another tab is
///   ignored rather than corrupting this buffer.
/// * [`EditorMessage::SaveRequested`] / [`EditorMessage::CancelRequested`] —
///   never mutate the buffer here. The reducer performs the write (UI-044)
///   and closes the overlay (UI-045); the view only *emits* them.
#[must_use]
pub fn apply_editor_message(vm: &mut EditorViewModel, message: &EditorMessage) -> bool {
    match message {
        EditorMessage::Action(action) => {
            vm.apply_action(action.clone());
            true
        }
        EditorMessage::Undo { path } if path == vm.path() => vm.undo(),
        EditorMessage::Redo { path } if path == vm.path() => vm.redo(),
        EditorMessage::Undo { .. } | EditorMessage::Redo { .. } => false,
        EditorMessage::SaveRequested { .. }
        | EditorMessage::CancelRequested
        | EditorMessage::DismissRequested { .. }
        | EditorMessage::Absorbed
        | EditorMessage::RecoveryRequested { .. } => false,
    }
}

/// The editor card's **outer** size for a viewport (§4.6.2).
///
/// Both axes come from [`EDITOR_SIZING`], so the card is viewport-
/// responsive instead of the fixed `800 x 600` it replaced: 80% of the
/// viewport, floored at `520 x 380` and capped at `960 x 760`, with the
/// `VIEWPORT_GUTTER` (`space::XXL`) clamp applied last by the framework.
///
/// Exposed as a function of the viewport so the clamp behaviour is
/// measurable at any window size rather than only asserted at one.
#[must_use]
pub fn editor_size(viewport: Size) -> Size {
    Size::new(
        EDITOR_SIZING.width.resolve(viewport.width, 0.0),
        EDITOR_SIZING.height.resolve(viewport.height, 0.0),
    )
}

/// The structured failure for a save that could not be queued
/// (`UIF-MOD-003`, `NFR-004` code [`EDITOR_SAVE_ERROR_CODE`]).
///
/// `Recovery::Retry` because the refusal is transient — the worker's
/// submission channel, not the document, is what said no.
#[must_use]
pub fn save_queue_error() -> ModalError {
    ModalError::new(EDITOR_SAVE_ERROR_CODE, strings::EDITOR_SAVE_QUEUE_ERROR)
        .with_recovery(Recovery::Retry)
}

/// The editor dialog card: title, inverted monospace text area, the
/// structured error slot, and the status-bar / Save / Cancel footer
/// (UI-038, §4.6.1).
///
/// Layout, with the §4.6.1 anatomy and the framework's own frame:
///
/// ```text
/// ┌─ editor card · ModalSizing::editor() · surface.overlay · radius 6 ─┐
/// │ Inline Editor                                                      │
/// │ ──────────────────────────────────────────────────────────────── │
/// │ ┌ inverted canvas (white, black monospace, black hairline) ──────┐│
/// │ └────────────────────────────────────────────────────────────────┘│
/// │ ──────────────────────────────────────────────────────────────── │
/// │ [ Markdown validation failed: … (line 3, column 1)   [ Fix line 3 ]│
/// │ [ Save ] [ Cancel ]                        Line: 1 | Col: 1        │
/// └──────────────────────────────────────────────────────────────────┘
/// ```
///
/// The error row is **conditional on the view-model** (AGENTS §6): the
/// card's outer size is fixed by the sizing policy, so adding or removing
/// the banner moves the canvas edge rather than resizing the card, and no
/// empty placeholder is needed to hold the shape open.
///
/// Rendered inside [`editor_modal`]; exposed separately so a caller can
/// mount the card in its own overlay arrangement.
#[must_use]
pub fn editor_card(vm: &EditorViewModel, viewport: Size) -> Element<'_, Message> {
    let frame = ModalFrame::resolve(&Palette::pro());
    let size = editor_size(viewport);

    let header = Row::with_children(vec![
        rung(
            strings::INLINE_EDITOR_WINDOW_TITLE,
            typography::TITLE_SM,
            Palette::fastmd_dark().text,
        )
        .into(),
    ])
    .align_y(Alignment::Center);

    let mut sections: Vec<Element<'_, Message>> = vec![
        header.into(),
        modal_divider(frame).into(),
        editor_area(vm),
        modal_divider(frame).into(),
    ];

    if let Some(error) = vm.modal_error() {
        sections.push(error_banner(&error, vm.path()));
    }

    sections.push(editor_footer(vm));

    modal_card(
        Column::with_children(sections).spacing(space::SM),
        frame,
        EDITOR_CARD_ID,
        Length::Fixed(size.width),
        Length::Fixed(size.height),
    )
    .into()
}

/// The inverted editing canvas (UI-038, UI-046).
///
/// Mounted under [`EDITOR_TEXT_ID`] **unconditionally** — no branch here
/// may take the `text_editor` out of the tree, because a rebuilt `Content`
/// identity loses the caret (AGENTS §6). Only its *appearance* changes.
///
/// The canvas is the inverted white surface `color.editor.inverted.*`
/// describes, not the dark card it sits in: raw editing on white, reading
/// in the dark.
fn editor_area(vm: &EditorViewModel) -> Element<'_, Message> {
    let colors = EditorColors::inverted();

    text_editor(&vm.content)
        .id(EDITOR_TEXT_ID)
        .font(iced::Font::MONOSPACE)
        .size(typography::BODY.size)
        .line_height(typography::BODY.iced_line_height())
        .placeholder(strings::EDITOR_INPUT_PLACEHOLDER)
        .height(Length::Fill)
        .style(
            move |_theme: &iced::Theme, _status: text_editor::Status| text_editor::Style {
                background: iced::Background::Color(colors.background),
                border: iced::Border {
                    color: colors.border,
                    width: shape::BORDER_HAIRLINE,
                    radius: shape::RADIUS_WIDGET.into(),
                },
                placeholder: colors.placeholder,
                value: colors.text,
                selection: colors.selection,
            },
        )
        .on_action(|action| Message::Editor(EditorMessage::Action(action)))
        .into()
}

/// The §4.6.1 inline status / error banner (`UIF-MOD-003`).
///
/// Painted on the **inverted canvas colour** rather than the dark card,
/// because `color.editor.inverted.err` is a red specified for white:
/// `RGB(180, 0, 0)` clears 4.5:1 on white and would not on
/// `surface.overlay`. Putting the banner on the editor's own surface is
/// what makes that token do the job its declaration claims.
///
/// The row carries the three things the requirement names — what failed
/// (the message), where (the location is already interpolated into the
/// message by [`strings::build_editor_validation_error`]), and what the
/// user can do about it (the recovery button, labelled from
/// [`Recovery`]).
fn error_banner(error: &ModalError, path: &Path) -> Element<'static, Message> {
    let colors = EditorColors::inverted();
    let message = rung(error.message(), typography::CAPTION, colors.error).width(Length::Fill);

    let recovery = footer_button(
        ButtonVariant::Danger,
        recovery_label(error),
        Message::Editor(EditorMessage::RecoveryRequested {
            path: path.to_path_buf(),
            recovery: error.recovery().clone(),
        }),
        EDITOR_RECOVERY_BUTTON_ID,
        true,
    );

    container(Row::with_children(vec![message.into(), recovery]).spacing(space::SM))
        .width(Length::Fill)
        .padding(space::XS)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(colors.background)),
            border: iced::Border {
                color: colors.border,
                width: shape::BORDER_HAIRLINE,
                radius: shape::RADIUS_WIDGET.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// The label the recovery button wears, read off the error's
/// [`Recovery`] affordance.
///
/// The button says what pressing it *does* — "Retry" re-runs the failed
/// save, "Fix line 3" moves the user to the offending line — rather than
/// showing a generic acknowledgement that would claim nothing.
#[must_use]
pub fn recovery_label(error: &ModalError) -> String {
    match error.recovery() {
        Recovery::Retry => strings::EDITOR_RETRY_LABEL.to_owned(),
        Recovery::FocusControl(_) => match error.location() {
            Some(ErrorLocation::Position { line, .. }) => {
                strings::format_editor_recovery_label(*line)
            }
            Some(ErrorLocation::Region(_)) | None => strings::EDITOR_FIX_LABEL.to_owned(),
        },
        Recovery::Acknowledge => strings::CLOSE_BUTTON.to_owned(),
    }
}

/// The card footer: the line/column read-out on the left and the
/// right-aligned `[Cancel] [Save]` action group.
///
/// In conformance with the action-row ordering rule (`UIF-PRIN-006`),
/// `Cancel` is `Secondary` and precedes `Save` (`Primary`). Neither is
/// `Danger` — discarding here is *not* the destructive act the Danger
/// variant is reserved for, and the framework keeps that word for the
/// discard **confirmation**, where the user is actually being asked to
/// give work up.
///
/// `Save` is disabled rather than removed while validation blocks it
/// (AGENTS §6), so the control stays visible and the reason stays in the
/// banner above it.
fn editor_footer(vm: &EditorViewModel) -> Element<'_, Message> {
    let save = footer_button(
        ButtonVariant::Primary,
        strings::SAVE_BUTTON,
        Message::Editor(EditorMessage::SaveRequested {
            path: vm.path().to_path_buf(),
        }),
        EDITOR_SAVE_BUTTON_ID,
        vm.save_enabled(),
    );

    let cancel = footer_button(
        ButtonVariant::Secondary,
        strings::CANCEL_BUTTON,
        Message::Editor(EditorMessage::CancelRequested),
        EDITOR_CANCEL_BUTTON_ID,
        true,
    );

    let status = rung(
        vm.status_text(),
        typography::CAPTION,
        Palette::fastmd_dark().text,
    );

    let actions = Row::with_children(vec![cancel, save])
        .spacing(space::SM)
        .align_y(Alignment::Center);

    // `Row` has no main-axis alignment knob, so a filling `Space` pushes the
    // secondary/primary action group to the right while the readout stays left.
    Row::with_children(vec![
        status.into(),
        Space::new().width(Length::Fill).into(),
        actions.into(),
    ])
    .spacing(space::SM)
    .align_y(Alignment::Center)
    .into()
}

/// A footer-width framework button carrying `label`.
///
/// The label is mounted in a fixed
/// [`BUTTON_MIN_WIDTH`](hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH)
/// box on purpose. `Button::is_icon_only` is a **geometric** test — any
/// content no wider and no taller than `size::CONTROL_SM` (28px) is
/// classed icon-only, and an icon-only button without a tooltip trips a
/// `debug_assert!` in `outer_size` (`button_widget.rs:776`). A short
/// label like `'Save'` sits close enough to that line that a different
/// font measurement would cross it, so the label is given a width the
/// test cannot fail.
fn footer_button(
    variant: ButtonVariant,
    label: impl Into<String>,
    on_press: Message,
    focus_id: &'static str,
    enabled: bool,
) -> Element<'static, Message> {
    let content = Column::new()
        .width(Length::Fixed(
            hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH,
        ))
        .align_x(Alignment::Center)
        .push(rung(
            label,
            typography::BODY,
            Palette::fastmd_dark().text_strong,
        ));

    let button = Button::new(variant, content, on_press, focus_id).palette(Palette::pro());

    if enabled {
        button.into()
    } else {
        button.disabled().into()
    }
}

/// Paints `value` on a type rung at its resting weight.
///
/// The single place this card turns a rung into a `text` widget, so no
/// call site restates a point size ([RUST-021], `UIF-TOK-004`).
///
/// **`line_height` must be handed to iced as `Pixels`, not as a bare
/// `f32`.** `impl From<f32> for iced::LineHeight` produces
/// `LineHeight::Relative(factor)` — a *multiple* of the font size
/// (`iced_core-0.14.0/src/text.rs:221-225`) — so `.line_height(20.0)`
/// on a 14px rung means 20 x 14 = **280px per line**, not 20px. Only
/// `From<Pixels>` reaches `LineHeight::Absolute`. [`TypeRung::iced_line_height`]
/// is the crossing point that makes picking the wrong unit impossible.
fn rung(
    value: impl Into<String>,
    rung: typography::TypeRung,
    color: iced::Color,
) -> text::Text<'static> {
    text(value.into())
        .size(rung.size)
        .line_height(rung.iced_line_height())
        .color(color)
}

/// The blocking editor modal (UI-038, UI-045).
///
/// [`editor_card`] centred over a scrim by
/// [`crate::ui_iced::overlay::modal_centered`], which supplies the
/// card-boundary absorption as well as the scrim itself.
///
/// # What each click outside the card means (`UIF-MOD-002`)
///
/// The scrim emits [`EditorMessage::DismissRequested`] with
/// [`LeaveGesture::Scrim`], **not** [`EditorMessage::CancelRequested`].
/// That distinction is the guard: a scrim click on a clean buffer closes
/// the editor exactly as it always did, but on a dirty one it is refused
/// and the discard confirmation is raised instead. Before this increment
/// the scrim sent `CancelRequested` directly, which is one of the three
/// ways the editor used to lose unsaved work without asking.
///
/// A press that lands on the card's padding ring, on a disabled `Save`,
/// or on dead space between rows is absorbed by
/// [`EditorMessage::Absorbed`] and never reaches the scrim
/// (`UIF-VER-003` / `UIF-INP-002`).
#[must_use]
pub fn editor_modal(vm: &EditorViewModel, viewport: Size) -> Element<'_, Message> {
    modal_centered(
        editor_card(vm, viewport),
        Message::Editor(EditorMessage::Absorbed),
        Message::Editor(EditorMessage::DismissRequested {
            cause: LeaveGesture::Scrim,
        }),
    )
}
