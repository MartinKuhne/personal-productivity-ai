//! Conversation context management for Discord bot.
//!
//! Unit tests live in the sibling `context_tests.rs` sidecar.

use fastmd_agent::utils::uuid::UuidGenerator;
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
#[path = "context_tests.rs"]
mod tests;
