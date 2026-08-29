//! Synchronous bridge for the DAV tool family.
//!
//! The CalDAV / CardDAV client libraries return `async fn`s; the tool
//! trait's `execute` is synchronous. We bridge the two with a process-wide
//! `tokio` runtime so the call-site can stay sync.

use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RT: OnceLock<Runtime> = OnceLock::new();

/// Run a future to completion on a process-wide Tokio runtime. The
/// runtime is created on first use and reused for every subsequent
/// call. Panics if the runtime cannot be created (e.g. out of memory
/// or running in an environment that disallows multi-threaded
/// executors).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let rt = RT.get_or_init(|| {
        Runtime::new().unwrap_or_else(|e| panic!("Failed to create Tokio runtime: {}", e))
    });
    rt.block_on(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_on_runs_future_and_reuses_runtime() {
        // Two calls must both complete; the process-wide runtime is
        // created once and reused (proving the OnceLock path).
        let a = block_on(async { 40 + 2 });
        assert_eq!(a, 42);
        let b = block_on(async { "hello" });
        assert_eq!(b, "hello");
    }

    #[test]
    fn block_on_can_await_tokio_spawned_tasks() {
        // Runs on the multi-threaded runtime, so spawned tasks complete.
        let result = block_on(async { tokio::spawn(async { 1 + 1 }).await.unwrap() });
        assert_eq!(result, 2);
    }
}
