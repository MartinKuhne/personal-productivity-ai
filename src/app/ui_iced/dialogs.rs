//! Iced port of the small individual dialogs — the shared name-entry
//! dialog (create-directory UI-011 / create-document UI-015), the rename
//! dialog, the move dialog and the delete-confirmation dialog.
//!
//! This is a **pure view over plain state**: [`dialog_card`] renders a
//! [`DialogState`] into an [`iced::Element`] of [`Message`] and never
//! touches the bus, the disk or `FastMdApp`. The [TASK-022]/[TASK-023]
//! reducer owns the filesystem action: it opens the dialog from the tree's
//! deferred intents, runs [`DialogState::submit`] on the confirm message,
//! and publishes the matching `UserCommand::Confirm*` only when
//! validation passes ([FMD-068]).
//!
//! # Ported state model
//!
//! [`DialogState`] replaces the per-dialog flag fields of
//! `crate::ui::dialogs::Dialogs` (`src/app/ui/dialogs.rs:25-63`) with a
//! single `Option<DialogKind>`. the legacy UI kept one independent `bool` per
//! dialog and did **not** enforce mutual exclusion; the iced port
//! consolidates to one dialog at a time because the overlay contract
//! ([`crate::ui_iced::overlay::OverlaySlot`]) allows exactly one blocking
//! modal — opening a dialog replaces whatever was open, the same rule
//! [`crate::ui_iced::overlay::OverlaySlot::open_modal`] enforces.
//!
//! # Validation lives in the executor, not the legacy UI dialog
//!
//! Verified against the legacy UI build: the name-entry window only checks
//! `!name.trim().is_empty()` before publishing (`src/app/ui/modals.rs:174-184`
//! for create-directory, `:216-226` for create-document, `:304-314` for
//! rename) and the move dialog only requires a selected folder
//! (`src/app/ui/modals.rs:138-148`). The real name rule lives in the
//! command executor — `crate::utils::path::is_safe_basename`
//! (`src/app/utils/path.rs:13-27`), called at
//! `src/app/command_executor.rs:309/338/365` — which silently skips the
//! operation on a bad name. [`validate_name`] merges both layers into one
//! pure rule and surfaces the failure as a visible error line, an
//! improvement over the legacy UI's silent close (flagged in the migration map).
//!
//! # Anchor model
//!
//! The legacy UI name-entry window captures the pointer position once per open
//! session and pins the window there with `fixed_pos`, falling back to the
//! viewport centre when no pointer is known
//! (`src/app/ui/modals.rs:26-45`). The iced port keeps the same shape:
//! the reducer captures the tracked [`crate::ui_iced::message::LayoutMessage::CursorMoved`]
//! point when it opens the dialog and passes it to [`dialog_card`], which
//! resolves the final top-left with [`resolve_dialog_anchor`] — the
//! cursor point clamped by [`crate::ui_iced::overlay::clamp_anchor`], or
//! the viewport centre when there is no cursor. The card's size is
//! deterministic ([`dialog_size`]) because every row is a fixed-height
//! widget, so the clamp cannot drift from the real layout — the same
//! argument [`crate::ui_iced::overlay::menu_size`] makes for menus.
//!
//! # The delete confirmation is new behaviour
//!
//! The legacy UI build has **no** delete confirmation: the tree's Delete row
//! publishes `UserCommand::Delete` directly
//! (`src/app/ui/tree/render.rs:63/112/270`). [`DialogKind::DeleteConfirm`]
//! is added because the migration brief requires a confirm step; the
//! reducer decides whether to route `UserCommand::Delete` through it
//! ([RUST-041] drift note in `doc/planning/iced-migration-map.md`).
//!
//! Unit tests live in the sibling `dialogs_tests.rs` sidecar.

use std::path::{Path, PathBuf};

