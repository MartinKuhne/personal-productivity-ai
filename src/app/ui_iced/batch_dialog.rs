//! The Batch prompt-processing dialog — directory / wildcard
//! pattern / prompt / mode / concurrency composition plus the running and
//! completed views (BATCH-001..BATCH-014).
//!
//! A **pure view over a plain snapshot**: [`batch_dialog_card`] renders a
//! [`BatchDialogViewModel`] into an [`iced::Element`] of [`Message`] and
//! touches neither the bus, the disk nor `FastMdApp`. The
//! reducer owns the open/close state
//! ([`BatchDialogState`]) and publishes the [`UserCommand`]s the card emits.
//!
//! # Data sources — reused unchanged ([RUST-051])
//!
//! | Field | Source |
//! | --- | --- |
//! | `BatchConfig` / `BatchMode` | `crate::agent::batch::types` — re-exported here, **not** re-declared. `BatchConfig` is the exact payload `UserCommand::StartBatch` carries (`src/app/bus/events/user_command.rs:134`) |
//! | `PromptInfo` | `crate::agent::batch::types::PromptInfo`, the discovered-prompt record `resolve_prompts` builds (`src/app/agent/batch/prompts.rs:128`) |
//! | Validity rule | `validate_batch_params` (`src/app/agent/batch/types.rs:48-65`) — the single gate the Process button checks |
//! | Candidate directories | `DirectoryTracker::dirs_sorted()` / the content-library roots, snapshotted by the reducer |
//! | Candidate prompts | `resolve_prompts(app.tags().prompt_paths(), &app.config().content_libraries)`, snapshotted by the reducer rather than at render time |
//! | Run status | `Dialogs::batch_handle` / `BatchHandle::thread.is_finished()` / `Dialogs::batch_cancel_flag` (`src/app/ui/dialogs.rs:49-50`), folded into [`BatchRunStatus`] |
//!
//! [`BatchDialogViewModel::from_config`] snapshots a
//! [`BatchDialogConfig`] with an idle run status, so the reducer can hand
//! the existing config straight over without a field-by-field translation.
//!
//! # Message channel
//!
//! Only the *composed selection* needs a new channel
//! ([`Message::Batch`]). Open, start and cancel already ride
//! [`Message::Command`]: `UserCommand::OpenBatchDialog`,
//! `UserCommand::StartBatch(config)` and `UserCommand::CancelBatch` are the
//! commands the card emits, and the composed [`BatchConfig`] travels inside
//! the command, so the intent stays self-contained and replayable.
//!
//! # Deliberately absent
//!
//! * **A selected-file list.** The Batch dialog has **no file selection at
//!   all** — it picks a *directory* plus a glob pattern and lets the batch
//!   subsystem discover the files itself (`agent/batch/file_matcher.rs`).
//!   The only multi-file selection in the app is the tree's merge action
//!   (`ui/tree/handlers.rs:71`), a different feature on a different
//!   surface. The scrollable, selectable rows provided here are the
//!   **prompt candidates** — a list that can outgrow a combo.
//! * **Merge-prompt candidates.** Merge prompts belong to
//!   `UserCommand::MergePrompt` from the tree context menu, never to the
//!   batch dialog.
//! * **Select-all / select-none.** Neither exists for this dialog
//!   (single-choice combos throughout), so no such transition is invented
//!   here. [`clamp_selection`] and [`index_of_selected`] are the two
//!   selection-state rules the dialog *does* run.
//! * **The spinner.** iced 0.14 has no built-in spinner and `iced_aw` is
//!   not a dependency; the running view carries the
//!   [`strings::BATCH_PROCESSING_TEXT`] plus the cancel hint instead.
//! * **The window chrome** (non-resizable `Window`, title-bar `X`).
//!   The card is mounted by the overlay host through
//!   [`crate::ui_iced::overlay::modal`], which supplies the scrim and the
//!   dismiss path.
//!
//! Unit tests live in the sibling `batch_dialog_tests.rs` sidecar.

#[cfg(test)]
#[path = "batch_dialog_tests.rs"]
mod tests;

