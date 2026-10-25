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
//! | Candidate directories | `Workspace::dirs_sorted()` / the content-library roots, snapshotted by the reducer |
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
    Column, Row, Space, container, mouse_area, pick_list, scrollable, text, text_input,
};
use iced::{Alignment, Background, Element, Length, Size};

use iced_pro::composite::{form_field, list_tile};
use iced_pro::state::BaseState;
use iced_pro::surface::modal::{CONTENT_HEIGHT_MAX, ModalFrame, ModalSizing};
use iced_pro::token::{space, typography};
use iced_pro::widget::button::ButtonVariant;
use iced_pro::widget::button_widget::Button;
use iced_pro::widget::combo_box;
use iced_pro::widget::progress;
use iced_pro::widget::progress_model::{ProgressModel, ProgressStatus};

use crate::agent::batch::types::{BatchDialogConfig, validate_batch_params};
use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{BatchDialogMessage, Message};
use crate::ui_iced::overlay::{modal_card, modal_centered, modal_divider};

/// The batch payload the Process button composes — reused unchanged from
/// the batch subsystem ([RUST-051]); it is the `UserCommand::StartBatch`
/// payload (`src/app/bus/events/user_command.rs:134`).
pub use crate::agent::batch::types::{BatchConfig, BatchMode, PromptInfo};

/// §4.6.2 — the Batch Processing card's sizing policy: 560px wide clamped
/// to the viewport gutter, content-fit up to [`CONTENT_HEIGHT_MAX`].
///
/// Held as the policy object rather than two numbers, so the card states
/// *which* modal it is and the framework does the clamping. This replaced
/// the app's fixed `BATCH_CARD_WIDTH = 500`, which was 60px narrower than
/// the spec's `BATCH_MODAL_MAX` and was the same width on a laptop and a
/// 4K monitor.
pub const BATCH_SIZING: ModalSizing = ModalSizing::batch_processing();

/// The Batch card's **outer** width for a viewport width (§4.6.2).
///
/// 560px on any viewport wide enough to hold it, clamped to
/// `viewport.width - VIEWPORT_GUTTER` (`space::XXL`, 32px) below that.
/// The card's padding is *inside* this figure — see
/// [`crate::ui_iced::overlay::modal_card`].
#[must_use]
pub fn batch_card_width(viewport_width: f32) -> f32 {
    BATCH_SIZING.width.resolve(viewport_width, 0.0)
}

/// The Batch card's **outer** height for a viewport height (§4.6.2).
///
/// The policy is content-fit capped at [`CONTENT_HEIGHT_MAX`] (480px),
/// clamped last by the viewport gutter. Because the app cannot measure its
/// own content before laying out, the card is mounted *at* that ceiling
/// with its body in a scrollable — which is exactly what the framework
/// constant's own words describe: "the tallest a content-fit modal body
/// is allowed to get before it has to scroll".
///
/// The alternative — shrinking to the content — is not available at this
/// card's shape: five stacked [`form_field`]
/// rows alone consume `5 × STACKED_HEIGHT` = 380px before the prompt
/// candidate list is given a single pixel, so a content-fit card would
/// already be at the ceiling with no room for the one region that actually
/// needs it. See the module docs.
#[must_use]
pub fn batch_card_height(viewport_height: f32) -> f32 {
    BATCH_SIZING
        .height
        .resolve(viewport_height, CONTENT_HEIGHT_MAX)
}

/// The Batch card's outer size for a viewport (§4.6.2).
#[must_use]
pub fn batch_card_size(viewport: Size) -> Size {
    Size::new(
        batch_card_width(viewport.width),
        batch_card_height(viewport.height),
    )
}

/// Stable id of the Batch card box itself, so a layout test can address
/// the rendered card rather than only its contents (AGENTS §5).
pub const BATCH_CARD_ID: &str = "batch.card";

/// Stable focus id of the footer `Process` button.
pub const BATCH_PROCESS_BUTTON_ID: &str = "batch.process";

