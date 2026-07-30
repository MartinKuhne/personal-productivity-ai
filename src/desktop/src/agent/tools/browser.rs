//! Playwright-based async browser automation — navigate, get page state, click, type, screenshot.

use playwright_rs::Page;

/// Navigates the page to the given URL.
pub async fn browser_navigate(page: &Page, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    page.goto(url, None).await?;
    Ok(())
}

/// Retrieves a simplified JSON string representing interactable elements.
pub async fn browser_get_page_state(page: &Page) -> Result<String, Box<dyn std::error::Error>> {
    let script = r#"
        () => {
            let elements = document.querySelectorAll('a, button, input, select, textarea');
            return Array.from(elements).map((el, i) => {
                el.setAttribute('data-agent-id', i);
                return { 
                    agent_id: i, 
                    tag: el.tagName, 
                    text: el.innerText || el.value || '', 
                    placeholder: el.getAttribute('placeholder') || '' 
                };
            });
        }
    "#;
    let state: serde_json::Value = page.evaluate(script, None::<&()>).await?;
    Ok(serde_json::to_string(&state)?)
}

/// Clicks an element by selector.
pub async fn browser_click(page: &Page, selector: &str) -> Result<(), Box<dyn std::error::Error>> {
    let locator = page.locator(selector).await;
    locator.click(None).await?;
    Ok(())
}

/// Fills an input element with text.
pub async fn browser_fill_input(
    page: &Page,
    selector: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let locator = page.locator(selector).await;
    locator.fill(text, None).await?;
    Ok(())
}

/// Selects an option in a dropdown.
pub async fn browser_select_dropdown(
    page: &Page,
    selector: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let locator = page.locator(selector).await;
    locator.select_option(value, None).await?;
    Ok(())
}

/// Simulates a keyboard press.
pub async fn browser_press_key(page: &Page, key: &str) -> Result<(), Box<dyn std::error::Error>> {
    page.keyboard().press(key, None).await?;
    Ok(())
}

/// Evaluates raw JavaScript in the page.
pub async fn browser_evaluate_js(
    page: &Page,
    script: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let val: serde_json::Value = page.evaluate(script, None::<&()>).await?;
    Ok(serde_json::to_string(&val)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use playwright_rs::{Browser, Playwright};

    // A helper to initialize Playwright.
    // In CI or environments without browsers, this might fail, so we ignore it by default.
    // Run with `cargo test -- --ignored` if browsers are installed.
    async fn setup_page() -> Result<(Playwright, Browser, Page), Box<dyn std::error::Error>> {
        let playwright = Playwright::launch().await?;
        let chromium = playwright.chromium();
        let browser = chromium.launch().await?;
        let page = browser.new_page().await?;
        Ok((playwright, browser, page))
    }

    #[tokio::test]
    #[ignore = "Requires playwright browsers installed locally"]
    async fn test_browser_navigate_and_state() -> Result<(), Box<dyn std::error::Error>> {
        let (_playwright, _browser, page) = setup_page().await?;

        browser_navigate(&page, "https://example.com").await?;

        let state = browser_get_page_state(&page).await?;
        assert!(state.contains("a")); // example.com has an <a> link

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires playwright browsers installed locally"]
    async fn test_browser_evaluate() -> Result<(), Box<dyn std::error::Error>> {
        let (_playwright, _browser, page) = setup_page().await?;

        browser_navigate(&page, "https://example.com").await?;

        let result = browser_evaluate_js(&page, "() => 2 + 2").await?;
        assert_eq!(result, "4");

        Ok(())
    }
}
