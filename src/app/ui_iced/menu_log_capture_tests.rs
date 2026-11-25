//! Test-only capture layer for the menu observability tests.
//!
//! Follows the idiom `background::logs_tests` already uses: install a
//! `tracing_subscriber::registry()` carrying a capturing layer inside
//! `tracing::subscriber::with_default`, drive the code under test, then assert
//! on what landed. Unlike the production layer this one keeps **every** level,
//! so a test can pin the level a route chose as well as the [NFR-004] code it
//! carried.
//!
//! Declared from `menu_surface` as `#[cfg(test)] mod log_capture` so the three
//! menu observability sidecars (`menu_surface_tests`, `update_tests`,
//! `app_tests`) share one capture implementation instead of each
//! re-implementing a subscriber.
//!
//! [NFR-002]: the capture is an in-memory push under a mutex, so it never
//! blocks on I/O.

use std::sync::{Arc, Mutex};

/// One captured `tracing` event.
#[derive(Clone, Debug)]
pub struct Captured {
    /// The event's level.
    pub level: tracing::Level,
    /// The event's target.
    pub target: &'static str,
    /// The formatted message.
    pub message: String,
    /// Every non-message field, in recording order.
    pub fields: Vec<(String, String)>,
}

impl Captured {
    /// The value recorded for `name`, if the event carried it.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The event's `error_code` field ([NFR-004]).
    #[must_use]
    pub fn error_code(&self) -> Option<&str> {
        self.field("error_code")
    }
}

/// A capture sink handed to the layer and read back by the test.
#[derive(Clone, Default)]
pub struct Capture {
    events: Arc<Mutex<Vec<Captured>>>,
}

impl Capture {
    /// An empty capture.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every event captured so far.
    #[must_use]
    pub fn events(&self) -> Vec<Captured> {
        self.events.lock().unwrap().clone()
    }

    /// The first event carrying `code` as its `error_code`.
    #[must_use]
    pub fn with_code(&self, code: &str) -> Option<Captured> {
        self.events()
            .into_iter()
            .find(|event| event.error_code() == Some(code))
    }

    /// Whether any event carried `code` at exactly `level`.
    #[must_use]
    pub fn has_code_at(&self, code: &str, level: tracing::Level) -> bool {
        self.with_code(code).is_some_and(|e| e.level == level)
    }

    /// Every code captured, in order — the shape a filtered live log would show.
    #[must_use]
    pub fn codes(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .filter_map(|event| event.error_code().map(str::to_string))
            .collect()
    }

    /// The distinct event targets, in first-seen order.
    ///
    /// Lets a test prove a code came from the module under test rather than an
    /// unrelated emitter that happened to reuse the code.
    #[must_use]
    pub fn targets(&self) -> Vec<&'static str> {
        let mut seen: Vec<&'static str> = Vec::new();
        for event in self.events() {
            if !seen.contains(&event.target) {
                seen.push(event.target);
            }
        }
        seen
    }
}

/// The `tracing` layer that pushes events into a [`Capture`].
struct CaptureLayer(Arc<Mutex<Vec<Captured>>>);

impl<S> tracing_subscriber::Layer<S> for CaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = CaptureVisitor::default();
        event.record(&mut visitor);
        let metadata = event.metadata();
        self.0.lock().unwrap().push(Captured {
            level: *metadata.level(),
            target: metadata.target(),
            message: visitor.message,
            fields: visitor.fields,
        });
    }
}

#[derive(Default)]
struct CaptureVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl tracing::field::Visit for CaptureVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields
                .push((field.name().to_string(), format!("{value:?}")));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
}

/// Runs `body` with `capture` installed as the default subscriber.
///
/// Scoped to the closure and this thread, which is what makes the capture safe
/// under `cargo nextest`'s parallel test runner — each worker thread carries
/// its own subscriber.
pub fn with_capture<T>(capture: &Capture, body: impl FnOnce() -> T) -> T {
    use tracing_subscriber::layer::SubscriberExt;

    let subscriber = tracing_subscriber::registry().with(CaptureLayer(Arc::clone(&capture.events)));
    tracing::subscriber::with_default(subscriber, body)
}
