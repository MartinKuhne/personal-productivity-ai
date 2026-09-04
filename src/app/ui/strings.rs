//! Centralised user interface text strings and display constants for FastMD.

// Top Panel
/// Application title displayed in the top toolbar.
/// The Document + Bolt logo is painted separately via [`crate::ui::logo::paint_logo`] —
///
/// the title itself is plain text so it remains searchable and accessible.
pub const APP_TITLE: &str = "FastMD Viewer";

/// Label for the batch prompt processing button (now in the hamburger menu).
pub const BATCH_BUTTON: &str = "Batch...";

/// Label for the batch prompt processing entry in the hamburger menu.
pub const MENU_BATCH: &str = "Batch...";

/// Label for the tools dialog button (now in the hamburger menu).
pub const TOOLS_BUTTON: &str = "Tools...";

/// Label for the tools dialog entry in the hamburger menu.
pub const MENU_TOOLS: &str = "Tools...";

/// Title of the tools dialog modal.
pub const TOOLS_DIALOG_TITLE: &str = "Tools";

/// Kind label for an internal (built-in) tool group row.
pub const TOOLS_KIND_INTERNAL: &str = "Internal";

/// Kind label for an MCP stdio (local subprocess) server tool group row.
pub const TOOLS_KIND_MCP_STDIO: &str = "MCP (stdio)";

/// Kind label for an MCP remote (SSE/HTTP) server tool group row.
pub const TOOLS_KIND_MCP_REMOTE: &str = "MCP (remote)";

/// Caption for the per-group tools list column header.
pub const TOOLS_LIST_COLUMN: &str = "Tools";

/// Caption for the per-group prompt char count column header.
pub const TOOLS_CHAR_COUNT_COLUMN: &str = "Prompt";

/// Button label that triggers the OAuth flow for an eligible MCP server.
pub const TOOLS_AUTH_BUTTON: &str = "Authenticate";

/// Button label while the OAuth flow is in flight.
pub const TOOLS_AUTH_RUNNING: &str = "Authenticating...";

/// Link on a group row to clear the recorded `last_error`.
pub const TOOLS_RESTART: &str = "Restart";

/// Link on an MCP group row to clear the `needs_auth` flag (i.e.
/// tell the manager to forget that the server once required auth).
pub const TOOLS_FORGET: &str = "Forget";

/// Label for selecting all tags in the tag filter combobox.
pub const TAG_FILTER_ALL: &str = "All";

/// ID salt for the tag filter combobox in egui.
pub const TAG_FILTER_ID_SALT: &str = "tag_combobox";

/// Default label shown when no tag is selected in the tag filter.
pub const TAG_FILTER_DEFAULT: &str = "Filter by Tag: All";

/// Placeholder text for the workspace content search box.
pub const SEARCH_PLACEHOLDER: &str = "Search workspace...";

/// Tooltip for the magnifying glass button that triggers search.
pub const SEARCH_TRIGGER_TOOLTIP: &str = "Search";

/// Tooltip for the button that clears the search query and restores the tree view.
pub const SEARCH_CLEAR_TOOLTIP: &str = "Clear search";

/// Empty-state message when content search yields zero matching files.
pub const SEARCH_NO_RESULTS: &str = "No matching files found";

/// Header label for the search result list.
pub const SEARCH_RESULTS_HEADER: &str = "Search Results";

/// Phosphor icon: caret down (U+E136)
pub const ICON_CARET_DOWN: &str = "\u{E136}";

/// Phosphor icon: caret right (U+E13A)
pub const ICON_CARET_RIGHT: &str = "\u{E13A}";

/// Phosphor icon: magnifying glass (U+E30C)
pub const ICON_MAGNIFYING_GLASS: &str = "\u{E30C}";

/// Phosphor icon: x / close (U+E4F6)
pub const ICON_X: &str = "\u{E4F6}";

/// Phosphor icon: copy (U+E1CA)
pub const ICON_COPY: &str = "\u{E1CA}";

/// Phosphor icon: list / hamburger menu (U+E2F0)
pub const ICON_LIST: &str = "\u{E2F0}";

/// Phosphor icon: stop (U+E46C)
pub const ICON_STOP: &str = "\u{E46C}";

/// Phosphor icon: robot (U+E762)
pub const ICON_ROBOT: &str = "\u{E762}";

/// Phosphor icon: lightning (U+E2DE)
pub const ICON_LIGHTNING: &str = "\u{E2DE}";