use iced::widget::{Column, container, mouse_area, scrollable, space, text, text_input};
use iced::{Background, Element, Length, Point, Size};

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{DialogMessage, Message};
use crate::ui_iced::overlay::{
    ConfirmCancel, MODAL_PADDING, clamp_anchor, confirm_cancel_buttons, modal_at,
};
use crate::utils::path::is_safe_basename;

#[cfg(test)]
#[path = "dialogs_tests.rs"]
mod tests;

/// Width of every small-dialog card ([TASK-017b1]).
pub const DIALOG_WIDTH: f32 = 320.0;

/// Logical point size of the dialog title.
pub const DIALOG_TITLE_SIZE: f32 = 16.0;

/// Logical point size of the prompt and destination rows.
pub const DIALOG_BODY_SIZE: f32 = 14.0;

/// Logical point size of the error line.
pub const DIALOG_ERROR_SIZE: f32 = 12.0;

/// Reserved height of the title row ([`DIALOG_WIDTH`]-independent; the
/// card's size must be deterministic for [`resolve_dialog_anchor`]).
pub const DIALOG_TITLE_HEIGHT: f32 = 24.0;

/// Reserved height of the prompt row.
pub const DIALOG_PROMPT_HEIGHT: f32 = 20.0;

/// Reserved height of the name-entry field row.
pub const DIALOG_INPUT_HEIGHT: f32 = 32.0;

/// Reserved height of the always-allocated error row.
pub const DIALOG_ERROR_HEIGHT: f32 = 20.0;

/// Reserved height of the confirm/cancel footer row.
pub const DIALOG_BUTTONS_HEIGHT: f32 = 32.0;

/// Vertical gap between adjacent card rows.
pub const DIALOG_SECTION_SPACING: f32 = 8.0;

/// Maximum height of the move dialog's destination list — legacy parity
/// (`ScrollArea::max_height(200.0)`, `src/app/ui/modals.rs:110`).
pub const MOVE_LIST_MAX_HEIGHT: f32 = 200.0;

/// Which small dialog is open, carrying the target(s) the reducer needs to
/// execute the action ([FMD-068]).
///
/// The payloads mirror the legacy UI `Dialogs` fields exactly:
/// `create_dir_parent` / `create_document_parent` become `parent`,
/// `file_to_rename` / `file_to_move` / the delete target become `path`.
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
    /// Confirm deleting the file or directory at `path` (new — see the
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

    /// The card title for this kind, from `crate::ui::strings` ([GUD-002]).
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

    /// The confirm button label: `Delete` for the confirmation, `Ok` for
    /// every other kind (legacy parity — all four the legacy UI dialogs used
    /// [`strings::OK_BUTTON`]).
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
}

/// The outcome of [`DialogState::submit`] ([TASK-017b1]).
///
/// `Completed` carries everything the reducer needs to publish the
/// matching `UserCommand` without re-reading dialog state that the close
/// has already cleared ([FMD-068]): the kind, the trimmed name (name-entry
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
        /// The user-facing error line ([GUD-002]).
        message: String,
    },
}

