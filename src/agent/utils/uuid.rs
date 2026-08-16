use std::fmt::Debug;
use uuid::Uuid;

pub trait UuidGenerator: Send + Sync + Debug {
    fn new_v4(&self) -> Uuid;
}

#[derive(Debug)]
pub struct SystemUuidGenerator;

impl UuidGenerator for SystemUuidGenerator {
    fn new_v4(&self) -> Uuid {
        Uuid::new_v4()
    }
}

#[derive(Debug)]
pub struct FixedUuidGenerator {
    pub value: Uuid,
}

impl FixedUuidGenerator {
    pub fn new(value: Uuid) -> Self {
        Self { value }
    }
}

impl Default for FixedUuidGenerator {
    fn default() -> Self {
        Self { value: Uuid::nil() }
    }
}

impl UuidGenerator for FixedUuidGenerator {
    fn new_v4(&self) -> Uuid {
        self.value
    }
}
