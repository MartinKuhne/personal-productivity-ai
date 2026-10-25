//! Multi-line model for `UIF-ATOM-002e`: the cursor, the growth solver,
//! the scroll extent and the undo ledger.
//!
//! # What is pure here
//!
//! Nothing in this file renders, reads a clock, or touches a filesystem.
//! Every operation is a function of the model and its arguments, which
//! is what lets the three things §3.2 actually specifies about a text
//! area — how tall it is, where the cursor is, whether it has scrolled
//! — be asserted without a window.
//!
//! # The growth solver is arithmetic on tokens
//!
//! §3.2e states the box model as a closed form:
//! `height = clamp(line_count, min_rows, max_rows) × line_height + 2 × space.sm`,
//! with the vertical padding spelled out as `8px` top plus `8px` bottom
//! and `line_height` taken from a type rung. Worked at `type.body` — a
//! `14px` face on a `20px` line — the spec's own example is
//! `3 × 20 + 16 = 76px`, and that is what [`Growth`] produces.
//!
//! The solver reads the tokens rather than the printed total, which is
//! what keeps the total and its parts from ever disagreeing: the `76` is
//! not a number this file knows, it is what `BODY.line_height` and
//! `space::SM` add up to. An earlier reading of this section printed
//! `72px`, which no combination of those two tokens reaches; the
//! corrected section prints `76`, and the arithmetic now closes.
//!
//! §3.2e permits two line rungs — `20px` (`type.body`) and `18px`
//! (`type.ui_compact`) — and this solver is built on the body rung only.
//! That is a permitted choice rather than a gap in the arithmetic: the
//! two rungs give `76px` and `70px` for a three-row well, and the
//! compact alternative is recorded as unavailable here rather than
//! silently averaged into a third figure.
//!
//! # Undo is ours because iced has none
//!
//! §3.2 asks for `on_undo()` / `on_redo()`. Iced's `text_editor` has no
//! undo: `iced_core::text::editor::Action` carries insert, delete and
//! movement and nothing else, and there is no history anywhere in the
//! editor's state. So [`EditHistory`] is a framework-side ledger of
//! whole-value snapshots. It records *before* states rather than diffs,
//! which is what makes an undo a single assignment instead of an
//! inverse-operation replay that has to be correct for every edit type.
//!
//! Unit tests live in the sibling `text_area_model_tests.rs` sidecar.

use std::fmt;

use crate::binding::Cancel;
use crate::binding::Commit;
use crate::binding::Live;
use crate::token::space;
use crate::token::typography;
use crate::validation::disallowed::DisallowedClass;
use crate::validation::punish::{Punishment, punish};
use crate::validation::stance::EditStance;
use crate::widget::text_input_model::TextRange;

/// A position in a multi-line buffer.
///
/// `line` and `col` are both zero-based. `col` counts **characters**, not
/// bytes and not graphemes: it is the column a user reads, and the two
/// disagree with the byte offset only for multi-byte text, where a byte
/// column would be visibly wrong.
///
/// The conversion helpers take the text as an argument rather than
/// storing a buffer, so a `LineCol` stays a plain coordinate that can be
/// compared, defaulted and moved without owning anything.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineCol {
    /// The zero-based line.
    pub line: u32,
    /// The zero-based column within that line.
    pub col: u32,
}

impl LineCol {
    /// A position at `line`, `col`.
    #[must_use]
    pub const fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }

    /// The very start of the buffer.
    #[must_use]
    pub const fn origin() -> Self {
        Self::new(0, 0)
    }

    /// Whether this is the first position of the first line.
    #[must_use]
    pub const fn is_origin(&self) -> bool {
        self.line == 0 && self.col == 0
    }

    /// Resolve this position to a byte offset into `text`.
    ///
    /// Clamped rather than trusted: a `LineCol` from a *different* buffer
    /// — a stale cursor the host kept across a value change — is a normal
    /// thing to receive and must not panic. It resolves to the end of the
    /// text it was given instead.
    #[must_use]
    pub fn offset_in(self, text: &str) -> usize {
        let mut offset = 0usize;
        let mut line = 0u32;

        while line < self.line {
            match text[offset..].find('\n') {
                Some(nl) => {
                    offset += nl + 1;
                    line += 1;
                }
                None => return text.len(),
            }
        }

        let line_start = offset;
        let line_end = text[offset..]
            .find('\n')
            .map_or(text.len(), |nl| offset + nl);

        let mut byte = line_start;
        for ch in text[line_start..].chars().take(self.col as usize) {
            byte += ch.len_utf8();
        }

        byte.min(line_end)
    }

    /// The position `offset` bytes into `text` occupies.
    #[must_use]
    pub fn from_offset(text: &str, offset: usize) -> Self {
        let offset = offset.min(text.len());
        let before = &text[..offset];
        let line = before.matches('\n').count() as u32;
        let line_start = before.rfind('\n').map_or(0, |nl| nl + 1);
        let col = before[line_start..].chars().count() as u32;
        Self::new(line, col)
    }

    /// This position moved inside `text`, never past its end.
    #[must_use]
    pub fn clamped_to(self, text: &str) -> Self {
        Self::from_offset(text, self.offset_in(text))
    }
}

