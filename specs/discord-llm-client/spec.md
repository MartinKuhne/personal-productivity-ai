# Feature Specification: Discord LLM Client Integration

**Feature Branch**: `discord-llm-client`

**Created**: 2026-08-02

**Status**: Draft

**Input**: User description: "A Discord client submodule that can chat with end users and execute commands against the LLM from the user. Code should go into /integrations/discord"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Chat with LLM via Discord (Priority: P1)

As a Discord user, I want to send messages to a bot and receive responses from an LLM, so that I can have conversational interactions directly in Discord.

**Why this priority**: Core value proposition — enables the primary use case of LLM-powered chat in Discord.

**Independent Test**: Can be fully tested by sending a DM to the bot and verifying a coherent LLM response is returned within 15 seconds.

**Acceptance Scenarios**:

1. **Given** the bot is online and connected to Discord, **When** a user sends a DM to the bot, **Then** the bot responds with an LLM-generated message.
2. **Given** the bot is in a guild channel where it has permission to read messages, **When** a user mentions the bot or replies to it, **Then** the bot responds with an LLM-generated message.
3. **Given** a conversation is ongoing, **When** the user sends follow-up messages, **Then** the bot maintains context across the conversation.

---

### User Story 2 - Execute LLM Commands via Discord (Priority: P1)

As a Discord user, I want to invoke specific LLM commands (e.g., summarization, code generation, analysis) via slash commands or prefixed commands, so that I can get structured outputs for specific tasks.

**Why this priority**: Extends chat into actionable tool use — the differentiator from a simple chatbot.

**Independent Test**: Can be fully tested by invoking a command (e.g., `/summarize` with attached text) and verifying the structured response.

**Acceptance Scenarios**:

1. **Given** the bot has slash commands registered, **When** a user invokes `/summarize` with text input, **Then** the bot returns a concise summary.
2. **Given** the bot has a code generation command, **When** a user invokes `/code` with a prompt, **Then** the bot returns formatted code blocks.
3. **Given** a command requires parameters, **When** a user provides incomplete parameters, **Then** the bot responds with usage guidance.

---

### User Story 3 - Session Management & Context (Priority: P2)

As a Discord user, I want my conversation history to be preserved per channel/DM, so that the LLM has context for multi-turn conversations.

**Why this priority**: Enables coherent multi-turn conversations without manual context passing.

**Independent Test**: Can be tested by having a 5-turn conversation and verifying the 5th response references earlier turns.

**Acceptance Scenarios**:

1. **Given** a user starts a conversation in a DM, **When** they send 5 messages over 10 minutes, **Then** all 5 turns are included in the LLM context.
2. **Given** a conversation is idle for 1 hour, **When** the user resumes, **Then** the context is either preserved (if within TTL) or gracefully reset with a notice.
3. **Given** multiple users in a guild channel interact with the bot, **Then** each user/DM/channel has isolated context.

---

### User Story 4 - Rate Limiting & Safety (Priority: P2)

As a system operator, I want the bot to enforce rate limits and content safety, so that the service remains stable and compliant.

**Why this priority**: Prevents abuse, controls costs, and ensures platform compliance.

**Independent Test**: Can be tested by sending 20 rapid messages and verifying appropriate rate limit responses.

**Acceptance Scenarios**:

1. **Given** a user exceeds the per-user rate limit, **When** they send another message, **Then** they receive a rate limit notice with retry-after time.
2. **Given** a message contains disallowed content, **When** the bot processes it, **Then** the response is a safety refusal, not an LLM completion.
3. **Given** the LLM API returns an error, **When** the bot handles it, **Then** the user receives a friendly error message, not a stack trace.

---

### Edge Cases

