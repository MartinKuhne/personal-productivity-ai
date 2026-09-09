//! User-command payload — the unified `UserCommand` enum that flows over
//! `Bus<UserCommand>` from every UI input surface to a single orchestrator-side
//! executor.
//!
//! See `doc/planning/user-command-bus.md` for the full audit and staged
//! migration plan. Stage 0 wires the bus and drain; no call sites publish yet.
//!
//! | Surface | Variants |
//! | --- | --- |
//! | Bottom panel (B) | `RunAgent`, `ShowModels`, `ShowDeprecatedModelMessage`, `CancelAgent` |
//! | Tabs (C) | `CloseTab`, `CloseOtherTabs`, `CloseAllTabs` |
//! | TOC (D) | `ScrollToHeader` |
//! | Toolbar + hamburger (A) | `OpenBatchDialog`, `OpenToolsDialog`, `OpenAboutDialog`, `ToggleBackgroundLogs`, `ToggleAgentDebugWindow`, `SelectChatModel`, `ChangeTableWidthStrategy`, `SelectTagFilter` |
//! | Tools dialog (F) | `SetToolGroupEnabled`, `ClearToolGroupError`, `StartMcpAuth`, `ForgetMcpAuth` |
//! | File tree (E) | `SelectFile`, `SelectDirectory`, `OpenInEditor`, `ShowInExplorer`, `CopyPath`, `SaveAsPdf`, `Rename`, `Move`, `Delete`, `CreateDirectory`, `CreateDocument`, `RunSkillPrompt`, `MergePrompt` |
//! | Dialog confirmations (F) | `ConfirmMove`, `ConfirmCreateDirectory`, `ConfirmCreateDocument`, `ConfirmRename`, `StartBatch`, `CancelBatch` |
//! | Keyboard shortcuts (G) | `ToggleAgentDebugWindowShortcut` |
//! | Agent session close (C) | `ClearAgentSession`, `ApplyTaskToggles` |
//! | Command input (B) | `ClearCommandInput` |
//! | Agent debug window (H) | `ToggleDebugWindow`, `ClearDebugEntries`, `SetDebugJsonRows`, `SetDebugSearchText`, `SetDebugAutoScroll` |

use std::path::PathBuf;

use crate::agent::batch::types::BatchConfig;
use crate::agent::tools::registry::groups::ToolGroupId;
use crate::bus::core::Bus;
use crate::markdown::table_width::DeficitStrategy;

/// One unit of user intent captured at the UI boundary and dispatched
/// centrally by the orchestrator.
///
/// Variants are grouped by surface. Every variant carries *all* data the
/// executor needs to apply the command — no back-references to UI state.
/// "Do nothing" is represented by *not publishing* (there is no `Empty`
/// variant), keeping the bus free of no-op traffic.
#[derive(Clone, Debug, PartialEq)]
pub enum UserCommand {
    // ── Command input / agent (surface B, C) ───────────────────────────
    /// Run the agent with the given prompt text.
    RunAgent(String),
    /// Show the model-picker view in the agent panel.
    ShowModels,
    /// Show the deprecated-model notice in the agent panel.
    ShowDeprecatedModelMessage,
    /// Cancel the running agent session and clear results.
    CancelAgent,
    /// Queue a prompt to run after the current agent session finishes.
    QueueAgentPrompt(String),

    // ── Tabs (surface C) ───────────────────────────────────────────────
    /// Close the tab at the given index.
    CloseTab(usize),
    /// Close every tab except the one at the given index.
    CloseOtherTabs(usize),
    /// Close all open tabs.
    CloseAllTabs,

    // ── TOC (surface D) ────────────────────────────────────────────────
    /// Scroll the markdown view to the heading with the given id.
    ScrollToHeader(String),

    // ── Toolbar buttons + hamburger menu submenus (surface A) ──────────
    /// Open the batch-processing dialog.
    OpenBatchDialog,
    /// Open the tools (MCP / tool-group) dialog.
    OpenToolsDialog,
    /// Open the about dialog.
    OpenAboutDialog,
    /// Toggle the background-operations log window.
    ToggleBackgroundLogs(bool),
    /// Toggle the agent debug window.
    ToggleAgentDebugWindow(bool),
    /// Select a new chat model by name.
    SelectChatModel(String),
    /// Change the table width-deficit strategy.
    ChangeTableWidthStrategy(DeficitStrategy),
    /// Set the tag filter (`None` clears it).
    SelectTagFilter(Option<String>),