/// Stable focus id of the footer `Cancel` / `Close` button.
pub const BATCH_CANCEL_BUTTON_ID: &str = "batch.cancel";

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

    /// The wildcard pattern field's own failure message, `None` when the
    /// field is acceptable.
    ///
    /// The **decision** is the batch subsystem's, not this card's: it is
    /// `validate_batch_params` pinned at [`CONCURRENCY_MIN`] so the
    /// concurrency term can never be the one that fails and what the field
    /// reports is genuinely the pattern's own fault. In Directory mode the
    /// subsystem skips the pattern entirely, so this is `None` there.
    ///
    /// Only the **wording** is local, and it is a `&'static str` on
    /// purpose: the `form_field` caption band borrows its message for as
    /// long as the card lives, and a `String` minted here could not
    /// outlive the frame that rendered it. The subsystem's raw
    /// `glob::PatternError` text is deliberately not surfaced — it is
    /// developer-facing, and §4 puts user-facing wording in
    /// `ui/strings.rs`.
    ///
    /// Purity: pure.
    #[must_use]
    pub fn pattern_error(&self) -> Option<&'static str> {
        if validate_batch_params(self.mode, &self.pattern, CONCURRENCY_MIN).is_ok() {
            return None;
        }
        // The trimmed-empty test picks *which* message to show, never
        // *whether* one is shown — the failure itself came from the
        // subsystem above, so this cannot widen or narrow the rule.
        if self.pattern.trim().is_empty() {
            Some(strings::BATCH_PATTERN_REQUIRED_ERROR)
        } else {
            Some(strings::BATCH_PATTERN_INVALID_ERROR)
        }
    }

    /// Renders this snapshot as the batch card ([`batch_dialog_card`]).
    #[must_use]
    pub fn view(&self, viewport: Size) -> Element<'_, Message> {
        batch_dialog_card(self, viewport)
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
            BatchDialogMessage::Absorbed => false,
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

/// Renders the batch card at its §4.6.2 size.
///
/// The card is the *content* only: [`batch_modal`] mounts it through
/// [`crate::ui_iced::overlay::modal_centered`], which supplies the scrim
/// and the card-boundary absorption.
///
/// ```text
/// ┌─ Batch card · ModalSizing::batch_processing() · radius 6 ────────┐
/// │ Batch Prompt Processing                                          │
/// │ ──────────────────────────────────────────────────────────────── │
/// │ Directory:                                            (required) │
/// │ [ Select directory...                                       v ]  │
/// │ Pattern:                                                    hint │
/// │ [ *.md                                                     | ]   │
/// │ Mode:                                                            │
/// │ ( ) File   ( ) Directory                                         │
/// │ Concurrency:                                                     │
/// │ [ 4 v ]                                                          │
/// │ Prompt:                                                          │
/// │ ┌ scrollable candidate rows (list_tile, 32px each) ─────────────┐│
/// │ └───────────────────────────────────────────────────────────────┘│
/// │ ──────────────────────────────────────────────────────────────── │
/// │                          [ Cancel ] [ Process ]                  │
/// └─────────────────────────────────────────────────────────────────┘
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
/// Purity: pure view over `view_model` and `viewport`.
#[must_use]
pub fn batch_dialog_card(
    view_model: &BatchDialogViewModel,
    viewport: Size,
) -> Element<'_, Message> {
    if view_model.status.shows_progress_view() {
        progress_card(view_model, viewport)
    } else {
        config_card(view_model, viewport)
    }
}

/// The card's title bar (§4.6.1 — "Title (16px Bold)").
fn card_title() -> Element<'static, Message> {
    rung(
        strings::BATCH_DIALOG_WINDOW,
        typography::TITLE_SM,
        Palette::fastmd_dark().text_strong,
    )
    .into()
}

