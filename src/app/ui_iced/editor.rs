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

use std::fmt;
use std::path::{Path, PathBuf};

use iced::widget::{Column, Row, container, space, text, text_editor};
use iced::{Alignment, Element, Length};

use crate::markdown::{DocumentContent, MarkdownErrorKind, MarkdownParseError, validate_markdown};
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{EditorMessage, Message};
use crate::ui_iced::overlay::{ConfirmCancel, confirm_cancel_buttons, modal};

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

/// Default editor dialog width in points (UI-038).
pub const EDITOR_DIALOG_WIDTH: f32 = 800.0;

/// Default editor dialog height in points (UI-038).
pub const EDITOR_DIALOG_HEIGHT: f32 = 600.0;

/// Text size of the dialog title.
pub const EDITOR_TITLE_SIZE: f32 = 18.0;

/// Text size of the editing area.
pub const EDITOR_BODY_SIZE: f32 = 14.0;

/// Text size of the status bar and validation message.
pub const EDITOR_STATUS_SIZE: f32 = 12.0;

/// Vertical gap between the dialog's stacked sections.
pub const EDITOR_SECTION_SPACING: f32 = 8.0;

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
        EditorMessage::SaveRequested { .. } | EditorMessage::CancelRequested => false,
    }
}

/// The editor dialog card: title, validation message, monospace text area
/// and the status-bar / Save / Cancel footer (UI-038).
///
/// Rendered inside [`editor_modal`]; exposed separately so a caller can
/// mount the card in its own overlay arrangement.
#[must_use]
pub fn editor_card(vm: &EditorViewModel) -> Element<'_, Message> {
    let error_color = Palette::fastmd_dark().error;

    let title = text(strings::INLINE_EDITOR_WINDOW_TITLE).size(EDITOR_TITLE_SIZE);

    // Always allocated so the card's shape does not change between the valid
    // and invalid states.
    let validation: Element<'_, Message> = text(vm.validation_message().unwrap_or_default())
        .size(EDITOR_STATUS_SIZE)
        .style(move |_theme| text::Style {
            color: Some(error_color),
        })
        .into();

    let area = text_editor(&vm.content)
        .font(iced::Font::MONOSPACE)
        .size(EDITOR_BODY_SIZE)
        .height(Length::Fill)
        .placeholder(strings::EDITOR_INPUT_PLACEHOLDER)
        .on_action(|action| Message::Editor(EditorMessage::Action(action)));

    let buttons = confirm_cancel_buttons(strings::SAVE_BUTTON, vm.save_enabled(), |outcome| {
        Message::Editor(match outcome {
            ConfirmCancel::Confirm => EditorMessage::SaveRequested {
                path: vm.path().to_path_buf(),
            },
            ConfirmCancel::Cancel => EditorMessage::CancelRequested,
        })
    });

    // Save/Cancel on the left, the line/column read-out pushed to the right.
    // `Row` has no main-axis alignment knob, so a filling `Space` does it.
    let footer = Row::with_children(vec![
        buttons.into(),
        space::Space::new().width(Length::Fill).into(),
        text(vm.status_text()).size(EDITOR_STATUS_SIZE).into(),
    ])
    .spacing(EDITOR_SECTION_SPACING)
    .align_y(Alignment::Center);

    Column::with_children(vec![title.into(), validation, area.into(), footer.into()])
        .spacing(EDITOR_SECTION_SPACING)
        .width(Length::Fixed(EDITOR_DIALOG_WIDTH))
        .height(Length::Fixed(EDITOR_DIALOG_HEIGHT))
        .into()
}

/// The blocking editor modal (UI-038, UI-045).
///
/// [`editor_card`] centred over a scrim by
/// [`crate::ui_iced::overlay::modal`]. Clicking outside dismisses the editor
/// the same way Cancel does — both discard the buffer (UI-045); the reducer
/// decides what "discard" means for the overlay slot.
#[must_use]
pub fn editor_modal(vm: &EditorViewModel) -> Element<'_, Message> {
    let card = container(editor_card(vm))
        .width(Length::Shrink)
        .height(Length::Shrink);

    modal(card.into(), Message::Editor(EditorMessage::CancelRequested))
}