/// Label for the hamburger menu button in the top toolbar.
pub const HAMBURGER_MENU_BUTTON: &str = ICON_LIST;

/// ID salt for the hamburger menu in egui.
pub const HAMBURGER_MENU_ID_SALT: &str = "top_hamburger_menu";

/// Submenu label for the Table Wrap Algorithm submenu in the top hamburger menu.
pub const MENU_TABLE_WRAP_ALGORITHM: &str = "Table wrap algorithm";

/// Submenu label for the Windows submenu in the top hamburger menu.
pub const MENU_WINDOWS: &str = "Windows";

/// Menu item label for opening/toggling the Background Operations (processes/logs) window.
pub const MENU_BACKGROUND_OPERATIONS: &str = "Background operations";

/// Menu item label for opening/toggling the Agent Debug window.
pub const MENU_AGENT_DEBUG: &str = "Agent debug";

/// `on_click` event name fired when the user toggles Background Operations from the menu.
pub const BACKGROUND_OPERATIONS_EVENT: &str = "background_operations";

/// `on_click` event name fired when the user toggles Agent Debug from the menu.
pub const AGENT_DEBUG_EVENT: &str = "agent_debug";

/// Submenu label for the Chat Models submenu in the top hamburger menu.
pub const MENU_CHAT_MODELS: &str = "Chat models";

/// Informational label shown in the Chat Models menu when no models are configured.
pub const NO_CHAT_MODELS_CONFIGURED: &str = "No models configured";

/// `on_click` event name fired when the user selects a chat model from the menu.
pub const CHAT_MODEL_SELECTION_EVENT: &str = "chat_model_selection";

/// Formats a model entry for the Chat Models dropdown menu.
pub fn format_chat_model_menu_label(name: &str, cost: i32) -> String {
    format!("{} (Cost: {})", name, cost)
}

/// Label preceding the table-width-strategy dropdown in the top toolbar.
pub const TABLE_WIDTH_STRATEGY_LABEL: &str = "Table wrap:";

/// ID salt for the table-width-strategy combobox in egui. Kept stable
/// across releases so the widget's persistent id and any open/closed
/// state survive a config save round-trip.
pub const TABLE_WIDTH_STRATEGY_ID_SALT: &str = "table_width_strategy_combobox";

/// Display label for the [`ProportionalToSlack`](crate::ui::table_width::DeficitStrategy::ProportionalToSlack)
/// deficit strategy in the top-bar dropdown. Fast (O(|S|)) but may
/// produce suboptimal G1 (total wrapped lines) on very wide tables.
pub const TABLE_WIDTH_STRATEGY_PROPORTIONAL: &str = "Proportional";

/// Display label for the [`BreakpointWaterFill`](crate::ui::table_width::DeficitStrategy::BreakpointWaterFill)
/// deficit strategy in the top-bar dropdown. Greedy marginal-cost
/// water-fill (O(K log |S|)) — minimizes G1 more aggressively.
pub const TABLE_WIDTH_STRATEGY_WATERFILL: &str = "Water-fill";

/// Display label for the [`WaterFillRatio`](crate::ui::table_width::DeficitStrategy::WaterFillRatio)
/// survey algorithm (doc §2.10) in the top-bar dropdown. Equalizes the
/// `max_j / w_j` ratio across columns — the "fair" baseline that
/// equalizes wrap pressure.
pub const TABLE_WIDTH_STRATEGY_RATIO: &str = "Ratio-equalize";

/// Display label for the [`LagrangePenalty`](crate::ui::table_width::DeficitStrategy::LagrangePenalty)
/// survey algorithm (doc §2.13) in the top-bar dropdown. Minimizes
/// total wrap via a global Lagrange-multiplier bisection on per-column
/// wrap cost.
pub const TABLE_WIDTH_STRATEGY_LAGRANGE: &str = "Lagrange-penalty";

/// Display label for the [`HybridMinPenaltyWaterFill`](crate::ui::table_width::DeficitStrategy::HybridMinPenaltyWaterFill)
/// survey algorithm (doc §2.14) in the top-bar dropdown. Per-column
/// "first-wrap boundary" target plus residual water-fill — the
/// production pattern.
pub const TABLE_WIDTH_STRATEGY_HYBRID: &str = "Hybrid (penalty + fill)";

/// `on_click` event name fired by the top-bar table-width-strategy
/// dropdown when the user picks a new strategy. Mirrors the
/// `"batch_button"` / `"tools_button"` event names already used by
/// the top toolbar so the test harness can capture it the same way.
pub const TABLE_WIDTH_STRATEGY_EVENT: &str = "table_width_strategy";