/// One reported parse problem with the buffer's contents.
///
/// §3.2 types the text area's `validation` as `Option<ParseDiagnostic>`
/// rather than the `Option<String>` every other control uses, which is
/// the one place in §3.2 where a validation payload carries a *place*
/// with it. The line and column are the same `LineCol` the cursor uses,
/// so a host can point the caret at the problem with the coordinate it
/// already knows how to move.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParseDiagnostic {
    /// Where the problem is.
    pub at: LineCol,
    /// What the host says is wrong.
    pub message: String,
}

impl ParseDiagnostic {
    /// A diagnostic at `at` reading `message`.
    #[must_use]
    pub fn new(at: LineCol, message: impl Into<String>) -> Self {
        Self {
            at,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.at.line + 1, self.at.col + 1)?;
        f.write_str(&self.message)
    }
}

/// The `min_rows` / `max_rows` band a text area grows inside.
///
/// Every height is derived, never stored: `rows × type.body.line_height +
/// 2 × space.sm`, which is §3.2e's closed form read straight off the
/// tokens. Deriving them means the solver cannot disagree with the
/// tokens, and that a change to `space.sm` moves the whole band instead
/// of leaving one constant behind.
///
/// The band's two ends are different kinds of number and are treated
/// differently. `min_rows` is a **floor**: the well is that tall when it
/// is empty and never shorter. `max_rows` is a **ceiling**, and `None`
/// means there is no ceiling at all rather than a ceiling of zero — the
/// distinction [`Self::max_height`] makes explicit by returning
/// [`f32::INFINITY`] for an uncapped band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Growth {
    /// The floor, in rows.
    pub min_rows: u32,
    /// The ceiling, in rows. `None` means "grows without a ceiling".
    pub max_rows: Option<u32>,
}

/// The vertical padding inside a text well: `space.sm` top and bottom.
///
/// §3.2e prints this as `2 × space.sm = 16px` and then names both
/// halves — `8px` top, `8px` bottom — so the well is symmetric and the
/// total is the sum of two token reads, never a literal.
const VERTICAL_PADDING: f32 = space::SM * 2.0;

impl Growth {
    /// A band from `min_rows` up to `max_rows`.
    ///
    /// Normalised: a ceiling below the floor is raised to it rather than
    /// inverted, because an inverted band would make the minimum height
    /// larger than the maximum and every clamp between them would pick
    /// the wrong end.
    #[must_use]
    pub const fn new(min_rows: u32, max_rows: Option<u32>) -> Self {
        let max_rows = match max_rows {
            Some(max) if max > min_rows => Some(max),
            Some(_) => Some(min_rows),
            None => None,
        };
        Self { min_rows, max_rows }
    }

    /// The height of one row of body text.
    #[must_use]
    pub const fn line_height() -> f32 {
        typography::BODY.line_height
    }

    /// The height of `rows` rows of text, padding excluded.
    #[must_use]
    pub const fn text_height(rows: u32) -> f32 {
        rows as f32 * Self::line_height()
    }

    /// The well's height at exactly `rows` rows.
    ///
    /// This is §3.2e's formula with the clamp removed, because the clamp
    /// is the band's business rather than the row's: `height_at(3)` is
    /// `3 × 20 + 16 = 76px` at the body rung, which is the worked
    /// example the spec prints.
    #[must_use]
    pub const fn height_at(rows: u32) -> f32 {
        Self::text_height(rows) + VERTICAL_PADDING
    }

