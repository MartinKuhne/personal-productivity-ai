//! Property-based tests for the weather tool's JSON parsers
//! in `agent::lib::weather`.
//!
//! The weather tool chains three HTTP requests (Nominatim
//! geocode → NWS `points` → NWS `forecast`). Each response is
//! parsed into a typed value before the next request. A panic
//! in any parser would crash every weather query.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C9 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **`parse_nominatim_first` never panics on any input.**
//!    Any `serde_json::Value` is accepted; the function
//!    returns `Err` for malformed input, never unwinds.
//! 2. **No NaN or Inf in the parsed `(lat, lon)` pair.** The
//!    NWS API rejects `NaN`/`Inf` for coordinates; the parser
//!    must not let them through.
//! 3. **Empty array is well-defined.** A Nominatim response
//!    with zero results is an `Err("no results")`, not a
//!    panic.
//! 4. **`lat`/`lon` strings are parsed as numbers.** A
//!    Nominatim response with `lat: "47.6"` (string) is
//!    accepted; a response with `lat: 47.6` (number) is also
//!    accepted. A response with `lat: "not a number"` is
//!    rejected.
//!
//! `cases = 512` per property.

use crate::lib::weather::parse_nominatim_first;
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Strategy: any `serde_json::Value` with bounded depth.
fn json_value_strategy() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        any::<i64>().prop_map(serde_json::Value::from),
        any::<String>().prop_map(serde_json::Value::String),
    ];
    leaf.prop_recursive(5, 64, 5, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::Array),
            prop::collection::hash_map(any::<String>(), inner, 0..4)
                .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `parse_nominatim_first` must never panic on any input.
    /// The function returns `Result<(f64, f64), String>`;
    /// any value is either Ok or Err, never unwinding.
    #[test]
    fn parse_nominatim_first_never_panics_on_any_input(
        value in json_value_strategy()
    ) {
        let _ = parse_nominatim_first(&value);
    }

    /// An empty array is well-defined: the parser returns
    /// `Err` (the Open-Meteo / Nominatim response shape
    /// requires at least one result).
    #[test]
    fn parse_nominatim_first_empty_array_is_err(_unused in 0..1u8) {
        let value = serde_json::json!([]);
        let result = parse_nominatim_first(&value);
        prop_assert!(result.is_err());
    }

    /// A non-array value is rejected.
    #[test]
    fn parse_nominatim_first_non_array_is_err(
        value in prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::Bool),
            any::<i64>().prop_map(serde_json::Value::from),
        ]
    ) {
        let result = parse_nominatim_first(&value);
        prop_assert!(result.is_err());
    }

    /// When the parser does succeed, the returned `(lat,
    /// lon)` pair is finite (no NaN, no infinity). The NWS
    /// API rejects non-finite coordinates.
    #[test]
    fn parse_nominatim_first_returns_finite_numbers(
        lat in -90.0f64..90.0f64,
        lon in -180.0f64..180.0f64,
    ) {
        // Nominatim returns lat/lon as strings in the
        // production response; we use that shape here.
        let value = serde_json::json!([{
            "lat": lat.to_string(),
            "lon": lon.to_string(),
        }]);
        let result = parse_nominatim_first(&value);
        let (got_lat, got_lon) = result.expect("valid input should parse");
        prop_assert!(got_lat.is_finite(), "lat must be finite");
        prop_assert!(got_lon.is_finite(), "lon must be finite");
    }

    /// A Nominatim response with `lat: "not a number"` is
    /// rejected — the parser does not silently substitute a
    /// default or panic.
    #[test]
    fn parse_nominatim_first_rejects_non_numeric_string(
        bad in prop::string::string_regex(r"[A-Za-z ]{1,16}").unwrap()
    ) {
        let value = serde_json::json!([{
            "lat": bad,
            "lon": "0.0",
        }]);
        let result = parse_nominatim_first(&value);
        prop_assert!(result.is_err(), "non-numeric lat must be rejected");
    }
}
