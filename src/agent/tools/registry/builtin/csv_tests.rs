//! Tests for CSV registry provider — descriptor, safety, prompt gate.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

#[test]
fn csv_provider_registers_five_tools() {
    let provider = CsvProvider;
    assert_eq!(provider.id(), "csv");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::CsvDb)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 5);
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    for expected in ["create_csv", "list_csv", "add_rows", "delete_rows", "query"] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn safety_assignments_match_spec() {
    assert_eq!(CsvCreateTool.safety(), Safety::Mutating);
    assert_eq!(CsvListTool.safety(), Safety::ReadOnly);
    assert_eq!(CsvAddRowsTool.safety(), Safety::Mutating);
    assert_eq!(CsvDeleteRowsTool.safety(), Safety::Mutating);
    assert_eq!(CsvQueryTool.safety(), Safety::ReadOnly);
}

#[test]
fn tool_groups_are_csv_db() {
    for tool in CsvProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(InternalToolGroup::CsvDb),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn descriptors_carry_csv_prompt_gate() {
    for t in CsvProvider.tools() {
        assert!(
            t.descriptor.config.prompt_rule.is_some(),
            "{} missing prompt gate",
            t.descriptor.name
        );
    }
}

#[test]
fn descriptors_have_input_schemas() {
    for t in CsvProvider.tools() {
        assert!(
            t.descriptor.parameters_schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(CsvCreateTool);
    assert_eq!(r.descriptor.name, "create_csv");
    assert_eq!(r.executor.descriptor().name, "create_csv");
}
