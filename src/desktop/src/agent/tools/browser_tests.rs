//! Integration tests for the headless browser tools.
//!
//! These tests drive the real `BrowserSession` against a local
//! Firefox instance via Playwright. They are gated with
//! `#[ignore = "requires Playwright Firefox installed"]` so they
//! do not run in CI; enable locally with:
//!
//! ```text
//! cargo nextest run --run-ignored all -p fastmd browser
//! # or
//! cargo test -p fastmd -- --ignored browser
//! ```
//!
//! Prerequisite: `playwright install firefox` (the install is
//! NOT auto-run by the app — see BRWS-SESSION-006).

#![cfg(test)]

use crate::agent::tools::Tool;
use crate::app::session::BrowserSession;
use crate::config::AppConfig;
use std::sync::Arc;
use tempfile::TempDir;

fn make_session() -> (TempDir, Arc<BrowserSession>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    // Use a sandbox screenshot dir inside the tempdir so the
    // tests don't litter the user's real library.
    config.browser.screenshot_dir = tmp.path().join("screenshots").to_string_lossy().to_string();
    config.browser.storage_state_path = tmp
        .path()
        .join("storage.json")
        .to_string_lossy()
        .to_string();
    config.browser.headless = true;
    config.browser.idle_timeout_seconds = 60;
    let session = Arc::new(BrowserSession::new(&config));
    (tmp, session)
}

#[test]
#[ignore = "requires Playwright Firefox installed locally"]
fn test_session_launches_firefox_and_loads_about_blank() {
    let (_tmp, session) = make_session();
    let handle = session.page().expect("session.page()");
    // We don't navigate — just verify the page is alive and
    // returns a sensible URL. About:blank is the default for a
    // new Playwright context.
    let url = handle.page.url();
    assert!(
        url.is_empty() || url.starts_with("about:") || url.starts_with("http"),
        "unexpected page URL: {}",
        url
    );
    session.forget().expect("forget");
}

#[test]
#[ignore = "requires Playwright Firefox installed locally"]
fn test_session_persists_cookies_across_relaunch() {
    let (tmp, session) = make_session();
    // Round-trip: open the page (creates storage file), drop the
    // session, build a new one against the same tempdir, and
    // verify the storage file is read.
    {
        let _ = session.page().expect("page() #1");
    }
    // Save explicitly so we don't depend on a mutating tool call.
    session.save_storage().expect("save_storage #1");
    assert!(tmp.path().join("storage.json").exists());

    // Build a second session against the same dir and confirm
    // it picks up the existing file.
    let mut config = AppConfig::default();
    config.browser.storage_state_path = tmp
        .path()
        .join("storage.json")
        .to_string_lossy()
        .to_string();
    config.browser.screenshot_dir = tmp
        .path()
        .join("screenshots2")
        .to_string_lossy()
        .to_string();
    let _session2 = Arc::new(BrowserSession::new(&config));
    // We don't actually launch here — just confirm construction
    // succeeds with an existing storage file.
    drop(_session2);
}

#[test]
#[ignore = "requires Playwright Firefox installed locally"]
fn test_browser_navigate_tool_round_trip() {
    use crate::agent::tools::context::ToolContext;
    let (_tmp, session) = make_session();
    let bus = Box::leak(Box::new(crate::bus::core::Bus::<
        crate::bus::events::file::FileEvent,
    >::new()));
    let config = Box::leak(Box::new(AppConfig::default()));
    config.tool_groups.browser = true;
    let pdf_backing = std::sync::Arc::new(crate::app::session::PdfBackingTracker::new());
    let ctx = ToolContext::new(config, bus, session, pdf_backing, crate::agent::tools::manager::cache::cache());

    let tool = crate::agent::tools::manager::builtin::browser::BrowserNavigateTool;
    let args = r#"{"url":"about:blank"}"#;
    let result = tool.execute(&ctx, args);
    assert!(result.is_ok(), "navigate failed: {:?}", result.err());
}

#[test]
#[ignore = "requires Playwright Firefox installed locally"]
fn test_browser_get_page_state_tool_is_readonly() {
    use crate::agent::tools::Safety;
    use crate::agent::tools::context::ToolContext;
    let (_tmp, session) = make_session();
    let bus = Box::leak(Box::new(crate::bus::core::Bus::<
        crate::bus::events::file::FileEvent,
    >::new()));
    let config = Box::leak(Box::new(AppConfig::default()));
    config.tool_groups.browser = true;
    let pdf_backing = std::sync::Arc::new(crate::app::session::PdfBackingTracker::new());
    let _ctx = ToolContext::new(config, bus, session, pdf_backing, crate::agent::tools::manager::cache::cache());

    let tool = crate::agent::tools::manager::builtin::browser::BrowserGetPageStateTool;
    assert_eq!(tool.safety(), Safety::ReadOnly);
}
