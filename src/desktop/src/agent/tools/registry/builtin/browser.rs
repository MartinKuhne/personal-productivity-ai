//! LLM tool wrappers for the headless browser (`BRWS-001..008`).
//!
//! Each tool's `Tool::execute` runs the underlying Playwright
//! future on the process-wide Tokio runtime via
//! [`crate::agent::tools::blocking::block_on`] — the same
//! sync-to-async bridge the CalDAV / CardDAV tools use. Mutating
//! tools trigger a `save_storage()` on the
//! [`crate::app::session::BrowserSession`] so cookies / local
//! storage survive an app restart. See
//! `doc/planning/browser_tools.md` for the design record and
//! `src/desktop/Tools.md` for the user-facing catalog.

use super::strings;
use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::ToolConfigSpec;
use crate::agent::tools::dtos;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

/// Convert any error string into a Tool error string. Most
/// Playwright errors already have decent `Display` impls; we
/// just wrap them with a stable prefix.
fn err(s: impl std::fmt::Display) -> String {
    format!("browser tool failed: {}", s)
}

fn browser_spec() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::Browser);
    ToolConfigSpec::group_only(group)
}

// ---------------------------------------------------------------------------
// browser_navigate (BRWS-001, Mutating)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_navigate",
    desc = strings::BROWSER_NAVIGATE_DESCRIPTION,
    input = dtos::BrowserNavigateInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_navigate,
)]
pub(crate) struct BrowserNavigateTool;
fn execute_browser_navigate(
    _self: &BrowserNavigateTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::BrowserNavigateInput =
        serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    crate::agent::tools::blocking::block_on(async { handle.page.goto(&input.url, None).await })
        .map_err(err)?;
    // After navigation, the cookies may have changed; persist.
    let _ = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .save_storage();
    let final_url = handle.page.url();
    let title = crate::agent::tools::blocking::block_on(async { handle.page.title().await })
        .unwrap_or_default();
    let resp = dtos::BrowserNavigateResponse {
        url: final_url,
        title,
    };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

// ---------------------------------------------------------------------------
// browser_get_page_state (BRWS-002, ReadOnly)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_get_page_state",
    desc = strings::BROWSER_GET_PAGE_STATE_DESCRIPTION,
    input = dtos::BrowserGetPageStateInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_get_page_state,
)]
pub(crate) struct BrowserGetPageStateTool;
fn execute_browser_get_page_state(
    _self: &BrowserGetPageStateTool,
    ctx: &ToolContext,
    _args: &str,
) -> Result<serde_json::Value, String> {
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    let script = r#"
        () => {
            const elements = document.querySelectorAll('a, button, input, select, textarea');
            const out = [];
            elements.forEach((el, i) => {
                el.setAttribute('data-agent-id', i);
                out.push({
                    agent_id: i,
                    tag: el.tagName,
                    text: (el.innerText || el.value || '').slice(0, 200),
                    placeholder: el.getAttribute('placeholder') || '',
                    name: el.getAttribute('name') || '',
                    type: el.getAttribute('type') || '',
                    href: el.getAttribute('href') || null
                });
            });
            return out;
        }
    "#;
    let value: serde_json::Value = crate::agent::tools::blocking::block_on(async {
        handle.page.evaluate(script, None::<&()>).await
    })
    .map_err(err)?;
    let elements_json = serde_json::to_string(&value).unwrap_or_else(|_| "[]".to_string());
    let total = value.as_array().map(|a| a.len()).unwrap_or(0);
    let url = handle.page.url();
    let title = crate::agent::tools::blocking::block_on(async { handle.page.title().await })
        .unwrap_or_default();
    let resp = dtos::BrowserGetPageStateResponse {
        url,
        title,
        elements: elements_json,
        total,
    };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

// ---------------------------------------------------------------------------
// browser_click (BRWS-003, Mutating)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_click",
    desc = strings::BROWSER_CLICK_DESCRIPTION,
    input = dtos::BrowserClickInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_click,
)]
pub(crate) struct BrowserClickTool;
fn execute_browser_click(
    _self: &BrowserClickTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::BrowserClickInput =
        serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    let selector = input.selector;
    let locator = handle.page.locator(&selector);
    crate::agent::tools::blocking::block_on(async { locator.click(None).await }).map_err(err)?;
    let _ = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .save_storage();
    let resp = dtos::BrowserClickResponse {
        result: "clicked".to_string(),
    };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

// ---------------------------------------------------------------------------
// browser_fill_input (BRWS-004, Mutating)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_fill_input",
    desc = strings::BROWSER_FILL_INPUT_DESCRIPTION,
    input = dtos::BrowserFillInputInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_fill_input,
)]
pub(crate) struct BrowserFillInputTool;
fn execute_browser_fill_input(
    _self: &BrowserFillInputTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::BrowserFillInputInput =
        serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    let selector = input.selector;
    let text = input.text;
    let locator = handle.page.locator(&selector);
    crate::agent::tools::blocking::block_on(async { locator.fill(&text, None).await })
        .map_err(err)?;
    let _ = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .save_storage();
    let resp = dtos::BrowserFillInputResponse {
        result: "filled".to_string(),
    };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

// ---------------------------------------------------------------------------
// browser_select_dropdown (BRWS-005, Mutating)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_select_dropdown",
    desc = strings::BROWSER_SELECT_DROPDOWN_DESCRIPTION,
    input = dtos::BrowserSelectDropdownInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_select_dropdown,
)]
pub(crate) struct BrowserSelectDropdownTool;
fn execute_browser_select_dropdown(
    _self: &BrowserSelectDropdownTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::BrowserSelectDropdownInput =
        serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    let selector = input.selector;
    let value = input.value;
    let locator = handle.page.locator(&selector);
    crate::agent::tools::blocking::block_on(async {
        locator.select_option(value.as_str(), None).await
    })
    .map_err(err)?;
    let _ = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .save_storage();
    let resp = dtos::BrowserSelectDropdownResponse {
        result: "selected".to_string(),
    };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

// ---------------------------------------------------------------------------
// browser_press_key (BRWS-006, Mutating)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_press_key",
    desc = strings::BROWSER_PRESS_KEY_DESCRIPTION,
    input = dtos::BrowserPressKeyInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_press_key,
)]
pub(crate) struct BrowserPressKeyTool;
fn execute_browser_press_key(
    _self: &BrowserPressKeyTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::BrowserPressKeyInput =
        serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    let key = input.key;
    crate::agent::tools::blocking::block_on(async {
        handle.page.keyboard().press(&key, None).await
    })
    .map_err(err)?;
    let _ = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .save_storage();
    let resp = dtos::BrowserPressKeyResponse {
        result: "pressed".to_string(),
    };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

// ---------------------------------------------------------------------------
// browser_evaluate_js (BRWS-007, Mutating — true escape hatch)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_evaluate_js",
    desc = strings::BROWSER_EVALUATE_JS_DESCRIPTION,
    input = dtos::BrowserEvaluateJsInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_evaluate_js,
)]
pub(crate) struct BrowserEvaluateJsTool;
fn execute_browser_evaluate_js(
    _self: &BrowserEvaluateJsTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::BrowserEvaluateJsInput =
        serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    let script = input.script;
    let value: serde_json::Value = crate::agent::tools::blocking::block_on(async {
        handle.page.evaluate(&script, None::<&()>).await
    })
    .map_err(err)?;
    let _ = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .save_storage();
    let result_str = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string());
    let resp = dtos::BrowserEvaluateJsResponse { result: result_str };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

