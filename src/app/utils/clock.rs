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