/// Format string builder for indexing status when finished.
pub fn build_indexing_finished_text(file_count: usize) -> String {
    format!("Indexing finished ({} files)", file_count)
}

/// Format string builder for indexing status while in progress.
pub fn build_indexing_progress_text(file_count: usize) -> String {
    format!("Indexing workspace (found {} files)...", file_count)
}

// Bottom Panel
/// Hint text for the command input multiline text field.
pub const COMMAND_INPUT_HINT: &str = "Type command (Enter to submit, Shift+Enter for new line)";

/// Menu item label for markdown formatting prompt generation.
pub const FORMAT_MARKDOWN_ACTION: &str = "Format Markdown";

/// Button label to cancel/stop a running agent task.
/// Uses Phosphor [`ICON_STOP`] icon.
pub const STOP_AGENT_BUTTON: &str = "\u{E46C} Stop";

/// Header text for the available models list output.
pub const MODELS_LIST_HEADER: &str = "Available Models:\n";

/// Message appended when no custom models are configured.
pub const MODELS_LIST_NO_ADDITIONAL: &str = "No additional models configured.\n";

/// Deprecation notice message for the `/model` command.
pub const DEPRECATED_MODEL_MESSAGE: &str = "The /model command is deprecated. Models are now automatically selected based on use case and cost.";

// Left Panel
/// Title header of the left file tree panel.
pub const WORKSPACE_HEADER: &str = "Workspace Files";

/// Default root node display name for the workspace tree.
pub const DEFAULT_WORKSPACE_NAME: &str = "Workspace";

/// Empty state message displayed when no markdown files exist in workspace.
pub const NO_MARKDOWN_FILES: &str = "No markdown files found.";

// Right Panel
/// Title header of the table of contents panel.
pub const TABLE_OF_CONTENTS_HEADER: &str = "Table of Contents";

// Center Panel
/// Header for the agent session view in the center panel.
/// Uses Phosphor [`ICON_ROBOT`] icon.
pub const AGENT_SESSION_HEADER: &str = "\u{E762} FastMD Agent Session";

/// Button label to close the agent session view (replaces "Back to document").
pub const AGENT_SESSION_CLOSE_BUTTON: &str = "Close";

/// Prefix label for agent status text.
pub const AGENT_STATUS_PREFIX: &str = "Status: ";

/// Header for collapsible thinking process block in agent session.
pub const AGENT_THINKING_PROCESS: &str = "Thinking Process";

/// Header for final response section in agent session.
pub const AGENT_RESPONSE: &str = "Response";

/// Placeholder text when agent is actively generating response.
pub const AGENT_THINKING_LABEL: &str = "Agent is thinking...";

/// Label for tool call parameter payload string in agent view.
pub const AGENT_TOOL_CALL_PARAM: &str = "Tool call parameter string";

/// Button text for copying rendered agent markdown.
pub const COPY_MARKDOWN_BUTTON: &str = "Copy markdown";

/// Temporary button label after markdown is copied to clipboard.
pub const COPIED_BUTTON: &str = "Copied!";

/// Button label for saving agent response as a file.
pub const SAVE_AS_FILE_BUTTON: &str = "Save as file...";

/// Temporary button label after saving agent response as a file.
pub const SAVED_BUTTON: &str = "Saved!";

/// Button tooltip / label for showing document formatting tools.
pub const SHOW_FORMATTING_TOOLS: &str = "Show formatting tools";

/// Button tooltip / label for hiding document formatting tools.
pub const HIDE_FORMATTING_TOOLS: &str = "Hide formatting tools";

/// Header for document markdown editor view.
pub const MARKDOWN_EDITOR_HEADER: &str = "Markdown Editor";

/// Button label for saving markdown editor content and exiting editor mode.
pub const SAVE_AND_EXIT_BUTTON: &str = "Save & Exit";

/// Generic cancel button label.
pub const CANCEL_BUTTON: &str = "Cancel";

/// Header for document YAML front-matter editor block.
pub const YAML_FRONT_MATTER_HEADER: &str = "YAML Front-Matter";

/// Button label for saving markdown document changes.
pub const SAVE_MARKDOWN_BUTTON: &str = "Save Markdown";

/// Error message prefix when document save fails.
pub const ERROR_SAVING_DOCUMENT_PREFIX: &str = "Error saving document: ";

