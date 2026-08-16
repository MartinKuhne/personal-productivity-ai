//! Weather integration — geocodes a location via Nominatim and
//! fetches current conditions and forecasts from the US National
//! Weather Service (api.weather.gov).
//!
//! This is the protocol layer. The LLM-tool-loop adapter that
//! exposes it as a single `Tool` impl lives in
//! crate::tools::registry::builtin::weather.

use serde_json::Value;

/// Production upstream base URLs. Tests swap in wiremock URIs via
/// [`WeatherConfig`].
const NOMINATIM_BASE: &str = "https://nominatim.openstreetmap.org";
/// The base URL for the National Weather Service API.
const NWS_BASE: &str = "https://api.weather.gov";

/// Per-call override of the upstream URLs.
///
/// `tool_get_weather` (the public entry point) builds one of these
/// from the production constants so production callers don't see
/// this type. Tests construct a `WeatherConfig` whose bases point
/// at a `wiremock::MockServer` and pass it to
/// [`tool_get_weather_with`].
#[derive(Debug, Clone)]
pub struct WeatherConfig {
    /// Base URL for the Nominatim geocoder (no trailing slash). In
    /// production this is `https://nominatim.openstreetmap.org`.
    pub nominatim_base: String,
    /// Base URL for the NWS `points` and `forecast` endpoints (no
    /// trailing slash). In production this is
    /// `https://api.weather.gov`.
    pub nws_base: String,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            nominatim_base: NOMINATIM_BASE.to_string(),
            nws_base: NWS_BASE.to_string(),
        }
    }
}

/// Build the URL for a Nominatim geocode search.
///
/// `location` is the user-supplied query — either `"lat,lon"` (used
/// directly), a 5-digit US zip (suffixed with `" US"`), or any free
/// text. Spaces are percent-encoded; everything else passes through
/// as-is. Extracted as `pub(crate)` so tests can sanity-check the
/// URL shape against a wiremock stub.
pub(crate) fn geocode_url(base: &str, location: &str) -> String {
    let query = if location.len() == 5 && location.chars().all(|c| c.is_ascii_digit()) {
        format!("{} US", location)
    } else {
        location.to_string()
    };
    // We must manually URL encode the query. But since we don't have url-encoding crate imported by default,
    // let's do a basic replace for spaces.
    let query_encoded = query.replace(" ", "%20");
    format!("{}/search?q={}&format=json&limit=1", base, query_encoded)
}

/// Build the URL for the NWS `points` lookup. Returns the
/// `{base}/points/{lat},{lon}` form the NWS API expects.
pub(crate) fn points_url(base: &str, lat: f64, lon: f64) -> String {
    format!("{}/points/{},{}", base, lat, lon)
}

/// Parse a `(lat, lon)` from a Nominatim search response body.
///
/// `body` is the JSON array Nominatim returns; we take the first
/// element and extract `lat`/`lon` as strings (Nominatim returns
/// them as strings, not numbers).
pub(crate) fn parse_nominatim_first(body: &Value) -> Result<(f64, f64), String> {
    let first = body.as_array().and_then(|a| a.first()).ok_or_else(|| {
        tracing::error!(
            name = "tool.weather.geocode.not_found",
            "Nominatim geocoding API returned no results. Operator should verify location name."
        );
        "Location not found".to_string()
    })?;

    let lat = first
        .get("lat")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .ok_or_else(|| {
            tracing::error!(
                name = "tool.weather.geocode.missing_lat",
                "Nominatim geocoding API response missing latitude."
            );
            "Missing lat".to_string()
        })?;
    let lon = first
        .get("lon")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .ok_or_else(|| {
            tracing::error!(
                name = "tool.weather.geocode.missing_lon",
                "Nominatim geocoding API response missing longitude."
            );
            "Missing lon".to_string()
        })?;

    Ok((lat, lon))
}

/// Geocode a free-text location string to `(lat, lon)`. If the input
/// is already `"lat,lon"` it short-circuits; otherwise it hits the
/// Nominatim search endpoint at `cfg.nominatim_base`.
fn geocode(cfg: &WeatherConfig, location: &str) -> Result<(f64, f64), String> {
    if let Some((lat_str, lon_str)) = location.split_once(',')
        && let (Ok(lat), Ok(lon)) = (lat_str.trim().parse::<f64>(), lon_str.trim().parse::<f64>())
    {
        return Ok((lat, lon));
    }

    let url = geocode_url(&cfg.nominatim_base, location);

    let req = match reqwest::blocking::Client::new()
        .get(&url)
        .header("User-Agent", "FastMD Weather Tool/1.0")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(name = "tool.weather.geocode.api_failed", error = %e, url = %url, "Nominatim geocoding API request failed. Operator should verify network or API limits.");
            return Err(format!("Nominatim API error: {}", e));
        }
    };

    let json: Value = match req.json() {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(name = "tool.weather.geocode.json_failed", error = %e, "Nominatim geocoding API returned invalid JSON. Operator should verify API response.");
            return Err(format!("Nominatim JSON error: {}", e));
        }
    };

    parse_nominatim_first(&json)
}

