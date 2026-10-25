//! The small individual dialogs — the shared name-entry dialog
//! (create-directory UI-011 / create-document UI-015), the rename
//! dialog, the move dialog and the delete-confirmation dialog.
//!
//! This is a **pure view over plain state**: [`dialog_card`] renders a
//! [`DialogState`] into an [`iced::Element`] of [`Message`] and never
//! touches the bus, the disk or `FastMdApp`. The reducer owns the
//! filesystem action: it opens the dialog from the tree's deferred
//! intents, runs [`DialogState::submit`] on the confirm message, and
//! publishes the matching `UserCommand::Confirm*` only when validation
//! passes.
//!
//! # The framework owns the box, the app owns the answer
//!
//! As of integration increment INT-4 this module is a consumer of
//! [`iced_pro::surface::input_dialog`] rather than a re-implementation of
//! §4.5. Nothing here carries a dialog measurement of its own:
//!
//! | Concern | Owner |
//! |---|---|
//! | Card width, row heights, gaps, padding | `input_dialog` tokens, re-exported below |
//! | Which control the middle row holds | [`DialogKind::input_control`] producing [`InputControl`] |
//! | Card size | [`iced_pro::surface::input_dialog::InputLayout::card_size`] via [`dialog_size`] |
//! | Where the card lands | [`iced_pro::geometry::anchor::solve`] with `Anchor::CursorProximal` |
//! | Whether the primary button is destructive | [`DialogKind::button_intent`] producing [`ButtonIntent`] |
//! | When an invalid field is allowed to *look* invalid | [`iced_pro::validation::punish::punish`] |
//! | Button and field painting | `iced_pro::widget::button_widget::Button`, `iced_pro::widget::text_input::TextInput` |
//!
//! The value, the verdict and the commit history stay here, because the
//! dialog is the thing that knows what a name means in this application.
//!
//! # State model
//!
//! [`DialogState`] holds a single `Option<DialogKind>` rather than one
//! independent flag per dialog. The overlay contract
//! ([`crate::ui_iced::overlay::OverlaySlot`]) allows exactly one blocking
//! modal, so opening a dialog replaces whatever was open — the same rule
//! [`crate::ui_iced::overlay::OverlaySlot::open_modal`] enforces.
//!
//! The name field is a [`iced_pro::widget::text_input_model::TextInputModel`]
//! rather than a bare `String`. That is not decoration: the field's
//! *commit history* — whether the user has ever finished with it — is a
//! fact about the field, and the framework will only carry it on the model
//! that owns the value. A `String` here could not have held it, which is
//! the same trap B-4 hit with the password mirror.
//!
//! # Validation lives in the dialog, timing lives in the framework
//!
//! A window-level check for a non-blank name and a selected destination is
//! not enough on its own: the real name rule lives in the command executor
//! — `crate::utils::path::is_safe_basename`
//! (`src/app/utils/path.rs:13-27`), called at
//! `src/app/command_executor.rs:309/338/365` — which silently skips the
//! operation on a bad name. [`validate_name`] merges both layers into one
//! pure rule and surfaces the failure as a visible error line rather than
//! closing silently.
//!
//! *Whether* the value is valid is therefore the app's answer, and it is
//! recomputed on every keystroke. **When the user is told** is not: that
//! goes through [`iced_pro::validation::punish::punish`], so a pristine
//! field with a bad name keeps its red border withheld while a character
//! the filesystem can never accept — a `/` in a basename — flags
//! immediately, before the user has finished typing the whole path
//! (`UIF-PRIN-009`, `UIF-INP-003`). Read the answer through
//! [`DialogState::painted_error`] and never through the raw verdict.
//!
//! # Anchor model
//!
//! The pointer position is captured once per open session and the window is
//! pinned there, falling back to the viewport centre when no pointer is
//! known. The reducer captures the tracked
//! [`crate::ui_iced::message::LayoutMessage::CursorMoved`] point when it
//! opens the dialog and passes it to [`dialog_card`], which resolves the
//! final top-left with [`resolve_dialog_anchor`] — a call to
//! [`iced_pro::geometry::anchor::solve`] with
//! [`iced_pro::geometry::anchor::Anchor::CursorProximal`]. This module owns
//! no clamping arithmetic and could not write one if it wanted to: the
//! `8px` viewport margin and the `+2px, +2px` cursor offset come out of
//! the shared solver, which is what `UIF-INP-001` asks for and what the
//! app's retired hand-rolled clamp did not do.
//!
//! # The delete confirmation
//!
//! [`DialogKind::DeleteConfirm`] adds an explicit confirm step: the tree's
//! Delete row can publish `UserCommand::Delete` on its own, so the reducer
//! decides whether to route it through the dialog ([RUST-041]). It is the
//! one kind whose primary button is [`ButtonIntent::Danger`]
//! (`UIF-INP-004`).
//!
//! Unit tests live in the sibling `dialogs_tests.rs` sidecar.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use iced::widget::{Column, container, mouse_area, scrollable, space, text};
use iced::{Alignment, Background, Element, Length, Padding, Point, Rectangle, Size};