    /// The smallest height this band allows.
    #[must_use]
    pub fn min_height(&self) -> f32 {
        Self::height_at(self.min_rows)
    }

    /// The largest height this band allows — `f32::INFINITY` when the
    /// band has no ceiling.
    #[must_use]
    pub fn max_height(&self) -> f32 {
        match self.max_rows {
            Some(rows) => Self::height_at(rows),
            None => f32::INFINITY,
        }
    }

    /// How tall the well should be to show `lines` lines of content.
    ///
    /// Grows with the content and stops at the ceiling. Below the floor
    /// it holds the floor, which is what keeps an empty three-row well
    /// the same height as a full one.
    #[must_use]
    pub fn height_for_lines(&self, lines: u32) -> f32 {
        let wanted = Self::height_at(lines.max(1));
        wanted.clamp(self.min_height(), self.max_height())
    }

    /// Whether the well would still grow at `lines` lines.
    #[must_use]
    pub fn can_grow(&self, lines: u32) -> bool {
        self.height_for_lines(lines) < self.max_height()
    }

    /// How many whole rows fit in `height`.
    #[must_use]
    pub fn rows_fitting(height: f32) -> u32 {
        let inner = (height - VERTICAL_PADDING).max(0.0);
        (inner / Self::line_height()).floor() as u32
    }

    /// A height moved inside this band — **both** bounds applied.
    ///
    /// The floor keeps an empty well from collapsing under `min_rows`.
    /// The ceiling matters just as much: a host that hands back a
    /// rectangle taller than `max_rows` must not get to keep it, or the
    /// cap is a suggestion. §3.2e's closed form clamps the *row count*
    /// between the two rungs, so the height-space equivalent has to
    /// clamp both ways too — a helper that only enforced the floor would
    /// let a caller step straight over the ceiling.
    ///
    /// Written as `max` then `min` rather than [`f32::clamp`] so a band
    /// built by bypassing [`Self::new`] — and therefore inverted —
    /// resolves to its ceiling instead of panicking on the
    /// `min <= max` assertion.
    #[must_use]
    pub fn clamp_height(&self, height: f32) -> f32 {
        height.max(self.min_height()).min(self.max_height())
    }
}

impl Default for Growth {
    /// Three rows, no ceiling — §3.2's stated minimum.
    fn default() -> Self {
        Self::new(3, None)
    }
}

