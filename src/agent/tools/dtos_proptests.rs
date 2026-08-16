//! Property-based tests for the tool input DTOs in `tools::dtos`.
//!
//! The LLM tool-loop passes a `String` of JSON for every
//! `tool_calls[*].function.arguments`. The string is deserialised into
//! the per-tool input DTO via `serde_json::from_str` (or, after the
//! `[agent]`: dispatch refactor, `serde_json::from_value` on a parsed
//! `Value`). The DTOs are the **trust boundary** between the untrusted
//! LLM and the rest of the agent; a panic in the deserialiser, or a
//! non-round-trippable DTO, is a denial-of-service vector.
//!
//! Phase 1 of the fuzzing plan (`doc/planning/fuzzing.md`) layers three
//! properties on every DTO:
//!
//! 1. **No panic on garbage JSON.** A random byte string handed to
//!    `serde_json::from_str::<T>(s)` returns `Result`; it never unwinds.
//!    A regression that lets an attacker reach `unwrap()` on the
//!    deserialiser path is caught immediately.
//! 2. **No panic on garbage `Value`.** Same as #1 but the input is a
//!    proptest-generated `serde_json::Value` (deeply nested, wrong
//!    types, etc.). Catches panics in the `from_value` path that
//!    `from_str` would mask.
//! 3. **Round-trip identity.** A `serde_json::Value` that successfully
//!    deserialises into `T` re-serialises to the same `Value` after a
//!    `from_value`/`to_value` round-trip. Catches a regression that
//!    silently drops fields (e.g. an accidentally-removed
//!    `#[serde(default)]`). Only applicable to DTOs that derive
//!    `Serialize` (a handful today; the rest skip this property and
//!    rely on the no-panic checks).
//!
//! Every input DTO listed in `dtos.rs` is covered. Output DTOs are not
//! covered here because they are produced by the Rust code, not parsed
//! from the LLM, and the equivalent guarantee is provided by the
//! `serde_json::to_string(&value).is_ok()` call in `execute_tool`
//! (which `unwrap_or_else` to a static error string).
//!
//! `cases = 1024` per property per DTO is large enough to surface
//! regressions in less-common shapes but small enough that the entire
//! sidecar finishes in well under 5 seconds.

use crate::tools::dtos::*;
use proptest::prelude::*;
use serde_json::Value;

/// One proptest case count for every property on every DTO. The
/// plan's acceptance criterion is `cases = 1024`.
const CASES: u32 = 1024;

/// Per-DTO strategy: an arbitrary `serde_json::Value`. We use
/// `any::<Value>()` because (a) the input is untrusted LLM JSON, (b)
/// the structure of an `any::<Value>()` is wide enough to exercise
/// every field type the DTO declares, and (c) it shrinks to a small
/// failing input on regression.
///
/// Depth is capped at 8 to keep the round-trip test fast (a fully
/// recursive `Value` can blow the input string up to multi-MB).
fn json_value_strategy() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<String>().prop_map(Value::String),
    ];
    leaf.prop_recursive(8, 256, 8, |inner| {
        prop_oneof![
            // Vec of depth-1-deeper values; the recursion budget
            // proptest gives us is enough for arrays of arrays.
            prop::collection::vec(inner.clone(), 0..8).prop_map(Value::Array),
            // Map of string keys to depth-1-deeper values.
            prop::collection::hash_map(any::<String>(), inner, 0..8)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Strategy: a random byte string, up to 256 bytes, for the
/// "any-bytes-into-from_str" property. The bytes are valid UTF-8 (we
/// use `String` rather than `&[u8]` because `serde_json::from_str`
/// takes `&str`); the printable-ASCII range covers the same surface
/// as the in-corpus real-LLM output.
fn utf8_byte_string_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F]{0,256}").unwrap()
}

/// Apply a `*_no_panic_on_garbage_json` property to one DTO type.
/// Macro because proptest! requires `Type` literals and we want a
/// single test name per DTO.
macro_rules! no_panic_on_garbage_str {
    ($name:ident, $ty:ty) => {
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(CASES))]

            #[test]
            fn $name(s in utf8_byte_string_strategy()) {
                // The contract: deserialise or fail. Never panic.
                let _ = serde_json::from_str::<$ty>(&s);
            }
        }
    };
}