/// The small-dialog state model — the iced port of the name-entry / move
/// fields of `crate::ui::dialogs::Dialogs`.
///
/// Pure state: every transition is a plain method with no I/O. Only one
/// dialog is open at a time (see the module header for why this is
/// stricter than the legacy UI flag fields).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DialogState {
    open: Option<DialogKind>,
    pending_name: String,
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

    /// The name-entry field's current text.
    #[must_use]
    pub fn pending_name(&self) -> &str {
        &self.pending_name
    }

    /// The current error line, if any.
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

    /// Opens `kind`, replacing whatever dialog was open (one-at-a-time —
    /// see the module header) and clearing all transient fields.
    pub fn open(&mut self, kind: DialogKind) {
        *self = Self {
            open: Some(kind),
            ..Self::default()
        };
    }

    /// Opens the move dialog for `path` with the reducer-supplied
    /// destination candidates.
    ///
    /// The candidate list is data the pure view cannot build itself — the
    /// the legacy UI dialog assembled it from the content libraries and the file
    /// processor at render time (`src/app/ui/modals.rs:95-106`); here the
    /// reducer snapshots it and hands it in on open.
    pub fn open_move(&mut self, path: PathBuf, destinations: Vec<PathBuf>) {
        *self = Self {
            open: Some(DialogKind::Move { path }),
            destinations,
            ..Self::default()
        };
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

    /// Sets the pending name from the text field and clears any error —
    /// typing after a failed submit hides the stale message.
    pub fn set_name(&mut self, text: String) {
        self.pending_name = text;
        self.error = None;
    }

    /// Sets the error line shown under the field.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Clears the error line.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Picks a move destination and clears any error.
    pub fn select_destination(&mut self, path: PathBuf) {
        self.selected_destination = Some(path);
        self.error = None;
    }

    /// Whether the confirm button may fire: name-entry kinds need a
    /// non-blank name, the move dialog needs a picked destination, the
    /// delete confirmation is always confirmable, and a closed dialog is
    /// never confirmable.
    #[must_use]
    pub fn can_confirm(&self) -> bool {
        match &self.open {
            None => false,
            Some(
                DialogKind::CreateDirectory { .. }
                | DialogKind::CreateDocument { .. }
                | DialogKind::Rename { .. },
            ) => !self.pending_name.trim().is_empty(),
            Some(DialogKind::Move { .. }) => self.selected_destination.is_some(),
            Some(DialogKind::DeleteConfirm { .. }) => true,
        }
    }

    /// Runs the submit transition: validates the current input, closes the
    /// dialog and returns the self-contained payload when valid, or keeps
    /// the dialog open with the error set when not.
    pub fn submit(&mut self) -> SubmitOutcome {
        let Some(kind) = self.open.clone() else {
            return SubmitOutcome::Invalid {
                message: strings::DIALOG_NOT_OPEN_ERROR.to_string(),
            };
        };

        match kind {
            kind @ (DialogKind::CreateDirectory { .. }
            | DialogKind::CreateDocument { .. }
            | DialogKind::Rename { .. }) => match validate_name(&kind, &self.pending_name) {
                Ok(()) => {
                    let name = self.pending_name.trim().to_string();
                    self.close();
                    SubmitOutcome::Completed {
                        kind,
                        name,
                        destination: None,
                    }
                }
                Err(message) => SubmitOutcome::Invalid { message },
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
                None => SubmitOutcome::Invalid {
                    message: strings::DIALOG_MOVE_NO_DESTINATION_ERROR.to_string(),
                },
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
}

/// Validates a name-entry value for `kind` ([TASK-017b1]).
///
/// The merged rule from the two the legacy UI layers (see the module header):
/// the dialog's own non-blank check plus the executor's
/// [`is_safe_basename`] single-segment rule (no `/`, no `\`, no `..`
/// traversal). Kinds without a name field ([`DialogKind::Move`],
/// [`DialogKind::DeleteConfirm`]) accept anything — their gating lives in
/// [`DialogState::can_confirm`].
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

/// The deterministic rendered size of a dialog card ([TASK-017b1]).
///
/// Every row is a fixed-height widget, so this is exact rather than an
/// estimate — the same property [`crate::ui_iced::overlay::menu_size`]
/// provides for menus, and what makes [`resolve_dialog_anchor`] safe to
/// run before the card has been laid out. The error row is always
/// allocated (the legacy UI conditional-rendering rule), so the size does not
/// change when an error appears.
#[must_use]
pub fn dialog_size(kind: &DialogKind) -> Size {
    let chrome =
        MODAL_PADDING * 2.0 + DIALOG_TITLE_HEIGHT + DIALOG_PROMPT_HEIGHT + DIALOG_BUTTONS_HEIGHT;
    match kind {
        DialogKind::CreateDirectory { .. }
        | DialogKind::CreateDocument { .. }
        | DialogKind::Rename { .. } => Size::new(
            DIALOG_WIDTH,
            chrome + DIALOG_INPUT_HEIGHT + DIALOG_ERROR_HEIGHT + DIALOG_SECTION_SPACING * 4.0,
        ),
        DialogKind::Move { .. } => Size::new(
            DIALOG_WIDTH,
            chrome + MOVE_LIST_MAX_HEIGHT + DIALOG_ERROR_HEIGHT + DIALOG_SECTION_SPACING * 4.0,
        ),
        DialogKind::DeleteConfirm { .. } => {
            Size::new(DIALOG_WIDTH, chrome + DIALOG_SECTION_SPACING * 2.0)
        }
    }
}

/// Resolves the card's top-left corner from the open-time anchor
/// ([TASK-017b1]).
///
/// `Some(point)` is the cursor position the reducer captured from
/// [`crate::ui_iced::message::LayoutMessage::CursorMoved`] when the
/// dialog opened, clamped by [`clamp_anchor`] so the card stays on
/// screen. `None` — no pointer known, e.g. a keyboard-opened dialog —
/// centres the card in the viewport (the legacy UI fallback,
/// `src/app/ui/modals.rs:33-35`, but truly centred rather than
/// top-left-at-centre). The result is never negative even when the
/// viewport is smaller than the card.
///
/// Purity: pure.
#[must_use]
pub fn resolve_dialog_anchor(anchor: Option<Point>, size: Size, viewport: Size) -> Point {
    match anchor {
        Some(point) => clamp_anchor(point, size, viewport),
        None => Point::new(
            ((viewport.width - size.width) / 2.0).max(0.0),
            ((viewport.height - size.height) / 2.0).max(0.0),
        ),
    }
}

/// Renders the open dialog as a blocking, anchored modal card
/// ([TASK-017b1]).
///
/// The card is mounted with [`modal_at`] at the position resolved by
/// [`resolve_dialog_anchor`]: near the captured cursor, clamped on
/// screen, or centred when `anchor` is `None`. A click outside the card
/// emits [`DialogMessage::Dismissed`].
///
/// * Name-entry kinds ([`DialogKind::CreateDirectory`] /
///   [`DialogKind::CreateDocument`] / [`DialogKind::Rename`]): title,
///   prompt, `text_input` (`on_input` → `NameChanged`, `on_submit` →
///   `Submitted`), the always-allocated error line, and the
///   [`confirm_cancel_buttons`] pair gated by [`DialogState::can_confirm`].
/// * [`DialogKind::Move`]: title, prompt, the scrollable destination
///   picker (row press → `DestinationSelected`), error line, and the
///   confirm/cancel pair.
/// * [`DialogKind::DeleteConfirm`]: title, the target-named prompt and
///   the confirm/cancel pair — no text input.
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

    let size = dialog_size(kind);
    let position = resolve_dialog_anchor(anchor, size, viewport);

    let card: Element<'_, Message> = match kind {
        DialogKind::CreateDirectory { .. }
        | DialogKind::CreateDocument { .. }
        | DialogKind::Rename { .. } => name_entry_card(state, kind),
        DialogKind::Move { .. } => move_card(state),
        DialogKind::DeleteConfirm { .. } => delete_confirm_card(kind),
    };

    modal_at(card, position, Message::Dialog(DialogMessage::Dismissed))
}

/// The name-entry card: title, prompt, text field, error line, buttons.
fn name_entry_card<'a>(state: &'a DialogState, kind: &DialogKind) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();

    let title = text(kind.title())
        .size(DIALOG_TITLE_SIZE)
        .color(palette.text_strong);

    let prompt = text(kind.prompt().unwrap_or_default()).size(DIALOG_BODY_SIZE);

    let input = text_input("", state.pending_name())
        .on_input(|text| Message::Dialog(DialogMessage::NameChanged { text }))
        .on_submit(Message::Dialog(DialogMessage::Submitted));

    let buttons =
        confirm_cancel_buttons(
            kind.confirm_label(),
            state.can_confirm(),
            |outcome| match outcome {
                ConfirmCancel::Confirm => Message::Dialog(DialogMessage::Submitted),
                ConfirmCancel::Cancel => Message::Dialog(DialogMessage::Cancelled),
            },
        );

    Column::with_children(vec![
        title.into(),
        prompt.into(),
        input.into(),
        error_line(state),
        buttons.into(),
    ])
    .spacing(DIALOG_SECTION_SPACING)
    .width(Length::Fixed(DIALOG_WIDTH))
    .into()
}

