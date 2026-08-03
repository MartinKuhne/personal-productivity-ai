//! Discord bot configuration types.

use serde::{Deserialize, Serialize};

/// Discord bot configuration.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscordConfig {
    /// Bot token from the Discord Developer Portal.
    #[serde(default)]
    pub bot_token: Option<String>,
    /// Channel IDs where the bot should respond (empty = all channels where mentioned).
    #[serde(default)]
    pub allowed_channels: Vec<String>,
    /// Guild IDs where the bot is active (empty = all guilds).
    #[serde(default)]
    pub allowed_guilds: Vec<String>,
    /// Enable slash command registration.
    #[serde(default = "default_true")]
    pub register_commands: bool,
    /// Default system prompt for the LLM.
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Maximum conversation history length (number of messages).
    #[serde(default = "default_discord_history_len")]
    pub max_history: usize,
    /// Per-user rate limit (requests per minute).
    #[serde(default = "default_discord_rate_limit")]
    pub rate_limit_per_minute: u32,
}

fn default_true() -> bool {
    true
}

fn default_discord_history_len() -> usize {
    20
}

fn default_discord_rate_limit() -> u32 {
    10
}
