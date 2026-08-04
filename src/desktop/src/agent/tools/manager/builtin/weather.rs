//! Weather tool implementation for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::config::AppConfig;
use std::any::TypeId;

use super::json_schema;
use super::strings;

/// Tool that fetches current weather conditions and forecasts for a location.
pub(crate) struct GetWeatherTool;
impl Tool for GetWeatherTool {
    fn name(&self) -> &'static str {
        "get_weather"
    }
    fn description(&self) -> &'static str {
        strings::GET_WEATHER_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetWeatherInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::GetWeatherInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.weather
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, _ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetWeatherInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::weather::tool_get_weather(&input.location, input.date_range.as_deref())
            .map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
    }
}