use std::fmt;
use std::path::{Path, PathBuf};

use iced::widget::{
    Column, Row, button, container, mouse_area, pick_list, rule, scrollable, space, text,
    text_input,
};
use iced::{Alignment, Background, Element, Length};

use crate::agent::batch::types::{BatchDialogConfig, validate_batch_params};
use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{BatchDialogMessage, Message};
use crate::ui_iced::overlay::{ConfirmCancel, confirm_cancel_buttons};

/// The batch payload the Process button composes — reused unchanged from
/// the batch subsystem ([RUST-051]); it is the `UserCommand::StartBatch`
/// payload (`src/app/bus/events/user_command.rs:134`).
pub use crate::agent::batch::types::{BatchConfig, BatchMode, PromptInfo};

/// Minimum width of the batch card, in logical pixels.
pub const BATCH_CARD_WIDTH: f32 = 500.0;

/// Vertical gap between card rows, in logical pixels.
pub const BATCH_SECTION_SPACING: f32 = 10.0;

/// Logical point size of a field label, row or hint.
pub const BATCH_BODY_SIZE: f32 = 14.0;

/// Logical point size of the running-view heading.
pub const BATCH_HEADING_SIZE: f32 = 18.0;

/// Maximum height of the scrollable prompt-candidate list.
pub const BATCH_PROMPT_LIST_MAX_HEIGHT: f32 = 180.0;

/// Width of the horizontal rules separating card sections.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// Lowest selectable concurrency — the lower bound of the pick list
/// (`(1u8..=8)`) and of
/// `validate_batch_params` (`agent/batch/types.rs:53`).
pub const CONCURRENCY_MIN: u8 = 1;

/// Highest selectable concurrency.
pub const CONCURRENCY_MAX: u8 = 8;

/// The default wildcard pattern — `BatchDialogConfig::default()`'s value
/// (`agent/batch/types.rs:247`), restated here only as a documented
/// accessor for tests and the reducer.
pub const DEFAULT_PATTERN: &str = "*.md";

/// Runtime status of the batch job the dialog may be showing progress for.
///
/// The three booleans read off `Dialogs`
/// (`src/app/ui/dialogs.rs:49-50`) folded into one plain value so
/// the view never has to reach for a `BatchHandle` (which owns a
/// `JoinHandle` and is therefore neither `Clone` nor `Debug`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchRunStatus {
    /// A batch worker handle exists (`Dialogs::batch_handle.is_some()`) —
    /// the flag that switches the card to the
    /// progress view.
    pub has_handle: bool,
    /// The worker thread has finished (`BatchHandle::thread.is_finished()`).
    pub finished: bool,
    /// The shared cancellation flag is set
    /// (`Dialogs::batch_cancel_flag`).
    pub cancel_requested: bool,
}

impl BatchRunStatus {
    /// No batch is running — the card shows the configuration view.
    #[must_use]
    pub fn idle() -> Self {
        Self::default()
    }

    /// Whether nothing is running.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        !self.has_handle
    }

    /// Whether the card should render the progress view instead of the
    /// configuration view (driven purely by `has_handle`).
    #[must_use]
    pub fn shows_progress_view(&self) -> bool {
        self.has_handle
    }

    /// The completion line: `Some` once the worker has finished.
    #[must_use]
    pub fn completion_text(&self) -> Option<&'static str> {
        (self.has_handle && self.finished).then_some(strings::BATCH_COMPLETED_TEXT)
    }

    /// The cancellation warning: `Some` only when a finished run was
    /// cancelled by the user (painted with `palette.warning`).
    #[must_use]
    pub fn cancellation_text(&self) -> Option<&'static str> {
        (self.has_handle && self.finished && self.cancel_requested)
            .then_some(strings::BATCH_CANCELLED_TEXT)
    }
}

/// One entry of a batch pick-list: the index into the snapshot list plus
/// the label shown in the combo.
///
/// iced's `pick_list` needs `T: ToString + Clone + PartialEq`
/// (`iced_widget-0.14.2/src/helpers.rs:1656-1667`), so the index travels
/// inside the option rather than being recovered from the label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchOption {
    /// Index into the list this option was built from.
    pub index: usize,
    /// The label rendered in the combo and the dropdown.
    pub label: String,
}

