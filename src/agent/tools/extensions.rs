use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Type-safe dependency injection container for tool resources.
#[derive(Default, Clone)]
pub struct Extensions {
    map: Arc<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extensions")
            .field("entries", &self.map.len())
            .finish()
    }
}

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, val: Arc<T>) {
        let mut new_map = (*self.map).clone();
        new_map.insert(TypeId::of::<T>(), val as Arc<dyn Any + Send + Sync>);
        self.map = Arc::new(new_map);
    }

    pub fn extend(&mut self, other: &Self) {
        let mut new_map = (*self.map).clone();
        new_map.extend(
            (*other.map)
                .iter()
                .map(|(key, value)| (*key, value.clone())),
        );
        self.map = Arc::new(new_map);
    }

    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|t| t.clone().downcast::<T>().ok())
    }
}

#[cfg(test)]
mod tests {
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
}
