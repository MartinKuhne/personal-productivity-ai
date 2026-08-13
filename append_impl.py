import sys

def append_trait_impl(filepath):
    with open(filepath, 'a', encoding='utf-8') as f:
        f.write("""
#[cfg(feature = "browser")]
impl crate::agent::tools::browser::BrowserAutomationExt for BrowserSession {
    fn navigate(&self, url: &str) -> Result<(String, String), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        crate::agent::tools::blocking::block_on(async { handle.page.goto(url, None).await })
            .map_err(|e| e.to_string())?;
        let final_url = handle.page.url().unwrap_or_default();
        let title = crate::agent::tools::blocking::block_on(async { handle.page.title().await })
            .unwrap_or_default();
        Ok((final_url, title))
    }

    fn get_page_state(&self) -> Result<(String, String, String, usize), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
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
        }).map_err(|e| e.to_string())?;
        
        let elements_json = serde_json::to_string(&value).unwrap_or_else(|_| "[]".to_string());
        let total = value.as_array().map(|a| a.len()).unwrap_or(0);
        let url = handle.page.url().unwrap_or_default();
        let title = crate::agent::tools::blocking::block_on(async { handle.page.title().await })
            .unwrap_or_default();
        Ok((url, title, elements_json, total))
    }

    fn click(&self, selector: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        let locator = handle.page.locator(selector);
        crate::agent::tools::blocking::block_on(async { locator.click(None).await })
            .map_err(|e| e.to_string())
    }

    fn fill_input(&self, selector: &str, text: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        let locator = handle.page.locator(selector);
        crate::agent::tools::blocking::block_on(async { locator.fill(text, None).await })
            .map_err(|e| e.to_string())
    }

    fn select_dropdown(&self, selector: &str, value: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        let locator = handle.page.locator(selector);
        crate::agent::tools::blocking::block_on(async { locator.select_option(value, None).await })
            .map_err(|e| e.to_string())
    }

    fn press_key(&self, key: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        crate::agent::tools::blocking::block_on(async { handle.page.keyboard().press(key, None).await })
            .map_err(|e| e.to_string())
    }

    fn evaluate_js(&self, script: &str) -> Result<serde_json::Value, String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        crate::agent::tools::blocking::block_on(async { handle.page.evaluate(script, None::<&()>).await })
            .map_err(|e| e.to_string())
    }

    fn screenshot(&self, filename: &str, full_page: bool) -> Result<(std::path::PathBuf, Vec<u8>), String> {
        let out_path = self.resolve_screenshot_path(filename).map_err(|e| e.to_string())?;
        let handle = self.page().map_err(|e| e.to_string())?;
        let bytes = crate::agent::tools::blocking::block_on(async {
            use playwright_rs::ScreenshotOptions;
            let opts = ScreenshotOptions::builder().full_page(full_page).build();
            handle.page.screenshot(Some(opts)).await
        }).map_err(|e| e.to_string())?;
        Ok((out_path, bytes))
    }

    fn save_storage(&self) -> Result<(), String> {
        BrowserSession::save_storage(self).map_err(|e| e.to_string())
    }
    
    fn resolve_screenshot_path(&self, filename: &str) -> Result<std::path::PathBuf, String> {
        BrowserSession::resolve_screenshot_path(self, filename).map_err(|e| e.to_string())
    }
}
""")

if __name__ == "__main__":
    append_trait_impl("C:/Users/mkuhn/src/ppai/src/desktop/src/app/session/browser_session.rs")