use iced_pro::geometry::anchor::{self, Anchor};
use iced_pro::surface::input_dialog::{
    self as input_dialog, BUTTON_MIN_WIDTH, ButtonIntent, CARD_WIDTH, CONTEXT_ROW_HEIGHT,
    DeleteTargets, FOOTER_HEIGHT, InputControl, InputLayout, ROW_GAP, Selection, TITLE_ROW_HEIGHT,
    VALIDATION_SLOT_HEIGHT, folder_list_height,
};
use iced_pro::token::typography;
use iced_pro::validation::disallowed::DisallowedClass;
use iced_pro::validation::punish::Punishment;
use iced_pro::widget::button::ButtonVariant;
use iced_pro::widget::button_widget::Button;
use iced_pro::widget::text_input::{InputKind, TextInput};
use iced_pro::widget::text_input_controller::TextInputEmission;
use iced_pro::widget::text_input_model::TextInputModel;

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{DialogMessage, Message};
use crate::ui_iced::overlay::modal_at;
use crate::utils::path::is_safe_basename;

#[cfg(test)]
#[path = "dialogs_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Token rebinding (INT-4)
// ---------------------------------------------------------------------------
//
// Every measurement a §4.5 card is built from is a re-export of the
// framework's own token. The app-side literals these replaced are gone;
// there is no number here for the two to disagree about.

/// Width of every small-dialog card (§4.5.1 `CARD_WIDTH`).
pub use input_dialog::CARD_WIDTH as DIALOG_WIDTH;

/// Reserved height of the title row (§4.5.1, `TITLE_ROW_HEIGHT`).
pub use input_dialog::TITLE_ROW_HEIGHT as DIALOG_TITLE_HEIGHT;

/// Reserved height of the prompt row (§4.5.1, `CONTEXT_ROW_HEIGHT`).
pub use input_dialog::CONTEXT_ROW_HEIGHT as DIALOG_PROMPT_HEIGHT;

/// Reserved height of the name-entry field row (§4.5.1, `TEXT_ENTRY_HEIGHT`).
pub use input_dialog::TEXT_ENTRY_HEIGHT as DIALOG_INPUT_HEIGHT;

/// Reserved height of the always-allocated error row (§4.5.1,
/// `VALIDATION_SLOT_HEIGHT`).
pub use input_dialog::VALIDATION_SLOT_HEIGHT as DIALOG_ERROR_HEIGHT;

/// Reserved height of the confirm/cancel footer row (§4.5.1, `FOOTER_HEIGHT`).
pub use input_dialog::FOOTER_HEIGHT as DIALOG_BUTTONS_HEIGHT;

/// Vertical gap between adjacent card rows (§4.5.1, `ROW_GAP`).
pub use input_dialog::ROW_GAP as DIALOG_SECTION_SPACING;

/// Maximum height of the move dialog's destination list (§4.5.1,
/// `FOLDER_LIST_MAX_HEIGHT`).
pub use input_dialog::FOLDER_LIST_MAX_HEIGHT as MOVE_LIST_MAX_HEIGHT;

/// Height of one destination row in the move list (§4.5.1,
/// `FOLDER_ROW_HEIGHT`).
pub use input_dialog::FOLDER_ROW_HEIGHT as MOVE_ROW_HEIGHT;

/// Logical point size of the dialog title — the `title_sm` rung.
pub const DIALOG_TITLE_SIZE: f32 = typography::TITLE_SM.size;

/// Logical point size of the prompt and destination rows — the `body` rung.
pub const DIALOG_BODY_SIZE: f32 = typography::BODY.size;

/// Logical point size of the error line — the `caption` rung.
pub const DIALOG_ERROR_SIZE: f32 = typography::CAPTION.size;

/// Horizontal inset inside one move-destination row — the `space.xs` step.
///
/// The last measurement the row needed a home for; naming it here keeps the
/// row builder free of bare numbers the same way the eight rebinds above do.
pub const MOVE_ROW_INSET: f32 = iced_pro::token::space::XS;

// ---------------------------------------------------------------------------
// Stable widget identities (§5 of the app's iced Id rules)
// ---------------------------------------------------------------------------
//
// One dialog is live at a time, so each of these addresses exactly one
// widget. They are `&'static str` rather than derived from the framework's
// per-surface `control_id` role strings because a `format!` in `view`
// would hand the same control a new id every frame.

/// The name-entry field's id — the framework's `ENTRY_ROLE` control.
pub const DIALOG_ENTRY_ID: &str = "dialog.entry";

/// The confirm button's id — the framework's `OK_ROLE` control.
pub const DIALOG_OK_ID: &str = "dialog.ok";

/// The cancel button's id — the framework's `CANCEL_ROLE` control.
pub const DIALOG_CANCEL_ID: &str = "dialog.cancel";

/// The move dialog's destination list scrollable id.
pub const DIALOG_MOVE_LIST_ID: &str = "dialog.move_list";

/// Which small dialog is open, carrying the target(s) the reducer needs to
/// execute the action.
///
/// Each payload is self-contained: the create kinds carry `parent`, the
/// rename / move / delete kinds carry `path`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogKind {
    /// Create a new directory under `parent` (UI-011).
    CreateDirectory {
        /// Directory the new one is created in.
        parent: PathBuf,
    },
    /// Create a new markdown document under `parent` (UI-015).
    CreateDocument {
        /// Directory the new document is created in.
        parent: PathBuf,
    },
    /// Rename the file or directory at `path` (UI-012).
    Rename {
        /// The path being renamed.
        path: PathBuf,
    },
    /// Move the file or directory at `path` to a picked destination
    /// (UI-013).
    Move {
        /// The path being moved.
        path: PathBuf,
    },
    /// Confirm deleting the file or directory at `path` (see the
    /// module header).
    DeleteConfirm {
        /// The path the user is being asked about.
        path: PathBuf,
    },
}

