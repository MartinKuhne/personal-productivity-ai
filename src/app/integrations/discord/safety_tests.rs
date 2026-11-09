//! Tests for [`SafetyFilter`] and [`SafetyResult`] (`integrations::discord::safety`).

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