// ---------------------------------------------------------------------------
// browser_screenshot (BRWS-008, Mutating — writes to a sandbox dir)
// ---------------------------------------------------------------------------

#[derive(ToolDescriptor)]
#[tool(
    name = "browser_screenshot",
    desc = strings::BROWSER_SCREENSHOT_DESCRIPTION,
    input = dtos::BrowserScreenshotInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = Browser,
    config = browser_spec(),
    execute_with = execute_browser_screenshot,
)]
pub(crate) struct BrowserScreenshotTool;
fn execute_browser_screenshot(
    _self: &BrowserScreenshotTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::BrowserScreenshotInput =
        serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
    // Sanitise the filename against the session's policy
    // before doing any Playwright work — fail fast on bad
    // input.
    let out_path = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .resolve_screenshot_path(&input.filename)
        .map_err(err)?;
    let handle = ctx
        .extensions
        .get::<crate::app::session::BrowserSession>()
        .unwrap()
        .page()
        .map_err(err)?;
    let bytes = crate::agent::tools::blocking::block_on(async {
        use playwright_rs::ScreenshotOptions;
        // full_page is the only option we set; everything
        // else stays at playwright's defaults. Use the
        // builder because `full_page` is only exposed on
        // `ScreenshotOptionsBuilder`, not on the struct.
        // `Page::screenshot` takes `Option<ScreenshotOptions>`.
        let opts = ScreenshotOptions::builder()
            .full_page(input.full_page)
            .build();
        handle.page.screenshot(Some(opts)).await
    })
    .map_err(err)?;
    std::fs::write(&out_path, &bytes).map_err(|e| err(format!("write screenshot: {}", e)))?;
    let resp = dtos::BrowserScreenshotResponse {
        path: out_path.to_string_lossy().to_string(),
        bytes: bytes.len(),
    };
    Ok(serde_json::to_value(resp).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
}

/// Self-registering provider for the browser family. Only
/// emitted when the `browser` Cargo feature is on; without it
/// the catalog simply doesn't include the browser group.
pub(crate) struct BrowserProvider;
impl ToolProvider for BrowserProvider {
    fn id(&self) -> &'static str {
        "browser"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Browser)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(BrowserNavigateTool),
            registered(BrowserGetPageStateTool),
            registered(BrowserClickTool),
            registered(BrowserFillInputTool),
            registered(BrowserSelectDropdownTool),
            registered(BrowserPressKeyTool),
            registered(BrowserEvaluateJsTool),
            registered(BrowserScreenshotTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}

#[cfg(test)]
mod tests {
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
        let mut config = crate::config::AppConfig::default();
        assert!(!BrowserNavigateTool.is_enabled(&config, ""));
        config.tool_groups.browser = true;
        assert!(BrowserNavigateTool.is_enabled(&config, ""));
    }

    #[test]
    fn test_only_get_page_state_is_readonly() {
        // BRWS-002 explicitly calls this out.
        let readonly = BrowserGetPageStateTool.safety();
        assert_eq!(readonly, crate::agent::tools::Safety::ReadOnly);
        for safety in [
            BrowserNavigateTool.safety(),
            BrowserClickTool.safety(),
            BrowserFillInputTool.safety(),
            BrowserSelectDropdownTool.safety(),
            BrowserPressKeyTool.safety(),
            BrowserEvaluateJsTool.safety(),
            BrowserScreenshotTool.safety(),
        ] {
            assert_eq!(safety, crate::agent::tools::Safety::Mutating);
        }
    }

    #[test]
    fn test_schema_serializes() {
        // Just touch the schema so a regression in
        // schemars types is caught at build time.
        let _ = BrowserNavigateTool.parameters_schema();
        let _ = BrowserScreenshotTool.parameters_schema();
    }
}
