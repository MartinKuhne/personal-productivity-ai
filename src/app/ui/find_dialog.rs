//! In-document find dialog — search term entry and navigation between matches.
//!
//! Unit tests live in the sibling `find_dialog_tests.rs` sidecar.

use eframe::egui;

use crate::bus::events::user_command::UserCommand;
use crate::ui::FastMdApp;
use crate::ui::render::render_event_contains_query;
use crate::ui::strings;

/// Purpose: Counts the number of occurrences of `query` within markdown text blocks.
/// Inputs: `markdown` - raw markdown string, `query` - search term
/// Outputs: Total count of matching blocks/events
/// Purity: Pure
pub fn count_document_matches(markdown: &str, query: &str) -> usize {
    if query.trim().is_empty() {
        return 0;
    }
    let query_lower = query.to_lowercase();
    let events = crate::markdown::parse_markdown_to_events(markdown);
    events
        .iter()
        .filter(|ev| render_event_contains_query(ev, &query_lower))
        .count()
}

/// Purpose: Advances to the next match index, wrapping around to 0.
/// Inputs: `current` - current match index, `total` - total number of matches
/// Outputs: Next match index
/// Purity: Pure
pub fn next_match_index(current: usize, total: usize) -> usize {
    if total == 0 { 0 } else { (current + 1) % total }
}

/// Purpose: Decrements to the previous match index, wrapping around to `total - 1`.
/// Inputs: `current` - current match index, `total` - total number of matches
/// Outputs: Previous match index
/// Purity: Pure
pub fn prev_match_index(current: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        (current + total - 1) % total
    }
}

/// Purpose: Formats the match status string for the find dialog UI.
/// Inputs: `current` - 0-based match index, `total` - total number of matches
/// Outputs: User-facing status string
/// Purity: Pure
pub fn format_match_status(current: usize, total: usize) -> String {
    if total == 0 {
        strings::FIND_NO_MATCHES.to_string()
    } else {
        strings::FIND_MATCH_STATUS_FORMAT
            .replacen("{}", &(current + 1).to_string(), 1)
            .replacen("{}", &total.to_string(), 1)
    }
}

/// Purpose: Renders the find-in-document dialog floating window.
/// Inputs: `ctx` - Egui context, `app` - mutable application state
/// Outputs: None
/// Purity: Impure (modifies UI state and publishes user commands)
pub fn show_find_dialog(ctx: &egui::Context, app: &mut FastMdApp) {
    let mut open = app.orchestrator.dialogs.find_dialog_open;
    if !open {
        return;
    }

    let mut query = app.orchestrator.dialogs.find_query.clone();
    let current_idx = app.orchestrator.dialogs.find_match_index;
    let has_doc = !app.orchestrator.tabs.current_markdown.is_empty();
    let total_matches = if has_doc && !query.trim().is_empty() {
        count_document_matches(&app.orchestrator.tabs.current_markdown, &query)
    } else {
        0
    };

    let mut should_next = false;
    let mut should_prev = false;
    let mut query_changed = false;
    let focus_requested = app.orchestrator.dialogs.find_focus_requested;

    egui::Window::new(strings::FIND_DIALOG_TITLE)
        .id(egui::Id::new("find_in_doc_dialog"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(320.0)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 56.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let text_edit = egui::TextEdit::singleline(&mut query)
                    .hint_text(strings::FIND_INPUT_HINT)
                    .desired_width(160.0);
                let response = ui.add(text_edit);

                if focus_requested {
                    response.request_focus();
                }

                if response.changed() {
                    query_changed = true;
                }

                if response.has_focus() {
                    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter)) {
                        should_prev = true;
                    } else if ctx
                        .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                    {
                        should_next = true;
                    }
                }

                let prev_btn = ui.add_enabled(
                    total_matches > 0,
                    egui::Button::new(strings::FIND_PREV_BUTTON),
                );
                if prev_btn.clicked() {
                    should_prev = true;
                }

                let next_btn = ui.add_enabled(
                    total_matches > 0,
                    egui::Button::new(strings::FIND_NEXT_BUTTON),
                );
                if next_btn.clicked() {
                    should_next = true;
                }
            });

            ui.horizontal(|ui| {
                let status_text = if !has_doc {
                    strings::FIND_NO_DOCUMENT.to_string()
                } else if query.trim().is_empty() {
                    String::new()
                } else {
                    format_match_status(current_idx, total_matches)
                };

                ui.label(
                    egui::RichText::new(status_text)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(160, 190, 220)),
                );
            });
        });

    if focus_requested {
        app.orchestrator.dialogs.find_focus_requested = false;
    }

    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        open = false;
    }

    if !open {
        app.orchestrator
            .user_command_bus
            .publish(UserCommand::CloseFindDialog);
    } else if query_changed {
        app.orchestrator
            .user_command_bus
            .publish(UserCommand::SetFindQuery(query));
    } else if should_next {
        app.orchestrator
            .user_command_bus
            .publish(UserCommand::FindNext);
    } else if should_prev {
        app.orchestrator
            .user_command_bus
            .publish(UserCommand::FindPrevious);
    }
}

#[cfg(test)]
#[path = "find_dialog_tests.rs"]
mod tests;
