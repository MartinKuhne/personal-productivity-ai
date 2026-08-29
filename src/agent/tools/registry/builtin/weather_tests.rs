//! Tests for weather registry provider — descriptor, safety, DTOs.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

#[test]
fn weather_provider_registers_one_tool() {
    let provider = WeatherProvider;
    assert_eq!(provider.id(), "weather");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Weather)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].descriptor.name, "get_weather");
}

#[test]
fn safety_is_readonly() {
    assert_eq!(GetWeatherTool.safety(), Safety::ReadOnly);
}

#[test]
fn tool_group_is_weather() {
    assert_eq!(
        WeatherProvider.tools()[0].descriptor.group,
        ToolGroupId::Internal(InternalToolGroup::Weather)
    );
}

#[test]
fn descriptor_has_input_schema() {
    assert!(GetWeatherTool.descriptor().parameters_schema.is_object());
}

#[test]
fn dto_get_weather_round_trip() {
    let p: dtos::GetWeatherInput = serde_json::from_str(r#"{"location":"Berlin"}"#).unwrap();
    assert_eq!(p.location, "Berlin");
    assert!(p.date_range.is_none());
    let with_range: dtos::GetWeatherInput =
        serde_json::from_str(r#"{"location":"Berlin","date_range":"2024-01-01/2024-01-02"}"#)
            .unwrap();
    assert_eq!(
        with_range.date_range.as_deref(),
        Some("2024-01-01/2024-01-02")
    );
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(GetWeatherTool);
    assert_eq!(r.descriptor.name, "get_weather");
    assert_eq!(r.executor.descriptor().name, "get_weather");
}
