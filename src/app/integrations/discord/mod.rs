//! Discord bot integration for FastMd.
//!
//! This module provides a Discord bot that can chat with users and execute
//! LLM commands via slash commands and message mentions.
//!
//! Requirements: see `SPEC.md` for the full specification.

pub mod bot;
pub mod commands;
pub mod config;
pub mod context;
pub mod gateway;
pub mod rate_limit;
pub mod safety;

use crate::config::AppConfig;
use anyhow::Result;

/// Initialize and run the Discord bot if configured.
pub async fn run_discord_bot(config: &AppConfig) -> Result<()> {
    let discord_config = config
        .discord
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("discord config not present"))?;
    let bot_token = discord_config
        .bot_token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("discord.bot_token not configured"))?;

    tracing::info!(name = "discord.bot.start", "Starting Discord bot");
    bot::run(bot_token, discord_config, config).await
}
