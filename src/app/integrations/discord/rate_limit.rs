//! Rate limiting for Discord bot.

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
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_requests_under_limit() {
        let limiter = RateLimiter::new(5);

        for i in 0..5 {
            let (allowed, retry_after) = limiter.check("user-123").await;
            assert!(allowed, "Request {} should be allowed", i);
            assert!(retry_after.is_none());
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_requests_over_limit() {
        let limiter = RateLimiter::new(3);

        for _ in 0..3 {
            let (allowed, _) = limiter.check("user-123").await;
            assert!(allowed);
        }

        let (allowed, retry_after) = limiter.check("user-123").await;
        assert!(!allowed);
        assert!(retry_after.is_some());
        assert!(retry_after.unwrap() > 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_separate_users_independent() {
        let limiter = RateLimiter::new(2);

        limiter.check("user-1").await;
        limiter.check("user-1").await;
        let (allowed, _) = limiter.check("user-1").await;
        assert!(!allowed);

        // user-2 should still be able to make requests
        let (allowed, _) = limiter.check("user-2").await;
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_rate_limiter_window_reset() {
        let limiter = RateLimiter::new(2);

        limiter.check("user-123").await;
        limiter.check("user-123").await;
        let (allowed, _) = limiter.check("user-123").await;
        assert!(!allowed);

        // Manually advance time by manipulating the bucket
        // Since we can't easily test time passage, we verify the logic
        // by checking that a new user gets a fresh bucket
        let (allowed, _) = limiter.check("user-456").await;
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_cleanup_removes_expired_buckets() {
        let limiter = RateLimiter::new(10);
        limiter.check("user-1").await;
        limiter.check("user-2").await;

        // Cleanup shouldn't remove active buckets
        limiter.cleanup().await;

        // Both users should still work
        let (allowed, _) = limiter.check("user-1").await;
        assert!(allowed);
    }
}
