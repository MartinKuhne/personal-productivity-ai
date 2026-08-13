//! Unit tests for the prompt-content rules in
//! [`super::prompt_rules`].

use super::prompt_rules::csv_prompt_rule;

#[test]
fn test_csv_prompt_rule_gates_on_prompt_keyword() {
    let spec = csv_prompt_rule();
    let mut c = crate::agent::config::AgentConfig::default();
    c.tool_groups.csv_db = true;
    // Negative cases — no keyword in the prompt.
    assert!(!spec.is_enabled_for(&c, "what is the weather today"));
    assert!(!spec.is_enabled_for(&c, "summarise my notes"));
    // Positive cases — the tool name itself or a synonym.
    assert!(spec.is_enabled_for(&c, "show me the csv database"));
    assert!(spec.is_enabled_for(&c, "add_rows to my csv"));
    assert!(spec.is_enabled_for(&c, "TABLE of contents"));
}

#[test]
fn test_csv_prompt_rule_gates_on_group_flag() {
    let spec = csv_prompt_rule();
    let mut c = crate::agent::config::AgentConfig::default();
    c.tool_groups.csv_db = false;
    // Even with a matching keyword, the group flag must also be on.
    assert!(!spec.is_enabled_for(&c, "show me the csv database"));
    c.tool_groups.csv_db = true;
    assert!(spec.is_enabled_for(&c, "show me the csv database"));
}

#[test]
fn test_csv_prompt_rule_case_insensitive() {
    let spec = csv_prompt_rule();
    let mut c = crate::agent::config::AgentConfig::default();
    c.tool_groups.csv_db = true;
    assert!(spec.is_enabled_for(&c, "query the CSV"));
    assert!(spec.is_enabled_for(&c, "what is a TABLE"));
}