/// The idle configuration card.
///
/// Every labelled row is a [`form_field`] band triple: the framework
/// resolves the label, the required marker and the caption's voice, so no
/// row here restates a colour or a point size. The rows are stacked in a
/// fixed-height card whose remaining space is taken by the prompt
/// candidate list — see [`batch_card_height`] for why the card is mounted
/// at the policy ceiling rather than shrinking.
fn config_card<'a>(view_model: &'a BatchDialogViewModel, viewport: Size) -> Element<'a, Message> {
    let frame = modal_frame();

    let directory_options = view_model.directory_options();
    let selected_directory = view_model
        .selected_dir_idx
        .and_then(|idx| directory_options.get(idx))
        .cloned();

    let directory = stacked_field(
        strings::BATCH_DIALOG_DIRECTORY_LABEL,
        true,
        None,
        view_model.selected_dir_idx.is_some(),
        None,
        None,
        pick_list(directory_options, selected_directory, |option| {
            Message::Batch(BatchDialogMessage::DirectorySelected(Some(option.index)))
        })
        .placeholder(strings::BATCH_DIALOG_SELECT_DIRECTORY)
        .width(Length::Fill)
        .into(),
    );

    let pattern_error = view_model.pattern_error();
    let pattern = stacked_field(
        strings::BATCH_DIALOG_PATTERN_LABEL,
        view_model.pattern_editable(),
        None,
        pattern_error.is_none(),
        view_model
            .pattern_editable()
            .then_some(strings::BATCH_PATTERN_FIELD_HELPER),
        pattern_error,
        text_input(strings::BATCH_DIALOG_PATTERN_HINT, &view_model.pattern)
            // In Directory mode the pattern is unused, so the field stays
            // allocated but stops reporting edits.
            .on_input_maybe(
                view_model
                    .pattern_editable()
                    .then_some(|text| Message::Batch(BatchDialogMessage::PatternChanged { text })),
            )
            .width(Length::Fill)
            .into(),
    );

    let mode = stacked_field(
        strings::BATCH_DIALOG_MODE_LABEL,
        false,
        None,
        true,
        None,
        None,
        Row::with_children(vec![
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
        .spacing(space::LG)
        .align_y(Alignment::Center)
        .into(),
    );

    let concurrency = stacked_field(
        strings::BATCH_DIALOG_CONCURRENCY_LABEL,
        false,
        None,
        true,
        Some(strings::BATCH_CONCURRENCY_HELPER),
        None,
        pick_list(
            BatchDialogViewModel::concurrency_options(),
            Some(view_model.concurrency),
            |value| Message::Batch(BatchDialogMessage::ConcurrencyChanged(value)),
        )
        .width(Length::Fixed(combo_box::MIN_WIDTH))
        .into(),
    );

    let prompt = prompt_section(view_model);

    let body = Column::with_children(vec![directory, pattern, mode, concurrency, prompt])
        .spacing(space::MD);

    let card = modal_card(
        Column::with_children(vec![
            card_title(),
            modal_divider(frame).into(),
            body.into(),
            modal_divider(frame).into(),
            config_footer(view_model),
        ])
        .spacing(space::SM),
        frame,
        BATCH_CARD_ID,
        Length::Fixed(batch_card_width(viewport.width)),
        Length::Fixed(batch_card_height(viewport.height)),
    );

    card.into()
}

/// The Prompt section: the framework-resolved header, the scrollable
/// candidate list, and the reserved caption band.
///
/// This is the one labelled row that **cannot** use
/// [`form_field::stacked`]: that widget fixes its control band at
/// [`CONTROL_SLOT_HEIGHT`](form_field::CONTROL_SLOT_HEIGHT) — 32px, one
/// control — and the candidate list is a multi-row region that has to fill
/// whatever the card has left. The *decisions* still come from the
/// framework ([`form_field::resolve_header`] and
/// [`form_field::resolve_caption`]); only the middle band is this card's
/// own height. See the module docs.
fn prompt_section(view_model: &BatchDialogViewModel) -> Element<'_, Message> {
    let palette = Palette::pro();
    let selected = view_model.selected_prompt().is_some();

    Column::with_children(vec![
        field_header(
            strings::BATCH_DIALOG_PROMPT_LABEL,
            true,
            Some(strings::BATCH_PROMPT_FIELD_HINT),
            &palette,
        ),
        scrollable(prompt_rows(view_model))
            .height(Length::Fill)
            .width(Length::Fill)
            .into(),
        field_caption(form_field::resolve_caption(selected, None, None, &palette)),
    ])
    .spacing(form_field::BAND_GAP)
    .height(Length::Fill)
    .into()
}