- What happens when the LLM API is unavailable or times out?
- How does the system handle messages exceeding Discord's 2000-character limit?
- What happens when a user tries to invoke a command in a channel where the bot lacks permissions?
- How are concurrent messages from the same user handled (queue vs. parallel)?
- What happens when the bot is mentioned in a thread vs. a main channel?
- How does the bot handle system messages (joins, pins, etc.)?
- What happens if the bot's token is revoked or rotated?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST connect to Discord via the Gateway API (WebSocket) and REST API using a bot token.
- **FR-002**: System MUST receive and parse `MESSAGE_CREATE` gateway events for DMs and guild messages where the bot is mentioned or replied to.
- **FR-003**: System MUST send responses via the REST API `POST /channels/{channel.id}/messages` endpoint.
- **FR-004**: System MUST maintain per-conversation context (DM, guild channel, thread) with a configurable TTL (default 1 hour).
- **FR-005**: System MUST register and handle Discord slash commands for LLM operations (at minimum: chat, summarize, code, analyze).
- **FR-006**: System MUST enforce per-user rate limits (configurable, default 10 requests/minute) with Discord-native 429 handling.
- **FR-007**: System MUST implement a content safety filter on both input (user messages) and output (LLM responses).
- **FR-008**: System MUST forward user messages to an LLM API (configurable provider: OpenAI, Anthropic, local, etc.) and return the response.
- **FR-009**: System MUST support streaming responses from the LLM, rendering incrementally in Discord via message edits.
- **FR-010**: System MUST handle Discord's 2000-character message limit by splitting long responses across multiple messages.
- **FR-011**: System MUST log all interactions (user ID, channel ID, timestamp, token counts, latency) for observability.
- **FR-012**: System MUST gracefully handle Gateway reconnections (Resume, Identify) per Discord's specification.
- **FR-013**: System MUST support configuration via environment variables and/or a config file (bot token, LLM provider, API keys, rate limits, context TTL).
- **FR-014**: System MUST provide a health check endpoint (HTTP) for container orchestration.
- **FR-015**: System MUST support running as a Docker container with a published image.
- **FR-016**: System MUST use the distilled Discord API reference (doc/distill/discord.md) as the authoritative source for API contracts.

### Key Entities

- **Conversation Context**: Represents the message history for a specific conversation scope (DM, channel, thread). Attributes: `scope_id` (snowflake), `messages` (array of {role, content, timestamp}), `created_at`, `last_accessed`, `token_count`.
- **LLM Request**: Represents a single request to the LLM provider. Attributes: `conversation_id`, `user_message`, `system_prompt`, `tools`, `stream`, `model`, `max_tokens`.
- **LLM Response**: Represents the response from the LLM provider. Attributes: `content`, `tool_calls?`, `usage` (prompt_tokens, completion_tokens), `finish_reason`, `latency_ms`.
- **Rate Limit Bucket**: Tracks per-user request counts. Attributes: `user_id`, `window_start`, `request_count`, `limit`.
- **Slash Command**: Represents a registered Discord application command. Attributes: `name`, `description`, `options`, `handler`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Bot responds to a simple chat message in under 3 seconds (p95) when LLM API is healthy.
- **SC-002**: Bot handles 100 concurrent conversations without message loss or duplication.
- **SC-003**: Slash commands return a response (acknowledgment or result) within 3 seconds (Discord requirement).
- **SC-004**: Rate limiting correctly rejects excess requests with a user-friendly message including retry-after.
- **SC-005**: Streaming responses render visible output within 500ms of first token, with full completion under 30 seconds.
- **SC-006**: Bot uptime exceeds 99.9% over a 30-day period (excluding planned deployments).
- **SC-007**: Memory usage stays under 512 MB for 1000 active conversation contexts.
- **SC-008**: Zero PII leaks in logs (user messages redacted or hashed in production).

## Assumptions

- The bot token is provisioned and stored securely (not in code).
- An LLM provider API (OpenAI-compatible or Anthropic) is available with valid credentials.
- The deployment target supports long-running WebSocket connections (Gateway).
- Discord API v10 is used (current stable).
- The `doc/distill/discord.md` reference is authoritative for Discord API behavior.
- Rust is the implementation language (consistent with the `src/desktop` Rust crate in this repo).
- The `serenity` or `twilight` crate will be used for Discord Gateway/REST interaction.
- Conversation context is stored in-memory with optional Redis backend for horizontal scaling.
- The bot is a "server bot" (not a user bot/self-bot) and complies with Discord ToS.