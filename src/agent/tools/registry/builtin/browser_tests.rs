//! Tests for the browser tool family: DTO round-trips, config gating, safety classification and schemas.

use super::*;

#[test]
fn test_dto_round_trip_browser_navigate() {
    let json = r#"{"url":"https://example.com"}"#;
    let parsed: dtos::BrowserNavigateInput = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.url, "https://example.com");
}

#[test]
fn test_dto_round_trip_browser_screenshot() {
    let json = r#"{"filename":"login.png","full_page":true}"#;
    let parsed: dtos::BrowserScreenshotInput = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.filename, "login.png");
    assert!(parsed.full_page);
}

#[test]
fn test_is_enabled_matches_config_flag() {
    let mut config = crate::config::AgentConfig::default();
    assert!(!BrowserNavigateTool.is_enabled(&config, ""));
    config.tool_groups.browser = true;
    assert!(BrowserNavigateTool.is_enabled(&config, ""));
}

#[test]
fn test_only_get_page_state_is_readonly() {
    // BRWS-002 explicitly calls this out.
    let readonly = BrowserGetPageStateTool.safety();
    assert_eq!(readonly, crate::tools::Safety::ReadOnly);
    for safety in [
        BrowserNavigateTool.safety(),
        BrowserClickTool.safety(),
        BrowserFillInputTool.safety(),
        BrowserSelectDropdownTool.safety(),
        BrowserPressKeyTool.safety(),
        BrowserEvaluateJsTool.safety(),
        BrowserScreenshotTool.safety(),
    ] {
        assert_eq!(safety, crate::tools::Safety::Mutating);
    }
}

#[test]
fn test_schema_serializes() {
    // Just touch the schema so a regression in
    // schemars types is caught at build time.
    let _ = BrowserNavigateTool.parameters_schema();
    let _ = BrowserScreenshotTool.parameters_schema();
}
