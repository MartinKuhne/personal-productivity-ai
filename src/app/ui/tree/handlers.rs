//! Merge-prompt builder for multi-file "merge" agent commands.
//!
//! Tree row clicks are owned by the left panel's
//! [`row_click_command`](crate::ui_iced::panels::left::row_click_command).
//! What lives here is the framework-free prompt builder that the
//! [`command_executor`](crate::command_executor) module uses for the
//! multi-file merge command.
//!
//! Unit tests live in the sibling `handlers_tests.rs` sidecar.

use std::collections::HashSet;
use std::path::PathBuf;

/// Build the agent prompt that merges the selected documents into one.
pub fn build_merge_prompt(
    content_libraries: &[crate::config::ContentLibrary],
    selected_files: &HashSet<PathBuf>,
) -> String {
    let mut prompt = "Please read each of the following documents using the read_note tool and merge their content into a new document. Consolidate overlapping content, deduplicate repeated information, and produce a single unified document that combines all of the source material:\n".to_string();
    let mut sorted_files: Vec<&PathBuf> = selected_files.iter().collect();
    sorted_files.sort();
    for file in sorted_files {
        let rel_str = crate::config::library_display_label(content_libraries, file)
            .unwrap_or_else(|| file.to_string_lossy().to_string());
        prompt.push_str(&format!("- {}\n", rel_str));
    }
    prompt
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `handlers_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
