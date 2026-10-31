//! Tests for the weather protocol layer (`agent::lib::weather`): URL building, Nominatim parsing and the mocked geocode → points → forecast chain.

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

// --- WeatherConfig::default tests ---

#[test]
fn weather_config_default_uses_production_bases() {
    let cfg = WeatherConfig::default();
    assert_eq!(cfg.nominatim_base, "https://nominatim.openstreetmap.org");
    assert_eq!(cfg.nws_base, "https://api.weather.gov");
}

// --- geocode_url edge cases ---

#[test]
fn geocode_url_does_not_suffix_a_five_char_non_zip() {
    // Only an all-digit 5-char query is treated as a US zip.
    let url = geocode_url("https://nominatim.example.com", "SEAT1");
    assert_eq!(
        url,
        "https://nominatim.example.com/search?q=SEAT1&format=json&limit=1"
    );
}

#[test]
fn geocode_url_only_percent_encodes_spaces() {
    // Commas and other characters are passed through untouched.
    let url = geocode_url("https://nominatim.example.com", "Berlin,Germany");
    assert!(url.contains("q=Berlin,Germany&"), "{url}");
    assert!(!url.contains("%2C"), "{url}");
}

#[test]
fn geocode_url_encodes_every_space_in_a_zip_like_query() {
    let url = geocode_url("https://nominatim.example.com", "New  York");
    assert!(url.contains("q=New%20%20York&"), "{url}");
}

// --- points_url edge cases ---

#[test]
fn points_url_formats_whole_numbers_without_decimal_point() {
    let url = points_url("https://nws.example.com", 0.0, 0.0);
    assert_eq!(url, "https://nws.example.com/points/0,0");
}

// --- parse_nominatim_first tests ---

#[test]
fn parse_nominatim_first_takes_only_the_first_element() {
    let body = serde_json::json!([
        { "lat": "47.6062", "lon": "-122.3321" },
        { "lat": "0.0", "lon": "0.0" }
    ]);
    assert_eq!(parse_nominatim_first(&body).unwrap(), (47.6062, -122.3321));
}

#[test]
fn parse_nominatim_first_errors_on_empty_array() {
    let body = serde_json::json!([]);
    assert_eq!(
        parse_nominatim_first(&body).unwrap_err(),
        "Location not found"
    );
}

#[test]
fn parse_nominatim_first_errors_on_non_array_body() {
    let body = serde_json::json!({ "error": "boom" });
    assert_eq!(
        parse_nominatim_first(&body).unwrap_err(),
        "Location not found"
    );
}

#[test]
fn parse_nominatim_first_errors_when_lat_is_absent() {
    let body = serde_json::json!([{ "lon": "8.5" }]);
    assert_eq!(parse_nominatim_first(&body).unwrap_err(), "Missing lat");
}

#[test]
fn parse_nominatim_first_errors_when_lat_is_a_number_not_a_string() {
    // Nominatim returns coordinates as strings; a numeric lat is rejected.
    let body = serde_json::json!([{ "lat": 47.6, "lon": "8.5" }]);
    assert_eq!(parse_nominatim_first(&body).unwrap_err(), "Missing lat");
}

#[test]
fn parse_nominatim_first_errors_when_lat_is_not_numeric() {
    let body = serde_json::json!([{ "lat": "north-ish", "lon": "8.5" }]);
    assert_eq!(parse_nominatim_first(&body).unwrap_err(), "Missing lat");
}

#[test]
fn parse_nominatim_first_rejects_non_finite_lat() {
    for (case, raw) in [("nan", "NaN"), ("infinity", "inf"), ("neg-inf", "-inf")] {
        let body = serde_json::json!([{ "lat": raw, "lon": "8.5" }]);
        let err = parse_nominatim_first(&body).unwrap_err();
        assert_eq!(
            err, "Missing lat",
            "case {case}: lat {raw} must be rejected"
        );
    }
}

