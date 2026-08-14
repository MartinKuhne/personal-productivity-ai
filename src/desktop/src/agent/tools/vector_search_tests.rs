//! Unit tests for the vector-search tool contract.

use super::*;

#[test]
fn input_limit_is_bounded_by_tool() {
    assert_eq!(default_limit(), 5);
    assert_eq!(
        serde_json::from_str::<VectorSearchInput>(r#"{"query":"x"}"#)
            .unwrap()
            .limit,
        5
    );
}