/// Public entry point — geocodes `location`, then walks the NWS
/// `points` → `forecast` chain, then filters forecast periods by
/// `date_range`. Returns the DTO consumed by the LLM-tool layer.
///
/// Thin wrapper over [`tool_get_weather_with`] that uses the
/// production upstream URLs.
pub fn tool_get_weather(
    location: &str,
    date_range: Option<&str>,
) -> Result<crate::tools::dtos::GetWeatherResponse, String> {
    tool_get_weather_with(&WeatherConfig::default(), location, date_range)
}

/// Like [`tool_get_weather`] but with a caller-supplied
/// [`WeatherConfig`]. Lets tests drive the call chain against a
/// `wiremock::MockServer` without monkey-patching production
/// constants or env vars.
pub fn tool_get_weather_with(
    cfg: &WeatherConfig,
    location: &str,
    date_range: Option<&str>,
) -> Result<crate::tools::dtos::GetWeatherResponse, String> {
    // Reference: https://www.weather.gov/documentation/services-web-api

    let (lat, lon) = geocode(cfg, location)?;

    let points_url = points_url(&cfg.nws_base, lat, lon);

    let req = match reqwest::blocking::Client::new()
        .get(&points_url)
        .header("User-Agent", "FastMD Weather Tool/1.0")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.points_api_failed", error = %e, url = %points_url, "NWS Points API request failed. Operator should verify network connectivity.");
            return Err(format!("NWS Points API error: {}", e));
        }
    };

    let json: Value = match req.json() {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.points_json_failed", error = %e, "NWS Points API returned invalid JSON. Operator should verify API status.");
            return Err(format!("NWS Points JSON error: {}", e));
        }
    };

    let forecast_url = match json
        .get("properties")
        .and_then(|p| p.get("forecast"))
        .and_then(|f| f.as_str())
    {
        Some(url) => url.to_string(),
        None => return Err("Could not find forecast URL in NWS response".to_string()),
    };

    let req = match reqwest::blocking::Client::new()
        .get(&forecast_url)
        .header("User-Agent", "FastMD Weather Tool/1.0")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.forecast_api_failed", error = %e, url = %forecast_url, "NWS Forecast API request failed. Operator should verify network connectivity.");
            return Err(format!("NWS Forecast API error: {}", e));
        }
    };

    let forecast_json: Value = match req.json() {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.forecast_json_failed", error = %e, "NWS Forecast API returned invalid JSON. Operator should verify API status.");
            return Err(format!("NWS Forecast JSON error: {}", e));
        }
    };

    let periods = match forecast_json
        .get("properties")
        .and_then(|p| p.get("periods"))
        .and_then(|p| p.as_array())
    {
        Some(p) => p,
        None => return Err("Could not find periods in NWS forecast response".to_string()),
    };

    let mut results = Vec::new();

    let dr = date_range.unwrap_or("").to_lowercase();
    let filter_dr = !dr.is_empty();

    for period in periods {
        let start = period
            .get("startTime")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Simple string containment check for date range (e.g. if dr is "2026-07-18")
        // Or if it's not filtered, just add it.
        if !filter_dr || (start.contains(&dr) || (start.len() >= 10 && dr.contains(&start[..10]))) {
            let name = period.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let temp = period
                .get("temperature")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let temp_unit = period
                .get("temperatureUnit")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let forecast = period
                .get("detailedForecast")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            results.push(serde_json::json!({
                "period_name": name,
                "start_time": start,
                "temperature": format!("{} {}", temp, temp_unit),
                "detailed_forecast": forecast
            }));
        }
    }

    if results.is_empty() {
        let err_msg = if filter_dr {
            format!(
                "No weather data found matching date range '{}'. Remember NWS only provides ~7 days of forecast.",
                dr
            )
        } else {
            "No forecast periods found.".to_string()
        };
        tracing::warn!(name = "tool.weather.no_results", location = %location, "No weather data found for the given location and date range. Operator should verify location query.");
        return Err(err_msg);
    }

    Ok(crate::tools::dtos::GetWeatherResponse {
        result: serde_json::to_string(&results).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{any, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Same shape as the Trello/DAV tests' `WiremockGuard` — a
    /// tokio runtime that owns the hyper task serving wiremock
    /// responses. Drop the guard and the server stops.
    struct WiremockGuard {
        server: MockServer,
        _runtime: tokio::runtime::Runtime,
    }

    impl WiremockGuard {
        fn start() -> Self {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");
            let server = runtime.block_on(MockServer::start());
            Self {
                server,
                _runtime: runtime,
            }
        }

        fn uri(&self) -> String {
            self.server.uri()
        }

        fn register(&self, mock: Mock) {
            self._runtime.block_on(self.server.register(mock));
        }
    }

    // --- geocode_url tests ---

    #[test]
    fn geocode_url_uses_search_endpoint() {
        let url = geocode_url("https://nominatim.example.com", "Seattle, WA");
        assert!(
            url.starts_with("https://nominatim.example.com/search?"),
            "{url}"
        );
        assert!(url.contains("format=json"), "{url}");
        assert!(url.contains("limit=1"), "{url}");
    }

    #[test]
    fn geocode_url_percent_encodes_spaces() {
        let url = geocode_url("https://nominatim.example.com", "New York");
        assert!(url.contains("New%20York"), "{url}");
    }

    #[test]
    fn geocode_url_appends_us_for_5_digit_zip() {
        let url = geocode_url("https://nominatim.example.com", "98101");
        assert!(url.contains("98101%20US"), "{url}");
    }

    // --- points_url tests ---

    #[test]
    fn points_url_includes_lat_lon() {
        let url = points_url("https://api.weather.example.com", 47.6, -122.3);
        assert_eq!(url, "https://api.weather.example.com/points/47.6,-122.3");
    }

    // --- geocode tests ---

    #[test]
    fn geocode_short_circuits_on_lat_lon_string() {
        let cfg = WeatherConfig::default();
        let (lat, lon) = geocode(&cfg, "47.6, -122.3").unwrap();
        assert_eq!(lat, 47.6);
        assert_eq!(lon, -122.3);
    }

    #[test]
    fn geocode_returns_lat_lon_on_success() {
        let mock = WiremockGuard::start();
        mock.register(
            Mock::given(method("GET"))
                .and(path("/search"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "application/json")
                        .set_body_string(r#"[{"lat": "47.6062", "lon": "-122.3321"}]"#),
                ),
        );

        let cfg = WeatherConfig {
            nominatim_base: mock.uri(),
            nws_base: WeatherConfig::default().nws_base,
        };
        let (lat, lon) = geocode(&cfg, "Seattle, WA").unwrap();
        assert_eq!(lat, 47.6062);
        assert_eq!(lon, -122.3321);
    }

    #[test]
    fn geocode_errors_on_empty_results() {
        let mock = WiremockGuard::start();
        mock.register(
            Mock::given(any()).respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("[]"),
            ),
        );

        let cfg = WeatherConfig {
            nominatim_base: mock.uri(),
            nws_base: WeatherConfig::default().nws_base,
        };
        let err = geocode(&cfg, "UnknownPlace").unwrap_err();
        assert_eq!(err, "Location not found");
    }

    #[test]
    fn geocode_errors_on_invalid_json() {
        let mock = WiremockGuard::start();
        mock.register(
            Mock::given(any()).respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("invalid json"),
            ),
        );

        let cfg = WeatherConfig {
            nominatim_base: mock.uri(),
            nws_base: WeatherConfig::default().nws_base,
        };
        let err = geocode(&cfg, "Seattle, WA").unwrap_err();
        assert!(err.starts_with("Nominatim JSON error"), "got: {err}");
    }

    // --- tool_get_weather_with end-to-end test ---

    #[test]
    fn tool_get_weather_chains_geocode_points_forecast() {
        let mock = WiremockGuard::start();

        // The NWS points response references the forecast URL — point
        // that URL at the wiremock server so the next call lands back
        // here.
        let points_body = format!(
            r#"{{"properties": {{"forecast": "{}/forecast"}}}}"#,
            mock.uri()
        );

        // 1. Nominatim geocode.
        mock.register(
            Mock::given(method("GET"))
                .and(path("/search"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "application/json")
                        .set_body_string(r#"[{"lat": "47.6062", "lon": "-122.3321"}]"#),
                ),
        );

        // 2. NWS points — returns the (mocked) forecast URL.
        mock.register(
            Mock::given(method("GET"))
                .and(path("/points/47.6062,-122.3321"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "application/json")
                        .set_body_string(points_body),
                ),
        );

        // 3. Forecast endpoint.
        mock.register(
            Mock::given(method("GET"))
                .and(path("/forecast"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "application/json")
                        .set_body_string(
                            r#"{"properties": {"periods": [
                                {
                                    "startTime": "2026-07-19T10:00:00Z",
                                    "name": "Today",
                                    "temperature": 75,
                                    "temperatureUnit": "F",
                                    "detailedForecast": "Sunny"
                                }
                            ]}}"#,
                        ),
                ),
        );

        let cfg = WeatherConfig {
            nominatim_base: mock.uri(),
            nws_base: mock.uri(),
        };

        let result = tool_get_weather_with(&cfg, "Seattle, WA", None).unwrap();
        assert!(result.result.contains("Today"), "got: {}", result.result);
        assert!(result.result.contains("75 F"), "got: {}", result.result);
        assert!(result.result.contains("Sunny"), "got: {}", result.result);

        // date-range filter — same day should still match.
        let result2 = tool_get_weather_with(&cfg, "Seattle, WA", Some("2026-07-19")).unwrap();
        assert!(result2.result.contains("Today"), "got: {}", result2.result);

        // date-range filter — different day returns the "no data" error.
        let err = tool_get_weather_with(&cfg, "Seattle, WA", Some("2026-07-20")).unwrap_err();
        assert!(err.contains("No weather data found"), "got: {err}");
    }
}

#[cfg(test)]
#[path = "../weather_proptests.rs"]
mod weather_proptests;
