use std::path::PathBuf;
use std::sync::Arc;

pub trait BrowserAutomationExt: Send + Sync {
    fn navigate(&self, url: &str) -> Result<(String, String), String>; // returns (url, title)
    fn get_page_state(&self) -> Result<(String, String, String, usize), String>; // returns (url, title, elements_json, total)
    fn click(&self, selector: &str) -> Result<(), String>;
    fn fill_input(&self, selector: &str, text: &str) -> Result<(), String>;
    fn select_dropdown(&self, selector: &str, value: &str) -> Result<(), String>;
    fn press_key(&self, key: &str) -> Result<(), String>;
    fn evaluate_js(&self, script: &str) -> Result<serde_json::Value, String>;
    fn screenshot(&self, filename: &str, full_page: bool) -> Result<(PathBuf, Vec<u8>), String>;

    // Additional methods for session management
    fn save_storage(&self) -> Result<(), String>;
    fn resolve_screenshot_path(&self, filename: &str) -> Result<PathBuf, String>;
}

/// Wrapper to store in `Extensions`.
#[derive(Clone)]
pub struct BrowserExt(pub Arc<dyn BrowserAutomationExt>);
