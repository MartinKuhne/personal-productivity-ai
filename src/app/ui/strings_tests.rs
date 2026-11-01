//! Tests for the UI text constants and formatting helpers in `ui::strings`.

use super::*;

#[test]
fn test_markdown_render_marker_strings() {
    assert_eq!(LIST_BULLET, "\u{2022}");
    assert_eq!(ORDERED_LIST_MARKER_SUFFIX, ". ");
}

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
fn test_editor_status_and_validation_text_builders() {
    assert_eq!(build_editor_status_text(1, 1), "Line: 1 | Col: 1");
    assert_eq!(build_editor_status_text(42, 7), "Line: 42 | Col: 7");
    assert_eq!(
        build_editor_validation_error(EDITOR_ERR_UNTERMINATED_FENCE, 3, 1),
        "Markdown validation failed: unterminated code fence (line 3, column 1)"
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
    assert_eq!(BATCH_DIALOG_PATTERN_HINT, "*.md");
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
fn test_tools_dialog_column_and_marker_strings() {
    assert_eq!(TOOLS_GROUP_COLUMN, "Group");
    assert_eq!(TOOLS_KIND_COLUMN, "Kind");
    assert_eq!(TOOLS_ACTIONS_COLUMN, "Actions");
    assert_eq!(TOOLS_ERROR_ICON, "⚠");
    assert_eq!(TOOLS_PARALLEL_CHIP, "✓ parallel");
    assert_eq!(TOOLS_EMPTY_STATE, "No tool groups registered.");
}

/// Defect #5: the summary line must pluralise, and must never fall back
/// to the column header's `"Group"`.
#[test]
fn test_tools_group_count_pluralises() {
    assert_eq!(TOOLS_GROUP_COUNT_TEMPLATE, "{count} groups");
    assert_eq!(TOOLS_GROUP_COUNT_ONE, "1 group");
    assert_eq!(tools_group_count(0), "0 groups");
    assert_eq!(tools_group_count(1), "1 group");
    assert_eq!(tools_group_count(2), "2 groups");
    assert_eq!(tools_group_count(10), "10 groups");
    assert_ne!(
        tools_group_count(10),
        format!("10 {TOOLS_GROUP_COLUMN}"),
        "the column header is not a pluralisable noun"
    );
}

/// The close affordance's glyph and its mandatory tooltip.
#[test]
fn test_tools_close_glyph_and_tooltip_strings() {
    assert_eq!(ICON_CLOSE, "\u{00D7}");
    assert_eq!(TOOLS_CLOSE_TOOLTIP, "Close");
    assert_eq!(CLOSE_BUTTON, "Close");
}

#[test]
fn test_format_tool_group_error_joins_kind_and_message() {
    assert_eq!(
        format_tool_group_error("Execution", "Permission denied"),
        "Execution: Permission denied"
    );
    // Empty message still yields the `Kind: ` shape, never an empty line.
    assert_eq!(format_tool_group_error("Discovery", ""), "Discovery: ");
}

#[test]
fn test_save_as_pdf_dialog_strings() {
    assert_eq!(SAVE_AS_PDF_ACTION, "Save as PDF...");
    assert_eq!(SAVE_AS_PDF_DIALOG_TITLE, "Save as PDF");
    assert_eq!(SAVE_AS_PDF_FILTER_LABEL, "PDF document");
    assert_eq!(SAVE_AS_PDF_FILTER_EXTENSION, "pdf");
}

#[test]
fn test_debug_window_strings() {
    assert_eq!(AGENT_DEBUG_WINDOW, "Agent Debug");
    assert_eq!(DEBUG_KIND_OUTGOING, "Outgoing");
    assert_eq!(DEBUG_KIND_INCOMING, "Incoming");
    assert_eq!(DEBUG_KIND_TOOL_RESULTS, "ToolResults");
    assert_eq!(DEBUG_JSON_ROWS_LABEL, "JSON rows:");
    assert_eq!(DEBUG_COPY_JSON_BUTTON, "Copy JSON");
    assert_eq!(
        DEBUG_OMITTED_TOOLS_PLACEHOLDER,
        "[omitted - see earlier turn]"
    );
}

#[test]
fn test_about_window_strings() {
    assert_eq!(ABOUT_VERSION_LABEL, "Version:");
    assert_eq!(
        ABOUT_ATTRIBUTIONS_EMPTY,
        "No third-party dependencies recorded."
    );
    assert_eq!(CLOSE_BUTTON, "Close");
}

#[test]
fn test_about_description_comes_from_the_manifest() {
    assert!(
        !ABOUT_DESCRIPTION.trim().is_empty(),
        "the manifest description must not be empty"
    );
    assert!(
        ABOUT_DESCRIPTION.to_lowercase().contains("markdown"),
        "ABOUT_DESCRIPTION is `CARGO_PKG_DESCRIPTION`; got {ABOUT_DESCRIPTION:?}"
    );
}

#[test]
fn test_format_attribution_count() {
    assert_eq!(
        format_attribution_count(0),
        "0 direct third-party dependencies"
    );
    assert_eq!(
        format_attribution_count(1),
        "1 direct third-party dependency"
    );
    // Plural form at an arbitrary count. 61 is the shipped catalog's size, but
    // this test is about the formatter; the catalog itself is pinned by
    // `ui_iced::attributions::tests::from_catalog_is_the_shared_catalog_verbatim.
    assert_eq!(
        format_attribution_count(61),
        "61 direct third-party dependencies"
    );
}

#[test]
fn test_hamburger_menu_strings() {
    assert_eq!(HAMBURGER_MENU_BUTTON, "☰");
    assert_eq!(HAMBURGER_MENU_ID_SALT, "top_hamburger_menu");
    assert_eq!(MENU_ABOUT, "About FastMD...");
    assert_eq!(ABOUT_EVENT, "about_button");
    assert_eq!(ABOUT_DIALOG_TITLE, "About FastMD");
    assert_eq!(ABOUT_APP_NAME, "FastMD AI");
    assert_eq!(MENU_TABLE_WRAP_ALGORITHM, "Table wrap algorithm");
    assert_eq!(MENU_WINDOWS, "Windows...");
    assert_eq!(MENU_WINDOWS_TITLE, "Windows");
    assert_eq!(MENU_BACK_TO_MAIN, "< Main menu");
    assert_eq!(MENU_BACKGROUND_OPERATIONS, "Background operations");
    assert_eq!(MENU_AGENT_DEBUG, "Agent debug");
    assert_eq!(MENU_CHAT_MODELS, "Chat models");
    assert_eq!(NO_CHAT_MODELS_CONFIGURED, "No models configured");
    assert_eq!(CHAT_MODEL_SELECTION_EVENT, "chat_model_selection");
    assert_eq!(format_menu_submenu_label(MENU_WINDOWS), "Windows... >");
    assert_eq!(
        format_chat_model_menu_label("gpt-4o", Some(15)),
        "gpt-4o (Cost: 15)"
    );
    assert_eq!(
        format_chat_model_menu_label("gpt-4o", None),
        "gpt-4o (Cost: unknown)",
        "an unconfigured price is unknown, never 0"
    );
    assert_eq!(COST_LABEL_UNKNOWN, "unknown");
}

#[test]
fn test_agent_panel_prompt_strings() {
    assert_eq!(AGENT_USER_TURN_MARKER, "**User:** ");
    assert_eq!(AGENT_SEND_BUTTON, "Send");
    assert_eq!(AGENT_QUEUE_BUTTON, "Queue");
    assert_eq!(AGENT_QUEUED_PREFIX, "Queued: ");
    assert_eq!(format_queued_prompts_label(0), "Queued: 0");
    assert_eq!(format_queued_prompts_label(3), "Queued: 3");
}

#[test]
fn test_ui_strings_use_phosphor_icons() {
    assert_eq!(
        APP_TITLE, "FastMD AI",
        "APP_TITLE is plain text; logo is rendered by the logo module"
    );
    assert!(
        !APP_TITLE.contains(ICON_LIGHTNING),
        "APP_TITLE should not embed Phosphor glyph; use logo module"
    );
    assert!(
        !APP_TITLE.contains('⚡'),
        "APP_TITLE should not use raw lightning emoji"
    );

    assert_eq!(HAMBURGER_MENU_BUTTON, "☰");

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
