//! Main Discord bot implementation.

use crate::agent::llm_client::{LLMClient, parse_usage_block};
use crate::config::{AppConfig, DiscordConfig};
use crate::integrations::discord::commands::{
    APPLICATION_COMMAND, InteractionResponse, deferred_response, ephemeral_response,
    get_default_commands,
};
use crate::integrations::discord::context::DiscordContext;
use crate::integrations::discord::context::Role;
use crate::integrations::discord::gateway::{
    GatewayClient, GatewayEvent, InteractionCreate, InteractionData, MessageCreate,
};
use crate::integrations::discord::rate_limit::RateLimiter;
use crate::integrations::discord::safety::{SafetyFilter, SafetyResult};
use anyhow::Result;
use reqwest::Client as HttpClient;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Maximum content length for a single Discord message. We leave a small
/// margin below Discord's 2000-character limit for formatting/code fences.
const DISCORD_MSG_MAX: usize = 1900;

/// Cloneable bundle of everything a background task (slash-command LLM
/// follow-up, message chunk) needs to talk to Discord and the LLM without
/// borrowing from [`DiscordBot`].
#[derive(Clone)]
struct BotHandles {
    bot_token: String,
    http_client: HttpClient,
    llm_client: Option<LLMClient>,
    context_manager: Arc<DiscordContext>,
    rate_limiter: Arc<RateLimiter>,
    safety_filter: Arc<SafetyFilter>,
    system_prompt: Option<String>,
}

/// Main Discord bot state.
pub struct DiscordBot {
    config: DiscordConfig,
    handles: BotHandles,
    gateway: GatewayClient,
    /// The bot's own user id, captured from `READY`. Used to detect
    /// self-mentions. `None` until READY is received.
    bot_user_id: Option<String>,
    event_receiver: mpsc::UnboundedReceiver<GatewayEvent>,
}

impl DiscordBot {
    /// Construct a bot. Fails fast if `bot_token` is missing or empty.
    pub fn new(config: DiscordConfig, app_config: AppConfig) -> Result<Self> {
        let bot_token = config
            .bot_token
            .clone()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| anyhow::anyhow!("discord.bot_token not configured"))?;