/// A bounded undo/redo ledger of whole-value snapshots.
///
/// Records the value *before* each accepted edit. Undo therefore pops a
/// value rather than computing one, and cannot be wrong about how a
/// deletion inverts a paste.
///
/// The redo stack is cleared by every new edit, which is the standard
/// rule and the only one that keeps the two stacks meaning "the timeline
/// you were on".
#[derive(Clone, Debug)]
pub struct EditHistory {
    undo: Vec<String>,
    redo: Vec<String>,
    limit: usize,
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl EditHistory {
    /// The default ledger depth.
    ///
    /// A bound rather than no bound: an unbounded history on a long
    /// buffer is an unbounded memory growth story, and a user who pastes
    /// a megabyte a hundred times should not get a gigabyte of
    /// snapshots.
    pub const DEFAULT_LIMIT: usize = 128;

    /// An empty ledger at [`DEFAULT_LIMIT`](Self::DEFAULT_LIMIT).
    #[must_use]
    pub fn new() -> Self {
        Self::with_limit(Self::DEFAULT_LIMIT)
    }

    /// An empty ledger holding at most `limit` undo steps.
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Record the value that stood before an accepted edit.
    pub fn record(&mut self, before: &str) {
        self.undo.push(before.to_string());
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Whether there is anything to step back to.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is anything to step forward to.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Step back, given the value that is current right now.
    ///
    /// The current value is pushed onto the redo stack before the older
    /// one comes back, so undo and redo are exact inverses of each other
    /// across any number of steps.
    #[must_use]
    pub fn undo(&mut self, current: &str) -> Option<String> {
        let previous = self.undo.pop()?;
        self.redo.push(current.to_string());
        Some(previous)
    }

    /// Step forward, given the value that is current right now.
    #[must_use]
    pub fn redo(&mut self, current: &str) -> Option<String> {
        let next = self.redo.pop()?;
        self.undo.push(current.to_string());
        Some(next)
    }

    /// How many undo steps remain.
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// How many redo steps remain.
    #[must_use]
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// Forget both stacks.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// The state of a multi-line text area (`UIF-ATOM-002e`).
///
/// Fields are private and read through accessors, the same reason
/// [`PasswordInputModel`](super::password_model::PasswordInputModel)
/// keeps its own: two of them — `dirty` and the scroll offset — are
/// facts a host writing them directly would get wrong, because they are
/// consequences of editing rather than inputs to it.
///
/// ## What the scroll model is
///
/// The well's *height* is decided by [`Growth`]: it grows with the
/// content until it hits its ceiling. Once it has hit the ceiling and the
/// content keeps growing, the surplus has to go somewhere, and that is
/// [`scroll_offset`](Self::scroll_offset). The two are the same
/// arithmetic read from opposite ends, which is why
/// [`max_scroll`](Self::max_scroll) returns `0` for a well that is
/// still growing — there is nothing scrolled there, the well simply
/// isn't finished getting taller.
#[derive(Clone, Debug, PartialEq)]
pub struct TextAreaModel {
    text: String,
    cursor: LineCol,
    selection: Option<TextRange>,
    growth: Growth,
    dirty: bool,
    validation: Option<ParseDiagnostic>,
    disabled: bool,
    read_only: bool,
    scroll_offset: f32,
    stance: EditStance,
}

impl Default for TextAreaModel {
    fn default() -> Self {
        Self::new()
    }
}

impl TextAreaModel {
    /// An empty, enabled, three-row well with no ceiling.
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: LineCol::origin(),
            selection: None,
            growth: Growth::default(),
            dirty: false,
            validation: None,
            disabled: false,
            read_only: false,
            scroll_offset: 0.0,
            stance: EditStance::Pristine,
        }
    }

    /// Seed the buffer.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Set the growth band.
    #[must_use]
    pub fn with_growth(mut self, growth: Growth) -> Self {
        self.growth = growth;
        self
    }

    /// Report the well disabled.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Report the well read-only.
    #[must_use]
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Declare the well's commit history at construction time.
    ///
    /// The same escape hatch `TextInputModel::with_stance` provides, for
    /// the same reason: a model rebuilt after a failed submit knows the
    /// user already committed.
    #[must_use]
    pub fn with_stance(mut self, stance: EditStance) -> Self {
        self.stance = stance;
        self
    }

    // -- reads ---------------------------------------------------------

    /// The buffer.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret.
    #[must_use]
    pub fn cursor(&self) -> LineCol {
        self.cursor
    }

    /// The selection, if there is one.
    #[must_use]
    pub fn selection(&self) -> Option<TextRange> {
        self.selection
    }

    /// The growth band.
    #[must_use]
    pub fn growth(&self) -> Growth {
        self.growth
    }

    /// Whether the buffer differs from what the host last settled.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// §3.2's `validation`.
    #[must_use]
    pub fn validation(&self) -> Option<&ParseDiagnostic> {
        self.validation.as_ref()
    }

    /// Whether a parse problem is outstanding.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.validation.is_some()
    }

    /// The well's commit history (`UIF-PRIN-009`).
    ///
    /// Distinct from [`is_dirty`](Self::is_dirty), which answers a
    /// different question. A well can be dirty — the buffer differs from
    /// what the host last settled — and still pristine, because the user
    /// typed into it and has not committed or blurred yet. Conflating the
    /// two would punish a field for being edited rather than for being
    /// finished with.
    #[must_use]
    pub const fn stance(&self) -> EditStance {
        self.stance
    }

    /// Whether the user has not yet committed to this well.
    #[must_use]
    pub const fn is_pristine(&self) -> bool {
        self.stance.is_pristine()
    }

    /// Whether the user has committed to, or left, this well.
    #[must_use]
    pub const fn is_touched(&self) -> bool {
        self.stance.is_touched()
    }

    /// `on_commit`: the user committed the buffer.
    pub fn note_commit(&mut self) {
        self.stance = EditStance::Touched;
    }

    /// `on_focus_change(false)`: the user left the well.
    pub fn note_blur(&mut self) {
        self.stance = EditStance::Touched;
    }

