//! YAML front-matter header tool implementations and provider for the tool registry.
//!
//! Unit tests live in the sibling `yaml_tests.rs` sidecar.

use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::dtos;
use crate::tools::provider::{RegisteredTool, ToolProvider};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::strings;

/// Tool that parses a YAML front-matter header from a Markdown file.
#[derive(ToolDescriptor)]
#[tool(
    name = "read_yaml_header",
    desc = strings::READ_YAML_HEADER_DESCRIPTION,
    input = dtos::ReadYamlHeaderInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_read_yaml_header,
)]
pub(crate) struct ReadYamlHeaderTool;
fn execute_read_yaml_header(
    _self: &ReadYamlHeaderTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::ReadYamlHeaderInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let path = ctx
        .resolve_virtual_path(&input.path, false)?
        .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?
        .path;
    crate::tools::yaml_header::tool_read_yaml_header(ctx, &path.to_string_lossy()).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that writes or updates a YAML front-matter header in a Markdown file.
#[derive(ToolDescriptor)]
#[tool(
    name = "write_yaml_header",
    desc = strings::WRITE_YAML_HEADER_DESCRIPTION,
    input = dtos::WriteYamlHeaderInput,
    safety = crate::tools::Safety::Mutating,
    group = Filesystem,
    execute_with = execute_write_yaml_header,
)]
pub(crate) struct WriteYamlHeaderTool;
fn execute_write_yaml_header(
    _self: &WriteYamlHeaderTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::WriteYamlHeaderInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let path = ctx.resolve_writable(&input.path)?;
    let observer = ctx.file_observer();
    crate::tools::yaml_header::tool_write_yaml_header(
        ctx,
        &path.to_string_lossy(),
        input.title.as_deref(),
        input.summary.as_deref(),
        input.tags,
        input.header_date.as_deref(),
        &*observer,
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Self-registering provider for the YAML front-matter family.
pub(crate) struct YamlProvider;
impl ToolProvider for YamlProvider {
    fn id(&self) -> &'static str {
        "yaml"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Filesystem)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(ReadYamlHeaderTool),
            registered(WriteYamlHeaderTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}

#[cfg(test)]
#[path = "yaml_tests.rs"]
mod tests;
