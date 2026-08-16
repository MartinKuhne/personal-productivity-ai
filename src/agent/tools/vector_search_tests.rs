//! Unit tests for the vector-search tool contract.

use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct MockVectorSearchService {
    last_query: Mutex<Option<String>>,
    last_limit: Mutex<Option<usize>>,
    last_max_distance: Mutex<Option<Option<f32>>>,
    returns: Mutex<Vec<VectorSearchHit>>,
}

impl VectorSearchService for MockVectorSearchService {
    fn search(
        &self,
        query: &str,
        limit: usize,
        max_distance: Option<f32>,
    ) -> Result<Vec<VectorSearchHit>, String> {
        *self.last_query.lock().unwrap() = Some(query.to_string());
        *self.last_limit.lock().unwrap() = Some(limit);
        *self.last_max_distance.lock().unwrap() = Some(max_distance);
        Ok(self.returns.lock().unwrap().clone())
    }
}

#[test]
fn input_limit_is_bounded_by_tool() {
    assert_eq!(default_limit(), 5);
    let parsed: VectorSearchInput = serde_json::from_str(r#"{"query":"x"}"#).unwrap();
    assert_eq!(parsed.limit, 5);
    assert_eq!(parsed.max_distance, None);
}

#[test]
fn input_deserializes_explicit_max_distance() {
    let parsed: VectorSearchInput =
        serde_json::from_str(r#"{"query":"bank statement","max_distance":0.85}"#).unwrap();
    assert_eq!(parsed.query, "bank statement");
    assert_eq!(parsed.max_distance, Some(0.85));
}

#[test]
fn execute_vector_search_forwards_max_distance_to_service() {
    let mock = Arc::new(MockVectorSearchService::default());
    let mut ctx = ToolContext::default();
    ctx.extensions.insert(VectorSearchExt(mock.clone()));

    let tool = VectorSearchTool;
    let result = execute_vector_search(
        &tool,
        &ctx,
        r#"{"query":"credit union","limit":10,"max_distance":0.75}"#,
    );

    assert!(result.is_ok());
    assert_eq!(
        mock.last_query.lock().unwrap().as_deref(),
        Some("credit union")
    );
    assert_eq!(*mock.last_limit.lock().unwrap(), Some(10));
    assert_eq!(*mock.last_max_distance.lock().unwrap(), Some(Some(0.75)));
}

#[test]
fn execute_vector_search_forwards_none_max_distance_when_omitted() {
    let mock = Arc::new(MockVectorSearchService::default());
    let mut ctx = ToolContext::default();
    ctx.extensions.insert(VectorSearchExt(mock.clone()));

    let tool = VectorSearchTool;
    let result = execute_vector_search(&tool, &ctx, r#"{"query":"credit union"}"#);

    assert!(result.is_ok());
    assert_eq!(*mock.last_max_distance.lock().unwrap(), Some(None));
}
