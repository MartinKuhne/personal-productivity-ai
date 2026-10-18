//! The iced `update` reducer ([TASK-022] minimal, [TASK-023] completes the
//! one-to-one mirror of the egui `FastMdApp::update_ui`).
//!
//! This is the single place user intent is executed and runtime reports are
//! folded into [`AppState`]. The panels never publish to the command bus
//! themselves — they emit [`Message::Command`] and this reducer performs the
//! publish + execution ([FMD-068], [FMD-069]).
//!
//! # What is wired here (TASK-022)
//!
//! * [`WindowMessage`] — the DEC-006 close/save flow (chained
//!   `window::size` → `window::position` snapshot, then persist, then
//!   `window::close`), the DEC-008 scale-baseline store and the viewport
//!   resize report.
//! * [`LayoutMessage::PaneResized`] — the [TASK-010] UI-016-clamped pane
//!   reduction, replayed into the pane grid and synced to the persisted
//!   snapshot.
//! * [`Message::Command`] — published on the orchestrator's
//!   `user_command_bus` and drained/executed via
//!   `crate::orchestrator::AppOrchestrator::drain_user_command_bus`.
//! * [`BootMessage`] — the font-registration failure [NFR-001] `ERROR` log
//!   and the DEC-009 publish confirmation.
//!
//! # What [TASK-023] completes
//!
//! The remaining channels (`Tree`, `Tab`, `Editor`, `Agent`, `Search`,
//! `Logs`, `Theme`, `Render`, `Dialog`, `About`, `Batch`, `Tools`,
//! `AgentDebug`, `Inbound`) plus `LayoutMessage::TreeAutoFit` /
//! `CursorMoved` storage. Until then they fall through to a single
//! `tracing::debug!` no-op arm (named in the catch-all) so the app launches
//! and the window/pane/command half is fully live.
//!
//! Unit tests live in the sibling `update_tests.rs` sidecar.

use crate::ui_iced::app::AppState;
use crate::ui_iced::message::{BootMessage, LayoutMessage, Message, WindowMessage};
use crate::ui_iced::persisted;
use iced::Task;

/// NFR-001/NFR-004 error-code name for a failed font registration.
const FONT_LOAD_ERROR_CODE: &str = "ui_iced.boot.font_load_failed";

/// Top-level iced reducer ([TASK-022]).
///
/// `impl UpdateFn ... for T where T: Fn(&mut State, Message) -> C, C:
/// Into<Task<Message>>` (`iced-0.14.0/src/application.rs:609-617`).
pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::Window(message) => update_window(state, message),
        Message::Layout(message) => update_layout(state, message),
        Message::Command(command) => {
            // Publish so every bus subscriber observes the intent
            // ([FMD-066], [FMD-071]), then drain-and-execute the same way
            // the egui frame loop did (`update_ui` → `drain_user_command_bus`).
            state.app.orchestrator.user_command_bus.publish(command);
            state.app.orchestrator.drain_user_command_bus();
            Task::none()
        }
        Message::Boot(message) => update_boot(message),
        // [TASK-023]: the panel + inbound channels are not yet reduced.
        other => {
            tracing::debug!(
                name = "ui_iced.update.unhandled",
                message = ?other,
                "TASK-023: message channel not yet wired into the iced reducer"
            );
            Task::none()
        }
    }
}

/// Window-lifecycle reductions ([TASK-009], DEC-006/DEC-008).
fn update_window(state: &mut AppState, message: WindowMessage) -> Task<Message> {
    match message {
        WindowMessage::CloseRequested(id) => {
            // DEC-006 phase 1: BEFORE closing, report the current window
            // metrics through the chained runtime tasks so the next message
            // can persist + close in one reduction.
            iced::window::size(id).then(move |size| {
                iced::window::position(id).map(move |position| {
                    Message::Window(WindowMessage::CloseSnapshotCaptured(
                        crate::ui_iced::message::WindowSnapshot { id, size, position },
                    ))
                })
            })
        }
        WindowMessage::CloseSnapshotCaptured(snapshot) => {
            // DEC-006 phase 2: FIRST persist, THEN close. Panel widths are
            // copied from the live `PanelLayout` right before serialising —
            // the faithful port of the egui `save()` sync
            // (`ui/app/mod.rs:251-252`).
            state.app.persisted_ui_state.left_panel_width = state.app.layout.left_panel_width;
            state.app.persisted_ui_state.right_panel_width = state.app.layout.right_panel_width;
            persisted::apply_close_metrics(
                &mut state.app.persisted_ui_state,
                snapshot.size,
                snapshot.position,
            );
            match persisted::save_ui_state(state.storage.as_ref(), &state.app.persisted_ui_state) {
                Ok(path) => tracing::info!(?path, "Persisted UI state saved on close"),
                Err(error) => {
                    tracing::error!(
                        name = persisted::UI_STATE_SAVE_ERROR_CODE,
                        %error,
                        "Failed to persist UI state on close"
                    );
                }
            }
            iced::window::close(snapshot.id)
        }
        WindowMessage::ScaleBaseline(baseline) => {
            // DEC-008: store the OS baseline; `.scale_factor(...)` composes
            // it with the persisted multiplier per frame ([FMD-069]).
            state.os_scale = Some(baseline);
            Task::none()
        }
        WindowMessage::Resized { size, .. } => {
            // Keep the logical viewport current ([TASK-010] input).
            state.viewport = size;
            Task::none()
        }
    }
}

/// Pane-geometry reductions ([TASK-010]).
fn update_layout(state: &mut AppState, message: LayoutMessage) -> Task<Message> {
    match message {
        LayoutMessage::PaneResized(event) => {
            // Reduce the drag onto the UI-016-clamped widths, replay the
            // CLAMPED ratio into the pane grid, and keep the persisted
            // snapshot consistent for the next close-time save.
            if let Some((split, ratio)) = crate::ui_iced::layout::apply_pane_resize(
                &state.band,
                event,
                state.viewport.width,
                &mut state.app.layout,
            ) {
                state.panes.resize(split, ratio);
                state.app.persisted_ui_state.left_panel_width = state.app.layout.left_panel_width;
                state.app.persisted_ui_state.right_panel_width = state.app.layout.right_panel_width;
            }
            Task::none()
        }
        // [TASK-023]: tree auto-fit and cursor anchoring.
        other => {
            tracing::debug!(
                name = "ui_iced.update.layout_unhandled",
                message = ?other,
                "TASK-023: layout report not yet wired"
            );
            Task::none()
        }
    }
}

/// Boot-infrastructure report reductions ([TASK-007], DEC-009).
fn update_boot(message: BootMessage) -> Task<Message> {
    match message {
        BootMessage::FontLoadFinished(Some(error)) => {
            // NFR-001: a font-registration failure must produce an ERROR
            // log. Non-fatal: iced keeps its default font set ([FMD-064]).
            tracing::error!(name = FONT_LOAD_ERROR_CODE, %error, "Font registration failed");
            Task::none()
        }
        BootMessage::FontLoadFinished(None) => Task::none(),
        BootMessage::ConfigPublished => {
            // The DEC-009 publish settled; the UI was direct-injected at
            // boot so there is nothing to apply here.
            tracing::debug!(
                name = "ui_iced.boot.config_publish_settled",
                "DEC-009 startup config publish settled"
            );
            Task::none()
        }
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
