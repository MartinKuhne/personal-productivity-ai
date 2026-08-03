//! Main Discord bot implementation.

use crate::agent::llm_client::LLMClient;
use crate::config::{AppConfig, DiscordConfig};
use crate::integrations::discord::commands::{
    InteractionResponse, get_default_commands, handle_slash_command,
};
use crate::integrations::discord::context::{ContextManager, Role};
use crate::integrations::discord::gateway::{
    GatewayClient, GatewayEvent, InteractionCreate, MessageCreate,
};
use crate::integrations::discord::rate_limit::RateLimiter;
use crate::integrations::discord::safety::{SafetyFilter, SafetyResult};
use anyhow::Result;
use reqwest::Client as HttpClient;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Main Discord bot state.
pub struct DiscordBot {
    config: DiscordConfig,
    llm_client: Option<LLMClient>,
    http_client: HttpClient,
    gateway: GatewayClient,
    context_manager: Arc<ContextManager>,
    rate_limiter: Arc<RateLimiter>,
    safety_filter: Arc<SafetyFilter>,
    event_receiver: mpsc::UnboundedReceiver<GatewayEvent>,
}

impl DiscordBot {
    pub fn new(config: DiscordConfig, app_config: AppConfig) -> Self {
        let http_client = HttpClient::new();
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let gateway =
            GatewayClient::new(config.bot_token.clone().unwrap_or_default(), event_sender);
        let context_manager = Arc::new(ContextManager::new(config.max_history, 3600)); // 1 hour TTL
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit_per_minute));
        let safety_filter = Arc::new(SafetyFilter::new());
        let llm_client = LLMClient::from_config(&app_config, None);

        Self {
            config,
            llm_client,
            http_client,
            gateway,
            context_manager,
            rate_limiter,
            safety_filter,
            event_receiver,
        }
    }

    /// Run the bot.
    pub async fn run(mut self) -> Result<()> {
        tracing::info!(name = "discord.bot.run", "Starting Discord bot event loop");

        // Connect to Gateway
        if let Err(e) = self.gateway.connect().await {
            tracing::error!(name = "discord.gateway.connect_failed", error = %e, "Failed to connect to Gateway");
            return Err(e);
        }

        // Register slash commands if enabled
        if self.config.register_commands {
            self.register_commands().await?;
        }

        // Start cleanup task
        let context_manager = self.context_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 min
            loop {
                interval.tick().await;
                context_manager.cleanup_expired().await;
            }
        });

        let rate_limiter = self.rate_limiter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                rate_limiter.cleanup().await;
            }
        });

        // Main event loop
        while let Some(event) = self.event_receiver.recv().await {
            match event {
                GatewayEvent::Ready(ready) => {
                    tracing::info!(name = "discord.bot.ready", user = %ready.user.username, "Bot ready");
                }
                GatewayEvent::MessageCreate(msg) => {
                    self.handle_message(msg).await;
                }
                GatewayEvent::InteractionCreate(interaction) => {
                    self.handle_interaction(interaction).await;
                }
                GatewayEvent::Reconnect => {
                    tracing::warn!(
                        name = "discord.gateway.reconnect",
                        "Gateway requested reconnect"
                    );
                    if let Err(e) = self.gateway.resume().await {
                        tracing::error!(name = "discord.gateway.resume_failed", error = %e, "Failed to resume session");
                    }
                }
                GatewayEvent::InvalidSession(resumable) => {
                    tracing::warn!(
                        name = "discord.gateway.invalid_session",
                        resumable,
                        "Invalid session"
                    );
                    if !resumable {
                        // Full reconnect needed
                        if let Err(e) = self.gateway.connect().await {
                            tracing::error!(name = "discord.gateway.reconnect_failed", error = %e, "Failed to reconnect");
                        }
                    } else if let Err(e) = self.gateway.resume().await {
                        tracing::error!(name = "discord.gateway.resume_failed", error = %e, "Failed to resume");
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_message(&self, msg: MessageCreate) {
        // Check if bot should respond
        let should_respond = self.should_respond_to_message(&msg);
        if !should_respond {
            return;
        }

        // Rate limit check
        let (allowed, retry_after) = self.rate_limiter.check(&msg.author.id).await;
        if !allowed {
            let _ = self
                .send_message(
                    &msg.channel_id,
                    &format!(
                        "Rate limited. Try again in {} seconds.",
                        retry_after.unwrap_or(60)
                    ),
                )
                .await;
            return;
        }

        // Safety check
        let safety_result = self.safety_filter.is_safe(&msg.content).await;
        if let SafetyResult::Blocked { reason } = safety_result {
            let _ = self
                .send_message(&msg.channel_id, &format!("Message blocked: {}", reason))
                .await;
            return;
        }

        // Add to context
        self.context_manager
            .add_message(&msg.channel_id, Role::User, msg.content.clone())
            .await;

        // Get context for LLM
        let messages = self
            .context_manager
            .get_messages_for_llm(&msg.channel_id, self.config.system_prompt.as_deref())
            .await;

        // Call LLM (placeholder - would integrate with fastmd's agent)
        let response = self.call_llm(messages).await;

        // Safety check on response
        let safety_result = self.safety_filter.is_safe(&response).await;
        let final_response = match safety_result {
            SafetyResult::Blocked { reason } => format!("Response blocked: {}", reason),
            SafetyResult::Safe => response,
        };

        // Add assistant response to context
        self.context_manager
            .add_message(&msg.channel_id, Role::Assistant, final_response.clone())
            .await;

        // Send response (split if needed)
        self.send_long_message(&msg.channel_id, &final_response)
            .await;
    }

    async fn handle_interaction(&self, interaction: InteractionCreate) {
        // Check if it's a slash command
        if interaction.interaction_type == 2 {
            // APPLICATION_COMMAND
            let response = handle_slash_command(&interaction, &self.config).await;

            // Send interaction response
            if let Ok(response) = response {
                let _ = self
                    .send_interaction_response(&interaction.id, &interaction.token, response)
                    .await;
            } else {
                tracing::error!(
                    name = "discord.interaction.error",
                    "Failed to handle slash command"
                );
            }
        }
    }

    fn should_respond_to_message(&self, msg: &MessageCreate) -> bool {
        // Don't respond to ourselves
        if msg.author.bot.unwrap_or(false) {
            return false;
        }

        // Check if mentioned or DM
        let mentioned = msg
            .mentioned_users
            .as_ref()
            .map(|users| users.iter().any(|u| u.bot.unwrap_or(false)))
            .unwrap_or(false);

        let is_dm = msg.guild_id.is_none();

        mentioned || is_dm
    }

    async fn call_llm(&self, messages: Vec<(Role, String)>) -> String {
        let llm = match &self.llm_client {
            Some(c) if c.api_key_valid() => c,
            Some(_) => {
                tracing::warn!(
                    name = "discord.llm.invalid_key",
                    "LLM API key not configured; returning error message"
                );
                return "I don't have a valid API key configured. Please set one in the config."
                    .to_string();
            }
            None => {
                tracing::warn!(
                    name = "discord.llm.no_model",
                    "No LLM model configured; returning error message"
                );
                return "No LLM model is configured. Please add a model to your config."
                    .to_string();
            }
        };

        let openai_messages = messages_to_openai(&messages);

        let tools = serde_json::Value::Array(Vec::new());

        match llm.chat_completion(&openai_messages, &tools) {
            Ok(resp) => {
                let content = extract_llm_content(&resp);
                tracing::info!(
                    name = "discord.llm.response",
                    tokens = resp.get("usage").is_some(),
                    "LLM response received"
                );
                content
            }
            Err(e) => {
                tracing::error!(
                    name = "discord.llm.error",
                    error = %e,
                    "LLM request failed"
                );
                format!(
                    "I encountered an error contacting the LLM: {}",
                    e.user_message()
                )
            }
        }
    }

    async fn send_message(&self, channel_id: &str, content: &str) -> Result<()> {
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            channel_id
        );
        let body = serde_json::json!({ "content": content });

        self.http_client
            .post(&url)
            .header(
                "Authorization",
                format!("Bot {}", self.config.bot_token.as_ref().unwrap()),
            )
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        Ok(())
    }

    async fn send_long_message(&self, channel_id: &str, content: &str) {
        const MAX_LEN: usize = 1900; // Leave room for formatting
        if content.len() <= MAX_LEN {
            let _ = self.send_message(channel_id, content).await;
        } else {
            let chunks: Vec<&str> = content
                .as_bytes()
                .chunks(MAX_LEN)
                .map(|c| std::str::from_utf8(c).unwrap_or(""))
                .collect();

            for chunk in chunks {
                let _ = self.send_message(channel_id, chunk).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    async fn send_interaction_response(
        &self,
        interaction_id: &str,
        token: &str,
        response: InteractionResponse,
    ) -> Result<()> {
        let url = format!(
            "https://discord.com/api/v10/interactions/{}/{}/callback",
            interaction_id, token
        );

        let body = serde_json::to_value(&response)?;

        self.http_client
            .post(&url)
            .header(
                "Authorization",
                format!("Bot {}", self.config.bot_token.as_ref().unwrap()),
            )
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        Ok(())
    }

    async fn register_commands(&self) -> Result<()> {
        let commands = get_default_commands();
        let url = format!(
            "https://discord.com/api/v10/applications/{}/commands",
            self.get_application_id().await?
        );

        for cmd in commands {
            self.http_client
                .post(&url)
                .header(
                    "Authorization",
                    format!("Bot {}", self.config.bot_token.as_ref().unwrap()),
                )
                .header("Content-Type", "application/json")
                .json(&cmd)
                .send()
                .await?;
        }

        Ok(())
    }

    async fn get_application_id(&self) -> Result<String> {
        let url = "https://discord.com/api/v10/oauth2/applications/@me";
        let resp = self
            .http_client
            .get(url)
            .header(
                "Authorization",
                format!("Bot {}", self.config.bot_token.as_ref().unwrap()),
            )
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        Ok(json["id"].as_str().unwrap_or("").to_string())
    }
}

/// Extract the text content from an OpenAI-style chat completion response.
fn extract_llm_content(resp: &serde_json::Value) -> String {
    resp.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "I received an empty response from the LLM.".to_string())
}

/// Map a Discord Role to its OpenAI string equivalent.
fn role_to_openai_string(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

/// Convert Discord context messages to the OpenAI JSON message array
/// format expected by [`LLMClient::chat_completion`].
fn messages_to_openai(messages: &[(Role, String)]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|(role, content)| {
            serde_json::json!({ "role": role_to_openai_string(role.clone()), "content": content })
        })
        .collect()
}

/// Run the Discord bot with the given token and config.
pub async fn run(bot_token: &str, config: &DiscordConfig, app_config: &AppConfig) -> Result<()> {
    let mut bot_config = config.clone();
    bot_config.bot_token = Some(bot_token.to_string());

    let bot = DiscordBot::new(bot_config, app_config.clone());
    bot.run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that Role values map to the correct OpenAI role strings.
    #[test]
    fn test_role_to_openai_string() {
        assert_eq!(role_to_openai_string(Role::System), "system");
        assert_eq!(role_to_openai_string(Role::User), "user");
        assert_eq!(role_to_openai_string(Role::Assistant), "assistant");
    }

    /// Verify that a Vec<(Role, String)> is serialized to the OpenAI
    /// message format with correct role mapping and content passthrough.
    #[test]
    fn test_messages_to_openai_format() {
        let messages = vec![
            (Role::System, "You are a helpful assistant.".to_string()),
            (Role::User, "Hello, bot!".to_string()),
            (Role::Assistant, "Hi there!".to_string()),
        ];
        let json = messages_to_openai(&messages);
        assert_eq!(json.len(), 3);
        assert_eq!(json[0]["role"], "system");
        assert_eq!(json[0]["content"], "You are a helpful assistant.");
        assert_eq!(json[1]["role"], "user");
        assert_eq!(json[1]["content"], "Hello, bot!");
        assert_eq!(json[2]["role"], "assistant");
        assert_eq!(json[2]["content"], "Hi there!");
    }

    /// Verify extract_llm_content pulls content from a standard OpenAI response.
    #[test]
    fn test_extract_llm_content_normal() {
        let resp = serde_json::json!({
            "choices": [{ "message": { "content": "Hello from LLM!" } }]
        });
        assert_eq!(extract_llm_content(&resp), "Hello from LLM!");
    }

    /// Verify extract_llm_content returns a fallback when choices is empty.
    #[test]
    fn test_extract_llm_content_empty_choices() {
        let resp = serde_json::json!({ "choices": [] });
        assert_eq!(
            extract_llm_content(&resp),
            "I received an empty response from the LLM."
        );
    }

    /// Verify extract_llm_content returns a fallback when content is null.
    #[test]
    fn test_extract_llm_content_null_content() {
        let resp = serde_json::json!({
            "choices": [{ "message": { "content": null } }]
        });
        assert_eq!(
            extract_llm_content(&resp),
            "I received an empty response from the LLM."
        );
    }
}
