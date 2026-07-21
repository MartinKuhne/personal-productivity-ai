pub mod caldav;
pub mod carddav;
pub mod context;
pub mod csv_db;
pub mod dtos;
pub mod filesystem;
pub mod jmap;
pub mod registry;
pub mod weather;
pub mod web;
pub mod yaml_header;

use crate::config::AppConfig;
use context::ToolContext;
use std::any::TypeId;

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_type(&self) -> TypeId;
    fn parameters_schema(&self) -> serde_json::Value;
    fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool;
    fn execute(
        &self,
        ctx: &ToolContext,
        input_json: &str,
    ) -> Result<serde_json::Value, String>;
}

pub use context::ToolContext as ToolContextType;
pub use registry::{execute_tool, get_tools_schema};