impl BatchOption {
    /// An option pointing at `index` and labelled `label`.
    #[must_use]
    pub fn new(index: usize, label: impl Into<String>) -> Self {
        Self {
            index,
            label: label.into(),
        }
    }
}

impl fmt::Display for BatchOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// Everything the batch card renders: the candidate lists, the composed
/// selection and the run status.
///
/// `Clone + Debug` only — [`PromptInfo`] derives neither `PartialEq` nor
/// `Eq` (`agent/batch/types.rs:68`), so the snapshot cannot either; the
/// sidecar asserts field by field.
#[derive(Clone, Debug)]
pub struct BatchDialogViewModel {
    /// Directories the user may pick as the batch root.
    pub available_dirs: Vec<PathBuf>,
    /// Discovered prompt candidates.
    pub available_prompts: Vec<PromptInfo>,
    /// The picked directory index.
    pub selected_dir_idx: Option<usize>,
    /// The picked prompt index.
    pub selected_prompt_idx: Option<usize>,
    /// Wildcard pattern (File mode only).
    pub pattern: String,
    /// File / Directory processing mode.
    pub mode: BatchMode,
    /// Concurrent LLM calls (1..=8).
    pub concurrency: u8,
    /// Status of the batch job, if any.
    pub status: BatchRunStatus,
}

impl Default for BatchDialogViewModel {
    fn default() -> Self {
        Self {
            available_dirs: Vec::new(),
            available_prompts: Vec::new(),
            selected_dir_idx: None,
            selected_prompt_idx: None,
            pattern: DEFAULT_PATTERN.to_string(),
            mode: BatchMode::File,
            concurrency: 4,
            status: BatchRunStatus::idle(),
        }
    }
}

impl BatchDialogViewModel {
    /// A snapshot with no candidates, nothing selected and no run in
    /// flight — the state the card shows before the reducer has filled it.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Snapshots a [`BatchDialogConfig`] with an idle
    /// run status ([RUST-051] — no field-by-field translation needed).
    #[must_use]
    pub fn from_config(config: &BatchDialogConfig) -> Self {
        Self {
            available_dirs: config.available_dirs.clone(),
            available_prompts: config.available_prompts.clone(),
            selected_dir_idx: config.selected_dir_idx,
            selected_prompt_idx: config.selected_prompt_idx,
            pattern: config.pattern.clone(),
            mode: config.mode,
            concurrency: config.concurrency,
            status: BatchRunStatus::idle(),
        }
    }

    /// The same snapshot carrying a run status.
    #[must_use]
    pub fn with_status(mut self, status: BatchRunStatus) -> Self {
        self.status = status;
        self
    }

    /// Whether the composed configuration is valid — delegating to
    /// `validate_batch_params` so the gate cannot drift from the subsystem's
    /// own rule.
    ///
    /// Purity: pure.
    #[must_use]
    pub fn is_config_valid(&self) -> bool {
        if self.selected_dir_idx.is_none() {
            return false;
        }
        if self.selected_prompt_idx.is_none() {
            return false;
        }
        validate_batch_params(self.mode, &self.pattern, self.concurrency).is_ok()
    }

    /// The start-enabled predicate: a valid composition **and** no batch
    /// already running. A run in flight must not be started twice; the
    /// cancel path stays available instead.
    ///
    /// Purity: pure.
    #[must_use]
    pub fn can_start(&self) -> bool {
        self.is_config_valid() && self.status.is_idle()
    }

    /// The directory the user picked, `None` when nothing is selected or
    /// the index is stale.
    #[must_use]
    pub fn selected_directory(&self) -> Option<&Path> {
        self.selected_dir_idx
            .and_then(|idx| self.available_dirs.get(idx))
            .map(PathBuf::as_path)
    }

    /// The prompt candidate the user picked.
    #[must_use]
    pub fn selected_prompt(&self) -> Option<&PromptInfo> {
        self.selected_prompt_idx
            .and_then(|idx| self.available_prompts.get(idx))
    }