impl DialogKind {
    /// The primary target of the dialog: the parent directory for the
    /// create kinds, the operated-on path otherwise.
    #[must_use]
    pub fn target(&self) -> &Path {
        match self {
            Self::CreateDirectory { parent } | Self::CreateDocument { parent } => parent,
            Self::Rename { path } | Self::Move { path } | Self::DeleteConfirm { path } => path,
        }
    }

    /// The card title for this kind, from `crate::ui::strings`.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::CreateDirectory { .. } => strings::CREATE_DIRECTORY_WINDOW,
            Self::CreateDocument { .. } => strings::CREATE_DOCUMENT_WINDOW,
            Self::Rename { .. } => strings::RENAME_WINDOW,
            Self::Move { .. } => strings::MOVE_FILE_WINDOW,
            Self::DeleteConfirm { .. } => strings::DELETE_CONFIRM_WINDOW,
        }
    }

    /// The static prompt line for this kind, `None` for the delete
    /// confirmation whose prompt interpolates the target name
    /// ([`strings::format_delete_confirm_prompt`]).
    #[must_use]
    pub fn prompt(&self) -> Option<&'static str> {
        match self {
            Self::CreateDirectory { .. } => Some(strings::ENTER_DIRECTORY_NAME),
            Self::CreateDocument { .. } => Some(strings::ENTER_DOCUMENT_NAME),
            Self::Rename { .. } => Some(strings::ENTER_NEW_NAME),
            Self::Move { .. } => Some(strings::SELECT_DESTINATION_FOLDER),
            Self::DeleteConfirm { .. } => None,
        }
    }

    /// The confirm button label: `Delete` for the confirmation,
    /// [`strings::OK_BUTTON`] for every other kind.
    ///
    /// Both labels are pinned verbatim by the Host `SPEC.md` Literal Pin
    /// Table and are deliberately **not** restyled by this module.
    #[must_use]
    pub fn confirm_label(&self) -> &'static str {
        match self {
            Self::DeleteConfirm { .. } => strings::DELETE_ACTION,
            _ => strings::OK_BUTTON,
        }
    }

    /// Whether this kind collects a name via the text field.
    #[must_use]
    pub fn is_name_entry(&self) -> bool {
        matches!(
            self,
            Self::CreateDirectory { .. } | Self::CreateDocument { .. } | Self::Rename { .. }
        )
    }

    /// The target's file name for display (the delete prompt's subject);
    /// falls back to the full path when the target has no file name
    /// (e.g. a filesystem root).
    #[must_use]
    pub fn target_name(&self) -> String {
        let target = self.target();
        target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| target.to_string_lossy().into_owned())
    }

    /// What the card's middle row holds for this kind, given `state`
    /// (§4.5.1, `UIF-INP-004`).
    ///
    /// This is the mapping the box model is driven from: the create kinds
    /// and [`DialogKind::Rename`] take a [`InputControl::TextEntry`],
    /// [`DialogKind::Move`] the [`InputControl::FolderList`], and
    /// [`DialogKind::DeleteConfirm`] the
    /// [`InputControl::Confirmation`] whose control row collapses to
    /// nothing while the validation slot beside it stays reserved.
    ///
    /// `Rename` selects the filename **stem** so the extension survives the
    /// first keystroke; a dotfile keeps its whole name selected, which is
    /// what [`Selection::rename_stem`] decides.
    #[must_use]
    pub fn input_control(&self, state: &DialogState) -> InputControl {
        match self {
            Self::Rename { .. } => InputControl::TextEntry {
                initial: state.pending_name().to_string(),
                selection: Selection::rename_stem(state.pending_name()),
            },
            Self::CreateDirectory { .. } | Self::CreateDocument { .. } => InputControl::TextEntry {
                initial: state.pending_name().to_string(),
                selection: Selection::all(state.pending_name()),
            },
            Self::Move { .. } => InputControl::FolderList {
                folders: state
                    .destinations()
                    .iter()
                    .map(|folder| folder.to_string_lossy().into_owned())
                    .collect(),
                selected: state.selected_index(),
            },
            Self::DeleteConfirm { .. } => InputControl::Confirmation {
                summary: strings::format_delete_confirm_prompt(&self.target_name()),
            },
        }
    }

    /// How this kind's primary button is styled (`UIF-INP-004`).
    ///
    /// Only the delete confirmation is destructive. Every other kind is
    /// the ordinary primary action, and the framework's default agrees.
    #[must_use]
    pub fn button_intent(&self) -> ButtonIntent {
        match self {
            Self::DeleteConfirm { .. } => ButtonIntent::Danger,
            Self::CreateDirectory { .. }
            | Self::CreateDocument { .. }
            | Self::Rename { .. }
            | Self::Move { .. } => ButtonIntent::Standard,
        }
    }

    /// The delete confirmation's targets, as the framework models them.
    ///
    /// The app confirms one path at a time today, so this is always
    /// [`DeleteTargets::Single`]; spelling it out keeps the multi-delete
    /// story in the framework's vocabulary rather than a bare string.
    #[must_use]
    pub fn delete_targets(&self) -> Option<DeleteTargets> {
        match self {
            Self::DeleteConfirm { .. } => Some(DeleteTargets::Single(self.target_name())),
            Self::CreateDirectory { .. }
            | Self::CreateDocument { .. }
            | Self::Rename { .. }
            | Self::Move { .. } => None,
        }
    }
}

