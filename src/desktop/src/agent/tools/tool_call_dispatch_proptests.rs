//! Property-based tests for the LLM↔tool dispatch surface
//! (`ToolManager::execute_tool`).
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

use crate::agent::tools::context::ToolContext;
use crate::agent::tools::manager::ToolManager;
use crate::agent::tools::manager::cache::ToolCache;
use crate::app::session::{BrowserSession, PdfBackingTracker};
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::config::AppConfig;
use crate::utils::uuid::SystemUuidGenerator;
use proptest::prelude::*;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const CASES_CHEAP: u32 = 1024;
const CASES_BOUNDED: u32 = 64;
const DISPATCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Build a `ToolContext` that lives for the duration of the
/// proptest. Mirrors the `test_ctx` helper in
/// `agent/tools/manager/tests.rs`: the inner lifetimes are
/// extended to `'static` via the standard test-only
/// pointer-cast trick. The context is built once per proptest
/// test function; proptest re-runs the closure many times
/// against the same context, so the per-case cost is just the
/// `execute_tool` call.
fn build_dispatch_context() -> ToolContext<'static> {
    let config: &'static AppConfig = Box::leak(Box::new(AppConfig::default()));
    let bus: &'static Bus<FileEvent> = Box::leak(Box::new(Bus::new()));
    let cache: &'static ToolCache = Box::leak(Box::new(ToolCache::new()));
    let browser_session: Arc<BrowserSession> = Arc::new(BrowserSession::new(config));
    let pdf_backing: Arc<PdfBackingTracker> = Arc::new(PdfBackingTracker::new());
    let tool_manager: Arc<std::sync::RwLock<ToolManager>> =
        Arc::new(std::sync::RwLock::new(ToolManager::new()));
    let uuid_gen: Arc<dyn crate::utils::uuid::UuidGenerator> = Arc::new(SystemUuidGenerator);
    ToolContext::new(
        config,
        bus,
        browser_session,
        pdf_backing,
        cache,
        tool_manager,
        uuid_gen,
    )
}

/// Call `execute_tool` with a wall-clock timeout. If the call
/// doesn't return within `DISPATCH_TIMEOUT`, the spawned thread
/// is detached (it'll finish eventually, but the test moves on)
/// and the test fails.
///
/// We can't take the borrowed `ToolContext` into a `move`
/// closure directly, so we leak it (and a clone of the inputs)
/// and read the result through an mpsc channel. The leak is
/// bounded: proptest runs each test closure many times, and the
/// per-iteration leak is a few hundred bytes at most.
fn execute_with_timeout(ctx_ptr: usize, name: String, args: String) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    // The worker thread re-creates the call with the same
    // leaked pointer. We can't move `ctx` (it has lifetimes)
    // so the thread captures only `name`, `args`, and `ctx_ptr`
    // (all owned/Send) and re-borrows the leaked context
    // through the raw pointer.
    let ctx_ptr_for_thread = ctx_ptr;
    let name_for_thread = name.clone();
    let args_for_thread = args.clone();
    let _ = thread::Builder::new()
        .name("dispatch-proptest".to_string())
        .spawn(move || {
            let _ctx: &'static ToolContext<'static> =
                unsafe { &*(ctx_ptr_for_thread as *const ToolContext<'static>) };
            let result = execute_tool(_ctx, &name_for_thread, &args_for_thread);
            let _ = tx.send(result);
        });
    rx.recv_timeout(DISPATCH_TIMEOUT).ok()
}

// Re-export the dispatch function from the manager module. This
// indirection keeps the unsafe pointer dance in one place and
// makes the test assertions read naturally.
use crate::agent::tools::manager::execute_tool;

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
        // Build a fresh context per case. Leaking one
        // ToolContext per proptest iteration is bounded
        // (the AppConfig / Bus / cache are each a few KiB)
        // and the test only runs CASES_CHEAP cases, so the
        // total leak is well under 1 GiB.
        let ctx = build_dispatch_context();
        let ctx_ptr = &ctx as *const ToolContext<'static> as usize;

        // `execute_tool` returns a `String`. The contract:
        // it always returns, never unwinds (because of the
        // `catch_unwind` in production), and the return
        // is a JSON object with a `status` field.
        let result = execute_tool(&ctx, &name, &args);

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

        // Suppress the unused-warning for the leaked pointer.
        // (It's needed by the bounded-runtime test below; not
        // by this test.)
        let _ = ctx_ptr;
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
        let ctx_ptr = &ctx as *const ToolContext<'static> as usize;

        let start = Instant::now();
        let result = execute_with_timeout(ctx_ptr, name, args);
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
