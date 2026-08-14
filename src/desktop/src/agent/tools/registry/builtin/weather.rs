//! Weather tool implementation and provider for the tool registry.

use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::dtos;
use crate::tools::provider::{RegisteredTool, ToolProvider};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::strings;

/// Tool that fetches current weather conditions and forecasts for a location.
#[derive(ToolDescriptor)]
#[tool(
    name = "get_weather",
    desc = strings::GET_WEATHER_DESCRIPTION,
    input = dtos::GetWeatherInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Weather,
    execute_with = execute_get_weather,
)]
pub(crate) struct GetWeatherTool;
fn execute_get_weather(
    _self: &GetWeatherTool,
    _ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::GetWeatherInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::weather::tool_get_weather(&input.location, input.date_range.as_deref()).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Self-registering provider for the weather family.
pub(crate) struct WeatherProvider;
impl ToolProvider for WeatherProvider {
    fn id(&self) -> &'static str {
        "weather"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Weather)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![registered(GetWeatherTool)]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
