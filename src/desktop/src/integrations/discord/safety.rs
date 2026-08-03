//! Content safety filter for Discord bot.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Safety filter for input and output content.
#[derive(Debug)]
pub struct SafetyFilter {
    blocked_patterns: Arc<RwLock<Vec<String>>>,
}

impl SafetyFilter {
    pub fn new() -> Self {
        Self {
            blocked_patterns: Arc::new(RwLock::new(vec![
                // Add default blocked patterns here
            ])),
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
        for pattern in patterns.iter() {
            if content.to_lowercase().contains(&pattern.to_lowercase()) {
                return SafetyResult::Blocked {
                    reason: format!("Content matches blocked pattern: {}", pattern),
                };
            }
        }
        SafetyResult::Safe
    }

    /// Add a blocked pattern.
    pub async fn add_pattern(&self, pattern: String) {
        self.blocked_patterns.write().await.push(pattern);
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