        let http_client = HttpClient::new();
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let gateway = GatewayClient::new(bot_token.clone(), event_sender);
        let context_manager = Arc::new(DiscordContext::new(
            config.max_history,
            3600,
            Arc::new(crate::utils::uuid::SystemUuidGenerator),
        )); // 1 hour TTL
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit_per_minute));
        let safety_filter = Arc::new(SafetyFilter::with_patterns(config.blocked_patterns.clone()));
        let llm_client = LLMClient::from_agent_config(
            &crate::agent::config::AgentConfig::from_app_config(&app_config),
            None,
        );
        let system_prompt = config.system_prompt.clone();

        let handles = BotHandles {
            bot_token,
            http_client,
            llm_client,
            context_manager,
            rate_limiter,
            safety_filter,
            system_prompt,
        };

        Ok(Self {
            config,
            handles,
            gateway,
            bot_user_id: None,
            event_receiver,
        })
    }

    /// Run the bot: connect, register commands, and drain gateway events.
    pub async fn run(mut self) -> Result<()> {
        tracing::info!(name = "discord.bot.run", "Starting Discord bot event loop");

        // Initial connection: fail fast if the socket cannot be opened.
        self.gateway.start().await?;

        // Register slash commands if enabled. Failures are logged but do
        // not abort startup — registration is idempotent and retried on
        // the next start.
        if self.config.register_commands
            && let Err(e) = self.handles.register_commands().await
        {
            tracing::warn!(name = "discord.commands.register_failed", error = %e, "Command registration failed; continuing");
        }

        // Periodic cleanup tasks. They run for the lifetime of the bot;
        // the tokio runtime reaps them when the process exits.
        let context_manager = self.handles.context_manager.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await;
                context_manager.cleanup_expired().await;
            }
        });
        let rate_limiter = self.handles.rate_limiter.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                rate_limiter.cleanup().await;
            }
        });

        // Main event loop. The gateway self-manages reconnection; we only
        // react to delivered events.
        while let Some(event) = self.event_receiver.recv().await {
            match event {
                GatewayEvent::Ready(ready) => {
                    self.bot_user_id = Some(ready.user.id.clone());
                    tracing::info!(name = "discord.bot.ready", user = %ready.user.username, "Bot ready");
                }
                GatewayEvent::MessageCreate(msg) => {
                    self.handle_message(msg).await;
                }
                GatewayEvent::InteractionCreate(interaction) => {
                    self.handle_interaction(interaction).await;
                }
                GatewayEvent::Reconnect => {
                    tracing::warn!(name = "discord.gateway.reconnect", "Gateway reconnecting");
                }
                GatewayEvent::InvalidSession(resumable) => {
                    tracing::warn!(
                        name = "discord.gateway.invalid_session",
                        resumable,
                        "Invalid session"
                    );
                }
                GatewayEvent::HeartbeatAck => {}
                GatewayEvent::Unknown(s) => {
                    tracing::debug!(name = "discord.gateway.unknown", event = %s, "Unknown gateway event");
                }
            }
        }

        self.gateway.shutdown();
        Ok(())
    }

    async fn handle_message(&self, msg: MessageCreate) {
        if !self.should_respond_to_message(&msg) {
            return;
        }

        // Rate limit check.
        let (allowed, retry_after) = self.handles.rate_limiter.check(&msg.author.id).await;
        if !allowed {
            let _ = self
                .handles
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

        // Input safety check.
        let safety_result = self.handles.safety_filter.is_safe(&msg.content).await;
        if let SafetyResult::Blocked { reason } = safety_result {
            let _ = self
                .handles
                .send_message(&msg.channel_id, &format!("Message blocked: {}", reason))
                .await;
            return;
        }

        // Record the user turn, then build the LLM context. `add_message`
        // creates the conversation if it does not yet exist.
        self.handles
            .context_manager
            .add_message(&msg.channel_id, Role::User, msg.content.clone())
            .await;
        let messages = self
            .handles
            .context_manager
            .get_messages_for_llm(&msg.channel_id, self.handles.system_prompt.as_deref())
            .await;

        let response = self.handles.llm_complete(messages).await;

        // Output safety check.
        let final_response = match self.handles.safety_filter.is_safe(&response).await {
            SafetyResult::Blocked { reason } => format!("Response blocked: {}", reason),
            SafetyResult::Safe => response,
        };

        // Record the assistant turn and send (split if needed).
        self.handles
            .context_manager
            .add_message(&msg.channel_id, Role::Assistant, final_response.clone())
            .await;
        self.handles
            .send_long_message(&msg.channel_id, &final_response)
            .await;
    }

    async fn handle_interaction(&self, interaction: InteractionCreate) {
        if interaction.interaction_type != APPLICATION_COMMAND {
            tracing::debug!(
                name = "discord.interaction.ignored",
                itype = interaction.interaction_type,
                "Ignoring non-application interaction"
            );
            return;
        }
        if let Err(e) = self.handle_slash_command(&interaction).await {
            tracing::error!(name = "discord.interaction.error", error = %e, "Failed to handle slash command");
        }
    }

    async fn handle_slash_command(&self, interaction: &InteractionCreate) -> Result<()> {
        let data = interaction
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No interaction data"))?;

        let token = interaction.token.clone();
        match data.name.as_str() {
            "chat" | "summarize" | "code" | "analyze" => {
                // Acknowledge within Discord's 3-second interaction deadline.
                self.handles
                    .send_interaction_callback(&interaction.id, &token, deferred_response())
                    .await?;

                // Run the (potentially slow) LLM call in the background and
                // deliver the result by editing the original response.
                let prompt = build_command_prompt(data);
                let handles = self.handles.clone();
                let app_id = interaction.application_id.clone();
                let command_name = data.name.clone();
                tokio::spawn(async move {
                    let messages = match &handles.system_prompt {
                        Some(p) => vec![(Role::System, p.clone()), (Role::User, prompt)],
                        None => vec![(Role::User, prompt)],
                    };
                    let response = handles.llm_complete(messages).await;
                    let response = match handles.safety_filter.is_safe(&response).await {
                        SafetyResult::Safe => response,
                        SafetyResult::Blocked { reason } => format!("Response blocked: {}", reason),
                    };
                    if let Err(e) = handles
                        .edit_interaction_original(&app_id, &token, &response)
                        .await
                    {
                        tracing::error!(
                            name = "discord.interaction.followup_failed",
                            command = %command_name,
                            error = %e,
                            "Failed to deliver slash command response"
                        );
                    }
                });
                Ok(())
            }
            _ => {
                self.handles
                    .send_interaction_callback(
                        &interaction.id,
                        &token,
                        ephemeral_response("Unknown command".to_string()),
                    )
                    .await?;
                Ok(())
            }
        }
    }

    /// Decide whether the bot should respond to a message: ignore bots,
    /// honour `allowed_channels`/`allowed_guilds`, and respond only on
    /// direct mentions of *this* bot or in DMs.
    fn should_respond_to_message(&self, msg: &MessageCreate) -> bool {
        if msg.author.bot.unwrap_or(false) {
            return false;
        }

        if !self.config.allowed_channels.is_empty()
            && !self.config.allowed_channels.contains(&msg.channel_id)
        {
            return false;
        }

        // DMs have no guild and bypass the guild allow-list.
        if let Some(guild_id) = &msg.guild_id
            && !self.config.allowed_guilds.is_empty()
            && !self.config.allowed_guilds.contains(guild_id)
        {
            return false;
        }

        let is_dm = msg.guild_id.is_none();
        let self_mentioned = match (&self.bot_user_id, &msg.mentioned_users) {
            (Some(me), Some(users)) => users.iter().any(|u| u.id == *me),
            _ => false,
        };
        self_mentioned || is_dm
    }
}