/// The move card: title, prompt, destination picker, error line, buttons.
fn move_card(state: &DialogState) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let title = text(strings::MOVE_FILE_WINDOW)
        .size(DIALOG_TITLE_SIZE)
        .color(palette.text_strong);

    let prompt = text(strings::SELECT_DESTINATION_FOLDER).size(DIALOG_BODY_SIZE);

    let rows =
        state
            .destinations()
            .iter()
            .fold(Column::<Message>::new().spacing(2.0), |rows, folder| {
                let selected = state.selected_destination() == Some(folder.as_path());
                rows.push(destination_row(folder, selected))
            });
    let list = scrollable(rows).height(Length::Fixed(MOVE_LIST_MAX_HEIGHT));

    let buttons =
        confirm_cancel_buttons(
            strings::OK_BUTTON,
            state.can_confirm(),
            |outcome| match outcome {
                ConfirmCancel::Confirm => Message::Dialog(DialogMessage::Submitted),
                ConfirmCancel::Cancel => Message::Dialog(DialogMessage::Cancelled),
            },
        );

    Column::with_children(vec![
        title.into(),
        prompt.into(),
        list.into(),
        error_line(state),
        buttons.into(),
    ])
    .spacing(DIALOG_SECTION_SPACING)
    .width(Length::Fixed(DIALOG_WIDTH))
    .into()
}

