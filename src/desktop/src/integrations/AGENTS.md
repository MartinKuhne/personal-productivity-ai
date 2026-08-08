# Third party integrations

- Modules in this folder are typically self-contained external services
  (chat bots, message platforms). In general they SHOULD NOT have
  references outside the `/src/desktop/src/integrations` folder.
- **MCP is a deliberate exception.** The wire-protocol client in
  [`mcp/`](mcp/) is a normal integration but it is also the
  source of dynamically-discovered tools. The LLM-tool-loop glue
  ([`crate::agent::tools::mcp::McpToolAdapter`](../agent/tools/mcp/adapter.rs))
  is allowed to depend on it; that coupling is by design. The
  tools-dialog `Authenticate` action and the `McpClientManager`
  handle live in the protocol layer and are surfaced to the UI
  through the agent's `ToolManager`, not through direct cross-folder
  imports in the UI layer.