#[test]
fn parse_nominatim_first_errors_when_lon_is_absent() {
    let body = serde_json::json!([{ "lat": "47.6" }]);
    assert_eq!(parse_nominatim_first(&body).unwrap_err(), "Missing lon");
}

#[test]
fn parse_nominatim_first_rejects_non_finite_lon() {
    let body = serde_json::json!([{ "lat": "47.6", "lon": "NaN" }]);
    assert_eq!(parse_nominatim_first(&body).unwrap_err(), "Missing lon");
}

#[test]
fn parse_nominatim_first_accepts_negative_and_zero_coordinates() {
    let body = serde_json::json!([{ "lat": "-33.8688", "lon": "0" }]);
    assert_eq!(parse_nominatim_first(&body).unwrap(), (-33.8688, 0.0));
}

// --- geocode transport-failure tests ---

/// A loopback URL whose port was bound and immediately released, so every
/// connect is refused instantly instead of timing out.
fn closed_loopback_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

#[test]
fn geocode_reports_transport_failure_when_nominatim_is_unreachable() {
    let cfg = WeatherConfig {
        nominatim_base: closed_loopback_url(),
        nws_base: WeatherConfig::default().nws_base,
    };
    let err = geocode(&cfg, "Seattle, WA").unwrap_err();
    assert!(err.starts_with("Nominatim API error: "), "got: {err}");
}

#[test]
fn geocode_short_circuit_does_not_touch_the_network() {
    // Even with an unreachable base URL, a "lat,lon" location never
    // issues a request.
    let cfg = WeatherConfig {
        nominatim_base: closed_loopback_url(),
        nws_base: closed_loopback_url(),
    };
    let (lat, lon) = geocode(&cfg, " 47.6 , -122.3 ").unwrap();
    assert_eq!((lat, lon), (47.6, -122.3));
}

#[test]
fn geocode_falls_through_to_http_when_lat_lon_pair_is_not_numeric() {
    // "a,b" parses as a pair but neither half is numeric, so the
    // short-circuit is skipped and the HTTP path is taken.
    let cfg = WeatherConfig {
        nominatim_base: closed_loopback_url(),
        nws_base: WeatherConfig::default().nws_base,
    };
    let err = geocode(&cfg, "a,b").unwrap_err();
    assert!(err.starts_with("Nominatim API error: "), "got: {err}");
}

// --- tool_get_weather_with error-branch tests ---

/// A [`WeatherConfig`] whose Nominatim base is unreachable; the location
/// used in these tests short-circuits geocoding so only the NWS legs run.
fn nws_only_config(nws_base: String) -> WeatherConfig {
    WeatherConfig {
        nominatim_base: closed_loopback_url(),
        nws_base,
    }
}

#[test]
fn tool_get_weather_with_reports_points_transport_failure() {
    let cfg = nws_only_config(closed_loopback_url());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert!(err.starts_with("NWS Points API error: "), "got: {err}");
}

#[test]
fn tool_get_weather_with_reports_points_json_failure() {
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string("not json at all"),
        ),
    );
    let cfg = nws_only_config(mock.uri());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert!(err.starts_with("NWS Points JSON error: "), "got: {err}");
}

#[test]
fn tool_get_weather_with_errors_when_points_has_no_forecast_url() {
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(r#"{"properties": {}}"#),
        ),
    );
    let cfg = nws_only_config(mock.uri());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert_eq!(err, "Could not find forecast URL in NWS response");
}

#[test]
fn tool_get_weather_with_errors_when_forecast_url_is_not_a_string() {
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(r#"{"properties": {"forecast": 42}}"#),
        ),
    );
    let cfg = nws_only_config(mock.uri());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert_eq!(err, "Could not find forecast URL in NWS response");
}