impl BotHandles {
    /// Run the LLM to completion off the async runtime (the underlying
    /// `LLMClient::chat_completion` uses blocking `reqwest`). Returns a
    /// user-facing string (either the LLM content or an error message).
    async fn llm_complete(&self, messages: Vec<(Role, String)>) -> String {
        let Some(llm) = &self.llm_client else {
            tracing::warn!(name = "discord.llm.no_model", "No LLM model configured");
            return "No LLM model is configured. Please add a model to your config.".to_string();
        };
        if !llm.api_key_valid() {
            tracing::warn!(
                name = "discord.llm.invalid_key",
                "LLM API key not configured"
            );
            return "I don't have a valid API key configured. Please set one in the config."
                .to_string();
        }

        let llm = llm.clone();
        let openai_messages = messages_to_openai(&messages);
        let tools = serde_json::Value::Array(Vec::new());
        match tokio::task::spawn_blocking(move || llm.chat_completion(&openai_messages, &tools))
            .await
        {
            Ok(Ok(resp)) => {
                if let Some(usage) = resp.get("usage") {
                    if let Some(info) = parse_usage_block(usage) {
                        tracing::info!(
                            name = "discord.llm.response",
                            total = info.total_tokens,
                            prompt = info.prompt_tokens,
                            completion = info.completion_tokens,
                            "LLM response received"
                        );
                    } else {
                        tracing::info!(name = "discord.llm.response", "LLM response received");
                    }
                } else {
                    tracing::info!(name = "discord.llm.response", "LLM response received");
                }
                extract_llm_content(&resp)
            }
            Ok(Err(e)) => {
                tracing::error!(name = "discord.llm.error", error = %e, "LLM request failed");
                format!(
                    "I encountered an error contacting the LLM: {}",
                    e.user_message()
                )
            }
            Err(e) => {
                tracing::error!(name = "discord.llm.panic", error = %e, "LLM blocking task panicked");
                "I encountered an internal error while contacting the LLM.".to_string()
            }
        }
    }