/// The scrollable prompt-candidate rows — real selectable rows rather
/// than a combo, the same shape the move dialog's destination picker uses
/// (`ui_iced/dialogs.rs:644-661`), now measured on the framework's
/// list-tile grid.
fn prompt_rows(view_model: &BatchDialogViewModel) -> Column<'_, Message> {
    view_model
        .available_prompts
        .iter()
        .enumerate()
        .fold(Column::new(), |rows, (index, prompt)| {
            rows.push(prompt_row(
                prompt,
                index,
                view_model.selected_prompt_idx == Some(index),
            ))
        })
}

/// One selectable prompt-candidate row, laid out and painted from
/// [`list_tile`] (`UIF-COMP-005`).
///
/// The row is a single-line tile: [`tile_height`](list_tile::tile_height)
/// tall, inset by [`TILE_PADDING`](list_tile::TILE_PADDING), painted by
/// [`resolve_style`](list_tile::resolve_style). The selected fill is the
/// framework's `accent.fill` rather than the app's older `accent`, and the
/// row is 32px rather than a content-height `padding(4)` strip — which is
/// what puts a candidate on the same grid as every other selectable row
/// in the app and clears the `size::HIT_TARGET_MIN` floor for the whole
/// line.
fn prompt_row(prompt: &PromptInfo, index: usize, selected: bool) -> Element<'_, Message> {
    let style = list_tile::resolve_style(selected, false, BaseState::Rest, &Palette::pro());

    let label = rung(
        prompt.display_name.clone(),
        *list_tile::TITLE_RUNG,
        style.title_colour,
    );

    let surface = container(label)
        .width(Length::Fill)
        .height(Length::Fixed(list_tile::tile_height(
            list_tile::TileLines::Single,
        )))
        .padding(iced::Padding::from([0.0, list_tile::TILE_PADDING.left]))
        .align_y(Alignment::Center)
        .style(move |_theme: &iced::Theme| container::Style {
            background: if style.background.a == 0.0 {
                None
            } else {
                Some(Background::Color(style.background))
            },
            ..container::Style::default()
        });

    mouse_area(surface)
        .on_press(Message::Batch(BatchDialogMessage::PromptSelected(Some(
            index,
        ))))
        .into()
}

/// The configuration footer, laid out the way §4.6.1 draws it: status
/// info, `Fill`, then `[Cancel] [Primary]` right-aligned.
///
/// `Process` is `Primary` and `Cancel` is `Secondary`. Neither is
/// `Danger`: starting a batch is the forward action and cancelling one is
/// the ordinary leave-without-committing control, the same roles `Save`
/// and `Cancel` play on the editor card. The framework reserves `Danger`
/// for the confirmation surface where the user is actually being asked to
/// give something up.
///
/// `Process` is disabled rather than removed while the composition is
/// invalid (AGENTS §6), so the control stays visible and the reason stays
/// in the field captions above it.
fn config_footer(view_model: &BatchDialogViewModel) -> Element<'_, Message> {
    let process = footer_button(
        ButtonVariant::Primary,
        strings::BATCH_PROCESS_BUTTON,
        start_message(view_model).unwrap_or_else(cancel_message),
        BATCH_PROCESS_BUTTON_ID,
        view_model.can_start(),
    );

    let cancel = footer_button(
        ButtonVariant::Secondary,
        strings::CANCEL_BUTTON,
        cancel_message(),
        BATCH_CANCEL_BUTTON_ID,
        true,
    );

    Row::with_children(vec![
        Space::new().width(Length::Fill).into(),
        cancel,
        process,
    ])
    .spacing(space::SM)
    .align_y(Alignment::Center)
    .into()
}