    /// The composed [`BatchConfig`]: `Some` only when both picks resolve
    /// to real entries.
    ///
    /// Purity: pure.
    #[must_use]
    pub fn build_batch_config(&self) -> Option<BatchConfig> {
        let directory = self.selected_directory()?.to_path_buf();
        let prompt = self.selected_prompt()?;
        Some(BatchConfig {
            directory,
            pattern: self.pattern.clone(),
            prompt_path: prompt.path.clone(),
            mode: self.mode,
            concurrency: self.concurrency,
        })
    }

    /// The directory combo's current label, falling back to
    /// [`strings::BATCH_DIALOG_SELECT_DIRECTORY`].
    #[must_use]
    pub fn directory_label(&self) -> String {
        self.selected_directory()
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(|| strings::BATCH_DIALOG_SELECT_DIRECTORY.to_string())
    }

    /// The prompt combo's current label, falling back to
    /// [`strings::BATCH_DIALOG_SELECT_PROMPT`].
    #[must_use]
    pub fn prompt_label(&self) -> String {
        self.selected_prompt()
            .map(|prompt| prompt.display_name.clone())
            .unwrap_or_else(|| strings::BATCH_DIALOG_SELECT_PROMPT.to_string())
    }

    /// The directory pick-list options, in snapshot order.
    #[must_use]
    pub fn directory_options(&self) -> Vec<BatchOption> {
        self.available_dirs
            .iter()
            .enumerate()
            .map(|(idx, dir)| BatchOption::new(idx, dir.display().to_string()))
            .collect()
    }

    /// The prompt pick-list options, in snapshot order.
    #[must_use]
    pub fn prompt_options(&self) -> Vec<BatchOption> {
        self.available_prompts
            .iter()
            .enumerate()
            .map(|(idx, prompt)| BatchOption::new(idx, prompt.display_name.clone()))
            .collect()
    }

    /// The concurrency pick-list options, `1..=8`.
    #[must_use]
    pub fn concurrency_options() -> Vec<u8> {
        (CONCURRENCY_MIN..=CONCURRENCY_MAX).collect()
    }

    /// Whether the wildcard pattern field accepts edits — it is inert in
    /// Directory mode while staying allocated (AGENTS §6).
    ///
    /// Purity: pure.
    #[must_use]
    pub fn pattern_editable(&self) -> bool {
        self.mode == BatchMode::File
    }

    /// Renders this snapshot as the batch card ([`batch_dialog_card`]).
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        batch_dialog_card(self)
    }
}

/// Drops a selection whose index no longer points at a list entry.
///
/// A stale index would otherwise read out of bounds after the list shrank
/// or was reordered, so this runs whenever the prompt list is rebuilt.
///
/// Purity: pure function of `selected` and `len`.
#[must_use]
pub fn clamp_selection(selected: Option<usize>, len: usize) -> Option<usize> {
    selected.filter(|idx| *idx < len)
}

/// Re-finds a previously selected value's index after its list was
/// rebuilt, so the selection follows the *path* rather than the position.
///
/// Purity: pure.
#[must_use]
pub fn index_of_selected<'list, T: PartialEq>(
    selected: Option<&'list T>,
    list: &'list [T],
) -> Option<usize> {
    let selected = selected?;
    list.iter().position(|item| item == selected)
}

/// The Process button's message: the composed config as
/// `UserCommand::StartBatch`.
///
/// `None` when the composition cannot be built, which is exactly when
/// [`BatchDialogViewModel::can_start`] is `false` and the button is inert.
///
/// Purity: pure.
#[must_use]
pub fn start_message(view_model: &BatchDialogViewModel) -> Option<Message> {
    view_model
        .build_batch_config()
        .map(|config| Message::Command(UserCommand::StartBatch(config)))
}

/// The Cancel button, the Close button and the scrim/Escape dismiss all
/// map to `UserCommand::CancelBatch`, so there is one cancel transition
/// for the reducer.
///
/// Purity: pure.
#[must_use]
pub fn cancel_message() -> Message {
    Message::Command(UserCommand::CancelBatch)
}