    /// The containing form was submitted.
    pub fn note_submit(&mut self) {
        self.stance = EditStance::Touched;
    }

    /// Return the well to pristine for a new form.
    pub fn reset_stance(&mut self) {
        self.stance = EditStance::Pristine;
    }

    /// What this well should paint this frame, with no disallowed-character
    /// class in play.
    #[must_use]
    pub fn punishment(&self) -> Punishment {
        punish(self.stance, self.is_invalid(), false)
    }

    /// What this well should paint this frame under `class`'s character
    /// rules.
    ///
    /// Multi-line buffers rarely carry a basename class, but the shape is
    /// the same one the single-line field uses so a renderer has one call
    /// to make rather than two rules to remember.
    #[must_use]
    pub fn punishment_under(&self, class: DisallowedClass) -> Punishment {
        punish(
            self.stance,
            self.is_invalid(),
            class.contains_any(self.text()),
        )
    }

    /// Whether the host has taken the well away.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Whether the buffer can be read and selected but not changed.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// How many lines the buffer holds.
    ///
    /// An empty buffer is one line, not zero: the caret sits on it, and a
    /// zero-line answer would make an empty well shorter than a
    /// one-character one.
    #[must_use]
    pub fn line_count(&self) -> u32 {
        self.text.matches('\n').count() as u32 + 1
    }

    /// The height the buffer's content needs, padding excluded.
    #[must_use]
    pub fn content_height(&self) -> f32 {
        Growth::text_height(self.line_count())
    }

    /// The well's outer height this frame.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.growth.height_for_lines(self.line_count())
    }

    /// How much of the well is given to text rather than padding.
    #[must_use]
    pub fn visible_text_height(&self) -> f32 {
        (self.height() - VERTICAL_PADDING).max(0.0)
    }

    /// How far the content may scroll before the last line leaves the
    /// bottom of the well.
    #[must_use]
    pub fn max_scroll(&self) -> f32 {
        (self.content_height() - self.visible_text_height()).max(0.0)
    }

    /// Whether the content is taller than the well can show.
    #[must_use]
    pub fn is_scrollable(&self) -> bool {
        self.max_scroll() > 0.0
    }

    /// The current scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Move the scroll, clamped to what the content allows.
    ///
    /// Returns the offset actually applied, which is not always the one
    /// asked for — and returning it rather than silently absorbing the
    /// difference is what lets a caller see it hit the end.
    pub fn set_scroll(&mut self, offset: f32) -> f32 {
        self.scroll_offset = offset.clamp(0.0, self.max_scroll());
        self.scroll_offset
    }

    /// Scroll the minimum needed to bring `at` into view.
    ///
    /// A line already wholly visible moves nothing. A line above the top
    /// pulls the offset back to its first pixel; a line below the bottom
    /// pushes it so the line's last pixel sits on the fold. This is the
    /// caret-follow rule the editor needs on every keystroke, expressed as
    /// a pure function so "the caret never scrolls when it is already
    /// visible" is a test rather than a hope.
    pub fn scroll_to_show(&mut self, at: LineCol) -> f32 {
        let visible = self.visible_text_height();
        let row = at.line as f32 * Growth::line_height();
        let row_bottom = row + Growth::line_height();

        if row < self.scroll_offset {
            self.set_scroll(row)
        } else if row_bottom > self.scroll_offset + visible {
            self.set_scroll(row_bottom - visible)
        } else {
            self.scroll_offset
        }
    }

    // -- writes --------------------------------------------------------

    /// Replace the buffer and mark it dirty.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.dirty = true;
    }

    /// Move the caret, clamped to the buffer.
    pub fn set_cursor(&mut self, at: LineCol) {
        self.cursor = at.clamped_to(&self.text);
    }

    /// Set or clear the selection.
    pub fn set_selection(&mut self, selection: Option<TextRange>) {
        self.selection = selection;
    }

    /// Report or clear a parse diagnostic.
    pub fn set_validation(&mut self, validation: Option<ParseDiagnostic>) {
        self.validation = validation;
    }

    /// Mark the buffer as settled — the host has taken the value.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}

