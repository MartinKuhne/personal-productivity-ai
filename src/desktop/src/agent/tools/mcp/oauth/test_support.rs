//! Test-only HTTP server double for the MCP OAuth integration tests.
//!
//! Compiled only under `cfg(test)`. Serves Protected Resource
//! Metadata, Authorization Server Metadata, and a token endpoint
//! (plus any caller-supplied routes) and records every request so
//! tests can assert on the exact bytes the client sent. The server
//! binds an ephemeral localhost port so tests are deterministic and
//! need no network access.
#![cfg(test)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// One captured HTTP request.
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

/// In-memory mock HTTP server.
///
/// Handles one request per accepted connection on a background
/// thread. `respond` receives each recorded request plus the mock's
/// `origin` (e.g. `http://127.0.0.1:41234`) and returns the response
/// to send. Every request is captured in [`MockHttpServer::recorded`]
/// so tests can assert on the client's behaviour.
pub struct MockHttpServer {
    /// Base URL of the mock, e.g. `http://127.0.0.1:41234`.
    pub origin: String,
    /// Every request the mock received, in order.
    pub recorded: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockHttpServer {
    /// Start a mock server with the given responder.
    pub fn start<F>(respond: F) -> Self
    where
        F: Fn(&RecordedRequest, &str) -> MockResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let origin = format!(
            "http://127.0.0.1:{}",
            listener
                .local_addr()
                .expect("mock server local addr")
                .port()
        );
        let recorded: Arc<Mutex<Vec<RecordedRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let respond = Arc::new(respond);
        let recorded_thread = Arc::clone(&recorded);
        let respond_thread = Arc::clone(&respond);
        let origin_thread = origin.clone();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    continue;
                };
                let recorded = Arc::clone(&recorded_thread);
                let respond = Arc::clone(&respond_thread);
                let origin = origin_thread.clone();
                std::thread::spawn(move || {
                    if let Some(req) = read_request(&mut stream) {
                        recorded.lock().expect("lock recorded").push(req.clone());
                        let response = respond(&req, &origin);
                        write_response(&mut stream, &response);
                    }
                });
            }
        });
        Self {
            origin,
            recorded,
            handle: Some(handle),
        }
    }
}

impl Drop for MockHttpServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<RecordedRequest> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    if request_line.trim().is_empty() {
        return None;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
    let mut headers = HashMap::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_owned();
            if key == "content-length" {
                content_length = value.parse().unwrap_or(0);
            }
            headers.insert(key, value);
        }
    }
    let mut body = String::new();
    if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf).ok()?;
        body = String::from_utf8_lossy(&buf).into_owned();
    }
    Some(RecordedRequest {
        method,
        path,
        headers,
        body,
    })
}

fn write_response(stream: &mut TcpStream, response: &MockResponse) {
    let mut head = format!(
        "{}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );
    for (name, value) in &response.extra_headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(response.body.as_bytes());
    let _ = stream.flush();
}
