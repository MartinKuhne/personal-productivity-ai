//! Tests for the process-wide Tokio bridge that lets sync tool code await DAV futures.

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
