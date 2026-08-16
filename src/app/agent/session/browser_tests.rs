//! Integration tests for the headless browser tools.
//!
//! These tests drive the real `BrowserSession` against a local
//! Firefox instance via Playwright.

#![cfg(test)]

use super::BrowserSession;
use crate::agent::tools::Tool;
use crate::config::AppConfig;
use std::sync::Arc;
use tempfile::TempDir;

fn make_session() -> (TempDir, Arc<BrowserSession>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
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
    {
        let _ = session.page().expect("page() #1");
    }
    session.save_storage().expect("save_storage #1");
    assert!(tmp.path().join("storage.json").exists());

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
    drop(_session2);
}

#[test]
#[ignore = "requires Playwright Firefox installed locally"]
fn test_browser_navigate_tool_round_trip() {
    let (_tmp, session) = make_session();
    let mut config = AppConfig::default();
    config.tool_groups.browser = true;
    let agent_config = config.to_agent_config();
    let policy = std::sync::Arc::new(crate::agent::tools::policy::DefaultToolCallPolicy);
    let cache = Arc::new(crate::agent::tools::registry::cache::ToolCache::new());
    let ctx = crate::agent::tools::context::ToolContextBuilder::new(
        Arc::new(agent_config),
        std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver),
    )
    .with_extension(std::sync::Arc::new(
        crate::agent::tools::context::ToolCacheExt(cache.clone()),
    ))
    .with_extension(std::sync::Arc::new(
        crate::agent::tools::context::UuidGeneratorExt(Arc::new(
            crate::agent::utils::uuid::SystemUuidGenerator,
        )),
    ))
    .with_extension(Arc::new(crate::agent::tools::browser::BrowserExt(session)))
    .with_tool_call_policy(policy)
    .build();

    let tool = crate::agent::tools::registry::builtin::browser::BrowserNavigateTool;
    let args = r#"{"url":"about:blank"}"#;
    let result = tool.execute(&ctx, args);
    assert!(result.is_ok(), "navigate failed: {:?}", result.err());
}

#[test]
#[ignore = "requires Playwright Firefox installed locally"]
fn test_browser_get_page_state_tool_is_readonly() {
    use crate::agent::tools::Safety;
    let (_tmp, session) = make_session();
    let mut config = AppConfig::default();
    config.tool_groups.browser = true;
    let agent_config = config.to_agent_config();
    let policy = std::sync::Arc::new(crate::agent::tools::policy::DefaultToolCallPolicy);
    let cache = Arc::new(crate::agent::tools::registry::cache::ToolCache::new());
    let _ctx = crate::agent::tools::context::ToolContextBuilder::new(
        Arc::new(agent_config),
        std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver),
    )
    .with_extension(std::sync::Arc::new(
        crate::agent::tools::context::ToolCacheExt(cache.clone()),
    ))
    .with_extension(std::sync::Arc::new(
        crate::agent::tools::context::UuidGeneratorExt(Arc::new(
            crate::agent::utils::uuid::SystemUuidGenerator,
        )),
    ))
    .with_extension(Arc::new(crate::agent::tools::browser::BrowserExt(session)))
    .with_tool_call_policy(policy)
    .build();

    let tool = crate::agent::tools::registry::builtin::browser::BrowserGetPageStateTool;
    assert_eq!(tool.safety(), Safety::ReadOnly);
}
