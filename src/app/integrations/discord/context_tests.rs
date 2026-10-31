//! Tests for [`DiscordContext`] (`integrations::discord::context`).

use super::*;
use fastmd_agent::utils::uuid::{FixedUuidGenerator, SystemUuidGenerator};

/// Regression: adding a message to a brand-new scope (no prior
/// `get_or_create`) must create the context instead of silently
/// dropping the message.
#[tokio::test]
async fn test_add_message_creates_context_if_missing() {
    let manager = DiscordContext::new(20, 3600, Arc::new(SystemUuidGenerator));

    manager
        .add_message("fresh-channel", Role::User, "first message".to_string())
        .await;

    let messages = manager.get_messages_for_llm("fresh-channel", None).await;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].0, Role::User);
    assert_eq!(messages[0].1, "first message");

    let scopes = manager.active_scopes().await;
    assert!(scopes.contains(&"fresh-channel".to_string()));
}

#[tokio::test]
async fn test_context_manager_creates_new_context() {
    let mock_uuid = uuid::Uuid::nil();
    let manager = DiscordContext::new(20, 3600, Arc::new(FixedUuidGenerator::new(mock_uuid)));
    let ctx = manager.get_or_create("channel-123").await;

    assert_eq!(ctx.id, mock_uuid);
    assert_eq!(ctx.scope_id, "channel-123");
    assert!(ctx.messages.is_empty());
    assert_eq!(ctx.token_count, 0);
}

#[tokio::test]
async fn test_context_manager_returns_existing_context() {
    let manager = DiscordContext::new(20, 3600, Arc::new(SystemUuidGenerator));
    let ctx1 = manager.get_or_create("channel-123").await;
    let ctx2 = manager.get_or_create("channel-123").await;

    assert_eq!(ctx1.id, ctx2.id);
    assert_eq!(ctx1.scope_id, ctx2.scope_id);
}

#[tokio::test]
async fn test_add_message_appends_to_context() {
    let manager = DiscordContext::new(20, 3600, Arc::new(SystemUuidGenerator));
    manager.get_or_create("channel-123").await; // Create context first
    manager
        .add_message("channel-123", Role::User, "Hello".to_string())
        .await;
    manager
        .add_message("channel-123", Role::Assistant, "Hi there!".to_string())
        .await;

    let messages = manager.get_messages_for_llm("channel-123", None).await;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].0, Role::User);
    assert_eq!(messages[0].1, "Hello");
    assert_eq!(messages[1].0, Role::Assistant);
    assert_eq!(messages[1].1, "Hi there!");
}

#[tokio::test]
async fn test_system_prompt_included_in_llm_messages() {
    let manager = DiscordContext::new(20, 3600, Arc::new(SystemUuidGenerator));
    manager.get_or_create("channel-123").await; // Create context first
    manager
        .add_message("channel-123", Role::User, "Hello".to_string())
        .await;

    let messages = manager
        .get_messages_for_llm("channel-123", Some("You are a helpful assistant"))
        .await;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].0, Role::System);
    assert_eq!(messages[0].1, "You are a helpful assistant");
    assert_eq!(messages[1].0, Role::User);
    assert_eq!(messages[1].1, "Hello");
}

#[tokio::test]
async fn test_max_history_trims_old_messages() {
    let manager = DiscordContext::new(3, 3600, Arc::new(SystemUuidGenerator));
    manager.get_or_create("channel-123").await; // Create context first
    manager
        .add_message("channel-123", Role::User, "1".to_string())
        .await;
    manager
        .add_message("channel-123", Role::Assistant, "2".to_string())
        .await;
    manager
        .add_message("channel-123", Role::User, "3".to_string())
        .await;
    manager
        .add_message("channel-123", Role::Assistant, "4".to_string())
        .await;
    manager
        .add_message("channel-123", Role::User, "5".to_string())
        .await;

    let messages = manager.get_messages_for_llm("channel-123", None).await;
    assert_eq!(messages.len(), 3);
    // Should keep the last 3 messages
    assert_eq!(messages[0].1, "3");
    assert_eq!(messages[1].1, "4");
    assert_eq!(messages[2].1, "5");
}

#[tokio::test]
async fn test_token_count_estimation() {
    let manager = DiscordContext::new(20, 3600, Arc::new(SystemUuidGenerator));
    manager.get_or_create("channel-123").await; // Create context first
    manager
        .add_message("channel-123", Role::User, "Hello world".to_string())
        .await; // ~2-3 tokens
    manager
        .add_message("channel-123", Role::Assistant, "Hi there".to_string())
        .await; // ~2 tokens

    let ctx = manager.get_or_create("channel-123").await;
    assert!(ctx.token_count > 0);
}

#[tokio::test]
async fn test_different_scopes_have_isolated_contexts() {
    let manager = DiscordContext::new(20, 3600, Arc::new(SystemUuidGenerator));
    manager.get_or_create("channel-1").await;
    manager.get_or_create("channel-2").await;
    manager
        .add_message("channel-1", Role::User, "Hello".to_string())
        .await;
    manager
        .add_message("channel-2", Role::User, "World".to_string())
        .await;

    let msg1 = manager.get_messages_for_llm("channel-1", None).await;
    let msg2 = manager.get_messages_for_llm("channel-2", None).await;

    assert_eq!(msg1.len(), 1);
    assert_eq!(msg1[0].1, "Hello");
    assert_eq!(msg2.len(), 1);
    assert_eq!(msg2[0].1, "World");
}

#[tokio::test]
async fn test_cleanup_expired_removes_old_contexts() {
    let manager = DiscordContext::new(20, 1, Arc::new(SystemUuidGenerator)); // 1 second TTL
    manager.get_or_create("channel-123").await; // Create context first
    manager
        .add_message("channel-123", Role::User, "Hello".to_string())
        .await;

    let scopes_before = manager.active_scopes().await;
    assert!(scopes_before.contains(&"channel-123".to_string()));

    // Wait for TTL to expire
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    manager.cleanup_expired().await;

    let scopes_after = manager.active_scopes().await;
    assert!(!scopes_after.contains(&"channel-123".to_string()));
}