/// Which view of the batch dialog is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BatchDialogView {
    /// The idle configuration view.
    #[default]
    Config,
    /// The in-flight / completed progress view.
    Progress,
}

/// Which batch dialog is open and what the user has composed so far —
/// the pure state the reducer holds.
///
/// Mirrors [`crate::ui_iced::about::AboutWindowState`]: one dialog at a
/// time, `open` replaces whatever was showing, and `close` reports
/// whether anything was consumed so an Escape handler can tell a
/// consumed key from a no-op.
#[derive(Clone, Debug, Default)]
pub struct BatchDialogState {
    open: bool,
    model: BatchDialogViewModel,
}

impl BatchDialogState {
    /// A closed dialog over an empty model.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A closed dialog seeded with a candidate snapshot.
    #[must_use]
    pub fn with_model(model: BatchDialogViewModel) -> Self {
        Self { open: false, model }
    }

    /// Whether the dialog is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The composed snapshot the card renders.
    #[must_use]
    pub fn model(&self) -> &BatchDialogViewModel {
        &self.model
    }

    /// Shows the dialog over `model`, clamping any selection that no
    /// longer resolves against the freshly supplied candidate lists.
    ///
    /// Returns whether the open state changed.
    pub fn open(&mut self, model: BatchDialogViewModel) -> bool {
        let mut model = model;
        model.selected_dir_idx =
            clamp_selection(model.selected_dir_idx, model.available_dirs.len());
        model.selected_prompt_idx =
            clamp_selection(model.selected_prompt_idx, model.available_prompts.len());
        self.model = model;
        let was_open = self.open;
        self.open = true;
        !was_open
    }

    /// Hides the dialog, keeping the composed selection so a reopen shows
    /// what the user had typed.
    ///
    /// Returns whether a dialog was actually open — the same
    /// "was anything consumed" contract as
    /// [`crate::ui_iced::overlay::OverlaySlot::close`].
    pub fn close(&mut self) -> bool {
        let was_open = self.open;
        self.open = false;
        was_open
    }

    /// Flips the visible state, returning the new state.
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        self.open
    }

    /// The view the card is rendering for the current status.
    #[must_use]
    pub fn current_view(&self) -> BatchDialogView {
        if self.model.status.shows_progress_view() {
            BatchDialogView::Progress
        } else {
            BatchDialogView::Config
        }
    }

    /// Whether the Process button may fire in the current state.
    #[must_use]
    pub fn can_start(&self) -> bool {
        self.model.can_start()
    }

    /// The config the Process button would compose.
    #[must_use]
    pub fn batch_config(&self) -> Option<BatchConfig> {
        self.model.build_batch_config()
    }

    /// Applies one [`BatchDialogMessage`] to the composed selection.
    ///
    /// Returns whether the model actually changed, so the caller can skip
    /// a redraw for a redundant message. Open/close is **not** here —
    /// those ride [`UserCommand`]s (see the module header).
    pub fn apply(&mut self, message: BatchDialogMessage) -> bool {
        match message {
            BatchDialogMessage::DirectorySelected(index) => {
                apply_field(&mut self.model.selected_dir_idx, index)
            }
            BatchDialogMessage::PromptSelected(index) => {
                apply_field(&mut self.model.selected_prompt_idx, index)
            }
            BatchDialogMessage::PatternChanged { text } => {
                apply_field(&mut self.model.pattern, text)
            }
            BatchDialogMessage::ModeChanged(mode) => apply_field(&mut self.model.mode, mode),
            BatchDialogMessage::ConcurrencyChanged(concurrency) => {
                apply_field(&mut self.model.concurrency, concurrency)
            }
        }
    }
}

/// Writes one field and reports whether it changed.
fn apply_field<T: PartialEq>(field: &mut T, value: T) -> bool {
    if *field == value {
        return false;
    }
    *field = value;
    true
}