/// The widget variant a [`ButtonIntent`] is drawn with (`UIF-INP-004`).
///
/// The two vocabularies are deliberately different layers — the intent is
/// what the dialog *means*, the variant is how a button is *painted* — so
/// the bridge is one named function rather than a match repeated at each
/// call site.
#[must_use]
pub fn button_variant(intent: ButtonIntent) -> ButtonVariant {
    match intent {
        ButtonIntent::Danger => ButtonVariant::Danger,
        ButtonIntent::Standard => ButtonVariant::Primary,
    }
}

/// The outcome of [`DialogState::submit`].
///
/// `Completed` carries everything the reducer needs to publish the
/// matching `UserCommand` without re-reading dialog state that the close
/// has already cleared: the kind, the trimmed name (name-entry
/// kinds) and the picked destination (move). `Invalid` keeps the dialog
/// open with the error surfaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitOutcome {
    /// Validation passed and the dialog closed.
    Completed {
        /// The dialog that was submitted.
        kind: DialogKind,
        /// The trimmed name (name-entry kinds; empty otherwise).
        name: String,
        /// The picked destination (move only; `None` otherwise).
        destination: Option<PathBuf>,
    },
    /// Validation failed; the dialog stays open with `message` shown.
    Invalid {
        /// The user-facing error line.
        message: String,
    },
}

/// The small-dialog state model behind the name-entry / move / delete
/// dialogs.
///
/// Pure state: every transition is a plain method with no I/O. Only one
/// dialog is open at a time (see the module header).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DialogState {
    open: Option<DialogKind>,
    field: TextInputModel,
    error: Option<String>,
    destinations: Vec<PathBuf>,
    selected_destination: Option<PathBuf>,
}

impl DialogState {
    /// A state with no dialog open.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a dialog is currently open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The open dialog's kind.
    #[must_use]
    pub fn kind(&self) -> Option<&DialogKind> {
        self.open.as_ref()
    }

    /// The name-entry field's model — the thing the framework widget is
    /// bound to, and the only place the field's value and commit history
    /// live.
    #[must_use]
    pub fn field(&self) -> &TextInputModel {
        &self.field
    }

    /// The name-entry field's current text.
    #[must_use]
    pub fn pending_name(&self) -> &str {
        &self.field.value
    }

    /// The raw validation message the last check produced, whether or not
    /// the user is allowed to see it yet.
    ///
    /// Read [`painted_error`](Self::painted_error) to decide what the card
    /// draws. This is the fact; that is the presentation of it.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The move dialog's destination candidates.
    #[must_use]
    pub fn destinations(&self) -> &[PathBuf] {
        &self.destinations
    }

    /// The move dialog's currently picked destination.
    #[must_use]
    pub fn selected_destination(&self) -> Option<&Path> {
        self.selected_destination.as_deref()
    }