    // ── Tools dialog — per-group controls (surface F) ──────────────────
    /// Enable or disable a tool group (internal family or MCP server).
    SetToolGroupEnabled { id: ToolGroupId, enabled: bool },
    /// Clear the recorded error on a tool group (the "Restart" link).
    ClearToolGroupError(ToolGroupId),
    /// Start the OAuth flow for an MCP server that needs authentication.
    StartMcpAuth(String),
    /// Forget the authentication state for an MCP server.
    ForgetMcpAuth(String),

    // ── File tree — selection (surface E) ──────────────────────────────
    /// Select a file in the tree. `multi` selects multi-select mode.
    SelectFile { path: PathBuf, multi: bool },
    /// Select a directory in the tree. `toggle_expand` flips expansion.
    SelectDirectory { path: PathBuf, toggle_expand: bool },
    /// Open the given file in the inline editor.
    OpenInEditor(PathBuf),

    // ── File tree — context menu (surface E) ───────────────────────────
    /// Reveal the path in the OS file explorer.
    ShowInExplorer(PathBuf),
    /// Copy the path to the clipboard.
    CopyPath(PathBuf),
    /// Export the path as PDF.
    SaveAsPdf(PathBuf),
    /// Rename the given path.
    Rename(PathBuf),
    /// Move the given path.
    Move(PathBuf),
    /// Delete the given path (to recycle bin).
    Delete(PathBuf),
    /// Create a directory under the given parent.
    CreateDirectory { parent: PathBuf },
    /// Create a document under the given parent.
    CreateDocument { parent: PathBuf },
    /// Run a skill prompt against a target directory and/or file.
    RunSkillPrompt {
        content: String,
        target_dir: Option<PathBuf>,
        target_file: Option<PathBuf>,
    },
    /// Merge multiple selected files into one agent prompt.
    MergePrompt(Vec<PathBuf>),

    // ── Dialog confirmations (surface F) ────────────────────────────────
    /// Confirm a move operation.
    ConfirmMove { file: PathBuf, destination: PathBuf },
    /// Confirm directory creation.
    ConfirmCreateDirectory { parent: PathBuf, name: String },
    /// Confirm document creation.
    ConfirmCreateDocument { parent: PathBuf, name: String },
    /// Confirm a rename operation.
    ConfirmRename { path: PathBuf, new_name: String },
    /// Start a batch processing run.
    StartBatch(BatchConfig),
    /// Cancel a batch processing run.
    CancelBatch,

    // ── Keyboard shortcuts (surface G) ─────────────────────────────────
    /// `ALT+A` — toggle the agent debug window.
    ToggleAgentDebugWindowShortcut,

    // ── Agent session (surface C) ─────────────────────────────────────
    /// Clear the agent session (close button in agent panel).
    ClearAgentSession,
    /// Apply task checkbox toggles to the agent transcript content.
    ApplyTaskToggles { toggles: Vec<(usize, bool)> },
    /// Clear the command input after sending.
    ClearCommandInput,

    // ── Agent debug window (surface H) ────────────────────────────────
    /// Toggle the agent debug window visibility.
    ToggleDebugWindow(bool),
    /// Clear all debug entries.
    ClearDebugEntries,
    /// Set the number of JSON rows to display in debug window.
    SetDebugJsonRows(usize),
    /// Set the search text filter in debug window.
    SetDebugSearchText(String),
    /// Set the auto-scroll preference in debug window.
    SetDebugAutoScroll(bool),
}

/// A thin handle for publishing [`UserCommand`]s from UI panels and modal
/// callbacks without borrowing the orchestrator. Cloning a producer is cheap
/// (it shares the same `Bus` sender via `Arc`).
///
/// Mirrors [`crate::bus::events::file::FileEventProducer`].
#[derive(Clone)]
pub struct UserCommandProducer {
    bus: Bus<UserCommand>,
}

impl UserCommandProducer {
    /// Construct a producer that publishes into the given bus.
    pub fn new(bus: Bus<UserCommand>) -> Self {
        Self { bus }
    }

    /// Publish a user command to the bus.
    pub fn publish(&self, command: UserCommand) {
        self.bus.publish(command);
    }
}
