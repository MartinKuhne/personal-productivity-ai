//! Tests for the file-change observer trait and its tool-context extension wrapper.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingObserver {
    calls: Arc<AtomicUsize>,
}

impl OnFileChanged for CountingObserver {
    fn on_file_changed(&self, _path: &Path) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn default_observer_is_noop() {
    let obs = DefaultFileObserver;
    // Must not panic and must not observe anything.
    obs.on_file_changed(Path::new("/tmp/x.md"));
}

#[test]
fn custom_observer_is_called_through_ext() {
    let calls = Arc::new(AtomicUsize::new(0));
    let ext = OnFileChangedExt(Arc::new(CountingObserver {
        calls: calls.clone(),
    }));
    ext.0.on_file_changed(Path::new("/a.md"));
    ext.0.on_file_changed(Path::new("/b.md"));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