    /// Index of the picked destination in [`destinations`](Self::destinations),
    /// or `0` when nothing is picked — the shape
    /// [`InputControl::FolderList`] wants its selection in.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.destinations
            .iter()
            .position(|folder| Some(folder.as_path()) == self.selected_destination.as_deref())
            .unwrap_or(0)
    }

    /// Whether the user has committed to the name field at least once
    /// (`UIF-PRIN-009`).
    #[must_use]
    pub fn is_touched(&self) -> bool {
        self.field.is_touched()
    }

    /// Opens `kind`, replacing whatever dialog was open (one-at-a-time —
    /// see the module header) and clearing all transient fields.
    pub fn open(&mut self, kind: DialogKind) {
        *self = Self {
            open: Some(kind),
            ..Self::default()
        };
        self.refresh_validation();
    }

    /// Opens the move dialog for `path` with the reducer-supplied
    /// destination candidates.
    ///
    /// The candidate list is data the pure view cannot build itself: the
    /// reducer snapshots the destinations and hands them in on open.
    pub fn open_move(&mut self, path: PathBuf, destinations: Vec<PathBuf>) {
        *self = Self {
            open: Some(DialogKind::Move { path }),
            destinations,
            ..Self::default()
        };
        self.refresh_validation();
    }

    /// Closes the dialog and clears every transient field.
    ///
    /// Returns whether a dialog was actually open, so an Escape handler can
    /// tell "closed the dialog" from "did nothing" — the same contract as
    /// [`crate::ui_iced::overlay::OverlaySlot::close`].
    pub fn close(&mut self) -> bool {
        let was_open = self.is_open();
        *self = Self::default();
        was_open
    }

    /// The Cancel button / dismiss transition: closes the dialog.
    pub fn cancel(&mut self) {
        self.close();
    }

    /// Sets the pending name from the text field and re-runs the live
    /// validation rule (`UIF-INP-003`).
    ///
    /// Typing does **not** advance the field's commit stance. Per
    /// `UIF-PRIN-009` the transition is tied to commit / blur / submit,
    /// not to "the value changed" — a host that filled the field from the
    /// current filename has told the framework nothing about whether the
    /// user has finished with it.
    pub fn set_name(&mut self, text: String) {
        self.field.value = text;
        self.error = None;
        self.refresh_validation();
    }

    /// The user left the field: from here an invalid value is fair to show
    /// (`UIF-PRIN-009`).
    pub fn blur(&mut self) {
        self.field.note_blur();
    }

    /// Sets the error line shown under the field.
    ///
    /// Setting an error *is* a commit: it only ever follows a refused
    /// submit, and the field must stop being pristine at that moment or the
    /// message the reducer just revealed would be withheld again by the
    /// punish rule.
    pub fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.error = Some(message.clone());
        if self.kind().is_some_and(|kind| kind.is_name_entry()) {
            self.field.validation = Some(message);
        }
        self.field.note_submit();
    }

    /// Clears the error line.
    pub fn clear_error(&mut self) {
        self.error = None;
        self.field.validation = None;
    }

    /// Picks a move destination and clears any error.
    pub fn select_destination(&mut self, path: PathBuf) {
        self.selected_destination = Some(path);
        self.error = None;
    }

    /// What the framework's punish rule says this field may show right now
    /// (`UIF-PRIN-009`, `UIF-INP-003`).
    ///
    /// The class is [`DisallowedClass::FilenameBasename`], so a `/`, a
    /// `\` or any of the other filesystem-illegal characters paints on
    /// the exact keystroke that produced it, pristine or not, while an
    /// ordinary invalid value waits for a commit.
    #[must_use]
    pub fn punishment(&self) -> Punishment {
        self.field
            .punishment_under(DisallowedClass::FilenameBasename)
    }

    /// The error line the card paints this frame.
    ///
    /// For a name-entry kind this is the live rule's message, gated by
    /// [`punishment`](Self::punishment) — so a withheld verdict draws
    /// nothing and recovery clears on the same frame the value becomes
    /// valid. For the move and delete kinds there is no field to punish,
    /// so the reducer's revealed [`error`](Self::error) is what shows,
    /// exactly as before.
    ///
    /// # Why the fallback exists
    ///
    /// `painted` and `validation` come from two different rules and can
    /// disagree. `DisallowedClass::FilenameBasename` refuses `: * ? " < > |`
    /// and control characters, which [`validate_name`] — delegating to the
    /// app's own [`is_safe_basename`] — does **not**. Such a value is
    /// `painted == true` with `validation == None`, which would render a
    /// disabled `Ok` and **no explanation at all**: a button that simply
    /// refuses. When the punish rule says show something, the card always
    /// has something to show.
    #[must_use]
    pub fn painted_error(&self) -> Option<&str> {
        match self.kind() {
            None => None,
            Some(kind) if kind.is_name_entry() => {
                if self.punishment().painted() {
                    Some(
                        self.field
                            .validation
                            .as_deref()
                            .unwrap_or(strings::DIALOG_NAME_UNSAFE_ERROR),
                    )
                } else {
                    None
                }
            }
            Some(_) => self.error.as_deref(),
        }
    }

    /// Whether the confirm button may fire: name-entry kinds are gated by
    /// the punish rule's submit answer, the move dialog needs a picked
    /// destination, the delete confirmation is always confirmable, and a
    /// closed dialog is never confirmable.
    ///
    /// Note that the name-entry gate reads `blocks_submit`, **not**
    /// `painted`. A pristine field holding an empty name is invalid and
    /// must not be submittable even though nothing is drawn to say so —
    /// withholding the visual is not the same as pretending the value is
    /// fine (`UIF-PRIN-009`).
    #[must_use]
    pub fn can_confirm(&self) -> bool {
        match &self.open {
            None => false,
            Some(DialogKind::Move { .. }) => self.selected_destination.is_some(),
            Some(DialogKind::DeleteConfirm { .. }) => true,
            Some(
                DialogKind::CreateDirectory { .. }
                | DialogKind::CreateDocument { .. }
                | DialogKind::Rename { .. },
            ) => !self.punishment().blocks_submit(),
        }
    }

    /// Runs the submit transition: validates the current input, closes the
    /// dialog and returns the self-contained payload when valid, or keeps
    /// the dialog open with the error set when not.
    ///
    /// A refused submit commits the field whether or not it was ever
    /// focused, which is what makes the pillar's "…or submits the form"
    /// clause real: the next frame paints whatever is still wrong.
    pub fn submit(&mut self) -> SubmitOutcome {
        let Some(kind) = self.open.clone() else {
            return SubmitOutcome::Invalid {
                message: strings::DIALOG_NOT_OPEN_ERROR.to_string(),
            };
        };

        match kind {
            kind @ (DialogKind::CreateDirectory { .. }
            | DialogKind::CreateDocument { .. }
            | DialogKind::Rename { .. }) => match validate_name(&kind, self.pending_name()) {
                Ok(()) => {
                    let name = self.pending_name().trim().to_string();
                    self.close();
                    SubmitOutcome::Completed {
                        kind,
                        name,
                        destination: None,
                    }
                }
                Err(message) => {
                    self.field.validation = Some(message.clone());
                    self.field.note_submit();
                    SubmitOutcome::Invalid { message }
                }
            },
            DialogKind::Move { .. } => match self.selected_destination.clone() {
                Some(destination) => {
                    self.close();
                    SubmitOutcome::Completed {
                        kind,
                        name: String::new(),
                        destination: Some(destination),
                    }
                }
                None => {
                    self.field.note_submit();
                    SubmitOutcome::Invalid {
                        message: strings::DIALOG_MOVE_NO_DESTINATION_ERROR.to_string(),
                    }
                }
            },
            DialogKind::DeleteConfirm { .. } => {
                self.close();
                SubmitOutcome::Completed {
                    kind,
                    name: String::new(),
                    destination: None,
                }
            }
        }
    }

    /// Re-derives the field's validation message from the live rule.
    ///
    /// Called from every transition that can change the value or the kind,
    /// so `UIF-INP-003`'s "validated continuously on keystroke" is a
    /// property of the state rather than something each caller remembers.
    fn refresh_validation(&mut self) {
        self.field.validation = match self.open.as_ref() {
            Some(kind) if kind.is_name_entry() => validate_name(kind, &self.field.value).err(),
            Some(_) | None => None,
        };
    }
}