    async fn send_message(&self, channel_id: &str, content: &str) -> Result<()> {
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            channel_id
        );
        let resp = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "content": content }))
            .send()
            .await?;
        check_discord_status(resp).await
    }

    async fn send_long_message(&self, channel_id: &str, content: &str) {
        for chunk in split_for_discord(content, DISCORD_MSG_MAX) {
            if let Err(e) = self.send_message(channel_id, &chunk).await {
                tracing::error!(name = "discord.message.send_failed", error = %e, "Failed to send message chunk");
            }
            // Light pacing to avoid Discord's burst rate limits.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn send_interaction_callback(
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
        let resp = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        check_discord_status(resp).await
    }

    /// Edit the original interaction response (used to deliver the final
    /// result after a deferred ack).
    async fn edit_interaction_original(
        &self,
        application_id: &str,
        token: &str,
        content: &str,
    ) -> Result<()> {
        let url = format!(
            "https://discord.com/api/v10/webhooks/{}/{}/messages/@original",
            application_id, token
        );
        let resp = self
            .http_client
            .patch(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "content": content }))
            .send()
            .await?;
        check_discord_status(resp).await
    }

    async fn register_commands(&self) -> Result<()> {
        let commands = get_default_commands();
        let app_id = self.get_application_id().await?;
        let url = format!(
            "https://discord.com/api/v10/applications/{}/commands",
            app_id
        );

        let mut failures = 0u32;
        for cmd in commands {
            let resp = self
                .http_client
                .post(&url)
                .header("Authorization", format!("Bot {}", self.bot_token))
                .header("Content-Type", "application/json")
                .json(&cmd)
                .send()
                .await?;
            match check_discord_status(resp).await {
                Ok(()) => {
                    tracing::info!(name = "discord.command.registered", command = %cmd.name, "Registered slash command");
                }
                Err(e) => {
                    tracing::warn!(name = "discord.command.register_failed", command = %cmd.name, error = %e, "Command registration failed");
                    failures += 1;
                }
            }
        }
        if failures > 0 {
            return Err(anyhow::anyhow!(
                "{} slash command(s) failed to register",
                failures
            ));
        }
        Ok(())
    }

    async fn get_application_id(&self) -> Result<String> {
        let resp = self
            .http_client
            .get("https://discord.com/api/v10/oauth2/applications/@me")
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "discord applications/@me {}: {}",
                status,
                body
            ));
        }
        let json: serde_json::Value = resp.json().await?;
        json.get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("no application id in response"))
    }
}

/// Convert a non-success Discord HTTP response into an error, including
/// the response body for diagnostics.
async fn check_discord_status(resp: reqwest::Response) -> Result<()> {
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let body = resp.text().await.unwrap_or_default();
    Err(anyhow::anyhow!("discord API error {}: {}", status, body))
}

/// Split `content` into chunks of at most `max_len` bytes, never breaking
/// inside a UTF-8 codepoint. A single character longer than `max_len` is
/// emitted on its own (rare for typical `max_len` values).
fn split_for_discord(content: &str, max_len: usize) -> Vec<String> {
    if content.len() <= max_len {
        return vec![content.to_string()];
    }
    let bytes = content.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < bytes.len() {
        let mut end = (start + max_len).min(bytes.len());
        // Walk back to the nearest UTF-8 char boundary.
        while end > start && end < bytes.len() && !content.is_char_boundary(end) {
            end -= 1;
        }
        // If a single character is larger than max_len, walk forward to
        // its end boundary and emit it whole.
        if end == start {
            end = (start + max_len).min(bytes.len());
            while end < bytes.len() && !content.is_char_boundary(end) {
                end += 1;
            }
        }
        chunks.push(content[start..end].to_string());
        start = end;
    }
    chunks
}

