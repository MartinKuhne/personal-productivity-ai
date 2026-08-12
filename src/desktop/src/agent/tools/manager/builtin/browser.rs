//! LLM tool wrappers for the headless browser (`BRWS-001..008`).
//!
//! Each tool's `Tool::execute` runs the underlying Playwright
//! future on the process-wide Tokio runtime via
//! [`crate::agent::tools::blocking::block_on`] â€” the same
//! sync-to-async bridge the CalDAV / CardDAV tools use. Mutating
//! tools trigger a `save_storage()` on the
//! [`crate::app::session::BrowserSession`] so cookies / local
//! storage survive an app restart. See
//! `doc/planning/browser_tools.md` for the design record and
//! `src/desktop/Tools.md` for the user-facing catalog.

use super::json_schema;
use super::strings;
use crate::agent::tools::Safety;
use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::config::AppConfig;
use std::any::TypeId;

/// Convert any error string into a Tool error string. Most
/// Playwright errors already have decent `Display` impls; we
/// just wrap them with a stable prefix.
fn err(s: impl std::fmt::Display) -> String {
    format!("browser tool failed: {}", s)
}

// ---------------------------------------------------------------------------
// browser_navigate (BRWS-001, Mutating)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserNavigateTool;
impl Tool for BrowserNavigateTool {
    fn name(&self) -> &'static str {
        "browser_navigate"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_NAVIGATE_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserNavigateInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserNavigateInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::BrowserNavigateInput =
            serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
        let handle = ctx.browser_session.page().map_err(err)?;
        crate::agent::tools::blocking::block_on(async { handle.page.goto(&input.url, None).await })
            .map_err(err)?;
        // After navigation, the cookies may have changed; persist.
        let _ = ctx.browser_session.save_storage();
        let final_url = handle.page.url();
        let title = crate::agent::tools::blocking::block_on(async { handle.page.title().await })
            .unwrap_or_default();
        let resp = dtos::BrowserNavigateResponse {
            url: final_url,
            title,
        };
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

// ---------------------------------------------------------------------------
// browser_get_page_state (BRWS-002, ReadOnly)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserGetPageStateTool;
impl Tool for BrowserGetPageStateTool {
    fn name(&self) -> &'static str {
        "browser_get_page_state"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_GET_PAGE_STATE_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserGetPageStateInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserGetPageStateInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, _args: &str) -> Result<serde_json::Value, String> {
        let handle = ctx.browser_session.page().map_err(err)?;
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
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

// ---------------------------------------------------------------------------
// browser_click (BRWS-003, Mutating)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserClickTool;
impl Tool for BrowserClickTool {
    fn name(&self) -> &'static str {
        "browser_click"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_CLICK_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserClickInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserClickInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::BrowserClickInput =
            serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
        let handle = ctx.browser_session.page().map_err(err)?;
        let selector = input.selector;
        let locator = handle.page.locator(&selector);
        crate::agent::tools::blocking::block_on(async { locator.click(None).await })
            .map_err(err)?;
        let _ = ctx.browser_session.save_storage();
        let resp = dtos::BrowserClickResponse {
            result: "clicked".to_string(),
        };
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

// ---------------------------------------------------------------------------
// browser_fill_input (BRWS-004, Mutating)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserFillInputTool;
impl Tool for BrowserFillInputTool {
    fn name(&self) -> &'static str {
        "browser_fill_input"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_FILL_INPUT_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserFillInputInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserFillInputInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::BrowserFillInputInput =
            serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
        let handle = ctx.browser_session.page().map_err(err)?;
        let selector = input.selector;
        let text = input.text;
        let locator = handle.page.locator(&selector);
        crate::agent::tools::blocking::block_on(async { locator.fill(&text, None).await })
            .map_err(err)?;
        let _ = ctx.browser_session.save_storage();
        let resp = dtos::BrowserFillInputResponse {
            result: "filled".to_string(),
        };
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

// ---------------------------------------------------------------------------
// browser_select_dropdown (BRWS-005, Mutating)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserSelectDropdownTool;
impl Tool for BrowserSelectDropdownTool {
    fn name(&self) -> &'static str {
        "browser_select_dropdown"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_SELECT_DROPDOWN_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserSelectDropdownInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserSelectDropdownInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::BrowserSelectDropdownInput =
            serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
        let handle = ctx.browser_session.page().map_err(err)?;
        let selector = input.selector;
        let value = input.value;
        let locator = handle.page.locator(&selector);
        // `Locator::select_option` expects `impl Into<SelectOption>`;
        // `SelectOption: From<&str>` so pass the str view.
        crate::agent::tools::blocking::block_on(async {
            locator.select_option(value.as_str(), None).await
        })
        .map_err(err)?;
        let _ = ctx.browser_session.save_storage();
        let resp = dtos::BrowserSelectDropdownResponse {
            result: "selected".to_string(),
        };
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

// ---------------------------------------------------------------------------
// browser_press_key (BRWS-006, Mutating)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserPressKeyTool;
impl Tool for BrowserPressKeyTool {
    fn name(&self) -> &'static str {
        "browser_press_key"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_PRESS_KEY_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserPressKeyInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserPressKeyInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::BrowserPressKeyInput =
            serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
        let handle = ctx.browser_session.page().map_err(err)?;
        let key = input.key;
        crate::agent::tools::blocking::block_on(async {
            handle.page.keyboard().press(&key, None).await
        })
        .map_err(err)?;
        let _ = ctx.browser_session.save_storage();
        let resp = dtos::BrowserPressKeyResponse {
            result: "pressed".to_string(),
        };
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

// ---------------------------------------------------------------------------
// browser_evaluate_js (BRWS-007, Mutating â€” true escape hatch)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserEvaluateJsTool;
impl Tool for BrowserEvaluateJsTool {
    fn name(&self) -> &'static str {
        "browser_evaluate_js"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_EVALUATE_JS_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserEvaluateJsInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserEvaluateJsInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::BrowserEvaluateJsInput =
            serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
        let handle = ctx.browser_session.page().map_err(err)?;
        let script = input.script;
        let value: serde_json::Value = crate::agent::tools::blocking::block_on(async {
            handle.page.evaluate(&script, None::<&()>).await
        })
        .map_err(err)?;
        let _ = ctx.browser_session.save_storage();
        let result_str = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string());
        let resp = dtos::BrowserEvaluateJsResponse { result: result_str };
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

// ---------------------------------------------------------------------------
// browser_screenshot (BRWS-008, Mutating â€” writes to a sandbox dir)
// ---------------------------------------------------------------------------

pub(crate) struct BrowserScreenshotTool;
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &'static str {
        "browser_screenshot"
    }
    fn description(&self) -> &'static str {
        strings::BROWSER_SCREENSHOT_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::BrowserScreenshotInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::BrowserScreenshotInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.browser
    }
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::BrowserScreenshotInput =
            serde_json::from_str(args).map_err(|e| err(format!("Invalid args: {}", e)))?;
        // Sanitise the filename against the session's policy
        // before doing any Playwright work â€” fail fast on bad
        // input.
        let out_path = ctx
            .browser_session
            .resolve_screenshot_path(&input.filename)
            .map_err(err)?;
        let handle = ctx.browser_session.page().map_err(err)?;
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
        Ok(serde_json::to_value(resp)
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::session::BrowserSession;
    use crate::config::AppConfig;
    use std::sync::Arc;

    fn ctx_with_session() -> crate::agent::tools::context::ToolContext {
        // `ToolContext` is now `'static` by construction (every
        // reference-shaped field is an owned `Arc` or a cheap-clone
        // `Bus`), so the previous `Box::leak`-and-pointer-cast trick
        // is gone. The helper just constructs a context by value.
        // The actual execute paths are covered by the integration
        // tests in `app/browser/session.rs` (and the gated
        // Playwright integration tests in `tools/browser_tests.rs`).
        let config = AppConfig::default();
        let bus = crate::bus::core::Bus::<crate::bus::events::file::FileEvent>::new();
        let session = Arc::new(BrowserSession::new(&config));
        let pdf_backing = Arc::new(crate::app::session::PdfBackingTracker::new());
        let tm = std::sync::Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        ));
        let cache = Arc::new(crate::agent::tools::manager::cache::ToolCache::new());
        crate::agent::tools::context::ToolContext::new(
            Arc::new(config),
            bus,
            session,
            pdf_backing,
            cache,
            tm,
            std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
        )
    }

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
        let mut config = AppConfig::default();
        assert!(!BrowserNavigateTool.is_enabled(&config, ""));
        config.tool_groups.browser = true;
        assert!(BrowserNavigateTool.is_enabled(&config, ""));
    }

    #[test]
    fn test_only_get_page_state_is_readonly() {
        // BRWS-002 explicitly calls this out.
        let readonly = BrowserGetPageStateTool.safety();
        assert_eq!(readonly, Safety::ReadOnly);
        for safety in [
            BrowserNavigateTool.safety(),
            BrowserClickTool.safety(),
            BrowserFillInputTool.safety(),
            BrowserSelectDropdownTool.safety(),
            BrowserPressKeyTool.safety(),
            BrowserEvaluateJsTool.safety(),
            BrowserScreenshotTool.safety(),
        ] {
            assert_eq!(safety, Safety::Mutating);
        }
    }

    #[test]
    fn test_schema_serializes() {
        // Just touch the schema so a regression in
        // schemars types is caught at build time.
        let _ = BrowserNavigateTool.parameters_schema();
        let _ = BrowserScreenshotTool.parameters_schema();
    }

    #[test]
    fn test_context_carries_session() {
        // The plumbing worked: ToolContext now owns the session.
        let _ = ctx_with_session();
    }
}
