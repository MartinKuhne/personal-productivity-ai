//! User-facing text for the agent crate: page copy the user reads in a browser
//! and error messages the application surfaces, kept out of the modules that
//! render or raise them.
//!
//! Scope is deliberately narrow — text a human reads. Log messages, `tracing`
//! field values, tool JSON-schema descriptions and NFR-004 error codes are not
//! user-facing and stay with the code that emits them.
//!
//! Unit tests live in the sibling `strings_tests.rs` sidecar.

/// Status-line reason phrase for a successful OAuth callback page.
pub const REASON_OK: &str = "OK";

/// Status-line reason phrase for a callback request using an unsupported method.
pub const REASON_METHOD_NOT_ALLOWED: &str = "Method Not Allowed";

/// Status-line reason phrase for a callback request to an unknown path.
pub const REASON_NOT_FOUND: &str = "Not Found";

/// Status-line reason phrase for a callback request the listener cannot serve.
pub const REASON_BAD_REQUEST: &str = "Bad Request";

/// Body copy shown once the OAuth callback has been captured successfully.
pub const MESSAGE_SUCCESS: &str = "You may now close this window and return to the application.";

/// Body copy shown when the callback listener receives a method it does not serve.
pub const MESSAGE_METHOD_REQUIRED: &str = "GET or POST required";

/// Body copy shown while the callback listener is still waiting for the provider.
pub const MESSAGE_WAITING_FOR_CALLBACK: &str = "waiting for OAuth callback";

/// Body copy shown when the provider redirect carried no authorization code.
pub const MESSAGE_MISSING_CODE: &str = "missing 'code' parameter on redirect";

/// Body copy shown when the provider redirect carried no state parameter.
pub const MESSAGE_MISSING_STATE: &str = "missing 'state' parameter on redirect";

/// Shown when model selection is asked to choose but no models are configured.
pub const ERR_NO_MODELS_CONFIGURED: &str = "No LLM models are configured.";

/// Shown when the selected model has no usable API key.
pub const ERR_API_KEY_INVALID: &str = "API key not set or invalid.";

/// Prefix for the JMAP error that reports a JSON parse failure together with the
/// parser message and a clipped excerpt of the offending body. The matching
/// `tracing` message is deliberately *not* catalogued here: log text is not
/// user-facing.
pub const ERR_JMAP_JSON_PARSE_PREFIX: &str = "Failed to parse response as JSON";

#[cfg(test)]
#[path = "strings_tests.rs"]
mod strings_tests;
