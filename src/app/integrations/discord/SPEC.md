# Discord Bot Integration Specification

## Requirements

### DISCORD-001: Bot Connection
The bot MUST connect to Discord using a bot token via the Gateway API (WebSocket) and REST API.

### DISCORD-002: Message Handling
The bot MUST receive and parse `MESSAGE_CREATE` gateway events for DMs and guild messages where the bot is mentioned or replied to.

### DISCORD-003: Message Sending
The bot MUST send responses via the REST API `POST /channels/{channel.id}/messages` endpoint.

### DISCORD-004: Conversation Context
The bot MUST maintain per-conversation context (DM, guild channel, thread) with a configurable TTL (default 1 hour).

### DISCORD-005: Slash Commands
The bot MUST register and handle Discord slash commands for LLM operations (at minimum: chat, summarize, code, analyze).

### DISCORD-006: Rate Limiting
The bot MUST enforce per-user rate limits (configurable, default 10 requests/minute) with Discord-native 429 handling.

### DISCORD-007: Content Safety
The bot MUST implement a content safety filter on both input (user messages) and output (LLM responses).

### DISCORD-008: LLM Integration
The bot MUST forward user messages to an LLM API (configurable provider) and return the response.

### DISCORD-009: Streaming Responses
The bot SHOULD support streaming responses from the LLM, rendering incrementally in Discord via message edits.

### DISCORD-010: Message Length Handling
The bot MUST handle Discord's 2000-character message limit by splitting long responses across multiple messages.

### DISCORD-011: Observability
The bot MUST log all interactions (user ID, channel ID, timestamp, token counts, latency) for observability.

### DISCORD-012: Gateway Reconnection
The bot MUST gracefully handle Gateway reconnections (Resume, Identify) per Discord's specification.

### DISCORD-013: Configuration
The bot MUST support configuration via the main `AppConfig` including `discord.bot_token`, allowed channels/guilds, rate limits, and context settings.

## Configuration

The Discord bot is configured under the `discord` key in `AppConfig`:

```yaml
discord:
  bot_token: "your-bot-token"        # Required: Bot token from Discord Developer Portal
  allowed_channels: []               # Optional: Channel IDs where bot responds (empty = all where mentioned)
  allowed_guilds: []                 # Optional: Guild IDs where bot is active (empty = all)
  register_commands: true            # Optional: Register slash commands on startup (default: true)
  system_prompt: null                # Optional: Default system prompt for LLM
  max_history: 20                    # Optional: Max conversation history length (default: 20)
  rate_limit_per_minute: 10          # Optional: Per-user rate limit (default: 10)
```

## Architecture

The Discord integration follows the existing FastMd architecture:

- **Gateway Connection**: `integrations/discord/gateway.rs` - WebSocket connection to Discord Gateway
- **Event Handling**: Processes `MESSAGE_CREATE`, `INTERACTION_CREATE`, `READY`, and other events
- **Context Management**: `integrations/discord/context.rs` - Per-channel/thread conversation history
- **Rate Limiting**: `integrations/discord/rate_limit.rs` - Sliding window per-user rate limiter
- **Safety Filter**: `integrations/discord/safety.rs` - Content filtering for input/output
- **Slash Commands**: `integrations/discord/commands.rs` - Command definitions and handlers
- **Main Bot**: `integrations/discord/bot.rs` - Orchestrates all components

## Integration Points

The bot integrates with FastMd's existing systems:

- **Configuration**: Uses `config::DiscordConfig` loaded from main config
- **Agent/LLM**: Calls into `agent::llm_client` for LLM responses (to be implemented)
- **Event Bus**: Publishes interaction events to `bus::events::messages` for UI observability
- **Logging**: Uses `tracing` for structured logging

## Security

- Bot token is stored in config with `[REDACTED]` in Debug output
- Per-user rate limiting prevents abuse
- Content safety filter on both input and output
- Only responds in allowed channels/guilds or when mentioned in DMs
- Follows Discord ToS (no self-bot behavior)

## Testing

- Unit tests for context management, rate limiting, message splitting
- Integration tests for Gateway connection, message handling, slash commands
- Mock Discord Gateway for deterministic testing