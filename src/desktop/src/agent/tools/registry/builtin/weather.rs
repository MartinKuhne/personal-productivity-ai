//! Weather tool implementation for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;

use super::strings;

/// Tool that fetches current weather conditions and forecasts for a location.
pub(crate) struct GetWeatherTool;
impl Tool for GetWeatherTool {
    crate::tool_descriptor! {
        name: "get_weather",
        desc: strings::GET_WEATHER_DESCRIPTION,
        input: dtos::GetWeatherInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Weather,
    }
    fn execute(&self, _ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetWeatherInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::weather::tool_get_weather(&input.location, input.date_range.as_deref())
            .map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
    }
}