/// Validates a name-entry value for `kind`.
///
/// The merged rule from both layers (see the module header): the dialog's
/// own non-blank check plus the executor's [`is_safe_basename`]
/// single-segment rule (no `/`, no `\`, no `..` traversal). Kinds without
/// a name field ([`DialogKind::Move`], [`DialogKind::DeleteConfirm`])
/// accept anything — their gating lives in [`DialogState::can_confirm`].
///
/// This rule is **stricter** than the framework's own
/// `ValidationRule::Name`, which accepts `..` because a traversal is not
/// an illegal *character*. The app keeps its own rule for exactly that
/// reason; swapping to the framework's verdict would silently allow a
/// traversal through the dialog.
///
/// Purity: pure.
pub fn validate_name(kind: &DialogKind, name: &str) -> Result<(), String> {
    if !kind.is_name_entry() {
        return Ok(());
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(strings::DIALOG_NAME_EMPTY_ERROR.to_string());
    }
    if !is_safe_basename(trimmed) {
        return Err(strings::DIALOG_NAME_UNSAFE_ERROR.to_string());
    }
    Ok(())
}

/// The deterministic rendered size of a dialog card.
///
/// A direct delegation to the framework's box model
/// ([`iced_pro::surface::input_dialog::InputLayout::card_size`]), so the
/// card's size is decided by the same code that decides where every row
/// inside it goes. The validation slot is in that total whatever the
/// control is, which is why the size does not change when an error
/// appears (`UIF-INP-003`) and why [`resolve_dialog_anchor`] can run
/// before the card has been laid out.
#[must_use]
pub fn dialog_size(control: &InputControl) -> Size {
    InputLayout::card_size(control)
}

/// Resolves the card's top-left corner from the open-time anchor
/// (`UIF-INP-001`).
///
/// `Some(point)` is the cursor position the reducer captured from
/// [`crate::ui_iced::message::LayoutMessage::CursorMoved`] when the
/// dialog opened; it is solved as a pointer-invoked
/// [`iced_pro::geometry::anchor::Anchor::CursorProximal`], which opens the
/// card `+2px, +2px` off the cursor and clamps it to an `8px` margin from
/// every viewport edge. `None` — no pointer known — is solved as
/// keyboard-invoked and centres the card.
///
/// # What `None` can and cannot mean
///
/// The app tracks the last reported cursor, not the device that triggered
/// the action. `None` therefore means *"no pointer position has ever been
/// reported"*, which is the closest honest stand-in for
/// `keyboard_invoked` this application can produce — see
/// [`crate::ui_iced::app::OverlayState::dialog_anchor`]. A dialog
/// summoned from the keyboard *after* the mouse has moved over the window
/// still opens at the stale cursor. Faking a stronger signal would mean
/// inventing an event the app does not receive.
///
/// Purity: pure.
#[must_use]
pub fn resolve_dialog_anchor(anchor: Option<Point>, size: Size, viewport: Size) -> Point {
    let (point, keyboard_invoked) = match anchor {
        Some(point) => (point, false),
        None => (Point::new(0.0, 0.0), true),
    };
    anchor::solve(
        Anchor::CursorProximal {
            point,
            keyboard_invoked,
        },
        size,
        Rectangle::new(Point::new(0.0, 0.0), viewport),
    )
    .origin
}

/// Renders the open dialog as a blocking, anchored modal card.
///
/// The card is mounted with [`modal_at`] at the position resolved by
/// [`resolve_dialog_anchor`]: near the captured cursor, clamped on
/// screen, or centred when `anchor` is `None`. A click outside the card
/// emits [`DialogMessage::Dismissed`].
///
/// Every kind renders the same five §4.5.1 rows — title, context, control,
/// validation slot, footer — pinned to the framework's reserved heights,
/// differing only in what the control row holds:
///
/// * Name-entry kinds ([`DialogKind::CreateDirectory`] /
///   [`DialogKind::CreateDocument`] / [`DialogKind::Rename`]): a framework
///   [`iced_pro::widget::text_input::TextInput`] bound to the state's
///   [`TextInputModel`], painted invalid only when
///   [`DialogState::painted_error`] has something to say.
/// * [`DialogKind::Move`]: the scrollable destination picker (row press →
///   [`DialogMessage::DestinationSelected`]), sized by
///   [`iced_pro::surface::input_dialog::folder_list_height`].
/// * [`DialogKind::DeleteConfirm`]: an empty control row — the whole
///   message is the context line — and a [`ButtonIntent::Danger`] primary.
///
/// When no dialog is open the function renders an empty [`space::Space`]
/// rather than panicking; the root view is expected not to mount the card
/// in that state.
///
/// Purity: pure view over `state`.
#[must_use]
pub fn dialog_card(
    state: &DialogState,
    anchor: Option<Point>,
    viewport: Size,
) -> Element<'_, Message> {
    let Some(kind) = state.kind() else {
        return space::Space::new().into();
    };

    let control = kind.input_control(state);
    let position = resolve_dialog_anchor(anchor, dialog_size(&control), viewport);
    let intent = kind.button_intent();

    let card: Element<'_, Message> = match kind {
        DialogKind::CreateDirectory { .. }
        | DialogKind::CreateDocument { .. }
        | DialogKind::Rename { .. } => name_entry_card(state, kind, intent),
        DialogKind::Move { .. } => move_card(state, intent),
        DialogKind::DeleteConfirm { .. } => delete_confirm_card(kind, intent),
    };

    modal_at(
        card,
        position,
        Message::Dialog(DialogMessage::Absorbed),
        Message::Dialog(DialogMessage::Dismissed),
    )
}

