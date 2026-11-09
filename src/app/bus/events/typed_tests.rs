//! Tests for [`BackgroundEvent`] and [`BackgroundEventSender`] (`bus::events::typed`).
//!
//! Smoke tests for the blanket `From` conversions on
//! [`BackgroundEvent`]. Producer ergonomics are covered by the
//! call-site tests in `app/background/*` and `agent/*`.

use super::*;
use crate::background::LogCategory;

#[test]
fn from_log_entry_wraps_process_variant() {
    let entry = BackgroundLogEntry::new(LogCategory::PdfConverter, "msg".into());
    let ev: BackgroundEvent = entry.into();
    match ev {
        BackgroundEvent::Process(ProcessEvent::LogEntry(e)) => {
            assert_eq!(e.message, "msg");
        }
        other => panic!("expected ProcessEvent::LogEntry, got {:?}", other),
    }
}

#[test]
fn from_mcp_auth_event_wraps_variant() {
    let ev: BackgroundEvent = McpAuthEvent::Completed {
        server_name: "srv".to_owned(),
        error: None,
    }
    .into();
    assert!(matches!(
        ev,
        BackgroundEvent::McpAuth(McpAuthEvent::Completed { error: None, .. })
    ));
}

#[test]
fn sender_calls_repaint_callback_after_delivery() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(tx);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls_for_callback = Arc::clone(&calls);
    sender.set_repaint_callback(Arc::new(move || {
        calls_for_callback.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));

    sender.send(FsEvent::Finished.into()).unwrap();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for test");
    rt.block_on(async {
        assert!(matches!(
            rx.recv().await.unwrap(),
            BackgroundEvent::Fs(FsEvent::Finished)
        ));
    });
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn sender_calls_repaint_callback_even_with_no_subscribers() {
    // With broadcast, send() succeeds even with no subscribers (returns Ok(0)).
    // The repaint callback is called after a successful send.
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    // Drop the receiver so there are no subscribers
    let sender = BackgroundEventSender::new(tx);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls_for_callback = Arc::clone(&calls);
    sender.set_repaint_callback(Arc::new(move || {
        calls_for_callback.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));

    assert!(sender.send(FsEvent::Finished.into()).is_ok());
    // Callback is called because send() succeeds (broadcast doesn't fail on no subscribers)
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
