//! Content safety filter for Discord bot.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_with_patterns_blocks_matching_content() {
        let filter = SafetyFilter::with_patterns(vec!["badword".to_string()]);

        let blocked = filter.is_safe("this contains badword").await;
        assert_eq!(
            blocked,
            SafetyResult::Blocked {
                reason: "Content matches blocked pattern: badword".to_string()
            }
        );

        let clean = filter.is_safe("totally clean").await;
        assert_eq!(clean, SafetyResult::Safe);
    }

    #[tokio::test]
    async fn test_safety_filter_allows_clean_content() {
        let filter = SafetyFilter::new();
        let result = filter.is_safe("Hello, how are you?").await;
        assert_eq!(result, SafetyResult::Safe);
    }

    #[tokio::test]
    async fn test_safety_filter_blocks_matching_pattern() {
        let filter = SafetyFilter::new();
        filter.add_pattern("badword".to_string()).await;

        let result = filter.is_safe("This contains badword in it").await;
        assert_eq!(
            result,
            SafetyResult::Blocked {
                reason: "Content matches blocked pattern: badword".to_string()
            }
        );
    }

    #[tokio::test]
    async fn test_safety_filter_case_insensitive() {
        let filter = SafetyFilter::new();
        filter.add_pattern("BADWORD".to_string()).await;

        let result = filter.is_safe("This contains badword in it").await;
        assert_eq!(
            result,
            SafetyResult::Blocked {
                reason: "Content matches blocked pattern: BADWORD".to_string()
            }
        );
    }

    #[tokio::test]
    async fn test_safety_filter_multiple_patterns() {
        let filter = SafetyFilter::new();
        filter.add_pattern("badword1".to_string()).await;
        filter.add_pattern("badword2".to_string()).await;

        let result1 = filter.is_safe("Contains badword1").await;
        assert_eq!(
            result1,
            SafetyResult::Blocked {
                reason: "Content matches blocked pattern: badword1".to_string()
            }
        );

        let result2 = filter.is_safe("Contains badword2").await;
        assert_eq!(
            result2,
            SafetyResult::Blocked {
                reason: "Content matches blocked pattern: badword2".to_string()
            }
        );

        let result3 = filter.is_safe("Clean content").await;
        assert_eq!(result3, SafetyResult::Safe);
    }

    #[tokio::test]
    async fn test_safety_filter_empty_content() {
        let filter = SafetyFilter::new();
        let result = filter.is_safe("").await;
        assert_eq!(result, SafetyResult::Safe);
    }
}