/// One selectable destination row — the iced re-typing of the legacy UI's
/// `selectable_label` (`src/app/ui/modals.rs:116-124`): the picked row
/// carries the accent background, a press emits `DestinationSelected`.
fn destination_row(folder: &Path, selected: bool) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let label = text(folder.to_string_lossy()).size(DIALOG_BODY_SIZE);
    let surface = container(label)
        .width(Length::Fill)
        .padding(4.0)
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
fn delete_confirm_card(kind: &DialogKind) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let title = text(strings::DELETE_CONFIRM_WINDOW)
        .size(DIALOG_TITLE_SIZE)
        .color(palette.text_strong);

    let prompt =
        text(strings::format_delete_confirm_prompt(&kind.target_name())).size(DIALOG_BODY_SIZE);

    let buttons = confirm_cancel_buttons(strings::DELETE_ACTION, true, |outcome| match outcome {
        ConfirmCancel::Confirm => Message::Dialog(DialogMessage::Submitted),
        ConfirmCancel::Cancel => Message::Dialog(DialogMessage::Cancelled),
    });

    Column::with_children(vec![title.into(), prompt.into(), buttons.into()])
        .spacing(DIALOG_SECTION_SPACING)
        .width(Length::Fixed(DIALOG_WIDTH))
        .into()
}

/// The always-allocated error line ([GUD-002]: the text is empty, never
/// absent, so the card's widget shape does not change across the
/// error/no-error transition).
fn error_line(state: &DialogState) -> Element<'_, Message> {
    let error_color = Palette::fastmd_dark().error;
    text(state.error().unwrap_or(""))
        .size(DIALOG_ERROR_SIZE)
        .style(move |_theme| iced::widget::text::Style {
            color: Some(error_color),
        })
        .into()
}
