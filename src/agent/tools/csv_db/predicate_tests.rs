use super::*;
use std::collections::HashMap;

fn make_ctx(items: Vec<(&str, Value)>) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    for (k, v) in items {
        map.insert(k.to_string(), v);
    }
    map
}

#[test]
fn test_tautology_and_contradiction() {
    let ctx = HashMap::new();

    let p1 = Predicate::parse("1 == 1").unwrap();
    assert_eq!(p1.eval_boolean(&ctx).unwrap(), true);

    let p2 = Predicate::parse("true").unwrap();
    assert_eq!(p2.eval_boolean(&ctx).unwrap(), true);

    let p3 = Predicate::parse("1 == 0").unwrap();
    assert_eq!(p3.eval_boolean(&ctx).unwrap(), false);

    let p4 = Predicate::parse("false").unwrap();
    assert_eq!(p4.eval_boolean(&ctx).unwrap(), false);
}

#[test]
fn test_numeric_comparisons() {
    let ctx = make_ctx(vec![("price", Value::Float(1.5)), ("qty", Value::Int(10))]);

    let p = Predicate::parse("price < 2.0").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("price > 2.0").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), false);

    let p = Predicate::parse("qty >= 10").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("qty <= 9").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), false);

    let p = Predicate::parse("qty != 5").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("qty == 10").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);
}

#[test]
fn test_mixed_int_and_float_comparisons() {
    let ctx = make_ctx(vec![
        ("int_val", Value::Int(5)),
        ("float_val", Value::Float(5.0)),
    ]);

    let p = Predicate::parse("int_val == float_val").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("int_val == 5.0").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("float_val == 5").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("int_val < 5.5").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("float_val > 4").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);
}

#[test]
fn test_string_comparisons() {
    let ctx = make_ctx(vec![("item", Value::String("apple".to_string()))]);

    let p = Predicate::parse("item == \"apple\"").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("item == 'apple'").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("item != \"banana\"").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("item == \"banana\"").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), false);
}

#[test]
fn test_logical_and_or_not() {
    let ctx = make_ctx(vec![
        ("item", Value::String("apple".to_string())),
        ("price", Value::Float(1.5)),
        ("qty", Value::Int(10)),
    ]);

    let p = Predicate::parse("item == \"apple\" && price < 2.0").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("item == \"apple\" and price > 2.0").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), false);

    let p = Predicate::parse("item == \"banana\" || qty == 10").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("item == \"banana\" or qty == 5").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), false);

    let p = Predicate::parse("!(item == \"banana\")").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("not (price > 5.0)").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);
}

#[test]
fn test_parenthesized_grouping() {
    let ctx = make_ctx(vec![
        ("a", Value::Bool(true)),
        ("b", Value::Bool(false)),
        ("c", Value::Bool(false)),
    ]);

    // true || (false && false) -> true
    let p = Predicate::parse("a || (b && c)").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    // (true || false) && false -> false
    let p = Predicate::parse("(a || b) && c").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), false);
}

#[test]
fn test_unbound_variable_returns_evaluation_error() {
    let ctx = make_ctx(vec![("name", Value::String("alice".to_string()))]);

    let p = Predicate::parse("missing_col == 1").unwrap();
    let err = p.eval_boolean(&ctx).unwrap_err();
    assert!(
        err.contains("Evaluation error"),
        "Expected 'Evaluation error', got: {err}"
    );
    assert!(
        err.contains("missing_col"),
        "Expected error to mention 'missing_col', got: {err}"
    );
}

#[test]
fn test_syntax_errors_return_invalid_predicate() {
    let cases = vec![
        "",
        "   ",
        "invalid syntax ++",
        "item == ",
        "== 5",
        "item = 'single_equals'",
        "'unclosed string",
        "(unbalanced paren",
        "a & b",
        "a | b",
    ];

    for expr in cases {
        let res = Predicate::parse(expr);
        assert!(
            res.is_err(),
            "Expected expression to fail parsing: '{expr}'"
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("Invalid predicate"),
            "Expected 'Invalid predicate' for '{expr}', got: {err}"
        );
    }
}

#[test]
fn test_arithmetic_in_predicate() {
    let ctx = make_ctx(vec![("qty", Value::Int(10)), ("price", Value::Float(2.5))]);

    let p = Predicate::parse("qty + 5 == 15").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("qty * price == 25.0").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("qty / 2 == 5").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);

    let p = Predicate::parse("-5 < 0").unwrap();
    assert_eq!(p.eval_boolean(&ctx).unwrap(), true);
}

#[test]
fn test_non_boolean_evaluation_result_fails() {
    let ctx = HashMap::new();
    let p = Predicate::parse("1 + 2").unwrap();
    let err = p.eval_boolean(&ctx).unwrap_err();
    assert!(
        err.contains("Evaluation error"),
        "Expected 'Evaluation error', got: {err}"
    );
}
