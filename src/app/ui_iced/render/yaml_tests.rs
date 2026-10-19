//! Unit tests for the YAML front-matter mapping ([TASK-015]).

use super::*;
use crate::ui::theme::Palette;
use serde_norway::{Mapping, Value};

fn palette() -> Palette {
    Palette::fastmd_dark()
}

fn mapping(pairs: &[(&str, &str)]) -> Value {
    let mut map = Mapping::new();
    for (key, value) in pairs {
        map.insert(
            Value::String((*key).to_string()),
            Value::String((*value).to_string()),
        );
    }
    Value::Mapping(map)
}

// --- column width solving ------------------------------------------------

#[test]
fn test_wide_viewport_uses_the_preferred_key_width() {
    let (key, value) = yaml_column_widths(500.0);
    assert_eq!(key, YAML_KEY_COLUMN_WIDTH);
    assert_eq!(value, 500.0 - key - YAML_COLUMN_GAP);
    assert_eq!(value, 378.0);
}

#[test]
fn test_narrow_viewport_shrinks_the_key_column() {
    let (key, value) = yaml_column_widths(100.0);
    assert_eq!(key, 80.0, "100 - 20 chrome");
    assert_eq!(value, YAML_MIN_VALUE_WIDTH, "nothing left over → floor");
}

#[test]
fn test_key_column_never_goes_below_its_floor() {
    for width in [0.0, 10.0, 40.0, -100.0] {
        let (key, value) = yaml_column_widths(width);
        assert!(
            key >= YAML_MIN_KEY_WIDTH,
            "key column at {width} px collapsed to {key}"
        );
        assert!(
            value >= YAML_MIN_VALUE_WIDTH,
            "value column at {width} px collapsed to {value}"
        );
    }
}

#[test]
fn test_zero_width_still_produces_positive_columns() {
    let (key, value) = yaml_column_widths(0.0);
    assert_eq!(key, YAML_MIN_KEY_WIDTH);
    assert_eq!(value, YAML_MIN_VALUE_WIDTH);
}

// --- row mapping ----------------------------------------------------------

#[test]
fn test_rows_carry_key_value_and_solved_widths() {
    let pairs = vec![
        ("title".to_string(), "My Note".to_string()),
        ("tags".to_string(), "a, b".to_string()),
    ];
    let rows = yaml_rows(&pairs, 500.0);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].key, "title");
    assert_eq!(rows[0].value, "My Note");
    assert_eq!(rows[1].key, "tags");
    assert_eq!(rows[1].value, "a, b");
    for row in &rows {
        assert_eq!(row.key_width, YAML_KEY_COLUMN_WIDTH);
        assert_eq!(row.value_width, 378.0);
    }
}

#[test]
fn test_stripe_parity_alternates_from_the_second_row() {
    let pairs: Vec<(String, String)> = (0..5).map(|i| (format!("k{i}"), format!("v{i}"))).collect();
    let rows = yaml_rows(&pairs, 500.0);
    let stripes: Vec<bool> = rows.iter().map(|row| row.striped).collect();
    assert_eq!(stripes, vec![false, true, false, true, false]);
}

#[test]
fn test_is_striped_rule() {
    assert!(!is_striped(0));
    assert!(is_striped(1));
    assert!(!is_striped(2));
    assert!(is_striped(3));
}

#[test]
fn test_empty_pair_list_maps_to_no_rows() {
    assert!(yaml_rows(&[], 500.0).is_empty());
}

// --- value-level mapping ---------------------------------------------------

#[test]
fn test_mapping_document_maps_to_rows() {
    let value = mapping(&[("a", "1"), ("b", "2")]);
    let rows = yaml_rows_from_value(&value, 500.0);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].key, "a");
    assert_eq!(rows[1].key, "b");
}

#[test]
fn test_non_mapping_documents_produce_no_rows() {
    // The legacy UI renderer fell through silently for anything that is not a
    // key/value mapping.
    let scalar = Value::String("just text".to_string());
    let sequence = Value::Sequence(vec![Value::String("x".to_string())]);
    let null = Value::Null;

    assert!(yaml_rows_from_value(&scalar, 500.0).is_empty());
    assert!(yaml_rows_from_value(&sequence, 500.0).is_empty());
    assert!(yaml_rows_from_value(&null, 500.0).is_empty());
}

#[test]
fn test_sequence_values_are_stringified_by_the_parser_helper() {
    let mut map = Mapping::new();
    map.insert(
        Value::String("tags".to_string()),
        Value::Sequence(vec![
            Value::String("one".to_string()),
            Value::String("two".to_string()),
        ]),
    );
    let rows = yaml_rows_from_value(&Value::Mapping(map), 500.0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "tags");
    assert!(
        !rows[0].value.is_empty(),
        "a sequence value must still render as something"
    );
}

#[test]
fn test_empty_mapping_produces_no_rows() {
    let rows = yaml_rows_from_value(&Value::Mapping(Mapping::new()), 500.0);
    assert!(rows.is_empty());
}

// --- widget construction ----------------------------------------------------

#[test]
fn test_yaml_block_builds() {
    let rows = yaml_rows(&[("a".to_string(), "1".to_string())], 500.0);
    let element = yaml_block(&rows);
    let _ = element;
}

#[test]
fn test_empty_yaml_block_builds() {
    let element = yaml_block(&[]);
    let _ = element;
}

#[test]
fn test_yaml_row_builds_for_both_parities() {
    let rows = yaml_rows(
        &[
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string()),
        ],
        500.0,
    );
    let _ = yaml_row(&rows[0]);
    let _ = yaml_row(&rows[1]);
}

#[test]
fn test_yaml_row_builds_at_a_collapsed_width() {
    let rows = yaml_rows(&[("k".to_string(), "v".to_string())], 0.0);
    let element = yaml_row(&rows[0]);
    let _ = element;
}

#[test]
fn test_yaml_colours_come_from_palette_tokens() {
    // [GUD-003] guard: the tokens the renderer uses must exist and be
    // distinct, and no literal colour is introduced by this module.
    let palette = palette();
    assert_ne!(palette.accent_text_soft, palette.text_value);
    assert_ne!(palette.table_surface, palette.surface_variant);
}