/// Renders the batch card.
///
/// The card is the *content* only: the root view mounts it through
/// [`crate::ui_iced::overlay::modal`] with [`cancel_message()`] as the
/// dismiss message so a scrim click or Escape cancels.
///
/// ```text
/// configuration view                          progress view
/// ───────────────────────────────           ───────────────────────────────
/// Directory:  [ pick list          v ]      Batch Processing
/// Pattern:    [ *.md             (inert    ───────────────────────────────
///                in Directory mode) ]       Processing...
/// Prompt:                                 Click Cancel to stop processing...
/// ┌ scrollable candidate rows ──┐         ───────────────────────────────
/// │ System / Skills/Batch/Foo   │                        [ Cancel ]
/// └─────────────────────────────┘
/// Mode:       (•) File   ( ) Directory
/// Concurrency:[ 4 v ]
/// ───────────────────────────────
///                    [ Process ] [ Cancel ]
/// ```
///
/// Emitted messages:
/// * directory combo → [`BatchDialogMessage::DirectorySelected`]
/// * prompt row → [`BatchDialogMessage::PromptSelected`]
/// * pattern field → [`BatchDialogMessage::PatternChanged`]
/// * mode radios → [`BatchDialogMessage::ModeChanged`]
/// * concurrency combo → [`BatchDialogMessage::ConcurrencyChanged`]
/// * Process → [`UserCommand::StartBatch`] via [`start_message`]
/// * Cancel / Close → [`UserCommand::CancelBatch`] via [`cancel_message`]
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn batch_dialog_card(view_model: &BatchDialogViewModel) -> Element<'_, Message> {
    if view_model.status.shows_progress_view() {
        progress_card(view_model)
    } else {
        config_card(view_model)
    }
}

