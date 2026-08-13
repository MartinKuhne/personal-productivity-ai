//! YAML front-matter header tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;

use super::strings;

/// Tool that parses a YAML front-matter header from a Markdown file.
pub(crate) struct ReadYamlHeaderTool;
impl Tool for ReadYamlHeaderTool {
    crate::tool_descriptor! {
        name: "read_yaml_header",
        desc: strings::READ_YAML_HEADER_DESCRIPTION,
        input: dtos::ReadYamlHeaderInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReadYamlHeaderInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        crate::agent::tools::yaml_header::tool_read_yaml_header(&path.to_string_lossy()).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that writes or updates a YAML front-matter header in a Markdown file.
pub(crate) struct WriteYamlHeaderTool;
impl Tool for WriteYamlHeaderTool {
    crate::tool_descriptor! {
        name: "write_yaml_header",
        desc: strings::WRITE_YAML_HEADER_DESCRIPTION,
        input: dtos::WriteYamlHeaderInput,
        safety: crate::agent::tools::Safety::Mutating,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WriteYamlHeaderInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        let producer = ctx.file_event_producer();
        crate::agent::tools::yaml_header::tool_write_yaml_header(
            &path.to_string_lossy(),
            input.title.as_deref(),
            input.summary.as_deref(),
            input.tags,
            input.header_date.as_deref(),
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}
