//! Conversation context management for Discord bot.

use crate::utils::uuid::UuidGenerator;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

/// A single message in a conversation.
#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: Instant,
}

/// Role of a message in the conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Conversation context for a specific channel/DM/thread.
#[derive(Clone, Debug)]
pub struct ConversationContext {
    pub id: Uuid,
    pub scope_id: String, // Discord channel/thread ID
    pub messages: Vec<Message>,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub token_count: usize,
}

/// Manages per-conversation contexts with TTL-based cleanup.
#[derive(Debug)]
pub struct DiscordContext {
    contexts: Arc<RwLock<HashMap<String, ConversationContext>>>,
    max_history: usize,
    ttl: Duration,
    uuid_gen: Arc<dyn UuidGenerator>,
}

impl DiscordContext {
    pub fn new(max_history: usize, ttl_seconds: u64, uuid_gen: Arc<dyn UuidGenerator>) -> Self {
        Self {
            contexts: Arc::new(RwLock::new(HashMap::new())),
            max_history,
            ttl: Duration::from_secs(ttl_seconds),
            uuid_gen,
        }
    }

    /// Get or create a conversation context for the given scope.
    pub async fn get_or_create(&self, scope_id: &str) -> ConversationContext {
        let mut contexts = self.contexts.write().await;
        if let Some(ctx) = contexts.get_mut(scope_id) {
            ctx.last_accessed = Instant::now();
            return ctx.clone();
        }
        let ctx = ConversationContext {
            id: self.uuid_gen.new_v4(),
            scope_id: scope_id.to_string(),
            messages: Vec::new(),
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            token_count: 0,
        };
        contexts.insert(scope_id.to_string(), ctx.clone());
        ctx
    }

    /// Add a message to the conversation, creating the context if it does
    /// not yet exist (so the first message in a channel is never dropped).
    pub async fn add_message(&self, scope_id: &str, role: Role, content: String) {
        let mut contexts = self.contexts.write().await;
        let ctx = contexts
            .entry(scope_id.to_string())
            .or_insert_with(|| ConversationContext {
                id: self.uuid_gen.new_v4(),
                scope_id: scope_id.to_string(),
                messages: Vec::new(),
                created_at: Instant::now(),
                last_accessed: Instant::now(),
                token_count: 0,
            });
        ctx.messages.push(Message {
            role,
            content,
            timestamp: Instant::now(),
        });
        ctx.last_accessed = Instant::now();
        // Trim to max_history
        if ctx.messages.len() > self.max_history {
            ctx.messages = ctx
                .messages
                .split_off(ctx.messages.len() - self.max_history);
        }
        // Rough token count estimate (bytes / 4)
        ctx.token_count = ctx.messages.iter().map(|m| m.content.len() / 4).sum();
    }

    /// Get messages for LLM context (with system prompt).
    pub async fn get_messages_for_llm(
        &self,
        scope_id: &str,
        system_prompt: Option<&str>,
    ) -> Vec<(Role, String)> {
        let contexts = self.contexts.read().await;
        let mut result = Vec::new();
        if let Some(prompt) = system_prompt {
            result.push((Role::System, prompt.to_string()));
        }
        if let Some(ctx) = contexts.get(scope_id) {
            for msg in &ctx.messages {
                result.push((msg.role.clone(), msg.content.clone()));
            }
        }
        result
    }

    /// Clean up expired contexts.
    pub async fn cleanup_expired(&self) {
        let mut contexts = self.contexts.write().await;
        let now = Instant::now();
        contexts.retain(|_, ctx| now.duration_since(ctx.last_accessed) < self.ttl);
    }

    /// Get all active context scope IDs.
    pub async fn active_scopes(&self) -> Vec<String> {
        let contexts = self.contexts.read().await;
        contexts.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: adding a message to a brand-new scope (no prior
    /// `get_or_create`) must create the context instead of silently
    /// dropping the message.
    #[tokio::test]
    async fn test_add_message_creates_context_if_missing() {
        let manager =
            DiscordContext::new(20, 3600, Arc::new(crate::utils::uuid::SystemUuidGenerator));

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
        let manager = DiscordContext::new(
            20,
            3600,
            Arc::new(crate::utils::uuid::FixedUuidGenerator::new(mock_uuid)),
        );
        let ctx = manager.get_or_create("channel-123").await;

        assert_eq!(ctx.id, mock_uuid);
        assert_eq!(ctx.scope_id, "channel-123");
        assert!(ctx.messages.is_empty());
        assert_eq!(ctx.token_count, 0);
    }

    #[tokio::test]
    async fn test_context_manager_returns_existing_context() {
        let manager =
            DiscordContext::new(20, 3600, Arc::new(crate::utils::uuid::SystemUuidGenerator));
        let ctx1 = manager.get_or_create("channel-123").await;
        let ctx2 = manager.get_or_create("channel-123").await;

        assert_eq!(ctx1.id, ctx2.id);
        assert_eq!(ctx1.scope_id, ctx2.scope_id);
    }

    #[tokio::test]
    async fn test_add_message_appends_to_context() {
        let manager =
            DiscordContext::new(20, 3600, Arc::new(crate::utils::uuid::SystemUuidGenerator));
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
        let manager =
            DiscordContext::new(20, 3600, Arc::new(crate::utils::uuid::SystemUuidGenerator));
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
        let manager =
            DiscordContext::new(3, 3600, Arc::new(crate::utils::uuid::SystemUuidGenerator));
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
        let manager =
            DiscordContext::new(20, 3600, Arc::new(crate::utils::uuid::SystemUuidGenerator));
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
        let manager =
            DiscordContext::new(20, 3600, Arc::new(crate::utils::uuid::SystemUuidGenerator));
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
        let manager = DiscordContext::new(20, 1, Arc::new(crate::utils::uuid::SystemUuidGenerator)); // 1 second TTL
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
}
