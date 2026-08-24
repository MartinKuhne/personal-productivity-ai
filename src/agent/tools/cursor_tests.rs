//! Unit tests for `CursorSessionManager`.

use super::*;
use crate::utils::uuid::{FixedUuidGenerator, SystemUuidGenerator};
use std::sync::Arc;
use std::time::Duration;

const TEST_HINT: &str = "Final page.";
const TEST_ERROR: &str = "Cursor expired or unknown; re-run the search with no cursor.";

#[test]
fn single_page_omits_cursor() {
    let mgr = CursorSessionManager::<i32>::new(5, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let items = vec![1, 2, 3];

    let page = mgr.create_session(items, &uuid_gen);
    assert_eq!(page.items, vec![1, 2, 3]);
    assert_eq!(page.total, 3);
    assert!(page.cursor.is_none());
    assert_eq!(page.hint.as_deref(), Some(TEST_HINT));
    assert!(mgr.is_empty());
    assert_eq!(mgr.len(), 0);
}

#[test]
fn empty_items_omits_cursor() {
    let mgr = CursorSessionManager::<i32>::new(5, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());

    let page = mgr.create_session(Vec::new(), &uuid_gen);
    assert!(page.items.is_empty());
    assert_eq!(page.total, 0);
    assert!(page.cursor.is_none());
    assert_eq!(page.hint.as_deref(), Some(TEST_HINT));
    assert!(mgr.is_empty());
}

#[test]
fn exact_page_size_boundary_omits_cursor() {
    let mgr = CursorSessionManager::<i32>::new(3, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let items = vec![10, 20, 30];

    let page = mgr.create_session(items, &uuid_gen);
    assert_eq!(page.items, vec![10, 20, 30]);
    assert_eq!(page.total, 3);
    assert!(page.cursor.is_none());
    assert_eq!(page.hint.as_deref(), Some(TEST_HINT));
    assert!(mgr.is_empty());
}

#[test]
fn multi_page_traversal() {
    let mgr = CursorSessionManager::<i32>::new(3, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let items = vec![1, 2, 3, 4, 5, 6, 7];

    // Page 1
    let p1 = mgr.create_session(items, &uuid_gen);
    assert_eq!(p1.items, vec![1, 2, 3]);
    assert_eq!(p1.total, 7);
    assert_eq!(p1.cursor.as_deref(), Some("c_00000000"));
    assert!(p1.hint.is_none());
    assert_eq!(mgr.len(), 1);

    let cursor = p1.cursor.unwrap();

    // Page 2
    let p2 = mgr.next_page(&cursor).unwrap();
    assert_eq!(p2.items, vec![4, 5, 6]);
    assert_eq!(p2.total, 7);
    assert_eq!(p2.cursor.as_deref(), Some("c_00000000"));
    assert!(p2.hint.is_none());

    // Page 3 (Final)
    let p3 = mgr.next_page(&cursor).unwrap();
    assert_eq!(p3.items, vec![7]);
    assert_eq!(p3.total, 7);
    assert!(p3.cursor.is_none());
    assert_eq!(p3.hint.as_deref(), Some(TEST_HINT));

    // After final page, cursor is invalidated and cleaned up
    let err = mgr.next_page(&cursor).unwrap_err();
    assert_eq!(err, TEST_ERROR);
    assert!(mgr.is_empty());
}

#[test]
fn exact_multiple_pages_traversal() {
    let mgr = CursorSessionManager::<i32>::new(2, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let items = vec![100, 200, 300, 400];

    // Page 1 (2 items)
    let p1 = mgr.create_session(items, &uuid_gen);
    assert_eq!(p1.items, vec![100, 200]);
    assert_eq!(p1.total, 4);
    assert!(p1.cursor.is_some());
    assert!(p1.hint.is_none());

    let cursor = p1.cursor.unwrap();

    // Page 2 (Final exact 2 items)
    let p2 = mgr.next_page(&cursor).unwrap();
    assert_eq!(p2.items, vec![300, 400]);
    assert_eq!(p2.total, 4);
    assert!(p2.cursor.is_none());
    assert_eq!(p2.hint.as_deref(), Some(TEST_HINT));

    // Attempting to paginate further fails
    let err = mgr.next_page(&cursor).unwrap_err();
    assert_eq!(err, TEST_ERROR);
}

#[test]
fn page_size_one_traversal() {
    let mgr = CursorSessionManager::<String>::new(1, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let items = vec!["a".to_string(), "b".to_string()];

    let p1 = mgr.create_session(items, &uuid_gen);
    assert_eq!(p1.items, vec!["a".to_string()]);
    assert_eq!(p1.total, 2);
    let cursor = p1.cursor.unwrap();

    let p2 = mgr.next_page(&cursor).unwrap();
    assert_eq!(p2.items, vec!["b".to_string()]);
    assert_eq!(p2.total, 2);
    assert!(p2.cursor.is_none());
    assert_eq!(p2.hint.as_deref(), Some(TEST_HINT));

    assert!(mgr.next_page(&cursor).is_err());
}

#[test]
fn unknown_cursor_returns_error() {
    let mgr = CursorSessionManager::<i32>::new(5, TEST_HINT, TEST_ERROR);
    let err = mgr.next_page("nonexistent").unwrap_err();
    assert_eq!(err, TEST_ERROR);

    let err_empty = mgr.next_page("").unwrap_err();
    assert_eq!(err_empty, TEST_ERROR);
}

#[test]
fn explicit_invalidation() {
    let mgr = CursorSessionManager::<i32>::new(2, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let page = mgr.create_session(vec![1, 2, 3, 4], &uuid_gen);
    let cursor = page.cursor.unwrap();

    assert_eq!(mgr.len(), 1);
    mgr.invalidate(&cursor);
    assert_eq!(mgr.len(), 0);

    let err = mgr.next_page(&cursor).unwrap_err();
    assert_eq!(err, TEST_ERROR);

    // Invalidating nonexistent cursor is a no-op and does not panic
    mgr.invalidate("bogus_cursor");
}

#[test]
fn session_expires_after_ttl() {
    let mgr = CursorSessionManager::<i32>::with_options(
        2,
        Duration::from_millis(10),
        100,
        TEST_HINT,
        TEST_ERROR,
    );
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let page = mgr.create_session(vec![1, 2, 3, 4], &uuid_gen);
    let cursor = page.cursor.unwrap();

    std::thread::sleep(Duration::from_millis(25));
    let err = mgr.next_page(&cursor).unwrap_err();
    assert_eq!(err, TEST_ERROR);
}

#[test]
fn shallow_clone_shares_sessions() {
    let mgr = CursorSessionManager::<i32>::new(2, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let page = mgr.create_session(vec![1, 2, 3, 4], &uuid_gen);
    let cursor = page.cursor.unwrap();

    let cloned = mgr.clone();
    let p2 = cloned.next_page(&cursor).unwrap();
    assert_eq!(p2.items, vec![3, 4]);
    assert!(p2.cursor.is_none());
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
struct ComplexItem {
    id: usize,
    tags: Vec<String>,
}

#[test]
fn complex_payload_pagination() {
    let mgr = CursorSessionManager::<ComplexItem>::new(1, TEST_HINT, TEST_ERROR);
    let uuid_gen = FixedUuidGenerator::new(uuid::Uuid::nil());
    let items = vec![
        ComplexItem {
            id: 1,
            tags: vec!["rust".to_string(), "ai".to_string()],
        },
        ComplexItem {
            id: 2,
            tags: vec!["fastmd".to_string()],
        },
    ];

    let p1 = mgr.create_session(items, &uuid_gen);
    assert_eq!(p1.items.len(), 1);
    assert_eq!(p1.items[0].id, 1);
    let cursor = p1.cursor.unwrap();

    let p2 = mgr.next_page(&cursor).unwrap();
    assert_eq!(p2.items.len(), 1);
    assert_eq!(p2.items[0].id, 2);
    assert!(p2.cursor.is_none());
}

#[test]
fn multithreaded_concurrent_access() {
    let mgr = Arc::new(CursorSessionManager::<usize>::new(
        10, TEST_HINT, TEST_ERROR,
    ));
    let sys_gen = SystemUuidGenerator;

    let page = mgr.create_session((0..100).collect(), &sys_gen);
    let cursor = page.cursor.unwrap();

    let mut handles = vec![];
    for _ in 0..4 {
        let mgr_clone = mgr.clone();
        let cursor_clone = cursor.clone();
        handles.push(std::thread::spawn(move || {
            // Multiple threads querying the manager
            let _ = mgr_clone.next_page(&cursor_clone);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
