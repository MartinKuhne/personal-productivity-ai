//! Rate limiting for Discord bot.
//!
//! Unit tests live in the sibling `rate_limit_tests.rs` sidecar.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limiter using a sliding window per user.
#[derive(Debug)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
    limit: u32,
    window: Duration,
}

#[derive(Clone, Debug)]
struct RateLimitBucket {
    count: u32,
    window_start: Instant,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            limit: limit_per_minute,
            window: Duration::from_secs(60),
        }
    }

    /// Check if the user is allowed to make a request.
    /// Returns (allowed, retry_after_seconds).
    pub async fn check(&self, user_id: &str) -> (bool, Option<u64>) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();

        let bucket = buckets
            .entry(user_id.to_string())
            .or_insert(RateLimitBucket {
                count: 0,
                window_start: now,
            });

        // Reset window if expired
        if now.duration_since(bucket.window_start) >= self.window {
            bucket.count = 0;
            bucket.window_start = now;
        }

        if bucket.count >= self.limit {
            let retry_after =
                self.window.as_secs() - now.duration_since(bucket.window_start).as_secs();
            return (false, Some(retry_after));
        }

        bucket.count += 1;
        (true, None)
    }

    /// Clean up old buckets.
    pub async fn cleanup(&self) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();
        buckets.retain(|_, bucket| now.duration_since(bucket.window_start) < self.window * 2);
    }
}

#[cfg(test)]
#[path = "rate_limit_tests.rs"]
mod tests;
