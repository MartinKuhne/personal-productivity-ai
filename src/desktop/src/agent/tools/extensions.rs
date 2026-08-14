use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Type-safe dependency injection container for tool resources.
#[derive(Default, Clone)]
pub struct Extensions {
    map: Arc<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
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

    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|t| t.clone().downcast::<T>().ok())
    }
}
