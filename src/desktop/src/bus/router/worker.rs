//! Generic background workers built on `std::sync::mpsc` channels.
//!
//! These are domain-free helpers: the path drainer spawns a Tokio runtime
//! per worker and runs a user-supplied async closure per item. The
//! [`ChannelWorker`] shim is kept for source-level compatibility with
//! older call sites that constructed workers via a builder API.

use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// Spawn a background thread that drains `rx` and runs `handle(path)`
/// for each item. `handle` is an async closure (or function) that
/// processes one path; it returns when the channel is closed or
/// `recv` returns an error.
///
/// The runtime is created on the worker thread. The worker terminates
/// cleanly when the channel sender is dropped.
pub fn spawn_path_worker<F, Fut>(rx: Receiver<PathBuf>, handle: F)
where
    F: Fn(PathBuf) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async {
                while let Ok(path) = rx.recv() {
                    handle(path).await;
                }
            });
        }
    });
}

/// Marker struct kept around so the existing `Worker::new(...)` /
/// `Worker::spawn(self)` call sites in `background_task.rs` continue
/// to compile unchanged while we migrate them onto
/// [`spawn_path_worker`]. New code should call `spawn_path_worker`
/// directly with a closure; new wrappers can be added under
/// `bus/router/` if a third worker appears.
pub struct ChannelWorker<F>(PhantomData<F>);

impl<F> ChannelWorker<F> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<F> Default for ChannelWorker<F> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