macro_rules! no_panic_on_garbage_value {
    ($name:ident, $ty:ty) => {
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(CASES))]

            #[test]
            fn $name(v in json_value_strategy()) {
                // Same contract as the string variant but for the
                // `from_value` path used when the agent loop has
                // already parsed the JSON into a `Value`.
                let _ = serde_json::from_value::<$ty>(v);
            }
        }
    };
}

/// Apply a `*_roundtrips` property. Only callable for DTOs that
/// derive both `Deserialize` and `Serialize` (the input DTOs that
/// also serialise to be echoed back in the agent loop). DTOs that
/// are `Deserialize`-only are not round-trippable and skip this
/// property — the no-panic properties still cover them.
///
/// The invariant under test is **stable `Value` round-trip**: a
/// `Value` that the DTO accepts, when serialised back to a `Value`
/// and re-parsed, must re-serialise to the same `Value`. This
/// catches asymmetric serialise/deserialise (a field accepted on
/// the way in but not emitted on the way out, or a type change
/// between the two) without flagging expected `serde` behaviour
/// (e.g. unknown JSON object keys being silently dropped, or
/// empty-string keys matching no field). Those cases are not bugs
/// in our DTOs; they are how `serde` is specified to work.
///
/// We compare on `Value`, not on the DTO, so the DTO doesn't need
/// to implement `PartialEq` (only the DTOs that already do — the
/// `*Response` structs — currently derive it).
macro_rules! roundtrips_through_value {
    ($name:ident, $ty:ty) => {
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(CASES))]

            #[test]
            fn $name(v in json_value_strategy()) {
                // If the value deserialises into the DTO, the
                // round-trip `to_value` -> `from_value` -> `to_value`
                // must yield the same `Value` as the first
                // `to_value`. A divergence here means the
                // serialise and deserialise sides of the DTO are
                // out of sync (e.g. a field was renamed on one
                // side but not the other, or a serialised
                // representation cannot be re-parsed).
                if let Ok(parsed1) = serde_json::from_value::<$ty>(v) {
                    let after_first =
                        serde_json::to_value(&parsed1)
                            .expect("DTO Serialise must not fail for a DTO we just deserialised");
                    let parsed2 = serde_json::from_value::<$ty>(after_first.clone())
                        .expect("DTO re-parse must not fail for a value we just serialised");
                    let after_second = serde_json::to_value(&parsed2)
                        .expect("DTO Serialise must not fail for a DTO we just re-deserialised");
                    prop_assert_eq!(
                        after_first, after_second,
                        "DTO round-trip is not stable: a value diverged between two trips"
                    );
                }
            }
        }
    };
}

// ---------------------------------------------------------------------------
// File-system tools
// ---------------------------------------------------------------------------

no_panic_on_garbage_str!(fs_search_notes_no_panic, SearchNotesInput);
no_panic_on_garbage_value!(fs_search_notes_value_no_panic, SearchNotesInput);

no_panic_on_garbage_str!(fs_read_tags_no_panic, ReadTagsInput);
no_panic_on_garbage_value!(fs_read_tags_value_no_panic, ReadTagsInput);

no_panic_on_garbage_str!(fs_list_notes_by_tag_no_panic, ListNotesByTagInput);
no_panic_on_garbage_value!(fs_list_notes_by_tag_value_no_panic, ListNotesByTagInput);

no_panic_on_garbage_str!(fs_list_notes_no_panic, ListNotesInput);
no_panic_on_garbage_value!(fs_list_notes_value_no_panic, ListNotesInput);

no_panic_on_garbage_str!(fs_read_note_no_panic, ReadNoteInput);
no_panic_on_garbage_value!(fs_read_note_value_no_panic, ReadNoteInput);

no_panic_on_garbage_str!(fs_window_note_no_panic, WindowNoteInput);
no_panic_on_garbage_value!(fs_window_note_value_no_panic, WindowNoteInput);

