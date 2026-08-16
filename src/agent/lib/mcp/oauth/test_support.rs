//! Test-only HTTP server double for the MCP OAuth integration tests.
//!
//! Compiled only under `cfg(test)`. Backed by [`wiremock`] but
//! exposed through a small, sync-friendly API so the existing
//! `MockHttpServer::start(closure)` shape stays usable from plain
//! `#[test]` functions. The wiremock library does the actual HTTP
//! serving, request recording, and content-length handling under
//! the hood; the wrapper here just translates a closure-style
//! responder into a wiremock catch-all stub and keeps the request
//! log accessible without `.await`.
//!
//! Every request the mock receives is recorded in
//! [`MockHttpServer::recorded`]; tests can assert on the exact
//! bytes the client sent without having to reach for a runtime.
#![cfg(test)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use wiremock::http::HeaderMap;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// One captured HTTP request, recorded by the mock for the test's
/// own assertions.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl RecordedRequest {
    /// Look up a request header by name (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A canned HTTP response the mock returns.
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: &'static str,
    pub content_type: &'static str,
    pub extra_headers: Vec<(String, String)>,
    pub body: String,
}

impl MockResponse {
    /// Build a JSON response.
    pub fn json(status: &'static str, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "application/json",
            extra_headers: Vec::new(),
            body: body.into(),
        }
    }

    /// Add an extra response header (e.g. `WWW-Authenticate`).
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.extra_headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

impl MockResponse {
    /// Convert a [`MockResponse`] into a wiremock [`ResponseTemplate`].
    /// Parses the standard `HTTP/1.1 <code> <reason>` status line.
    fn into_response_template(self) -> ResponseTemplate {
        let code = parse_status_code(self.status).unwrap_or(200);
        let mut tpl = ResponseTemplate::new(code).set_body_string(self.body);
        if !self.content_type.is_empty() {
            tpl = tpl.insert_header("Content-Type", self.content_type);
        }
        for (name, value) in self.extra_headers {
            tpl = tpl.append_header(name, value);
        }
        tpl
    }
}

/// Parse the integer status code out of a standard HTTP reason
/// line, e.g. `"HTTP/1.1 401 Unauthorized"` → `401`. Returns `None`
/// if the line is malformed; callers fall back to 200 in that case.
fn parse_status_code(status_line: &str) -> Option<u16> {
    // Either the full `HTTP/1.1 401 Unauthorized` form or just `401`.
    let digits = status_line
        .split_whitespace()
        .find_map(|tok| tok.parse::<u16>().ok())?;
    Some(digits)
}

/// In-memory mock HTTP server. The wrapper owns a small tokio
/// runtime and a wiremock [`MockServer`] that does the actual
/// serving. `recorded` exposes the captured requests without
/// `.await`.
pub struct MockHttpServer {
    /// Base URL of the mock, e.g. `http://127.0.0.1:41234`.
    pub origin: String,
    /// Every request the mock received, in order.
    pub recorded: Arc<Mutex<Vec<RecordedRequest>>>,
    _runtime: tokio::runtime::Runtime,
    _server: MockServer,
}

impl MockHttpServer {
    /// Start a mock server with the given responder. The responder
    /// is called for every request and returns the response to send.
    pub fn start<F>(respond: F) -> Self
    where
        F: Fn(&RecordedRequest, &str) -> MockResponse + Send + Sync + 'static,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for mock server");
        let _enter = runtime.enter();

        let recorded: Arc<Mutex<Vec<RecordedRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let server = runtime.block_on(MockServer::start());
        let origin = server.uri();

        // Catch-all stub: every request is matched, recorded, and
        // answered by the caller's closure.
        let recorded_for_stub = Arc::clone(&recorded);
        let origin_for_stub = origin.clone();
        let respond_for_stub = Arc::new(respond);
        let stub =
            Mock::given(wiremock::matchers::any()).respond_with(move |req: &wiremock::Request| {
                let recorded = RecordedRequest::from_wiremock(req);
                recorded_for_stub
                    .lock()
                    .expect("lock recorded")
                    .push(recorded.clone());
                let response = (respond_for_stub)(&recorded, &origin_for_stub);
                response.into_response_template()
            });
        runtime.block_on(server.register(stub));

        Self {
            origin,
            recorded,
            _runtime: runtime,
            _server: server,
        }
    }
}

impl RecordedRequest {
    /// Build a [`RecordedRequest`] from a wiremock request. Header
    /// keys are lowercased to match the previous hand-rolled helper
    /// (and HTTP's case-insensitive reality).
    fn from_wiremock(req: &wiremock::Request) -> Self {
        let mut headers = HashMap::new();
        for (name, value) in HeaderMap::iter(&req.headers) {
            let key = name.as_str().to_ascii_lowercase();
            let value = value
                .to_str()
                .unwrap_or("<non-utf8 header value>")
                .to_string();
            headers.insert(key, value);
        }
        Self {
            method: req.method.as_str().to_string(),
            path: req.url.path().to_string(),
            headers,
            body: String::from_utf8_lossy(&req.body).into_owned(),
        }
    }
}
