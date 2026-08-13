//! MCP tool family — the LLM-tool-loop adapter that turns tools
//! advertised by an MCP server into [`Tool`](super::Tool) implementations.
//!
//! The actual MCP wire-protocol client (transports, sessions, OAuth,
//! manager) lives in
//! [`crate::agent::lib::mcp`]. This module only owns the
//! [`McpToolAdapter`] glue that lets the LLM tool loop discover and
//! call MCP-provided tools, plus the in-process auth actions the
//! tools dialog needs to surface.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (MCP-001..MCP-021) for the
//! full MCP requirements set covering both the protocol layer
//! (in `crate::agent::lib::mcp`) and the LLM-tool-loop glue
//! (this module).

mod adapter;

pub use adapter::McpToolAdapter;

// Re-export the protocol-level types so existing callers can keep
// using `crate::agent::tools::mcp::{McpClients,
// McpToolDescriptor, ...}`. The protocol implementation itself lives
// in `crate::agent::lib::mcp`.
pub use crate::agent::lib::mcp::{
    AuthorizationServerMetadata, ClientRegistrationRequest, ClientRegistrationResponse, McpError,
    McpToolDescriptor, OAuthClient, OAuthError, OAuthFlowInputs, OAuthFlowOutput,
    PreRegisteredClient, ProtectedResourceMetadata, StoredToken, TokenResponse, TokenStore,
    WwwAuthenticateChallenge, parse_bearer_challenge, parse_redirect_uri, run_oauth_flow,
};
pub use crate::agent::lib::mcp::{
    CLIENT_NAME, CLIENT_VERSION, DEFAULT_REQUEST_TIMEOUT, DynamicToolSource, MAX_REQUEST_TIMEOUT,
    PROTOCOL_VERSION, is_valid_session_id,
};
pub use crate::agent::lib::mcp::{McpClientSession, McpClients};

#[cfg(test)]
mod adapter_tests;