#[test]
fn tool_get_weather_with_reports_forecast_transport_failure() {
    let mock = WiremockGuard::start();
    // The points response points the client at a dead port.
    let dead = closed_loopback_url();
    let points_body = format!(r#"{{"properties": {{"forecast": "{dead}/forecast"}}}}"#);
    mock.register(
        Mock::given(method("GET"))
            .and(path("/points/47.6,-122.3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(points_body),
            ),
    );
    let cfg = nws_only_config(mock.uri());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert!(err.starts_with("NWS Forecast API error: "), "got: {err}");
}

#[test]
fn tool_get_weather_with_reports_forecast_json_failure() {
    let mock = WiremockGuard::start();
    let points_body = format!(
        r#"{{"properties": {{"forecast": "{}/forecast"}}}}"#,
        mock.uri()
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/points/47.6,-122.3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(points_body),
            ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/forecast"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("{oops"),
            ),
    );
    let cfg = nws_only_config(mock.uri());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert!(err.starts_with("NWS Forecast JSON error: "), "got: {err}");
}

#[test]
fn tool_get_weather_with_errors_when_forecast_has_no_periods() {
    let mock = WiremockGuard::start();
    let points_body = format!(
        r#"{{"properties": {{"forecast": "{}/forecast"}}}}"#,
        mock.uri()
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/points/47.6,-122.3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(points_body),
            ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/forecast"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(r#"{"properties": {"periods": {}}}"#),
            ),
    );
    let cfg = nws_only_config(mock.uri());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert_eq!(err, "Could not find periods in NWS forecast response");
}

#[test]
fn tool_get_weather_with_errors_when_periods_array_is_empty() {
    let mock = WiremockGuard::start();
    let points_body = format!(
        r#"{{"properties": {{"forecast": "{}/forecast"}}}}"#,
        mock.uri()
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/points/47.6,-122.3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(points_body),
            ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/forecast"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(r#"{"properties": {"periods": []}}"#),
            ),
    );
    let cfg = nws_only_config(mock.uri());
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap_err();
    assert_eq!(err, "No forecast periods found.");
}

/// Register a points → forecast chain whose forecast carries `periods_json`.
fn register_forecast_periods(mock: &WiremockGuard, periods_json: &str) {
    let points_body = format!(
        r#"{{"properties": {{"forecast": "{}/forecast"}}}}"#,
        mock.uri()
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/points/47.6,-122.3"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(points_body),
            ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(path("/forecast"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(format!(
                        r#"{{"properties": {{"periods": {periods_json}}}}}"#
                    )),
            ),
    );
}

#[test]
fn tool_get_weather_with_defaults_missing_period_fields() {
    let mock = WiremockGuard::start();
    // A period with no name / temperature / unit / forecast at all.
    register_forecast_periods(&mock, r#"[{"startTime": "2026-07-19T10:00:00Z"}]"#);
    let cfg = nws_only_config(mock.uri());

    let result = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.result).expect("valid JSON");
    let arr = parsed.as_array().expect("array of periods");
    assert_eq!(arr.len(), 1, "got: {}", result.result);
    assert_eq!(arr[0]["period_name"], "");
    assert_eq!(arr[0]["start_time"], "2026-07-19T10:00:00Z");
    assert_eq!(arr[0]["temperature"], "0 ");
    assert_eq!(arr[0]["detailed_forecast"], "");
}

#[test]
fn tool_get_weather_with_ignores_non_string_period_fields() {
    let mock = WiremockGuard::start();
    register_forecast_periods(
        &mock,
        r#"[{"startTime": "2026-07-19T10:00:00Z", "name": 7, "temperature": "cold",
             "temperatureUnit": 70, "detailedForecast": null}]"#,
    );
    let cfg = nws_only_config(mock.uri());

    let result = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.result).expect("valid JSON");
    let arr = parsed.as_array().expect("array of periods");
    assert_eq!(arr[0]["period_name"], "");
    assert_eq!(arr[0]["temperature"], "0 ");
    assert_eq!(arr[0]["detailed_forecast"], "");
}

