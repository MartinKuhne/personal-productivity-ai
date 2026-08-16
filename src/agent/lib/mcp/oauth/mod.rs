//! MCP client OAuth 2.1 authorization flow.
//!
//! Implements the requirements in `doc/distill/mcp.md` §4. This
//! module is the public entry point used by the MCP client
//! session to:
//!
//! * Trigger an authorization flow on a 401 from the resource
//!   server.
//! * Carry the resulting access token as a `Authorization: Bearer
//!   <token>` header on every subsequent request.
//! * Handle step-up authorization when the resource server
//!   returns 403 with `error="insufficient_scope"` (MCP §4.7).
//! * Cache tokens in a [`TokenStore`] so the user only signs in
//!   once per MCP server.
//!
//! ## Spec compliance status (mcp.md §4)
//!
//! | Spec section | Status |
//! |--------------|--------|
//! | §4.1 Overview | Implemented (client acts as OAuth 2.1 client) |
//! | §4.2 Requirements | Implemented (HTTP only; stdio bypasses) |
//! | §4.3 Discovery (PRM) | Implemented (challenge + well-known probes) |
//! | §4.3.1 Discovery (AS) | Implemented (full candidate order) |
//! | §4.4 Client Registration | Pre-registered + DCR implemented; CIDM + manual TODO |
//! | §4.5 Scope Selection | Implemented (challenge > scopes_supported) |
//! | §4.6 Auth Flow | Implemented (PKCE S256, resource param, loopback redirect) |
//! | §4.7 Step-Up | Implemented (`insufficient_scope` → re-flow with new scope) |
//! | §4.8 Error Handling | Implemented (401 → flow, 403 → step-up, 400 → no retry) |
//! | §4.9 Security | Implemented (PKCE verify, HTTPS-only, state check) |

mod client;
mod discovery;
mod flow;
mod pkce;
mod redirect;
mod store;
pub mod types;

#[cfg(test)]
pub mod test_support;

pub use client::OAuthClient;
pub use flow::{
    BrowserOverride, OAuthFlowInputs, OAuthFlowOutput, PreRegisteredClient, build_resource_uri,
    refresh, run_flow, run_flow as run_oauth_flow,
};
pub use pkce::{
    AuthorizationUrlInputs, PkcePair, State, build_authorization_url, percent_encode, s256,
};
pub use redirect::{
    DEFAULT_CALLBACK_PATH, DEFAULT_CALLBACK_TIMEOUT, LoopbackServer, open_browser,
    parse_redirect_uri, start as start_loopback,
};
pub use store::{DEFAULT_EXPIRY_SKEW, StoredToken, TOKEN_STORE_FILE_NAME, TokenStore};
pub use types::{
    AuthorizationServerMetadata, ClientRegistrationRequest, ClientRegistrationResponse, OAuthError,
    OAuthErrorBody, ProtectedResourceMetadata, TokenResponse, WwwAuthenticateChallenge,
    parse_bearer_challenge,
};

// Re-export the discovery helpers at the module level too.
pub use discovery::{
    DISCOVERY_TIMEOUT, discover_authorization_server_metadata, discover_resource_metadata,
    well_known_as_metadata_candidates, well_known_resource_metadata_candidates,
};