/// The in-flight / completed progress card.
///
/// The bar is an **indeterminate** [`ProgressModel`]
/// (`UIF-ATOM-009`): [`BatchRunStatus`] carries only
/// `has_handle` / `finished` / `cancel_requested`, so there is no
/// completed-of-total count to render. Drawing a determinate bar here
/// would be inventing a number the batch subsystem never reported, so the
/// track is painted at the framework's height and radius in the
/// framework's status colour and left unfilled.
fn progress_card(view_model: &BatchDialogViewModel, viewport: Size) -> Element<'_, Message> {
    let frame = modal_frame();
    let palette = Palette::pro();
    let status = view_model.status;

    let model = ProgressModel::indeterminate().with_status(progress_status(status));
    let colour = progress::fill_colour(model.status(), &palette);

    let body = if let Some(completed) = status.completion_text() {
        Column::with_children(vec![
            rung(completed, typography::BODY, Palette::fastmd_dark().text).into(),
            status
                .cancellation_text()
                .map(|cancelled| {
                    rung(cancelled, typography::BODY, Palette::fastmd_dark().warning).into()
                })
                .unwrap_or_else(|| Space::new().height(Length::Shrink).into()),
        ])
        .spacing(space::XXS)
        .into()
    } else {
        Column::with_children(vec![
            rung(
                strings::BATCH_PROCESSING_TEXT,
                typography::BODY,
                Palette::fastmd_dark().text,
            )
            .into(),
            rung(
                strings::BATCH_RUNNING_CANCEL_HINT,
                typography::CAPTION,
                Palette::fastmd_dark().text_muted,
            )
            .into(),
        ])
        .spacing(space::XXS)
        .into()
    };

    let button_label = if status.finished {
        strings::BATCH_CLOSE_BUTTON
    } else {
        strings::CANCEL_BUTTON
    };

    let footer = Row::with_children(vec![
        Space::new().width(Length::Fill).into(),
        footer_button(
            ButtonVariant::Secondary,
            button_label,
            cancel_message(),
            BATCH_CANCEL_BUTTON_ID,
            true,
        ),
    ])
    .spacing(space::SM)
    .align_y(Alignment::Center);

    modal_card(
        Column::with_children(vec![
            card_title(),
            modal_divider(frame).into(),
            progress_track(colour),
            body,
            modal_divider(frame).into(),
            footer.into(),
        ])
        .spacing(space::SM),
        frame,
        BATCH_CARD_ID,
        Length::Fixed(batch_card_width(viewport.width)),
        Length::Shrink,
    )
    .into()
}

/// The indeterminate progress track — the framework's own bar box.
///
/// [`TRACK_HEIGHT`](progress::TRACK_HEIGHT) tall with
/// [`TRACK_RADIUS`](progress::TRACK_RADIUS) ends, painted in the resolved
/// fill colour. No fraction is drawn, because the model has none.
fn progress_track(colour: iced::Color) -> Element<'static, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(progress::TRACK_HEIGHT))
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: iced::Border {
                color: colour,
                width: 0.0,
                radius: progress::TRACK_RADIUS.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// The [`ProgressStatus`] a batch run maps onto.
///
/// A cancellation request is `Paused`, not `Error`: the user stopped it,
/// nothing failed. A finished run is reported as `Active` because
/// [`ProgressStatus`] has **no completed position** — the completion
/// text, not the bar, carries "done". That is a gap in `UIF-ATOM-009`
/// reported rather than papered over with a colour that means something
/// else.
#[must_use]
pub fn progress_status(status: BatchRunStatus) -> ProgressStatus {
    if status.cancel_requested {
        ProgressStatus::Paused
    } else {
        ProgressStatus::Active
    }
}

/// The frame this card is drawn with (§4.6.1).
#[must_use]
pub fn modal_frame() -> ModalFrame {
    ModalFrame::resolve(&Palette::pro())
}

/// A labelled field whose control fits the framework's fixed 32px control
/// band (`UIF-COMP-004`, §3B.1).
fn stacked_field<'a>(
    label: &'static str,
    required: bool,
    hint: Option<&'static str>,
    valid: bool,
    helper: Option<&'static str>,
    error: Option<&'static str>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let palette = Palette::pro();
    form_field::stacked(
        form_field::resolve_header(label, required, hint, &palette),
        control,
        form_field::resolve_caption(valid, helper, error, &palette),
    )
}