/// The idle configuration card.
fn config_card<'a>(view_model: &'a BatchDialogViewModel) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();
    let label = |text_value: &'static str| {
        text(text_value)
            .size(BATCH_BODY_SIZE)
            .color(palette.text_strong)
    };

    let directory_options = view_model.directory_options();
    let selected_directory = view_model
        .selected_dir_idx
        .and_then(|idx| directory_options.get(idx))
        .cloned();

    let directory_row = Row::with_children(vec![
        label(strings::BATCH_DIALOG_DIRECTORY_LABEL).into(),
        pick_list(directory_options, selected_directory, |option| {
            Message::Batch(BatchDialogMessage::DirectorySelected(Some(option.index)))
        })
        .placeholder(strings::BATCH_DIALOG_SELECT_DIRECTORY)
        .into(),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .align_y(Alignment::Center);

    let pattern_row = Row::with_children(vec![
        label(strings::BATCH_DIALOG_PATTERN_LABEL).into(),
        text_input(strings::BATCH_DIALOG_PATTERN_HINT, &view_model.pattern)
            // In Directory mode the pattern is unused, so the field stays
            // allocated but stops reporting edits.
            .on_input_maybe(
                view_model
                    .pattern_editable()
                    .then_some(|text| Message::Batch(BatchDialogMessage::PatternChanged { text })),
            )
            .into(),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .align_y(Alignment::Center);

    let prompt_section = Column::with_children(vec![
        label(strings::BATCH_DIALOG_PROMPT_LABEL).into(),
        scrollable(prompt_rows(view_model))
            .height(Length::Fixed(BATCH_PROMPT_LIST_MAX_HEIGHT))
            .into(),
    ])
    .spacing(2.0);

    let mode_row = Row::with_children(vec![
        label(strings::BATCH_DIALOG_MODE_LABEL).into(),
        iced::widget::radio(
            strings::BATCH_MODE_FILE,
            BatchMode::File,
            Some(view_model.mode),
            |mode| Message::Batch(BatchDialogMessage::ModeChanged(mode)),
        )
        .into(),
        iced::widget::radio(
            strings::BATCH_MODE_DIRECTORY,
            BatchMode::Directory,
            Some(view_model.mode),
            |mode| Message::Batch(BatchDialogMessage::ModeChanged(mode)),
        )
        .into(),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .align_y(Alignment::Center);

    let concurrency_row = Row::with_children(vec![
        label(strings::BATCH_DIALOG_CONCURRENCY_LABEL).into(),
        pick_list(
            BatchDialogViewModel::concurrency_options(),
            Some(view_model.concurrency),
            |value| Message::Batch(BatchDialogMessage::ConcurrencyChanged(value)),
        )
        .into(),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .align_y(Alignment::Center);

    Column::with_children(vec![
        directory_row.into(),
        pattern_row.into(),
        prompt_section.into(),
        mode_row.into(),
        concurrency_row.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        config_footer(view_model),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .padding(BATCH_SECTION_SPACING)
    .width(Length::Fixed(BATCH_CARD_WIDTH))
    .into()
}

/// The scrollable prompt-candidate rows — real selectable rows rather than
/// a combo, the same shape the move dialog's destination
/// picker uses (`ui_iced/dialogs.rs:644-661`).
fn prompt_rows(view_model: &BatchDialogViewModel) -> Column<'_, Message> {
    view_model.available_prompts.iter().enumerate().fold(
        Column::new().spacing(2.0),
        |rows, (index, prompt)| {
            rows.push(prompt_row(
                prompt,
                index,
                view_model.selected_prompt_idx == Some(index),
            ))
        },
    )
}

/// One selectable prompt-candidate row.
fn prompt_row(prompt: &PromptInfo, index: usize, selected: bool) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let label = text(prompt.display_name.clone()).size(BATCH_BODY_SIZE);
    let surface = container(label)
        .width(Length::Fill)
        .padding(4.0)
        .style(move |_theme| container::Style {
            background: selected.then_some(Background::Color(palette.accent)),
            ..container::Style::default()
        });

    mouse_area(surface)
        .on_press(Message::Batch(BatchDialogMessage::PromptSelected(Some(
            index,
        ))))
        .into()
}

/// The configuration footer: spacer plus the Process / Cancel pair.
fn config_footer(view_model: &BatchDialogViewModel) -> Element<'_, Message> {
    let confirm = start_message(view_model).unwrap_or_else(cancel_message);
    let buttons = confirm_cancel_buttons(
        strings::BATCH_PROCESS_BUTTON,
        view_model.can_start(),
        move |outcome| match outcome {
            ConfirmCancel::Confirm => confirm.clone(),
            ConfirmCancel::Cancel => cancel_message(),
        },
    );

    Row::with_children(vec![
        space::Space::new().width(Length::Fill).into(),
        buttons.into(),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .align_y(Alignment::Center)
    .into()
}

/// The in-flight / completed progress card.
fn progress_card(view_model: &BatchDialogViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();
    let status = view_model.status;

    let heading = text(strings::BATCH_RUNNING_HEADING)
        .size(BATCH_HEADING_SIZE)
        .color(palette.text_strong);

    let body = if let Some(completed) = status.completion_text() {
        let cancelled = status.cancellation_text().unwrap_or("");
        Column::with_children(vec![
            text(completed).size(BATCH_BODY_SIZE).into(),
            text(cancelled)
                .size(BATCH_BODY_SIZE)
                .style(move |_theme| iced::widget::text::Style {
                    color: Some(palette.warning),
                })
                .into(),
        ])
        .spacing(2.0)
        .into()
    } else {
        Column::with_children(vec![
            text(strings::BATCH_PROCESSING_TEXT)
                .size(BATCH_BODY_SIZE)
                .into(),
            text(strings::BATCH_RUNNING_CANCEL_HINT)
                .size(BATCH_BODY_SIZE)
                .color(palette.text_muted)
                .into(),
        ])
        .spacing(2.0)
        .into()
    };

    let button_label = if status.finished {
        strings::BATCH_CLOSE_BUTTON
    } else {
        strings::CANCEL_BUTTON
    };

    let footer = Row::with_children(vec![
        space::Space::new().width(Length::Fill).into(),
        button(text(button_label)).on_press(cancel_message()).into(),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .align_y(Alignment::Center);

    Column::with_children(vec![
        heading.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        body,
        rule::horizontal(SEPARATOR_WIDTH).into(),
        footer.into(),
    ])
    .spacing(BATCH_SECTION_SPACING)
    .padding(BATCH_SECTION_SPACING)
    .width(Length::Fixed(BATCH_CARD_WIDTH))
    .into()
}