no_panic_on_garbage_str!(fs_create_note_no_panic, CreateNoteInput);
no_panic_on_garbage_value!(fs_create_note_value_no_panic, CreateNoteInput);

no_panic_on_garbage_str!(fs_insert_into_note_no_panic, InsertIntoNoteInput);
no_panic_on_garbage_value!(fs_insert_into_note_value_no_panic, InsertIntoNoteInput);

no_panic_on_garbage_str!(fs_patch_note_no_panic, PatchNoteInput);
no_panic_on_garbage_value!(fs_patch_note_value_no_panic, PatchNoteInput);

no_panic_on_garbage_str!(fs_move_note_no_panic, MoveNoteInput);
no_panic_on_garbage_value!(fs_move_note_value_no_panic, MoveNoteInput);

no_panic_on_garbage_str!(fs_read_yaml_header_no_panic, ReadYamlHeaderInput);
no_panic_on_garbage_value!(fs_read_yaml_header_value_no_panic, ReadYamlHeaderInput);

no_panic_on_garbage_str!(fs_write_yaml_header_no_panic, WriteYamlHeaderInput);
no_panic_on_garbage_value!(fs_write_yaml_header_value_no_panic, WriteYamlHeaderInput);

// ---------------------------------------------------------------------------
// Web tools
// ---------------------------------------------------------------------------

no_panic_on_garbage_str!(web_fetch_no_panic, WebFetchInput);
no_panic_on_garbage_value!(web_fetch_value_no_panic, WebFetchInput);

no_panic_on_garbage_str!(web_search_no_panic, WebSearchInput);
no_panic_on_garbage_value!(web_search_value_no_panic, WebSearchInput);

no_panic_on_garbage_str!(web_delegate_no_panic, WebDelegateInput);
no_panic_on_garbage_value!(web_delegate_value_no_panic, WebDelegateInput);

// ---------------------------------------------------------------------------
// Calendar tools (CalDAV)
// ---------------------------------------------------------------------------

no_panic_on_garbage_str!(cal_search_calendar_no_panic, SearchCalendarInput);
no_panic_on_garbage_value!(cal_search_calendar_value_no_panic, SearchCalendarInput);

no_panic_on_garbage_str!(cal_get_calendar_no_panic, GetCalendarInput);
no_panic_on_garbage_value!(cal_get_calendar_value_no_panic, GetCalendarInput);

no_panic_on_garbage_str!(cal_get_calendar_item_no_panic, GetCalendarItemInput);
no_panic_on_garbage_value!(cal_get_calendar_item_value_no_panic, GetCalendarItemInput);

// `AddCalendarItemInput` and `UpdateCalendarItemInput` derive
// `Serialize` (the agent loop can echo them back), so they get the
// round-trip property too.
no_panic_on_garbage_str!(cal_add_calendar_item_no_panic, AddCalendarItemInput);
no_panic_on_garbage_value!(cal_add_calendar_item_value_no_panic, AddCalendarItemInput);
roundtrips_through_value!(cal_add_calendar_item_roundtrips, AddCalendarItemInput);

no_panic_on_garbage_str!(cal_update_calendar_item_no_panic, UpdateCalendarItemInput);
no_panic_on_garbage_value!(
    cal_update_calendar_item_value_no_panic,
    UpdateCalendarItemInput
);
roundtrips_through_value!(cal_update_calendar_item_roundtrips, UpdateCalendarItemInput);

no_panic_on_garbage_str!(cal_delete_calendar_item_no_panic, DeleteCalendarItemInput);
no_panic_on_garbage_value!(
    cal_delete_calendar_item_value_no_panic,
    DeleteCalendarItemInput
);

// ---------------------------------------------------------------------------
// Email tools (JMAP)
// ---------------------------------------------------------------------------

no_panic_on_garbage_str!(email_search_no_panic, SearchEmailInput);
no_panic_on_garbage_value!(email_search_value_no_panic, SearchEmailInput);

no_panic_on_garbage_str!(email_get_by_id_no_panic, GetEmailByIdInput);
no_panic_on_garbage_value!(email_get_by_id_value_no_panic, GetEmailByIdInput);