/// One tier's worth of output from a multi-line text area.
///
/// §3.2 names five callbacks and this carries all five, plus the three
/// the crate's tier discipline (§3.0.4.2) requires and §3.2 does not
/// mention for this control: `Committed` on focus loss, `Cancelled` on
/// `Escape`, and `FocusChanged`. They are not extras — without a settle
/// tier there is no moment at which the host learns the user has
/// finished, and without a cancel tier `Escape` has nothing to restore.
///
/// # `Submitted` is not `Committed`
///
/// `Ctrl+Enter` sends the text and leaves the user in the field; focus
/// loss settles the field and sends nobody anywhere. Collapsing the two
/// into one arm would make "sent" and "finished" the same event, and a
/// host that clears the composer on submit would clear it on blur too.
#[derive(Clone)]
pub enum TextAreaEmission {
    /// `Live<String>` — `on_change`, one coalesced tick per delivery.
    Changed(Live<String>),
    /// `Commit<String>` — settled by focus loss.
    Committed(Commit<String>),
    /// `Commit<String>` — `on_submit`, `Ctrl+Enter` / `Cmd+Enter`.
    Submitted(Commit<String>),
    /// `Cancel<String>` — `Escape` inside an open session.
    Cancelled(Cancel<String>),
    /// `on_cursor_change(LineCol)`.
    CursorChanged(LineCol),
    /// `on_undo()` — the buffer that came back.
    Undone(String),
    /// `on_redo()` — the buffer that went forward.
    Redone(String),
    /// `on_focus_change(bool)`.
    FocusChanged(bool),
}

impl TextAreaEmission {
    /// The §3.0.4.2 tier name this arm belongs to.
    #[must_use]
    pub const fn tier(&self) -> &'static str {
        match self {
            Self::Changed(_) => "live",
            Self::Committed(_) | Self::Submitted(_) => "commit",
            Self::Cancelled(_) => "cancel",
            Self::CursorChanged(_) => "cursor",
            Self::Undone(_) | Self::Redone(_) => "history",
            Self::FocusChanged(_) => "focus",
        }
    }

    /// The value this arm carries, if it carries one.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Changed(live) => Some(live.value()),
            Self::Committed(commit) | Self::Submitted(commit) => Some(commit.value()),
            Self::Cancelled(cancel) => Some(cancel.restored()),
            Self::Undone(text) | Self::Redone(text) => Some(text),
            Self::CursorChanged(_) | Self::FocusChanged(_) => None,
        }
    }

    /// Whether this arm settles the editing session.
    ///
    /// `Submitted` deliberately does **not**: the user is still in the
    /// field, and a session closed by a submit would make the next
    /// `Escape` restore nothing.
    #[must_use]
    pub const fn settles(&self) -> bool {
        matches!(self, Self::Committed(_) | Self::Cancelled(_))
    }
}

impl PartialEq for TextAreaEmission {
    /// Tier-matched comparison over the payload each arm carries.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Changed(a), Self::Changed(b)) => a.value() == b.value(),
            (Self::Committed(a), Self::Committed(b)) => a.value() == b.value(),
            (Self::Submitted(a), Self::Submitted(b)) => a.value() == b.value(),
            (Self::Cancelled(a), Self::Cancelled(b)) => a.restored() == b.restored(),
            (Self::CursorChanged(a), Self::CursorChanged(b)) => a == b,
            (Self::Undone(a), Self::Undone(b)) => a == b,
            (Self::Redone(a), Self::Redone(b)) => a == b,
            (Self::FocusChanged(a), Self::FocusChanged(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for TextAreaEmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Changed(live) => f.debug_tuple("Changed").field(live.value()).finish(),
            Self::Committed(commit) => f.debug_tuple("Committed").field(commit.value()).finish(),
            Self::Submitted(commit) => f.debug_tuple("Submitted").field(commit.value()).finish(),
            Self::Cancelled(cancel) => f.debug_tuple("Cancelled").field(cancel.restored()).finish(),
            Self::CursorChanged(at) => f.debug_tuple("CursorChanged").field(at).finish(),
            Self::Undone(text) => f.debug_tuple("Undone").field(text).finish(),
            Self::Redone(text) => f.debug_tuple("Redone").field(text).finish(),
            Self::FocusChanged(focused) => f.debug_tuple("FocusChanged").field(focused).finish(),
        }
    }
}

#[cfg(test)]
#[path = "text_area_model_tests.rs"]
mod text_area_model_tests;