/// Build a single user-turn prompt for a slash command from its options.
fn build_command_prompt(data: &InteractionData) -> String {
    let option = |name: &str| {
        data.options
            .as_ref()
            .and_then(|opts| opts.iter().find(|o| o.name == name))
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    match data.name.as_str() {
        "chat" => option("message").to_string(),
        "summarize" => format!("Summarize the following text:\n\n{}", option("text")),
        "code" => {
            let language = option("language");
            if language.is_empty() {
                format!("Generate code for: {}", option("prompt"))
            } else {
                format!("Generate {} code for: {}", language, option("prompt"))
            }
        }
        "analyze" => format!("Analyze the following content:\n\n{}", option("content")),
        _ => String::new(),
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
    let bot = DiscordBot::new(bot_config, app_config.clone())?;
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

    /// Short content is returned as a single chunk.
    #[test]
    fn test_split_for_discord_short() {
        let chunks = split_for_discord("hello", 1900);
        assert_eq!(chunks, vec!["hello".to_string()]);
    }

    /// ASCII content is split on byte boundaries that align with char boundaries.
    #[test]
    fn test_split_for_discord_ascii_boundary() {
        let content = "0".repeat(5000);
        let chunks = split_for_discord(&content, 1900);
        assert_eq!(chunks.len(), 3);
        for c in &chunks {
            assert!(c.len() <= 1900);
        }
        assert_eq!(chunks.concat(), content);
    }

    /// UTF-8 content is never split in the middle of a multibyte codepoint:
    /// concatenating the chunks reproduces the original string exactly.
    #[test]
    fn test_split_for_discord_multibyte_preserved() {
        // Each emoji is 4 bytes; force splits inside them.
        let content = "😀".repeat(1000); // 4000 bytes
        let chunks = split_for_discord(&content, 1900);
        assert!(chunks.len() > 1);
        for c in &chunks {
            assert!(c.len() <= 1900);
            // Every chunk must be valid UTF-8 (from_utf8 panics otherwise).
            std::str::from_utf8(c.as_bytes()).expect("chunk is valid UTF-8");
        }
        assert_eq!(chunks.concat(), content);
    }

    /// A split point that lands exactly on a char boundary needs no back-up.
    #[test]
    fn test_split_for_discord_boundary_exact() {
        let content = "abcDEF".to_string();
        let chunks = split_for_discord(&content, 3);
        assert_eq!(chunks, vec!["abc".to_string(), "DEF".to_string()]);
    }

    /// `build_command_prompt` reflects the documented slash-command shapes.
    #[test]
    fn test_build_command_prompt() {
        let opts = |name: &str, value: &str| {
            serde_json::json!([{
                "name": name,
                "type": 3,
                "value": value,
            }])
        };
        let mk = |name: &str, options: serde_json::Value| InteractionData {
            id: "x".to_string(),
            name: name.to_string(),
            options: Some(
                options
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|o| serde_json::from_value(o.clone()).unwrap())
                    .collect(),
            ),
        };

        let chat = mk("chat", opts("message", "hi there"));
        assert_eq!(build_command_prompt(&chat), "hi there");

        let code = mk(
            "code",
            serde_json::json!([
                { "name": "prompt", "type": 3, "value": "hello world" },
                { "name": "language", "type": 3, "value": "python" },
            ]),
        );
        assert_eq!(
            build_command_prompt(&code),
            "Generate python code for: hello world"
        );
    }

    /// `new` fails fast without a bot token.
    #[test]
    fn test_new_requires_bot_token() {
        let config = DiscordConfig::default(); // bot_token None
        let app = AppConfig::default();
        assert!(DiscordBot::new(config, app).is_err());
    }
}