/// Empty state prompt shown when no document tab is open.
pub const NO_FILE_SELECTED_PROMPT: &str =
    "Select a markdown file from the left pane to view its content";

/// Button label to switch to editor mode for current document.
pub const EDIT_BUTTON: &str = "Edit";

/// Tooltip for tab close button.
pub const CLOSE_TAB_TOOLTIP: &str = "Close";

/// Context menu item to close current tab.
pub const CLOSE_TAB_MENU: &str = "Close Tab";

/// Context menu item to close other open tabs.
pub const CLOSE_OTHERS_MENU: &str = "Close Others";

/// Context menu item to close all open tabs.
pub const CLOSE_ALL_TABS_MENU: &str = "Close All Tabs";

/// Context menu item to open document in system text editor.
pub const OPEN_IN_EDITOR_ACTION: &str = "Open in Editor";

// Modals
/// Window title for move file dialog.
pub const MOVE_FILE_WINDOW: &str = "Move File";

/// Folder selection label in move file dialog.
pub const SELECT_DESTINATION_FOLDER: &str = "Select destination folder:";

/// Generic OK button label.
pub const OK_BUTTON: &str = "Ok";

/// Window title for create directory dialog.
pub const CREATE_DIRECTORY_WINDOW: &str = "Create Directory";

/// Label prompt in create directory dialog.
pub const ENTER_DIRECTORY_NAME: &str = "Enter directory name:";

/// Window title for create document dialog.
pub const CREATE_DOCUMENT_WINDOW: &str = "New Document";

/// Label prompt in create document dialog.
pub const ENTER_DOCUMENT_NAME: &str = "Enter document name:";

/// Window title for rename dialog.
pub const RENAME_WINDOW: &str = "Rename";

/// Label prompt in rename dialog.
pub const ENTER_NEW_NAME: &str = "Enter new name:";

// Batch Dialog
/// Window title for the batch prompt processing dialog.
pub const BATCH_DIALOG_WINDOW: &str = "Batch Prompt Processing";

/// Label for the directory selector in the batch dialog.
pub const BATCH_DIALOG_DIRECTORY_LABEL: &str = "Directory:";

/// Placeholder text for the directory selector when none is selected.
pub const BATCH_DIALOG_SELECT_DIRECTORY: &str = "Select directory...";

/// Label for the file pattern input in the batch dialog.
pub const BATCH_DIALOG_PATTERN_LABEL: &str = "Pattern:";

/// Label for the prompt selector in the batch dialog.
pub const BATCH_DIALOG_PROMPT_LABEL: &str = "Prompt:";

/// Placeholder text for the prompt selector when none is selected.
pub const BATCH_DIALOG_SELECT_PROMPT: &str = "Select prompt...";

/// Label for the batch mode selector in the batch dialog.
pub const BATCH_DIALOG_MODE_LABEL: &str = "Mode:";

/// Batch mode option label for processing files matching a pattern.
pub const BATCH_MODE_FILE: &str = "File";

/// Batch mode option label for processing all files in a directory.
pub const BATCH_MODE_DIRECTORY: &str = "Directory";

/// Label for the concurrency selector in the batch dialog.
pub const BATCH_DIALOG_CONCURRENCY_LABEL: &str = "Concurrency:";

/// Button label to start batch processing.
pub const BATCH_PROCESS_BUTTON: &str = "Process";

/// Heading shown in the batch dialog while processing is running.
pub const BATCH_RUNNING_HEADING: &str = "Batch Processing";

/// Status text shown when batch processing has completed.
pub const BATCH_COMPLETED_TEXT: &str = "Batch processing completed.";

/// Status text shown when batch processing was cancelled by the user.
pub const BATCH_CANCELLED_TEXT: &str = "Batch was cancelled by user.";

/// Status text shown while batch processing is running.
pub const BATCH_PROCESSING_TEXT: &str = "Processing...";

/// Hint text shown while batch processing is running.
pub const BATCH_RUNNING_CANCEL_HINT: &str =
    "Click Cancel to stop processing. In-flight jobs will finish.";

/// Button label to close the batch dialog after processing completes.
pub const BATCH_CLOSE_BUTTON: &str = "Close";

// Tree Widget Actions
/// Context menu item to open file location in OS file explorer.
pub const SHOW_IN_EXPLORER_ACTION: &str = "Show in File Explorer";

/// Context menu item to copy file path to clipboard.
pub const COPY_PATH_ACTION: &str = "Copy path";

