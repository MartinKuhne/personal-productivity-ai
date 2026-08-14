//! Unit tests for [`super::provider`] — `RegisteredTool`, `ToolProvider`.

use super::provider::{RegisteredTool, ToolProvider};
use crate::tools::Tool;
use crate::tools::descriptor::ToolDescriptor;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use std::any::TypeId;
use std::sync::Arc;

struct DummyTool;

impl Tool for DummyTool {
    fn descriptor(&self) -> &ToolDescriptor {
        use std::sync::OnceLock;
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            ToolDescriptor::new::<DummyInput>(
                "dummy",
                "test tool",
                crate::tools::Safety::ReadOnly,
                crate::tools::descriptor::ToolConfigSpec::group_only(ToolGroupId::Internal(
                    InternalToolGroup::Filesystem,
                )),
                ToolGroupId::Internal(InternalToolGroup::Filesystem),
            )
        })
    }
    fn execute(
        &self,
        _ctx: &crate::tools::context::ToolContext,
        _args: &str,
    ) -> Result<serde_json::Value, crate::tools::ToolError> {
        Ok(serde_json::json!({"ok": true}))
    }
}

#[derive(schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
struct DummyInput {
    q: String,
}

struct DummyProvider;

impl ToolProvider for DummyProvider {
    fn id(&self) -> &'static str {
        "dummy"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Filesystem)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        let descriptor = ToolDescriptor::new::<DummyInput>(
            "dummy",
            "test tool",
            crate::tools::Safety::ReadOnly,
            crate::tools::descriptor::ToolConfigSpec::group_only(ToolGroupId::Internal(
                InternalToolGroup::Filesystem,
            )),
            ToolGroupId::Internal(InternalToolGroup::Filesystem),
        );
        vec![RegisteredTool {
            descriptor: Arc::new(descriptor),
            executor: Arc::new(DummyTool),
        }]
    }
}

#[test]
fn test_provider_returns_one_tool() {
    let provider = DummyProvider;
    let tools = provider.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].descriptor.name, "dummy");
}

#[test]
fn test_provider_id_and_group() {
    let provider = DummyProvider;
    assert_eq!(provider.id(), "dummy");
    assert_eq!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Filesystem)
    );
}

#[test]
fn test_registered_tool_clone_shares_arc() {
    let descriptor = ToolDescriptor::new::<DummyInput>(
        "dummy",
        "test",
        crate::tools::Safety::ReadOnly,
        crate::tools::descriptor::ToolConfigSpec::group_only(ToolGroupId::Internal(
            InternalToolGroup::Filesystem,
        )),
        ToolGroupId::Internal(InternalToolGroup::Filesystem),
    );
    let entry = RegisteredTool {
        descriptor: Arc::new(descriptor),
        executor: Arc::new(DummyTool),
    };
    let cloned = entry.clone();
    assert_eq!(
        Arc::strong_count(&cloned.descriptor),
        2,
        "Arc descriptor is shared between original and clone"
    );
}

#[test]
fn test_descriptor_input_type_is_dummy() {
    let provider = DummyProvider;
    let tool = &provider.tools()[0];
    // The descriptor's `input_type` should match the DTO type the
    // tool was built from.
    let expected = TypeId::of::<DummyInput>();
    assert_eq!(tool.descriptor.input_type, expected);
}

#[test]
fn test_provider_refresh_default_is_noop() {
    // `ToolProvider::refresh` defaults to no-op; verify it
    // compiles and runs without touching state.
    let mut provider = DummyProvider;
    provider.refresh(&crate::config::AgentConfig::default());
}