/// The header band [`stacked_field`] renders internally, exposed for the
/// one row whose control is taller than the fixed band
/// ([`prompt_section`]).
fn field_header<'a>(
    label: &'static str,
    required: bool,
    hint: Option<&'static str>,
    palette: &iced_pro::token::color::Palette,
) -> Element<'a, Message> {
    let parts = form_field::resolve_header(label, required, hint, palette);

    let mut row = Row::new()
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .push(rung(
            parts.label,
            typography::UI_COMPACT,
            parts.label_colour,
        ));

    if let Some(marker) = parts.marker {
        row = row.push(rung(
            marker.to_string(),
            typography::UI_COMPACT,
            parts.marker_colour,
        ));
    }

    if let Some(hint_text) = parts.hint {
        row = row.push(
            container(rung(hint_text, typography::CAPTION, parts.hint_colour))
                .width(Length::Fill)
                .align_x(Alignment::End),
        );
    }

    container(row)
        .height(Length::Fixed(form_field::HEADER_HEIGHT))
        .into()
}

/// The reserved caption band [`stacked_field`] renders internally,
/// exposed for [`prompt_section`].
///
/// The band is always [`CAPTION_SLOT_HEIGHT`](form_field::CAPTION_SLOT_HEIGHT)
/// tall whether or not it has anything to say — that is the whole point of
/// a *reserved* band: a caption arriving cannot push the control above it
/// and a caption leaving cannot pull it back.
fn field_caption(parts: form_field::CaptionParts<'_>) -> Element<'static, Message> {
    let mut row = Row::new()
        .width(Length::Fill)
        .spacing(space::XXS)
        .align_y(Alignment::Center);

    if let Some(glyph) = parts.glyph {
        row = row.push(rung(glyph.to_string(), typography::CAPTION, parts.colour));
    }

    if let Some(value) = parts.text {
        row = row.push(rung(value, typography::CAPTION, parts.colour));
    }

    container(row)
        .height(Length::Fixed(form_field::CAPTION_SLOT_HEIGHT))
        .align_y(Alignment::Center)
        .into()
}

/// A footer-width framework button carrying `label`.
///
/// The label is mounted in a fixed
/// [`BUTTON_MIN_WIDTH`](iced_pro::surface::input_dialog::BUTTON_MIN_WIDTH)
/// box on purpose. `Button::is_icon_only` is a **geometric** test — any
/// content no wider and no taller than `size::CONTROL_SM` (28px) is
/// classed icon-only, and an icon-only button without a tooltip trips a
/// `debug_assert!` in `outer_size` (`button_widget.rs:776`). A short
/// label like `'Cancel'` sits close enough to that line that a different
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
            iced_pro::surface::input_dialog::BUTTON_MIN_WIDTH,
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
/// **`line_height` must be handed to iced as a resolved `LineHeight`, not
/// a bare `f32`.** `impl From<f32> for iced::LineHeight` produces
/// `LineHeight::Relative(factor)` — a *multiple* of the font size — so
/// `.line_height(20.0)` on a 14px rung means 280px per line.
/// [`TypeRung::iced_line_height`] is the crossing point that makes
/// picking the wrong unit impossible.
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

/// The blocking batch modal: [`batch_dialog_card`] centred over a scrim
/// by [`crate::ui_iced::overlay::modal_centered`].
///
/// # What each click means
///
/// The scrim emits [`UserCommand::CancelBatch`] on [`cancel_message`]'s
/// channel, the same single cancel transition the Cancel button uses. A
/// press that lands on the card's padding ring, on a disabled `Process`,
/// or on dead space between rows is absorbed by
/// [`BatchDialogMessage::Absorbed`] and never reaches the scrim
/// (`UIF-VER-003`) — before this increment the card was mounted through
/// the older `overlay::modal`, which had **no absorb boundary at all**,
/// so a press on dead space inside the card fell through and cancelled
/// the dialog.
#[must_use]
pub fn batch_modal(view_model: &BatchDialogViewModel, viewport: Size) -> Element<'_, Message> {
    modal_centered(
        batch_dialog_card(view_model, viewport),
        Message::Batch(BatchDialogMessage::Absorbed),
        cancel_message(),
    )
}
