//! Content safety filter for Discord bot.
//!
//! Unit tests live in the sibling `safety_tests.rs` sidecar.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Safety filter for input and output content.
///
/// Patterns are matched as case-insensitive substrings. Patterns are
/// typically loaded from `DiscordConfig::blocked_patterns` at startup
/// (see [`SafetyFilter::with_patterns`]); [`SafetyFilter::add_pattern`]
/// can be used for runtime additions.
#[derive(Debug)]
pub struct SafetyFilter {
    blocked_patterns: Arc<RwLock<Vec<String>>>,
}

impl SafetyFilter {
    /// Create an empty filter (blocks nothing).
    pub fn new() -> Self {
        Self {
            blocked_patterns: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a filter pre-populated with the given patterns.
    pub fn with_patterns(patterns: Vec<String>) -> Self {
        Self {
            blocked_patterns: Arc::new(RwLock::new(patterns)),
        }
    }
}

impl Default for SafetyFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyFilter {
    /// Check if content is safe.
    pub async fn is_safe(&self, content: &str) -> SafetyResult {
        let patterns = self.blocked_patterns.read().await;
        let content_lower = content.to_lowercase();
        for pattern in patterns.iter() {
            if content_lower.contains(&pattern.to_lowercase()) {
                return SafetyResult::Blocked {
                    reason: format!("Content matches blocked pattern: {}", pattern),
                };
            }
        }
        SafetyResult::Safe
    }

    /// Add a blocked pattern at runtime.
    pub async fn add_pattern(&self, pattern: String) {
        self.blocked_patterns.write().await.push(pattern);
    }

    /// Replace all blocked patterns.
    pub async fn set_patterns(&self, patterns: Vec<String>) {
        *self.blocked_patterns.write().await = patterns;
    }
}

/// Result of a safety check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyResult {
    Safe,
    Blocked { reason: String },
}

impl SafetyResult {
    /// `true` for the `Safe` variant, `false` for `Blocked`.
    /// Convenience accessor used by the safety proptest sidecar
    /// to keep the assertion lines readable.
    pub fn is_safe(&self) -> bool {
        matches!(self, SafetyResult::Safe)
    }
}

#[cfg(test)]
#[path = "safety_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "safety_proptests.rs"]
mod safety_proptests;
