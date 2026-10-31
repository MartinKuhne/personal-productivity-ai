//! Injectable clock abstraction — the [`Clock`] trait with a system
//! implementation and a fixed-time test implementation.
//!
//! Unit tests live in the sibling `clock_tests.rs` sidecar.

use chrono::{DateTime, Local};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Local>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}

#[cfg(test)]
pub struct FixedClock {
    pub time: DateTime<Local>,
}

#[cfg(test)]
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Local> {
        self.time
    }
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
