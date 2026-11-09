//! Tests for prompt-context logging and `web_delegate` trace stripping (`agent::agent_impl`).

use super::*;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// `MakeWriter` that appends formatted tracing output into a
/// shared byte buffer so tests can assert on it.
#[derive(Clone)]
struct BufferWriter(Arc<Mutex<Vec<u8>>>);

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for BufferWriter {
    type Writer = BufferWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

/// Run `body` under a `fmt` subscriber whose output is captured
/// into a `String`. Returns everything logged at `INFO` or above
/// during the call.
fn capture_log(body: impl FnOnce()) -> String {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(BufferWriter(buf.clone()))
        .with_ansi(false)
        .with_max_level(tracing::level_filters::LevelFilter::INFO)
        .finish();
    tracing::subscriber::with_default(subscriber, body);
    String::from_utf8(buf.lock().unwrap().clone()).unwrap_or_default()
}

/// AGENT-026: starting a new prompt must log the file and
/// directory context handed to the LLM, so operators can see what
/// context each prompt carried.
#[test]
fn test_log_prompt_context_emits_file_and_directory_context() {
    let mut files = HashSet::new();
    files.insert(PathBuf::from("notes.md"));
    let out = capture_log(|| {
        log_prompt_context(
            &Some(PathBuf::from("doc.md")),
            &Some(PathBuf::from("lib")),
            &files,
        );
    });
    assert!(out.contains("agent.prompt.started"), "log: {out}");
    assert!(out.contains("doc.md"), "active file must be logged: {out}");
    assert!(
        out.contains("lib"),
        "active directory must be logged: {out}"
    );
    assert!(
        out.contains("notes.md"),
        "selected files must be logged: {out}"
    );
}

/// AGENT-026: with no file or directory context, the log must
/// still fire and record that the context was empty.
#[test]
fn test_log_prompt_context_emits_empty_context() {
    let out = capture_log(|| {
        log_prompt_context(&None, &None, &HashSet::new());
    });
    assert!(out.contains("agent.prompt.started"), "log: {out}");
    assert!(
        out.contains("active_file=None") && out.contains("active_dir=None"),
        "empty context must be logged as None: {out}"
    );
}

/// `strip_web_delegate_trace` must remove the `tool_call_trace` field
/// from a `web_delegate` tool result so it is never sent to the LLM,
/// while preserving `status` and `result`.
#[test]
fn test_strip_web_delegate_trace_removes_trace_field() {
    let result = serde_json::json!({
        "status": "success",
        "data": {
            "result": "Fetched content",
            "tool_call_trace": "**Executing tool `web_fetch`**\n"
        }
    })
    .to_string();
    let stripped = strip_web_delegate_trace("web_delegate", &result);
    let parsed: serde_json::Value =
        serde_json::from_str(&stripped).expect("stripped result is valid JSON");
    assert!(
        parsed
            .get("data")
            .and_then(|d| d.get("tool_call_trace"))
            .is_none(),
        "tool_call_trace must be removed from LLM-bound payload"
    );
    assert_eq!(
        parsed
            .get("data")
            .and_then(|d| d.get("result"))
            .and_then(|r| r.as_str()),
        Some("Fetched content")
    );
    assert_eq!(
        parsed.get("status").and_then(|s| s.as_str()),
        Some("success")
    );
}

/// Non-`web_delegate` tools must pass through unchanged — the strip
/// is scoped to the one tool that carries a `tool_call_trace` field.
#[test]
fn test_strip_web_delegate_trace_leaves_other_tools_unchanged() {
    let result = r#"{"status":"success","data":{"bytes":42}}"#.to_string();
    let stripped = strip_web_delegate_trace("read_file", &result);
    assert_eq!(stripped, result);
}

/// Malformed JSON must round-trip unchanged rather than panic — the
/// LLM still receives the original tool result on a parse failure.
#[test]
fn test_strip_web_delegate_trace_passthrough_on_parse_error() {
    let result = "not json at all".to_string();
    let stripped = strip_web_delegate_trace("web_delegate", &result);
    assert_eq!(stripped, result);
}

/// `web_delegate` result with no `tool_call_trace` field must round-trip
/// unchanged (e.g. when the delegate produced no sub-agent tool calls).
#[test]
fn test_strip_web_delegate_trace_no_trace_field_unchanged() {
    let result = serde_json::json!({
        "status": "success",
        "data": {"result": "answer"}
    })
    .to_string();
    let stripped = strip_web_delegate_trace("web_delegate", &result);
    let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
    assert_eq!(
        parsed
            .get("data")
            .and_then(|d| d.get("result"))
            .and_then(|r| r.as_str()),
        Some("answer")
    );
}
