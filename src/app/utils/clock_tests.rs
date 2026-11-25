//! Tests for [`Clock`], [`SystemClock`] and [`FixedClock`] (`utils::clock`).

use super::*;
use chrono::TimeZone;

#[test]
fn system_clock_returns_recent_time() {
    let clock = SystemClock;
    let before = Local::now();
    let now = clock.now();
    let after = Local::now();
    assert!(now >= before && now <= after);
}

#[test]
fn fixed_clock_returns_set_time() {
    let t = Local.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap();
    let clock = FixedClock { time: t };
    assert_eq!(clock.now(), t);
}