no_panic_on_garbage_str!(email_send_no_panic, SendEmailInput);
no_panic_on_garbage_value!(email_send_value_no_panic, SendEmailInput);

no_panic_on_garbage_str!(email_delete_no_panic, DeleteEmailInput);
no_panic_on_garbage_value!(email_delete_value_no_panic, DeleteEmailInput);

// ---------------------------------------------------------------------------
// Contact tools (CardDAV)
// ---------------------------------------------------------------------------

no_panic_on_garbage_str!(contact_search_no_panic, SearchContactInput);
no_panic_on_garbage_value!(contact_search_value_no_panic, SearchContactInput);

no_panic_on_garbage_str!(contact_get_no_panic, GetContactInput);
no_panic_on_garbage_value!(contact_get_value_no_panic, GetContactInput);

// `AddContactInput` and `UpdateContactInput` derive `Serialize`.
// `AddressInput` (the addresses sub-DTO) is nested inside both; it
// also derives `Serialize` and is exercised by the round-trip
// property through the parent DTOs, but a direct property is
// cheap and pins the sub-DTO's contract.
no_panic_on_garbage_str!(contact_add_no_panic, AddContactInput);
no_panic_on_garbage_value!(contact_add_value_no_panic, AddContactInput);
roundtrips_through_value!(contact_add_roundtrips, AddContactInput);

no_panic_on_garbage_str!(contact_update_no_panic, UpdateContactInput);
no_panic_on_garbage_value!(contact_update_value_no_panic, UpdateContactInput);
roundtrips_through_value!(contact_update_roundtrips, UpdateContactInput);

no_panic_on_garbage_str!(contact_delete_no_panic, DeleteContactInput);
no_panic_on_garbage_value!(contact_delete_value_no_panic, DeleteContactInput);

// `AddressInput` sub-DTO: round-trip is a cheap additional check.
no_panic_on_garbage_str!(contact_address_no_panic, AddressInput);
no_panic_on_garbage_value!(contact_address_value_no_panic, AddressInput);
roundtrips_through_value!(contact_address_roundtrips, AddressInput);

// ---------------------------------------------------------------------------
// Weather tool
// ---------------------------------------------------------------------------

no_panic_on_garbage_str!(weather_get_no_panic, GetWeatherInput);
no_panic_on_garbage_value!(weather_get_value_no_panic, GetWeatherInput);

// ---------------------------------------------------------------------------
// Browser tools — gated on the `browser` feature. Compiled out
// otherwise so the sidecar stays green when the feature is off.
// ---------------------------------------------------------------------------

#[cfg(feature = "browser")]
mod browser_dtos {
    use super::*;

    no_panic_on_garbage_str!(browser_navigate_no_panic, BrowserNavigateInput);
    no_panic_on_garbage_value!(browser_navigate_value_no_panic, BrowserNavigateInput);

    no_panic_on_garbage_str!(browser_click_no_panic, BrowserClickInput);
    no_panic_on_garbage_value!(browser_click_value_no_panic, BrowserClickInput);

    no_panic_on_garbage_str!(browser_fill_input_no_panic, BrowserFillInputInput);
    no_panic_on_garbage_value!(browser_fill_input_value_no_panic, BrowserFillInputInput);

    no_panic_on_garbage_str!(browser_select_dropdown_no_panic, BrowserSelectDropdownInput);
    no_panic_on_garbage_value!(
        browser_select_dropdown_value_no_panic,
        BrowserSelectDropdownInput
    );

    no_panic_on_garbage_str!(browser_press_key_no_panic, BrowserPressKeyInput);
    no_panic_on_garbage_value!(browser_press_key_value_no_panic, BrowserPressKeyInput);

    no_panic_on_garbage_str!(browser_evaluate_js_no_panic, BrowserEvaluateJsInput);
    no_panic_on_garbage_value!(browser_evaluate_js_value_no_panic, BrowserEvaluateJsInput);

    no_panic_on_garbage_str!(browser_screenshot_no_panic, BrowserScreenshotInput);
    no_panic_on_garbage_value!(browser_screenshot_value_no_panic, BrowserScreenshotInput);
}