#[test]
fn tool_get_weather_with_matches_a_date_range_embedded_in_longer_text() {
    let mock = WiremockGuard::start();
    register_forecast_periods(
        &mock,
        r#"[{"startTime": "2026-07-19T10:00:00Z", "name": "Today", "temperature": 75,
             "temperatureUnit": "F", "detailedForecast": "Sunny"}]"#,
    );
    let cfg = nws_only_config(mock.uri());

    // The range string is longer than the ISO date prefix, so the match
    // goes through the `dr.contains(&start[..10])` branch.
    let result = tool_get_weather_with(&cfg, "47.6,-122.3", Some("on 2026-07-19 pm")).unwrap();
    assert!(result.result.contains("Today"), "got: {}", result.result);
}

#[test]
fn tool_get_weather_with_date_range_is_case_insensitive_for_the_query() {
    let mock = WiremockGuard::start();
    register_forecast_periods(
        &mock,
        r#"[{"startTime": "2026-07-19T10:00:00Z", "name": "Today", "temperature": 75,
             "temperatureUnit": "F", "detailedForecast": "Sunny"}]"#,
    );
    let cfg = nws_only_config(mock.uri());

    // A non-numeric range never matches, so the filtered result is empty.
    let err = tool_get_weather_with(&cfg, "47.6,-122.3", Some("TODAY")).unwrap_err();
    assert!(err.contains("date range 'today'"), "got: {err}");
}

#[test]
fn tool_get_weather_with_keeps_every_period_matching_the_range() {
    let mock = WiremockGuard::start();
    register_forecast_periods(
        &mock,
        r#"[{"startTime": "2026-07-19T06:00:00Z", "name": "Morning", "temperature": 60,
             "temperatureUnit": "F", "detailedForecast": "Clear"},
            {"startTime": "2026-07-19T18:00:00Z", "name": "Evening", "temperature": 70,
             "temperatureUnit": "F", "detailedForecast": "Cloudy"},
            {"startTime": "2026-07-20T06:00:00Z", "name": "Tomorrow", "temperature": 65,
             "temperatureUnit": "F", "detailedForecast": "Rain"}]"#,
    );
    let cfg = nws_only_config(mock.uri());

    let result = tool_get_weather_with(&cfg, "47.6,-122.3", Some("2026-07-19")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.result).expect("valid JSON");
    let arr = parsed.as_array().expect("array of periods");
    assert_eq!(arr.len(), 2, "got: {}", result.result);
    assert_eq!(arr[0]["period_name"], "Morning");
    assert_eq!(arr[1]["period_name"], "Evening");
}

#[test]
fn tool_get_weather_with_period_without_start_time_is_kept_when_unfiltered() {
    let mock = WiremockGuard::start();
    register_forecast_periods(
        &mock,
        r#"[{"name": "No Start", "temperature": 10, "temperatureUnit": "C"}]"#,
    );
    let cfg = nws_only_config(mock.uri());

    let result = tool_get_weather_with(&cfg, "47.6,-122.3", None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.result).expect("valid JSON");
    let arr = parsed.as_array().expect("array of periods");
    assert_eq!(arr.len(), 1, "got: {}", result.result);
    assert_eq!(arr[0]["start_time"], "");
    assert_eq!(arr[0]["temperature"], "10 C");
}

#[test]
fn tool_get_weather_with_filtered_search_drops_a_period_without_start_time() {
    let mock = WiremockGuard::start();
    register_forecast_periods(
        &mock,
        r#"[{"name": "No Start", "temperature": 10, "temperatureUnit": "C"}]"#,
    );
    let cfg = nws_only_config(mock.uri());

    let err = tool_get_weather_with(&cfg, "47.6,-122.3", Some("2026-07-19")).unwrap_err();
    assert!(err.contains("No weather data found"), "got: {err}");
}
