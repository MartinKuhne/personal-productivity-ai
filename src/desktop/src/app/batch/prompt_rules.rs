//! Prompt-content rules that determine tool eligibility.
//!
//! Today the only such rule is the CSV family's TOOL-001 gate: the
//! LLM only sees the CSV tools (`add_rows`, `delete_rows`,
//! `create_csv`, `list_csv`, `query`) when the prompt mentions one
//! of the CSV keywords. The keyword list and the rule constructor
//! live here, in the application domain, because the choice of
//! which words trigger which tools is a product decision, not a
//! tool-system concern.
//!
//! Add a sibling function in this file for any future
//! prompt-gated tool family. The `ToolConfigSpec` builder
//! returned by each function is what the tool's `descriptor()`
//! method then hands to the tool system.

use crate::agent::tools::descriptor::{PromptPredicate, ToolConfigSpec};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};

/// The keywords the CSV family looks for in the prompt (TOOL-001).
/// Case-insensitive: the matcher lower-cases the prompt before
/// scanning.
fn csv_keywords() -> &'static [&'static str] {
    &[
        "table",
        "csv",
        "database",
        "add_rows",
        "delete_rows",
        "create_csv",
        "list_csv",
        "query",
    ]
}

/// Build the [`ToolConfigSpec`] for the CSV family of tools. The
/// spec is gated on:
/// 1. the `tool_groups.csv_db` flag being on (the group
///    enable), and
/// 2. the current prompt containing at least one of the CSV
///    keywords (TOOL-001).
pub fn csv_prompt_rule() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::CsvDb);
    ToolConfigSpec {
        group: Some(group),
        requires: Vec::new(),
        prompt_rule: Some(PromptPredicate::ContainsAny(csv_keywords())),
    }
}
