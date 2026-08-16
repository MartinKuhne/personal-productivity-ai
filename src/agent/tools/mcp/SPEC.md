# MCP Client Tools Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.
>
> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate)

## Requirements

The requirements below have been formatted using the **Easy Approach to Requirements Syntax (EARS)**, utilizing Ubiquitous, Event-Driven (When), State-Driven (While), Unwanted Behavior (If), and Optional Feature (Where) templates.

### Model Context Protocol Client

* [MCP-001] The MCP client shall implement all required aspects of the /doc/distill/mcp.md protocol
* [MCP-002] When the FastMD application starts, the MCP client shall connect to all configured MCP servers and issue a ping request
* [MCP-003] When a ping request has been successful, the MCP client shall connect to all configured MCP servers and retrieve a list of tools
* [MCP-004] When new tools are discovered, the MCP client shall add them to the tools registry
* [MCP-005] When the LLM issues a tool call for a tool that is connected via the MCP client, the MCP client shall make the tool call and return the results.

### OAuth 2.1 Authorization (HTTP transports only)

The following requirements cover the OAuth 2.1 flow from
`/doc/distill/mcp.md` §4. The flow runs on Streamable HTTP
transports only; stdio servers retrieve credentials from the
environment and never enter the OAuth path.

* [MCP-006] The MCP client shall implement the OAuth 2.1 authorization code flow with PKCE (`S256`) per `/doc/distill/mcp.md` §4.6
* [MCP-007] When the configured server has no static `Authorization` header, the MCP client shall attach a bearer access token to every request, sourced from persistent token storage.
* [MCP-008] When the MCP server returns `401 Unauthorized` with a `WWW-Authenticate` header, the MCP client shall discover the authorization server per RFC 9728 (Protected Resource Metadata) and RFC 8414 (Authorization Server Metadata), obtain a token via the authorization code flow, and retry the original request
* [MCP-009] When the MCP server returns `403 Forbidden` with `WWW-Authenticate: Bearer error="insufficient_scope"`, the MCP client shall run a step-up authorization flow that includes the required scopes and retry the original request once
* [MCP-010] The MCP client shall cache access and refresh tokens in secure token storage keyed by the canonical MCP server resource URI, with file-level read/write restricted to the current user
* [MCP-011] The MCP client shall register the OAuth client in the priority order: pre-registered client configuration → Client ID Metadata Document (scaffolded) → Dynamic Client Registration (RFC 7591) → error
* [MCP-012] The MCP client shall select scopes per spec §4.5: prefer the `scope` parameter from the initial `WWW-Authenticate` header, falling back to `scopes_supported` from the resource metadata; the client shall not request scopes beyond the union of those sources plus any explicit `McpOAuthConfig.scopes`
* [MCP-013] The MCP client shall include the `resource` parameter (RFC 8707) on both the authorization and token requests, set to the canonical MCP server URI
* [MCP-014] The MCP client shall use a loopback HTTP server bound to `127.0.0.1` on a random port to receive the authorization code redirect (RFC 8252 §7.3)
* [MCP-015] The MCP client shall refuse to proceed with the authorization flow if the server metadata does not advertise `S256` in `code_challenge_methods_supported`
* [MCP-016] The MCP client shall verify the `state` parameter on the redirect callback against the value sent on the authorization request; a mismatch shall produce an error and no token exchange
* [MCP-017] The MCP client shall limit OAuth retries to one per top-level call to prevent infinite loops on a misconfigured server
* [MCP-018] While the bearer access token is past its expiry skew, the MCP client shall omit the token on the next request and let the 401 / refresh path handle re-authorization

### Tools Dialog Authenticate Action

* [MCP-019] Authentication Entry Point: The system shall provide an authentication flow that, for an SSE server with no static `Authorization` header, triggers a probe request and runs the OAuth 2.1 authorization code flow on a `401 Unauthorized` with `WWW-Authenticate`. For ineligible servers (stdio, or SSE with a static `Authorization` header), authentication shall be reported as not required.
* [MCP-020] Authentication Eligibility: The system shall evaluate authentication eligibility, returning true only for SSE servers with no `Authorization` header. This eligibility rule determines whether the `Authenticate` action is rendered in the UI.
* [MCP-021] Authentication Error Propagation: When server authentication fails (loopback startup, discovery, token exchange, or step-up), the system shall return a structured error containing the OAuth step that failed and record the authentication error on the server group.