/// The five §4.5.1 rows, pinned to the framework's reserved heights.
///
/// The control row is passed through unpinned because its own height is
/// the control's — [`TEXT_ENTRY_HEIGHT`], a content-sized folder list, or
/// nothing at all. Every other row is fixed, including the validation
/// slot, so the footer cannot move when an error appears: the slot is
/// there when empty and is the same rectangle when full
/// (`UIF-INP-003`).
fn card_body<'a>(
    title: Element<'a, Message>,
    prompt: Element<'a, Message>,
    control: Element<'a, Message>,
    error: Element<'a, Message>,
    footer: Element<'a, Message>,
) -> Element<'a, Message> {
    Column::with_children(vec![
        pinned(title, TITLE_ROW_HEIGHT),
        pinned(prompt, CONTEXT_ROW_HEIGHT),
        control,
        pinned(error, VALIDATION_SLOT_HEIGHT),
        pinned(footer, FOOTER_HEIGHT),
    ])
    .spacing(ROW_GAP)
    .width(Length::Fixed(CARD_WIDTH))
    .into()
}

/// Pin `content` to a §4.5.1 reserved row height, centred in it.
fn pinned<'a>(content: impl Into<Element<'a, Message>>, height: f32) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .align_y(Alignment::Center)
        .into()
}

/// The card's title row.
fn card_title<'a>(label: impl Into<Cow<'a, str>>) -> Element<'a, Message> {
    let label: Cow<'a, str> = label.into();
    text(label)
        .size(DIALOG_TITLE_SIZE)
        .color(Palette::fastmd_dark().text_strong)
        .into()
}

/// A footer button's label, floored at the §4.5.1 minimum button width.
///
/// See [`card_footer`] for why the floor is load-bearing rather than
/// decorative.
fn footer_label<'a>(label: &'static str) -> Element<'a, Message> {
    container(text(label))
        .width(Length::Fixed(BUTTON_MIN_WIDTH))
        .align_x(Alignment::Center)
        .into()
}

/// The card's context / target row.
fn card_prompt<'a>(label: impl Into<Cow<'a, str>>) -> Element<'a, Message> {
    let label: Cow<'a, str> = label.into();
    text(label).size(DIALOG_BODY_SIZE).into()
}

/// The footer action row: `[Ok (Primary|Danger)] [Cancel (Secondary)]`.
///
/// Both buttons are framework buttons, so the disabled `Ok` is the same
/// control the focus router knows about rather than a greyed-out iced
/// button that is a different thing again. `Ok` stays pressable-shaped
/// while disabled rather than disappearing, and a click on it is absorbed
/// by the card (`UIF-INP-002`).
///
/// Each label is mounted in a [`BUTTON_MIN_WIDTH`] box rather than at its
/// natural text width. That is not cosmetic: the framework decides whether
/// a button is "icon-only" **geometrically** — content no wider and no
/// taller than `size.control.sm` (28px) is classed as an icon with no
/// readable label, and a debug build asserts such a button carry a tooltip
/// (`UIF-ATOM-001`). The pinned label `'Ok'` measures about 16px, so a
/// bare `text("Ok")` trips that shield. Giving the footer the §4.5.1
/// button floor makes the content genuinely wider than an icon, which is
/// what it is: a labelled button, not a glyph.
fn card_footer<'a>(
    intent: ButtonIntent,
    confirm_label: &'static str,
    enabled: bool,
) -> Element<'a, Message> {
    let palette = Palette::pro();

    let ok = Button::new(
        button_variant(intent),
        footer_label(confirm_label),
        Message::Dialog(DialogMessage::Submitted),
        DIALOG_OK_ID,
    )
    .palette(palette);
    let ok = if enabled { ok } else { ok.disabled() };

    let cancel = Button::new(
        ButtonVariant::Secondary,
        footer_label(strings::CANCEL_BUTTON),
        Message::Dialog(DialogMessage::Cancelled),
        DIALOG_CANCEL_ID,
    )
    .palette(palette);

    iced::widget::Row::with_children(vec![ok.into(), cancel.into()])
        .spacing(ROW_GAP)
        .align_y(Alignment::Center)
        .into()
}