/// Context menu item to open rename dialog.
pub const RENAME_ACTION: &str = "Rename";

/// Context menu item to open move file dialog.
pub const MOVE_ACTION: &str = "Move";

/// Context menu item to open create directory dialog.
pub const CREATE_DIRECTORY_ACTION: &str = "Create Directory ...";

/// Context menu item to create a new markdown document.
pub const NEW_DOCUMENT_ACTION: &str = "New document";

/// Context menu item to delete selected file or directory.
pub const DELETE_ACTION: &str = "Delete";

/// Context menu item to merge multiple selected markdown files.
pub const MERGE_ACTION: &str = "Merge";

/// Context menu item to run document content as agent prompt.
pub const RUN_AS_PROMPT_ACTION: &str = "Run as prompt";

/// Context menu item to print document content.
pub const PRINT_ACTION: &str = "Print";

/// Context menu item to export the current Markdown file to PDF via
/// the official `typst` CLI binary. Sits next to `PRINT_ACTION` in the
/// file context menu; dynamically displayed only when `typst` is found
/// in the system PATH.
pub const SAVE_AS_PDF_ACTION: &str = "Save as PDF...";

// Background Logs Window
/// Window title for background logs monitor.
pub const BACKGROUND_PROCESSES_WINDOW: &str = "Background Processes";

/// Search input label in background logs window.
pub const SEARCH_LABEL: &str = "Search:";

/// Category filter label in background logs window.
pub const CATEGORY_LABEL: &str = "Category:";

/// Checkbox label for auto-scrolling log view.
pub const AUTO_SCROLL_CHECKBOX: &str = "Auto-scroll";

/// Button label to clear log entries.
pub const CLEAR_BUTTON: &str = "Clear";

/// Category filter label for all categories.
pub const LOG_CATEGORY_ALL: &str = "All";

/// Category filter label for Indexer logs.
pub const LOG_CATEGORY_INDEXER: &str = "Indexer";

/// Category filter label for Watcher logs.
pub const LOG_CATEGORY_WATCHER: &str = "Watcher";

/// Category filter label for PDF Converter logs.
pub const LOG_CATEGORY_PDF_CONVERTER: &str = "PDF Converter";

/// Category filter label for Image Vision logs.
pub const LOG_CATEGORY_IMAGE_VISION: &str = "Image Vision";

/// Category filter label for LLM Tools logs.
pub const LOG_CATEGORY_LLM_TOOLS: &str = "LLM Tools";

/// Error label when background manager lock fails.
pub const BACKGROUND_MGR_ACCESS_ERROR: &str = "Error: could not access background manager";

// Agent Debug Window
/// Window title for the agent debug window.
pub const AGENT_DEBUG_WINDOW: &str = "Agent Debug";

/// Label for outgoing debug entries.
pub const DEBUG_KIND_OUTGOING: &str = "Outgoing";

/// Label for incoming debug entries.
pub const DEBUG_KIND_INCOMING: &str = "Incoming";

/// Label for tool results debug entries.
pub const DEBUG_KIND_TOOL_RESULTS: &str = "ToolResults";

/// Label for the JSON row count combobox in the agent debug window.
pub const DEBUG_JSON_ROWS_LABEL: &str = "JSON rows:";

/// Label for the copy-JSON button in a debug entry's expanded body.
pub const DEBUG_COPY_JSON_BUTTON: &str = "Copy JSON";

// Inline Editor Widget
/// Save button label in inline text editor.
pub const SAVE_BUTTON: &str = "Save";

