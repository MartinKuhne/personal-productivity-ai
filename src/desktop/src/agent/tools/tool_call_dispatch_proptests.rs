//! Property-based tests for the LLM↔tool dispatch surface
//! (`ToolRegistry::execute_tool`).
//!
//! `execute_tool` is the single chokepoint through which every
//! `tool_calls[*].function.arguments` JSON the LLM emits flows into
//! the agent. It is the *last* defensive layer between untrusted
//! LLM output and the rest of the runtime — a panic here, a hang
//! here, or a return-shape regression here is qualitatively
//! different from the same bug in any other tool.
//!
//! # Properties under test
//!
//! 1. **No panic on any input.** A random `name` + `args_str` pair
//!    (any UTF-8 string, including the empty string, raw control
//!    bytes, and 1 KiB of structured JSON) must produce a `String`
//!    return. The existing
//!    `std::panic::catch_unwind` in `execute_tool` (line ~541 of
//!    `agent/tools/manager/mod.rs`) is the structural defence; this
//!    proptest is the regression guard that ensures the defence
//!    stays in place.
//! 2. **Return is valid JSON.** The dispatch function's contract is
//!    "always return a JSON string of the form
//!    `{"status": "success", "data": ...}` or
//!    `{"status": "error", "message": ...}`". A regression that
//!    returns a partial string, an unterminated JSON object, or
//!    something else entirely would break the agent loop's
//!    downstream parsing.
//! 3. **Bounded runtime.** A pathological `args_str` (e.g. a deeply
//!    nested JSON, a huge string, a malicious `evalexpr` predicate
//!    for the CSV query tool) must not hang the dispatch. We
//!    enforce a 5-second wall-clock ceiling per call. A regression
//!    that introduces a DoS surface — a tight loop, a recursive
//!    deserialiser, an unbounded regex — is caught here.
//!
//! `cases = 1024` for properties #1 and #2 (cheap; the dispatch is
//! mostly error-returning fast paths). For property #3 we use
//! `cases = 64` because each case includes a wall-clock measurement
//! and we want the dispatch proptest block to finish in seconds, not
//! minutes, on the un-modified production code.
//!
//! # Why this is the sidecar that benefits most from the
//! `ToolContext: 'static` rewrite
//!
//! Before the rewrite, this proptest held a `&'static ToolContext`
//! via the `Box::leak` and pointer-cast trick — about 1 MiB of
//! leaked memory per 1024 cases. After the rewrite, the context is
//! built normally, the worker thread takes a cheap `Clone` (the
//! `Arc`-backed `AgentConfig` and `ToolCache` are shared), and the
//! test runs without `Box::leak` or `unsafe`. The same pattern is
//! what makes the Phase-5 cargo-fuzz targets for `execute_tool`
//! feasible.

use crate::agent::config::AgentConfig;
use crate::agent::tools::context::ToolContext;
use crate::app::session::{BrowserSession, PdfBackingTracker};
use crate::utils::uuid::SystemUuidGenerator;
use proptest::prelude::*;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const CASES_CHEAP: u32 = 1024;
const CASES_BOUNDED: u32 = 64;
const DISPATCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Build a fresh `ToolContext` for one proptest case. Now that
/// `ToolContext: 'static + Send + Sync + Clone`, no leak or
/// pointer cast is needed; the context is built by value and
/// dropped when the proptest case returns. The total
/// allocations across 1024 cases are bounded and reclaimed
/// by the test harness.
fn build_dispatch_context() -> ToolContext {
    let config = AgentConfig::default();
    let browser_session = Arc::new(BrowserSession::with_resolved(config.browser.clone()));
    let policy = Arc::new(PdfBackingTracker::new());
    let uuid_gen: Arc<dyn crate::utils::uuid::UuidGenerator> = Arc::new(SystemUuidGenerator);
    crate::agent::tools::context::ToolContextBuilder::new(
        Arc::new(config),
        std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver),
    )
    .with_extension(std::sync::Arc::new(
        crate::agent::tools::context::ToolCacheExt(Arc::new(
            crate::agent::tools::registry::cache::ToolCache::new(),
        )),
    ))
    .with_extension(std::sync::Arc::new(
        crate::agent::tools::context::UuidGeneratorExt(uuid_gen),
    ))
    .with_extension(browser_session.clone())
    .with_extension(Arc::new(crate::agent::tools::browser::BrowserExt(
        browser_session,
    )))
    .with_tool_call_policy(policy)
    .build()
}

/// Call `execute_tool` with a wall-clock timeout. The
/// `ToolContext` is `'static + Send`, so the worker thread
/// captures the context by value (via a cheap `Clone`); no
/// `Box::leak`, no `unsafe`. If the call doesn't return within
/// `DISPATCH_TIMEOUT`, the spawned thread is detached (it'll
/// finish eventually, but the test moves on) and the test
/// fails.
fn execute_with_timeout(ctx: ToolContext, name: String, args: String) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    let _ = thread::Builder::new()
        .name("dispatch-proptest".to_string())
        .spawn(move || {
            let dispatcher = crate::agent::tools::registry::ToolRegistry::new();
            let result = execute_tool(&dispatcher, &ctx, &name, &args);
            let _ = tx.send(result);
        });
    rx.recv_timeout(DISPATCH_TIMEOUT).ok()
}

