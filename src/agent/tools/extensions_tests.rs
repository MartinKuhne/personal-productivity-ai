//! Tests for the type-safe `Extensions` dependency-injection container used by tool contexts.

use super::*;

#[derive(Debug, PartialEq)]
struct Widget {
    name: String,
}

#[derive(Debug, PartialEq)]
struct Gadget {
    id: u32,
}

#[test]
fn insert_and_get_round_trip() {
    let mut ext = Extensions::new();
    ext.insert(Arc::new(Widget {
        name: "w".to_string(),
    }));
    assert_eq!(ext.get::<Widget>().unwrap().name, "w");
}

#[test]
fn get_missing_type_returns_none() {
    let ext = Extensions::new();
    assert!(ext.get::<Widget>().is_none());
}

#[test]
fn insert_overwrites_existing_type() {
    let mut ext = Extensions::new();
    ext.insert(Arc::new(Widget {
        name: "first".to_string(),
    }));
    ext.insert(Arc::new(Widget {
        name: "second".to_string(),
    }));
    assert_eq!(ext.get::<Widget>().unwrap().name, "second");
}

#[test]
fn different_types_coexist() {
    let mut ext = Extensions::new();
    ext.insert(Arc::new(Widget {
        name: "w".to_string(),
    }));
    ext.insert(Arc::new(Gadget { id: 7 }));
    assert_eq!(ext.get::<Widget>().unwrap().name, "w");
    assert_eq!(ext.get::<Gadget>().unwrap().id, 7);
}

#[test]
fn extend_overrides_self_with_other_on_collision() {
    let mut ext = Extensions::new();
    ext.insert(Arc::new(Widget {
        name: "self".to_string(),
    }));

    let mut other = Extensions::new();
    other.insert(Arc::new(Widget {
        name: "other".to_string(),
    }));
    other.insert(Arc::new(Gadget { id: 3 }));

    ext.extend(&other);
    // `other` wins on collision; new types are added.
    assert_eq!(ext.get::<Widget>().unwrap().name, "other");
    assert_eq!(ext.get::<Gadget>().unwrap().id, 3);
}

#[test]
fn extend_does_not_mutate_other() {
    let mut ext = Extensions::new();
    let mut other = Extensions::new();
    other.insert(Arc::new(Gadget { id: 9 }));
    ext.extend(&other);
    // `other` retains its own entry.
    assert_eq!(other.get::<Gadget>().unwrap().id, 9);
}

#[test]
fn clone_is_shallow_and_shares_entries() {
    let mut ext = Extensions::new();
    ext.insert(Arc::new(Widget {
        name: "shared".to_string(),
    }));
    let cloned = ext.clone();
    assert_eq!(cloned.get::<Widget>().unwrap().name, "shared");
}