/// Error message for the inline editor when opened on a PDF-backed Markdown
/// file. The file is auto-generated from a PDF. Use `write_yaml_header` to
/// modify front-matter.
pub const PDF_BACKED_ERROR: &str = "Cannot modify this file. The system generates the file from a PDF. \
     Use write_yaml_header to modify front-matter.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_session_close_button_is_close() {
        assert_eq!(AGENT_SESSION_CLOSE_BUTTON, "Close");
    }

    #[test]
    fn test_indexing_status_builders() {
        assert_eq!(
            build_indexing_finished_text(5),
            "Indexing finished (5 files)"
        );
        assert_eq!(
            build_indexing_progress_text(12),
            "Indexing workspace (found 12 files)..."
        );
    }

    #[test]
    fn test_create_document_dialog_strings() {
        assert_eq!(CREATE_DOCUMENT_WINDOW, "New Document");
        assert_eq!(ENTER_DOCUMENT_NAME, "Enter document name:");
    }

    #[test]
    fn test_batch_dialog_strings() {
        assert_eq!(BATCH_DIALOG_WINDOW, "Batch Prompt Processing");
        assert_eq!(BATCH_DIALOG_DIRECTORY_LABEL, "Directory:");
        assert_eq!(BATCH_DIALOG_SELECT_DIRECTORY, "Select directory...");
        assert_eq!(BATCH_DIALOG_PATTERN_LABEL, "Pattern:");
        assert_eq!(BATCH_DIALOG_PROMPT_LABEL, "Prompt:");
        assert_eq!(BATCH_DIALOG_SELECT_PROMPT, "Select prompt...");
        assert_eq!(BATCH_DIALOG_MODE_LABEL, "Mode:");
        assert_eq!(BATCH_MODE_FILE, "File");
        assert_eq!(BATCH_MODE_DIRECTORY, "Directory");
        assert_eq!(BATCH_DIALOG_CONCURRENCY_LABEL, "Concurrency:");
        assert_eq!(BATCH_PROCESS_BUTTON, "Process");
        assert_eq!(BATCH_RUNNING_HEADING, "Batch Processing");
        assert_eq!(BATCH_COMPLETED_TEXT, "Batch processing completed.");
        assert_eq!(BATCH_CANCELLED_TEXT, "Batch was cancelled by user.");
        assert_eq!(BATCH_PROCESSING_TEXT, "Processing...");
        assert_eq!(BATCH_CLOSE_BUTTON, "Close");
        assert_eq!(
            BATCH_RUNNING_CANCEL_HINT,
            "Click Cancel to stop processing. In-flight jobs will finish."
        );
    }

    #[test]
    fn test_debug_window_strings() {
        assert_eq!(AGENT_DEBUG_WINDOW, "Agent Debug");
        assert_eq!(DEBUG_KIND_OUTGOING, "Outgoing");
        assert_eq!(DEBUG_KIND_INCOMING, "Incoming");
        assert_eq!(DEBUG_KIND_TOOL_RESULTS, "ToolResults");
    }

    #[test]
    fn test_hamburger_menu_strings() {
        assert_eq!(HAMBURGER_MENU_BUTTON, ICON_LIST);
        assert_eq!(HAMBURGER_MENU_ID_SALT, "top_hamburger_menu");
        assert_eq!(MENU_TABLE_WRAP_ALGORITHM, "Table wrap algorithm");
        assert_eq!(MENU_WINDOWS, "Windows");
        assert_eq!(MENU_BACKGROUND_OPERATIONS, "Background operations");
        assert_eq!(MENU_AGENT_DEBUG, "Agent debug");
        assert_eq!(MENU_CHAT_MODELS, "Chat models");
        assert_eq!(NO_CHAT_MODELS_CONFIGURED, "No models configured");
        assert_eq!(CHAT_MODEL_SELECTION_EVENT, "chat_model_selection");
        assert_eq!(
            format_chat_model_menu_label("gpt-4o", 15),
            "gpt-4o (Cost: 15)"
        );
    }

    #[test]
    fn test_ui_strings_use_phosphor_icons() {
        assert_eq!(
            APP_TITLE, "FastMD Viewer",
            "APP_TITLE is plain text; logo is painted via ui::logo::paint_logo"
        );
        assert!(
            !APP_TITLE.contains(ICON_LIGHTNING),
            "APP_TITLE should not embed Phosphor glyph; use logo module"
        );
        assert!(
            !APP_TITLE.contains('⚡'),
            "APP_TITLE should not use raw lightning emoji"
        );

        assert_eq!(
            HAMBURGER_MENU_BUTTON, ICON_LIST,
            "HAMBURGER_MENU_BUTTON should use Phosphor LIST icon"
        );

        assert!(
            STOP_AGENT_BUTTON.contains(ICON_STOP),
            "STOP_AGENT_BUTTON should use Phosphor STOP icon"
        );
        assert!(
            !STOP_AGENT_BUTTON.contains('⏹'),
            "STOP_AGENT_BUTTON should not use raw stop emoji"
        );

        assert!(
            AGENT_SESSION_HEADER.contains(ICON_ROBOT),
            "AGENT_SESSION_HEADER should use Phosphor ROBOT icon"
        );
        assert!(
            !AGENT_SESSION_HEADER.contains('🤖'),
            "AGENT_SESSION_HEADER should not use raw robot emoji"
        );
    }
}
