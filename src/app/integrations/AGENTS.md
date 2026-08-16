# Third party integrations

- Keep modules in this folder self-contained unless a clear exception exists.
- Keep integration code in this folder. Do not add cross-folder references unless the dependency is required by design.
- Treat MCP as a deliberate exception. The protocol client in [mcp/](mcp/) MAY be used by the tool adapter layer in [../agent/tools/mcp/adapter.rs](../agent/tools/mcp/adapter.rs).
- Keep the protocol layer and UI layer separated. Do not route UI concerns through direct cross-folder imports.
- When you change requirements, update the implementation and tests in the same change.
- When you detect drift, report it before you continue.