/// The name-entry card: title, prompt, text field, error line, buttons.
fn name_entry_card<'a>(
    state: &'a DialogState,
    kind: &DialogKind,
    intent: ButtonIntent,
) -> Element<'a, Message> {
    let punishment = state.punishment();

    let field = TextInput::controlled(
        InputKind::TextEntry,
        state.field().controlled(),
        DIALOG_ENTRY_ID,
        dialog_field_emit,
    )
    .width(Length::Fill)
    .palette(Palette::pro());
    // The punish decision, not the raw verdict, decides the border. A
    // withheld invalid paints nothing; a disallowed character paints now.
    let field = if punishment.painted() {
        field.invalid()
    } else {
        field.valid()
    };

    card_body(
        card_title(kind.title()),
        card_prompt(kind.prompt().unwrap_or_default()),
        field.into(),
        error_line(state.painted_error()),
        card_footer(intent, kind.confirm_label(), state.can_confirm()),
    )
}

/// Route one field tier onto the dialog's message channel.
///
/// `Changed` is the keystroke stream, `Committed` is `Enter`, and
/// `FocusChanged(false)` is the blur that commits the field. A
/// [`TextInputEmission::Cancelled`] (Escape inside the field) is
/// deliberately dropped: Escape belongs to the overlay's dismiss chain,
/// not to the field.
fn dialog_field_emit(emission: TextInputEmission) -> Option<Message> {
    match emission {
        TextInputEmission::Changed(live) => Some(Message::Dialog(DialogMessage::NameChanged {
            text: live.value().to_string(),
        })),
        TextInputEmission::Committed(_) => Some(Message::Dialog(DialogMessage::Submitted)),
        TextInputEmission::Cleared(_) => Some(Message::Dialog(DialogMessage::NameChanged {
            text: String::new(),
        })),
        TextInputEmission::FocusChanged(false) => Some(Message::Dialog(DialogMessage::Blurred)),
        TextInputEmission::FocusChanged(true) | TextInputEmission::Cancelled(_) => None,
    }
}

/// The move card: title, prompt, destination picker, error line, buttons.
fn move_card<'a>(state: &'a DialogState, intent: ButtonIntent) -> Element<'a, Message> {
    let rows =
        state
            .destinations()
            .iter()
            .fold(Column::<Message>::new().spacing(0.0), |rows, folder| {
                let selected = state.selected_destination() == Some(folder.as_path());
                rows.push(destination_row(folder, selected))
            });
    // Content-sized up to the §4.5.1 maximum, so a three-destination move
    // dialog is a three-row list rather than a 200px box with two rows in
    // it. The same `folder_list_height` drives the card's own size, so
    // the anchor clamp cannot drift from the rendered list.
    let list = scrollable(rows)
        .id(DIALOG_MOVE_LIST_ID)
        .height(Length::Fixed(folder_list_height(
            state.destinations().len(),
        )));

    card_body(
        card_title(strings::MOVE_FILE_WINDOW),
        card_prompt(strings::SELECT_DESTINATION_FOLDER),
        list.into(),
        error_line(state.painted_error()),
        card_footer(intent, strings::OK_BUTTON, state.can_confirm()),
    )
}

/// One selectable destination row: the picked row carries the accent
/// background, a press emits [`DialogMessage::DestinationSelected`].
fn destination_row(folder: &Path, selected: bool) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let label = text(folder.to_string_lossy()).size(DIALOG_BODY_SIZE);
    let surface = container(label)
        .width(Length::Fill)
        .height(Length::Fixed(MOVE_ROW_HEIGHT))
        .align_y(Alignment::Center)
        .padding(Padding::ZERO.horizontal(MOVE_ROW_INSET))
        .style(move |_theme| container::Style {
            background: selected.then_some(Background::Color(palette.accent)),
            ..container::Style::default()
        });

    mouse_area(surface)
        .on_press(Message::Dialog(DialogMessage::DestinationSelected {
            path: folder.to_path_buf(),
        }))
        .into()
}

/// The delete-confirmation card: title, target-named prompt, buttons.
///
/// The control row is an empty [`space::Space`] rather than an omitted
/// child: the §4.5.1 box model counts five rows and four gaps whether or
/// not the middle one holds anything, and dropping it would move the
/// footer relative to the size [`dialog_size`] reported.
fn delete_confirm_card<'a>(kind: &DialogKind, intent: ButtonIntent) -> Element<'a, Message> {
    card_body(
        card_title(strings::DELETE_CONFIRM_WINDOW),
        card_prompt(strings::format_delete_confirm_prompt(&kind.target_name())),
        space::Space::new().height(Length::Fixed(0.0)).into(),
        // A confirmation has no field to punish, so there is never a
        // message to draw — but the slot is still there, at the same
        // height it has in every other card.
        error_line(None),
        card_footer(intent, strings::DELETE_ACTION, true),
    )
}

/// The always-allocated error line: the text is empty, never absent, so
/// the card's widget shape does not change across the error/no-error
/// transition.
///
/// The text drawn is [`DialogState::painted_error`], which is already the
/// punish rule's answer — so this function cannot paint a withheld
/// verdict even if it wanted to.
fn error_line(message: Option<&str>) -> Element<'_, Message> {
    let error_color = Palette::fastmd_dark().error;
    text(message.unwrap_or(""))
        .size(DIALOG_ERROR_SIZE)
        .style(move |_theme| iced::widget::text::Style {
            color: Some(error_color),
        })
        .into()
}
