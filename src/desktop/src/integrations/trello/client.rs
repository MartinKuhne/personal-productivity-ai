//! Trello REST client — thin wrapper over `https://api.trello.com/1`.
//!
//! The LLM-tool-loop adapters that call into this client live in
//! [`crate::agent::tools::manager::builtin::trello`].

use crate::config::TrelloClient;

/// Build the authenticated request URL for a Trello REST call.
///
/// `endpoint` is the path *after* `/1` (e.g. `/members/me/boards`).
/// The API key and OAuth token are appended as query parameters per
/// Trello's authentication model.
///
/// Extracted from [`trello_request`] so the URL shape is unit-testable
/// without touching the network.
pub fn build_trello_url(client_config: &TrelloClient, endpoint: &str) -> String {
    format!(
        "https://api.trello.com/1{}?key={}&token={}",
        endpoint, client_config.api_key, client_config.token
    )
}

/// Send an authenticated request to the Trello REST API and return
/// the parsed JSON body.
pub fn trello_request(
    client_config: &TrelloClient,
    method: reqwest::Method,
    endpoint: &str,
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let url = build_trello_url(client_config, endpoint);
    let safe_url = format!("https://api.trello.com/1{}", endpoint);

    tracing::debug!(name = "trello.request", method = %method, url = %safe_url, "Sending request to Trello API");

    let client = reqwest::blocking::Client::new();
    let mut req = client.request(method.clone(), &url);
    if let Some(b) = body {
        req = req
            .header("Content-Type", "application/json")
            .body(b.to_string());
    }

    let res = req.send().map_err(|e| {
        tracing::error!(name = "trello.request.error", error = %e, url = %safe_url, "Trello request failed");
        e.to_string()
    })?;

    let status = res.status();
    tracing::debug!(name = "trello.response", status = %status, url = %safe_url, "Received response from Trello API");

    if status.is_success() {
        let text = res.text().map_err(|e| {
            tracing::error!(name = "trello.response.read_error", error = %e, "Failed to read Trello response text");
            e.to_string()
        })?;
        serde_json::from_str(&text).map_err(|e| {
            tracing::error!(name = "trello.response.parse_error", error = %e, text = %text, "Failed to parse Trello JSON");
            e.to_string()
        })
    } else {
        let error_text = res.text().unwrap_or_default();
        tracing::error!(name = "trello.response.status_error", status = %status, url = %safe_url, response = %error_text, "Trello API returned error status");
        Err(format!("Trello API error: {} - {}", status, error_text))
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