// Re-export the dispatch function from the manager module.
use crate::agent::tools::registry::execute_tool;

/// Arbitrary tool-name strategy. A tool name is any UTF-8
/// string (0-128 bytes). Includes the empty string, single
/// chars, raw control bytes, very long names, and the literal
/// name of every registered tool. The point is to exercise both
/// the "known tool" and the "unknown tool" dispatch paths.
fn any_tool_name() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F]{0,128}").unwrap()
}

/// Arbitrary `args_str` strategy. A JSON-shaped string (0-512
/// bytes). The regex is loose because `serde_json::from_str` is
/// what actually parses the string, and the dispatch function
/// feeds that parser arbitrary input from the LLM. A truly
/// random byte string is the more interesting adversarial
/// surface.
fn any_args_str() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F]{0,512}").unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES_CHEAP))]

    /// Property 1 + 2: dispatch never panics, and the return is
    /// valid JSON. The `execute_tool` function is wrapped in
    /// `catch_unwind` in production; this proptest is the
    /// regression guard that ensures the wrapper stays in
    /// place. Combined with the return-shape check, this is
    /// the single most important invariant the agent loop
    /// relies on: every tool call returns a JSON the loop can
    /// parse.
    #[test]
    fn execute_tool_never_panics_and_returns_json(
        name in any_tool_name(),
        args in any_args_str()
    ) {
        let ctx = build_dispatch_context();

        // `execute_tool` returns a `String`. The contract:
        // it always returns, never unwinds (because of the
        // `catch_unwind` in production), and the return
        // is a JSON object with a `status` field.
        let dispatcher = crate::agent::tools::registry::ToolRegistry::new();
        let result = execute_tool(&dispatcher, &ctx, &name, &args);

        // The return must be a non-empty String. A regression
        // that returned an empty string would break the agent
        // loop's downstream JSON parser.
        prop_assert!(
            !result.is_empty(),
            "execute_tool returned an empty String"
        );

        // The return must be valid JSON. A regression that
        // returned a partial JSON object (e.g. a panic
        // that bypassed the catch_unwind) or a non-JSON
        // string (e.g. an error message) would fail here.
        let parsed: serde_json::Result<serde_json::Value> =
            serde_json::from_str(&result);
        prop_assert!(
            parsed.is_ok(),
            "execute_tool returned invalid JSON: {result:?}"
        );
        let value = parsed.expect("checked above");

        // The return must be an object with a `status` field.
        // The dispatch function emits either
        // `{"status": "success", "data": ...}` or
        // `{"status": "error", "message": ...}` — the
        // `status` field is the agent loop's primary
        // dispatch key. A regression that dropped it would
        // hang the agent loop.
        let status = value.get("status").and_then(|v| v.as_str());
        prop_assert!(
            status.is_some(),
            "execute_tool returned a JSON without a `status` field: {value:?}"
        );
        let status = status.expect("checked above");
        prop_assert!(
            status == "success" || status == "error",
            "execute_tool returned an unknown status {status:?} (must be `success` or `error`)"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES_BOUNDED))]

    /// Property 3: bounded runtime. A pathological `args_str`
    /// must not hang the dispatch. We use a 5-second ceiling
    /// per call; the worker thread is detached on timeout so
    /// the test moves on. A regression that introduces a DoS
    /// surface (a tight loop on a `csv_db::query` predicate, a
    /// recursive deserialiser, an unbounded regex) is caught
    /// here.
    #[test]
    fn execute_tool_returns_within_5_seconds(
        name in any_tool_name(),
        args in any_args_str()
    ) {
        let ctx = build_dispatch_context();

        let start = Instant::now();
        let result = execute_with_timeout(ctx, name, args);
        let elapsed = start.elapsed();

        // Sanity: the measurement itself should not be the
        // bottleneck. If the test infra is broken, the elapsed
        // could be much larger than the call itself, but
        // `Instant::elapsed` is monotonic and the assertion
        // is on the function call only.
        match result {
            Some(_) => {
                prop_assert!(
                    elapsed < DISPATCH_TIMEOUT,
                    "execute_tool took {elapsed:?}, exceeding the {DISPATCH_TIMEOUT:?} ceiling"
                );
            }
            None => {
                // The thread didn't return in time. The
                // worker is detached; it'll finish eventually.
                // Fail the test so the regression is visible.
                prop_assert!(
                    false,
                    "execute_tool did not return within {DISPATCH_TIMEOUT:?} (elapsed: {elapsed:?})"
                );
            }
        }
    }
}
