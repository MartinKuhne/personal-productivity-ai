# Discord API Reference

**Source Specifications:**
- Documentation index (llms.txt): https://docs.discord.com/llms.txt
- API Reference: https://docs.discord.com/developers/reference.md
- Gateway: https://docs.discord.com/developers/events/gateway.md
- Opcodes and Status Codes: https://docs.discord.com/developers/topics/opcodes-and-status-codes.md
- Rate Limits: https://docs.discord.com/developers/topics/rate-limits.md
- Permissions: https://docs.discord.com/developers/topics/permissions.md
- OAuth2: https://docs.discord.com/developers/topics/oauth2.md
- Threads: https://docs.discord.com/developers/topics/threads.md
- Application: https://docs.discord.com/developers/resources/application.md
- Application Commands: https://docs.discord.com/developers/interactions/application-commands.md
- Channel: https://docs.discord.com/developers/resources/channel.md
- Emoji: https://docs.discord.com/developers/resources/emoji.md
- Guild: https://docs.discord.com/developers/resources/guild.md
- Interactions: https://docs.discord.com/developers/interactions/receiving-and-responding.md
- Invite: https://docs.discord.com/developers/resources/invite.md
- Message: https://docs.discord.com/developers/resources/message.md
- User: https://docs.discord.com/developers/resources/user.md
- Webhook: https://docs.discord.com/developers/resources/webhook.md

## 1. Overview

| Property | Value |
|---|---|
| Name | Discord API |
| Type | HTTP REST API + WebSocket (Gateway) API |
| Latest API Version | 10 |
| Base URL | `https://discord.com/api` |
| CDN Base URL | `https://cdn.discordapp.com/` |
| Transport | HTTPS with TLS 1.2 (all HTTP-layer services and protocols) |
| Authentication | Bot token or OAuth2 bearer token |
| Documentation Date | 2026-08-02 |

The Discord API is a REST API that allows you to interact with Discord data from your own applications. It is the primary way to interact with Discord from your own code. A companion WebSocket API, the Gateway, delivers real-time events.

The keywords MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, RECOMMENDED, MAY, and OPTIONAL in this document follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119.html).

## 2. Architecture & Core Concepts

### 2.1 API Versioning

Discord exposes different versions of its API. You MUST specify the version by including it in the request path, for example `https://discord.com/api/v10`. Omitting the version number routes requests to the current default version.

| Version | Status | Default |
|---|---|---|
| 10 | Available | |
| 9 | Available | |
| 8 | Deprecated | |
| 7 | Deprecated | |
| 6 | Deprecated | ✓ |
| 5, 4, 3 | Discontinued | |

Non-functioning (discontinued) versions return `400 Bad Request` when used.

### 2.2 Authentication

You MUST authenticate using the `Authorization` HTTP header in the format `Authorization: TOKEN_TYPE TOKEN`.

Two token types exist:

- **Bot token** — found on the Bot page within your app's settings. Format: `Authorization: Bot <token>`.
- **OAuth2 bearer token** — gained through the [OAuth2 API](#6-api-reference). Format: `Authorization: Bearer <token>`.

### 2.3 Snowflakes

Discord uses Twitter's snowflake format for uniquely identifiable descriptors (IDs). Snowflake IDs are up to 64 bits in size. They are always returned as strings in the HTTP API to prevent integer overflows in some languages.

| Field | Bits | Number of bits | Description | Retrieval |
|---|---|---|---|---|
| Timestamp | 63 to 22 | 42 | Milliseconds since Discord Epoch (first second of 2015, `1420070400000`) | `(snowflake >> 22) + 1420070400000` |
| Internal worker ID | 21 to 17 | 5 | | `(snowflake & 0x3E0000) >> 17` |
| Internal process ID | 16 to 12 | 5 | | `(snowflake & 0x1F000) >> 12` |
| Increment | 11 to 0 | 12 | Incremented for every ID generated on that process | `snowflake & 0xFFF` |

Generate a snowflake ID from a timestamp with `(timestamp_ms - DISCORD_EPOCH) << 22`.

IDs MUST be treated as strings. If you send an `id` value that is not bigint-sized, Discord may serialize it back as an integer rather than a string; this never happens with IDs that originate from Discord.

### 2.4 Date/Time and Field Conventions

- **ISO8601** — Discord uses the ISO8601 format for most Date/Times returned in models. This type is referred to as `ISO8601`.
- **Nullable fields** — types prefixed with a question mark (`?string`) may contain `null`.
- **Optional fields** — names suffixed with a question mark (`optional_field?`) may be omitted.
- **Combined** — `optional_and_nullable_field?` with type `?string` is both.

### 2.5 Consistency

Discord operates at a scale where true consistency is impossible. Many operations are eventually consistent. Client actions can never be serialized and may be executed in any order (if at all). Events may:

- Never be sent to a client
- Be sent exactly one time to the client
- Be sent up to N times per client

You MUST operate on events and API results idempotently.

### 2.6 HTTP API Conventions

#### User Agent

You MUST provide a valid User Agent (RFC 9110) in the format `DiscordBot ($url, $versionNumber)`. You MAY append more information. Requests without a valid User Agent may be blocked and return a Cloudflare error.

#### Content Type

You MUST provide a valid `Content-Type` header on requests, one of `application/json`, `application/x-www-form-urlencoded`, or `multipart/form-data`, except where specified. A missing or invalid content type results in a `50035` "Invalid form body" error.

#### Boolean Query Strings

Discord represents booleans in query strings as `True`, `true`, or `1` for true, and `False`, `false`, or `0` for false.

#### Array Query Strings

Unless otherwise specified, arrays in query strings use multiple instances of the same parameter. Example: `?id=123&id=456` for `["123", "456"]`.

#### Pagination

Snowflake IDs are typically used for pagination. You may specify `before` and `after` in combination with `limit` to retrieve a page of results. Because snowflake IDs embed a timestamp, you can generate an ID for a given time to page from that point.

#### Uploading Files

Some endpoints support file attachments via the `files[n]` parameter. When used, the `application/json` body MUST be replaced by a `multipart/form-data` body. You MAY provide the JSON body in the `payload_json` parameter. Each file parameter MUST be uniquely named in the format `files[n]` (for example `files[0]`, `files[1]`). Each `files[n]` parameter MUST include a valid `Content-Disposition` header with a `filename` and unique `name`. The suffixed index `n` is the snowflake placeholder you use in the `attachments` field. You MAY reference files in embeds with the `attachment://filename` URL scheme.

The default upload size limit per file is `10 MiB`; the limit may be higher depending on Nitro status or server Boost Tier. On PATCH, only files listed in `attachments` are retained; files not listed are removed. Only `.jpg`, `.jpeg`, `.png`, `.webp`, and `.gif` file types are supported in uploads.

### 2.7 Message Formatting

Discord renders a subset of markdown in message content and supports custom syntax for mentions and formatting.

| Type | Structure | Example |
|---|---|---|
| User | `<@USER_ID>` | `<@80351110224678912>` |
| User (deprecated) | `<@!USER_ID>` | `<@!80351110224678912>` |
| Channel | `<#CHANNEL_ID>` | `<#103735883630395392>` |
| Role | `<@&ROLE_ID>` | `<@&165511591545143296>` |
| Slash command | `</NAME:COMMAND_ID>` | `</airhorn:816437322781949972>` |
| Slash command with subcommand | `</NAME SUBCOMMAND:ID>` | `</foo bar:123456789012345678>` |
| Slash command with subcommand group | `</NAME SUBCOMMAND_GROUP SUBCOMMAND:ID>` | `</foo group bar:123456789012345678>` |
| Standard emoji | Unicode characters | 🪴 |
| Custom emoji | `<:NAME:ID>` | `<:mmLol:216154654256398347>` |
| Animated custom emoji | `<a:NAME:ID>` | `<a:b1nzy:392938283556143104>` |
| Unix timestamp | `<t:TIMESTAMP>` | `<t:1618953630>` |
| Styled unix timestamp | `<t:TIMESTAMP:STYLE>` | `<t:1618953630:d>` |

Timestamp styles: `t` Short Time, `T` Medium Time, `d` Short Date, `D` Long Date, `f` Long Date/Short Time (default), `F` Full Date/Short Time, `s` Short Date/Short Time, `S` Short Date/Medium Time, `R` Relative Time. Timestamps are expressed in seconds.

Guild navigation links use `<id:TYPE>` where `TYPE` is `customize`, `browse`, `guide`, `linked-roles`, or `linked-roles:<role_id>`.

Using the markdown for users or roles mentions the targets and notifies them, depending on sender permissions and the `allowed_mentions` field.

### 2.8 Image Formatting and CDN

The CDN base URL is `https://cdn.discordapp.com/`. Image hashes are retrieved through API requests. Change the returned format by changing the extension; change the size with a `?size=desired_size` querystring (any power of two between 16 and 4096).

Image formats: JPEG (`.jpg`, `.jpeg`), PNG (`.png`), WebP (`.webp`), GIF (`.gif`), Lottie (`.json`).

Key CDN endpoints:

| Type | Path |
|---|---|
| Custom Emoji | `emojis/{emoji_id}.png` |
| Guild Icon | `icons/{guild_id}/{guild_icon}.png` |
| User Avatar | `avatars/{user_id}/{user_avatar}.png` |
| Default User Avatar | `embed/avatars/{index}.png` |
| Guild Member Avatar | `guilds/{guild_id}/users/{user_id}/avatars/{member_avatar}.png` |
| Application Icon | `app-icons/{application_id}/{icon}.png` |
| Sticker | `stickers/{sticker_id}.png` |
| Role Icon | `role-icons/{role_id}/{role_icon}.png` |

Hashes that are available in animated format begin with `a_`. For those, request `.webp` or `.gif` with `?animated=true`. Sticker GIFs are served from `https://media.discordapp.net/stickers/<sticker_id>.gif`, not the CDN base URL.

Attachment URLs on the CDN are signed with expiry. `ex` is the expiration hex timestamp, `is` the issued hex timestamp, `hm` the signature. Discord refreshes attachment URLs automatically; you MAY pass CDN URLs into API fields (for example embed image `url`, webhook `avatar_url`) and Discord will render and refresh them. Standard CDN endpoints are not signed and do not expire.

### 2.9 Locales

Supported locales include: `id`, `da`, `de`, `en-GB`, `en-US`, `es-ES`, `es-419`, `fr`, `hr`, `it`, `lt`, `hu`, `nl`, `no`, `pl`, `pt-BR`, `ro`, `fi`, `sv-SE`, `vi`, `tr`, `cs`, `el`, `bg`, `ru`, `uk`, `hi`, `th`, `zh-CN`, `ja`, `zh-TW`, `ko`.

## 3. API Reference

This section documents the resource modules. Each resource section follows this schema: Overview, Prerequisites & Requirements, Type references, Syntax / Method Signature (endpoints), Return values, Side effects, and References.

Unless noted otherwise:

- Endpoints accept `X-Audit-Log-Reason` as an optional header to set the audit log reason.
- Endpoint permission requirements are expressed as Discord permission names; the requester MUST hold the stated permissions.
- Endpoints marked with a scope requirement MUST be called with an OAuth2 bearer token carrying that scope.

### 3.1 REST Conventions

#### Overview

Common REST behavior shared by all resources.

#### Prerequisites & Requirements

- You MUST provide a valid `Authorization` header on requests that require it.
- You MUST respect the [rate limits](#4-configuration-reference) and honor the returned headers.
- You MUST send a valid User Agent and Content-Type on every request.

#### Syntax / Method Signature

```
Base URL: https://discord.com/api/v{version}
Methods:   GET, POST, PUT, PATCH, DELETE
```

Responses: `200 OK`, `201 CREATED`, `204 NO CONTENT` (empty body), `304 NOT MODIFIED`. See [Error Handling](#8-error-handling) for error statuses.

#### Return values

- `200` — success with a body (object or array).
- `201` — entity created.
- `204` — success, no content.

#### Side effects

Many write operations fire Gateway events and write to the audit log. Endpoint pages note which events fire.

### 3.2 OAuth2

#### Overview

OAuth2 enables applications to build applications that use Discord authentication and data. Discord supports the authorization code grant, the implicit grant, the client credentials grant, and modified special-for-Discord flows for Bots and Webhooks.

#### Prerequisites & Requirements

- You MUST register a developer application and retrieve your `client_id` and `client_secret`.
- The token and token revocation URLs MUST be called with content type `application/x-www-form-urlencoded`. JSON content is not permitted and returns an error.
- All calls to OAuth2 endpoints MUST use either HTTP Basic authentication or `client_id` and `client_secret` supplied in the form data body.
- You MUST NOT rely on the `state` parameter being required by Discord; but you MUST implement it to protect against CSRF and clickjacking. It should bind the user's request to their authenticated session.
- You MUST NOT hard-code `client_id` or `client_secret` in client-side source code.

#### Type references

**OAuth2 URLs**

| URL | Purpose |
|---|---|
| `https://discord.com/oauth2/authorize` | Base authorization URL |
| `https://discord.com/api/oauth2/token` | Token URL |
| `https://discord.com/api/oauth2/token/revoke` | Token revocation URL |

**Common OAuth2 scopes**

| Scope | Description |
|---|---|
| `bot` | Adds the bot to the user's selected guild (default in bot flow) |
| `identify` | Allows `/users/@me` without `email` |
| `email` | Adds `email` to `/users/@me` |
| `guilds` | Allows `/users/@me/guilds` |
| `guilds.join` | Allows joining users to a guild |
| `guilds.members.read` | Returns a user's member info in a guild |
| `connections` | Returns linked third-party accounts |
| `applications.commands` | Allows adding commands to a guild; included by default with `bot` |
| `applications.commands.update` | Updates commands with a Bearer token (client credentials only) |
| `webhook.incoming` | Returns a webhook in the OAuth token response for authorization code grants |
| `role_connections.write` | Updates a user's connection and metadata for the app |

Some scopes require approval from Discord. The `role_connections.write` scope MUST NOT be used with the implicit grant.

#### Syntax / Method Signature — Flows

**Authorization Code Grant**

1. Redirect the user to:
   `https://discord.com/oauth2/authorize?response_type=code&client_id=CLIENT_ID&scope=identify%20guilds.join&state=STATE&redirect_uri=REDIRECT_URI`
2. On acceptance the user is redirected to `redirect_uri` with `code` and `state` query parameters. You MUST validate that `state` matches the stored value.
3. Exchange the code with `POST /api/oauth2/token`:
   - `grant_type` MUST be `authorization_code`
   - `code` — the code from the querystring
   - `redirect_uri` — the redirect URI associated with the authorization
4. Response: `access_token`, `token_type` (`Bearer`), `expires_in`, `refresh_token`, `scope`.
5. Refresh with `grant_type=refresh_token` and `refresh_token`.
6. Revoke with `POST /api/oauth2/token/revoke` with `token` and optional `token_type_hint` (`access_token` or `refresh_token`). Revoking a token revokes all active access and refresh tokens for that authorization.

`prompt` controls re-approval: `consent` re-prompts; `none` skips the authorization screen. `integration_type` (0 = GUILD_INSTALL, 1 = USER_INSTALL) is only relevant when `scope` contains `applications.commands`.

**Implicit Grant**

- URL: `https://discord.com/oauth2/authorize?response_type=token&client_id=CLIENT_ID&state=STATE&scope=identify`
- On redirect, the token is returned in URI fragments (`#access_token=...&token_type=...&expires_in=...&scope=...&state=...`), NOT query string parameters.
- No refresh token is returned; the user MUST re-authorize after expiry.

**Client Credentials Grant**

- `POST /api/oauth2/token` with `grant_type=client_credentials` and optional `scope`, using Basic authentication (client id as username, client secret as password).
- Returns an access token without a refresh token. Team applications are limited to the `identify` and `applications.commands.update` scopes.

**Bot Authorization Flow**

- URL: `https://discord.com/oauth2/authorize?client_id=CLIENT_ID&scope=bot&permissions=1`
- `scope` MUST include `bot`. `permissions` is an integer of requested permissions. `response_type` and `redirect_uri` are not required.
- Optional params: `guild_id` (pre-selects a guild), `disable_guild_select` (`true`/`false`).
- Bots with elevated permissions (marked `*` in the permissions table) require the owner's account to have two-factor authentication enabled when added to guilds with server-wide 2FA.

**Webhook Flow**

- URL: `https://discord.com/oauth2/authorize?response_type=code&client_id=CLIENT_ID&scope=webhook.incoming&state=STATE&redirect_uri=REDIRECT_URI`
- On acceptance, the token response includes a `webhook` object with `id` and `token`. You MUST store these to execute the webhook later.

**Bot vs User Accounts**

Bots are added to guilds through OAuth2 and cannot accept normal invites. Bots cannot have friends or join group DMs. You MUST NOT automate standard user accounts (self-bots) outside the OAuth2/bot API.

#### Syntax / Method Signature — Endpoints

```
GET /oauth2/applications/@me
- Description: Returns the bot's Application object.

GET /oauth2/@me
- Description: Returns the current authorization. Requires bearer token.
- Return: application (partial Application), scopes (array of strings), expires (ISO8601), user? (User object, when authorized with the `identify` scope).
```

#### Return values

Token exchange responses return `access_token`, `token_type`, `expires_in`, and `scope`; authorization code and webhook flows additionally return `refresh_token`.

#### References

https://docs.discord.com/developers/topics/oauth2.md

### 3.3 Permissions

#### Overview

Permissions limit and grant abilities to users in Discord. Base permissions are set at the guild level per role; overwrites modify them per channel for roles or members.

#### Prerequisites & Requirements

- Permissions MUST be serialized as strings in API v8 and above, including the `allow` and `deny` fields in overwrites.
- You MUST deserialize permissions using big-integer libraries for long-term stability.
- New permissions are only rolled into the base `permissions` field.

#### Type references

Permissions are stored in a variable-length integer serialized into a string. Combine individual values with OR (`|`); check flags with AND (`&`).

**Bitwise Permission Flags** (selected; `*` = requires owner 2FA on guilds with server-wide 2FA; channel-type abbreviations: T = text, V = voice, S = stage):

| Permission | Value | Applies |
|---|---|---|
| CREATE_INSTANT_INVITE | `1 << 0` | T, V, S |
| KICK_MEMBERS * | `1 << 1` | |
| BAN_MEMBERS * | `1 << 2` | |
| ADMINISTRATOR * | `1 << 3` | |
| MANAGE_CHANNELS * | `1 << 4` | T, V, S |
| MANAGE_GUILD * | `1 << 5` | |
| ADD_REACTIONS | `1 << 6` | T, V, S |
| VIEW_AUDIT_LOG | `1 << 7` | |
| PRIORITY_SPEAKER | `1 << 8` | V |
| STREAM | `1 << 9` | V, S |
| VIEW_CHANNEL | `1 << 10` | T, V, S |
| SEND_MESSAGES | `1 << 11` | T, V, S |
| SEND_TTS_MESSAGES | `1 << 12` | T, V, S |
| MANAGE_MESSAGES * | `1 << 13` | T, V, S |
| EMBED_LINKS | `1 << 14` | T, V, S |
| ATTACH_FILES | `1 << 15` | T, V, S |
| READ_MESSAGE_HISTORY | `1 << 16` | T, V, S |
| MENTION_EVERYONE | `1 << 17` | T, V, S |
| USE_EXTERNAL_EMOJIS | `1 << 18` | T, V, S |
| CONNECT | `1 << 20` | V, S |
| SPEAK | `1 << 21` | V |
| MUTE_MEMBERS | `1 << 22` | V, S |
| DEAFEN_MEMBERS | `1 << 23` | V |
| MOVE_MEMBERS | `1 << 24` | V, S |
| USE_VAD | `1 << 25` | V |
| CHANGE_NICKNAME | `1 << 26` | |
| MANAGE_NICKNAMES | `1 << 27` | |
| MANAGE_ROLES * | `1 << 28` | T, V, S |
| MANAGE_WEBHOOKS * | `1 << 29` | T, V, S |
| MANAGE_GUILD_EXPRESSIONS * | `1 << 30` | |
| USE_APPLICATION_COMMANDS | `1 << 31` | T, V, S |
| REQUEST_TO_SPEAK | `1 << 32` | S |
| MANAGE_EVENTS | `1 << 33` | V, S |
| MANAGE_THREADS * | `1 << 34` | T |
| CREATE_PUBLIC_THREADS | `1 << 35` | T |
| CREATE_PRIVATE_THREADS | `1 << 36` | T |
| USE_EXTERNAL_STICKERS | `1 << 37` | T, V, S |
| SEND_MESSAGES_IN_THREADS | `1 << 38` | T |
| USE_EMBEDDED_ACTIVITIES | `1 << 39` | T, V |
| MODERATE_MEMBERS | `1 << 40` | |
| VIEW_CREATOR_MONETIZATION_ANALYTICS * | `1 << 41` | |
| USE_SOUNDBOARD | `1 << 42` | V |
| CREATE_GUILD_EXPRESSIONS | `1 << 43` | |
| CREATE_EVENTS | `1 << 44` | V, S |
| USE_EXTERNAL_SOUNDS | `1 << 45` | V |
| SEND_VOICE_MESSAGES | `1 << 46` | T, V, S |
| SET_VOICE_CHANNEL_STATUS | `1 << 48` | V |
| SEND_POLLS | `1 << 49` | T, V, S |
| USE_EXTERNAL_APPS | `1 << 50` | T, V, S |
| PIN_MESSAGES | `1 << 51` | T |
| BYPASS_SLOWMODE | `1 << 52` | T, V, S |

**Role Object** (key fields): `id` (snowflake), `name`, `color` (deprecated integer), `colors` (Role Colors object with `primary_color`, `secondary_color?`, `tertiary_color?`), `hoist`, `icon?`, `unicode_emoji?`, `position`, `permissions` (string), `managed`, `mentionable`, `tags?`, `flags`. The `@everyone` role has the same ID as its guild.

**Role Tags Structure**: `bot_id?`, `integration_id?`, `premium_subscriber?` (null), `subscription_listing_id?`, `available_for_purchase?` (null), `guild_connections?` (null). Tags with type `null` are booleans: present-and-null means true, absent means false.

**Role Flags**: `IN_PROMPT` (`1 << 0`) — role can be selected in an onboarding prompt.

#### Syntax / Method Signature — Permission Hierarchy

Permissions apply in this order:

1. Base permissions for `@everyone` at guild level.
2. Permissions allowed by the user's roles at guild level.
3. `@everyone` overwrite denies at channel level.
4. `@everyone` overwrite allows at channel level.
5. Role overwrite denies at channel level.
6. Role overwrite allows at channel level.
7. Member overwrite denies at channel level.
8. Member overwrite allows at channel level.

Hierarchy rules for bots:

- A bot can grant roles of a lower position than its own highest role.
- A bot can edit roles of a lower position, granting only permissions it has.
- A bot can sort only roles lower than its highest role.
- A bot can kick, ban, and edit nicknames only for users whose highest role is lower than its highest role.

Otherwise permissions do not obey the role hierarchy; role positions do not resolve permission conflicts.

#### Return values

Overwrites use `allow` and `deny` strings. `ADMINISTRATOR` overrides all overwrites.

#### Side effects

- Denying `VIEW_CHANNEL` implicitly denies other permissions on the channel.
- Denying `SEND_MESSAGES` implicitly denies `MENTION_EVERYONE`, `SEND_TTS_MESSAGES`, `ATTACH_FILES`, and `EMBED_LINKS`.
- For voice and stage channels, denying `CONNECT` implicitly denies other permissions such as `MANAGE_CHANNEL`.
- Threads inherit permissions from their parent channel, except `SEND_MESSAGES` (threads require `SEND_MESSAGES_IN_THREADS`). `VIEW_CHANNEL` is required to view any thread.
- Timed out members lose all permissions except `VIEW_CHANNEL` and `READ_MESSAGE_HISTORY`. Owners and `ADMINISTRATOR` users are exempt.
- Channels synced to a parent category inherit changes; modifying a child channel desyncs it.

#### References

https://docs.discord.com/developers/topics/permissions.md

### 3.4 Rate Limits

#### Overview

Rate limits prevent spam, abuse, and service overload. Limits apply per route and globally, per bot or user.

#### Prerequisites & Requirements

- You MUST NOT hard-code rate limits into your app. They depend on many factors and change.
- You MUST parse the rate limit response headers and honor `Retry-After` or `retry_after`.
- You MUST use `X-RateLimit-Bucket` as the unique identifier for a rate limit.

#### Type references

| Header | Description |
|---|---|
| `X-RateLimit-Limit` | Number of requests that can be made |
| `X-RateLimit-Remaining` | Number of remaining requests |
| `X-RateLimit-Reset` | Epoch time (seconds) when the rate limit resets |
| `X-RateLimit-Reset-After` | Seconds until the bucket resets (may be fractional) |
| `X-RateLimit-Bucket` | Unique string denoting the rate limit encountered |
| `X-RateLimit-Global` | Only on `429`; `true` if the global limit was hit |
| `X-RateLimit-Scope` | Only on `429`; `user`, `global`, or `shared` |

Per-route limits account for top-level resources: channels (`channel_id`), guilds (`guild_id`), and webhooks (`webhook_id` or `webhook_id + webhook_token`). Two endpoints with different top-level resources are limited independently.

**429 Response Body**

| Field | Type | Description |
|---|---|---|
| message | string | Message saying you are being rate limited |
| retry_after | float | Seconds to wait before submitting another request |
| global | boolean | Whether the global limit was hit |
| code? | integer | An error code for some limits |

#### Syntax / Method Signature

Global rate limit: all bots MAY make up to 50 requests per second. With no authorization header, the limit applies to the IP address. Interaction endpoints are NOT bound to the bot's global rate limit.

Invalid request limit: IP addresses that make too many invalid HTTP requests are automatically and temporarily restricted. The limit is 10,000 invalid requests per 10 minutes. An invalid request results in `401`, `403`, or `429`. `429` errors with `X-RateLimit-Scope: shared` are not counted against you.

#### Return values

When a limit is exceeded, the API returns HTTP `429` with the JSON body above plus the normal route rate-limit headers.

#### Side effects

Users that regularly hit and ignore rate limits have their API keys revoked and are blocked from the platform. Emoji control routes are limited per guild and their reported quotas may be inaccurate.

#### References

https://docs.discord.com/developers/topics/rate-limits.md

### 3.5 Gateway (WebSocket)

#### Overview

The Gateway API is a persistent, stateful WebSocket connection to receive real-time events. Connections use secure WebSockets (RFC 6455).

#### Prerequisites & Requirements

- You MUST fetch and cache a WebSocket URL from `GET /gateway` or `GET /gateway/bot`.
- You MUST send an `Identify` (op 2) with `token`, `intents`, and `properties` (`os`, `browser`, `device`).
- Payloads MUST be JSON or ETF and MUST NOT exceed 4096 bytes (else close `4002`).
- You MUST heartbeat at the interval from Hello (op 10); include the last received sequence `s`.
- Intents are mandatory as of API v8. You MUST pass valid intent bits; invalid intents close with `4013`, unapproved privileged intents close with `4014`.
- Apps in 2500+ guilds MUST shard.

#### Type references

**Gateway Opcodes**

| Code | Name | Direction | Purpose |
|---|---|---|---|
| 0 | Dispatch | Receive | An event was dispatched |
| 1 | Heartbeat | Send/Receive | Keep connection alive |
| 2 | Identify | Send | Start a new session |
| 3 | Presence Update | Send | Update presence |
| 4 | Voice State Update | Send | Join/leave/move in voice |
| 6 | Resume | Send | Resume a disconnected session |
| 7 | Reconnect | Receive | Reconnect and resume immediately |
| 8 | Request Guild Members | Send | Request offline members |
| 9 | Invalid Session | Receive | Session invalidated; reconnect and identify/resume |
| 10 | Hello | Receive | Contains `heartbeat_interval` |
| 11 | Heartbeat ACK | Receive | Acknowledge a heartbeat |

**Gateway Intents** (`*` = privileged)

| Intent | Value | Privileged |
|---|---|---|
| GUILDS | `1 << 0` | no |
| GUILD_MEMBERS | `1 << 1` | yes |
| GUILD_MODERATION | `1 << 2` | no |
| GUILD_EXPRESSIONS | `1 << 3` | no |
| GUILD_INTEGRATIONS | `1 << 4` | no |
| GUILD_WEBHOOKS | `1 << 5` | no |
| GUILD_INVITES | `1 << 6` | no |
| GUILD_VOICE_STATES | `1 << 7` | no |
| GUILD_PRESENCES | `1 << 8` | yes |
| GUILD_MESSAGES | `1 << 9` | no |
| GUILD_MESSAGE_REACTIONS | `1 << 10` | no |
| GUILD_MESSAGE_TYPING | `1 << 11` | no |
| DIRECT_MESSAGES | `1 << 12` | no |
| DIRECT_MESSAGE_REACTIONS | `1 << 13` | no |
| DIRECT_MESSAGE_TYPING | `1 << 14` | no |
| MESSAGE_CONTENT | `1 << 15` | yes |
| GUILD_SCHEDULED_EVENTS | `1 << 16` | no |
| AUTO_MODERATION_CONFIGURATION | `1 << 20` | no |
| AUTO_MODERATION_EXECUTION | `1 << 21` | no |
| GUILD_MESSAGE_POLLS | `1 << 24` | no |
| DIRECT_MESSAGE_POLLS | `1 << 25` | no |

Privileged intents (`GUILD_PRESENCES`, `GUILD_MEMBERS`, `MESSAGE_CONTENT`) MUST be enabled in the Developer Portal before passing; verified apps MUST be approved. Without `MESSAGE_CONTENT`, apps receive empty `content`, `embeds`, `attachments`, and `components` fields and no `poll` in messages.

#### Syntax / Method Signature — Connection Lifecycle

1. Connect to `wss://gateway.discord.gg/?v=10&encoding=json`.
2. Receive Hello (op 10) with `heartbeat_interval`.
3. Start heartbeating (op 1); Discord ACKs with Heartbeat ACK (op 11).
4. Send Identify (op 2).
5. Receive Ready (op 0); cache `resume_gateway_url` and `session_id`.
6. On disconnect, resume (op 6) over `resume_gateway_url` with `token`, `session_id`, and last `seq`, or re-Identify after a fresh connection.

Heartbeat: wait `heartbeat_interval * jitter` (jitter = random 0–1), then send every interval. Respond immediately if Discord sends a Heartbeat. If no ACK arrives (zombied connection), terminate with any close code besides `1000`/`1001` and reconnect to resume.

Identify is limited to 1000 calls/24h globally across all shards (Resume is not counted). Exceeding it terminates all sessions, resets the token, and emails the owner.

Rate limiting: 120 gateway events per connection per 60 seconds (avg 2/s); exceeding disconnects you.

Sharding: `shard_id = (guild_id >> 22) % num_shards`. Pass `shard: [current_shard, num_shards]` in Identify. Events without `guild_id` only go to shard 0.

#### Return values

`GET /gateway` (unauthenticated) returns `{ "url" }`. `GET /gateway/bot` returns `url`, recommended `shards`, and `session_start_limit { total, remaining, reset_after, max_concurrency }`.

#### Side effects

Close codes determine whether you can resume:

| Code | Meaning | Reconnect |
|---|---|---|
| 4000 | Unknown error | yes |
| 4001 | Unknown opcode | yes |
| 4002 | Decode error | yes |
| 4003 | Not authenticated | yes |
| 4004 | Authentication failed | no |
| 4005 | Already authenticated | yes |
| 4007 | Invalid `seq` | yes |
| 4008 | Rate limited | yes |
| 4009 | Session timed out | yes |
| 4010 | Invalid shard | no |
| 4011 | Sharding required | no |
| 4012 | Invalid API version | no |
| 4013 | Invalid intent(s) | no |
| 4014 | Disallowed intent(s) | no |

Closing with code `1000`/`1001` invalidates the session; other closes keep the session alive for a few minutes.

#### References

https://docs.discord.com/developers/events/gateway.md

### 3.6 Application

#### Overview

Applications ("apps") are containers for developer-platform features, installable to servers and/or user accounts.

#### Prerequisites & Requirements

- A server-installed app MUST be authorized by a member with `MANAGE_GUILD`.
- Application integration types: 0 `GUILD_INSTALL`, 1 `USER_INSTALL`.

#### Type references

**Application Object** (key fields): `id`, `name`, `icon?`, `description`, `rpc_origins`, `bot_public`, `bot_require_code_grant`, `bot`, `terms_of_service_url`, `privacy_policy_url`, `owner`, `verify_key`, `team?`, `guild_id?`, `guild?`, `primary_sku_id?`, `slug`, `cover_image`, `flags`, `flags_new`, `approximate_guild_count`, `approximate_user_install_count`, `redirect_uris`, `interactions_endpoint_url?`, `role_connections_verification_url?`, `event_webhooks_url?`, `event_webhooks_status`, `event_webhooks_types`, `tags`, `install_params`, `integration_types_config`, `custom_install_url`.

**Application Flags** (selected): `GATEWAY_PRESENCE` `1<<12`, `GATEWAY_PRESENCE_LIMITED` `1<<13`, `GATEWAY_GUILD_MEMBERS` `1<<14`, `GATEWAY_GUILD_MEMBERS_LIMITED` `1<<15`, `EMBEDDED` `1<<17`, `GATEWAY_MESSAGE_CONTENT` `1<<18`, `GATEWAY_MESSAGE_CONTENT_LIMITED` `1<<19`. `flags` is number-serialized at 31 bits; new bits only appear in `flags_new` (string). Requests continue to use `flags`.

**Application Event Webhook Status**: 1 `DISABLED`, 2 `ENABLED`, 3 `DISABLED_BY_DISCORD`.

**Install Params Object**: `scopes` (array of strings), `permissions` (string).

#### Syntax / Method Signature

```
GET /applications/@me
- Description: Application object for the requesting bot user.
- Return: 200 Application.

PATCH /applications/@me
- Description: Edit app properties; only passed properties updated. All params optional.
- Params: custom_install_url, description, role_connections_verification_url, install_params, integration_types_config, flags (only the *_LIMITED flags updatable), icon, cover_image, interactions_endpoint_url, tags (max 20 chars each, max 5), event_webhooks_url, event_webhooks_status, event_webhooks_types.
- Return: 200 updated Application.

GET /applications/{application.id}/activity-instances/{instance_id}
- Description: Serialized activity instance, if it exists.
- Return: 200 Activity Instance object { application_id, instance_id, launch_id, location, users }.
```

#### Return values

Application objects as shown above; `200` on success.

#### References

https://docs.discord.com/developers/resources/application.md

### 3.7 User

#### Overview

Users are the base entity for Discord accounts. Bot users are owned by another user and have no guild limit.

#### Prerequisites & Requirements

- Fields marked with a scope in the User object require an OAuth2 bearer token carrying that scope.
- Usernames are 2–32 characters, nicknames 1–32 characters. Usernames MUST NOT contain `@`, `#`, `:`, `` ` ``, or the substring `discord`, and MUST NOT equal `everyone` or `here`.

#### Type references

**User Object** (key fields; `scope` column shows the required OAuth2 scope):

| Field | Type | Scope |
|---|---|---|
| id | snowflake | identify |
| username | string | identify |
| discriminator | string | identify |
| global_name | ?string | identify |
| avatar | ?string | identify |
| bot | boolean | identify |
| system | boolean | identify |
| mfa_enabled | boolean | identify |
| banner | ?string | identify |
| accent_color | ?integer | identify |
| locale | string | identify |
| verified | boolean | email |
| email | ?string | email |
| flags | integer | identify |
| premium_type | integer | identify.premium |
| public_flags | integer | identify |

**User Flags** (selected): `STAFF` `1<<0`, `PARTNER` `1<<1`, `BUG_HUNTER_LEVEL_1` `1<<3`, `PREMIUM_EARLY_SUPPORTER` `1<<9`, `BUG_HUNTER_LEVEL_2` `1<<14`, `VERIFIED_BOT` `1<<16`, `VERIFIED_DEVELOPER` `1<<17`, `CERTIFIED_MODERATOR` `1<<18`, `BOT_HTTP_INTERACTIONS` `1<<19`.

**Premium Types**: 0 None, 1 Nitro Classic, 2 Nitro, 3 Nitro Basic.

#### Syntax / Method Signature

```
GET /users/@me
- Description: Current user's User object. Requires `identify` scope; `email` scope adds `email`.
- Return: 200 User.

GET /users/{user.id}
- Description: User object for a given user id.
- Return: 200 User.

PATCH /users/@me
- Description: Modify current user settings. All params optional. Fires User Update.
- Params: username, avatar (?image data), banner (?image data).
- Return: 200 updated User.

GET /users/@me/guilds
- Description: List partial Guild objects. Requires `guilds` scope.
- Query: before, after, limit (1-200, default 200), with_counts.
- Return: 200 array of partial Guild.

GET /users/@me/guilds/{guild.id}/member
- Description: Current user's Guild Member object. Requires `guilds.members.read`.
- Return: 200 Guild Member.

DELETE /users/@me/guilds/{guild.id}
- Description: Leave guild. Fires Guild Delete and Guild Member Remove.
- Return: 204.

POST /users/@me/channels
- Description: Create DM channel (returns existing if present).
- Body: recipient_id (snowflake).
- Return: 200 DM Channel.

GET /users/@me/connections
- Description: List Connection objects. Requires `connections`.
- Return: 200 array of Connection.

GET/PUT/DELETE /users/@me/applications/{application.id}/role-connection
- Description: Get/update/delete the application role connection. Requires `role_connections.write`.
- Return: 200 Application Role Connection (204 on DELETE).
```

#### Return values

User, Guild Member, DM Channel, Connection, and Application Role Connection objects as documented.

#### Side effects

`PATCH /users/@me` fires `USER_UPDATE`; changing the username may randomize the discriminator.

#### References

https://docs.discord.com/developers/resources/user.md

### 3.8 Guild

#### Overview

Guilds ("servers") are isolated collections of users and channels.

#### Prerequisites & Requirements

- Most mutation endpoints require the stated permissions (typically `MANAGE_GUILD`).
- `GET /guilds/{guild.id}/members` REQUIRES the `GUILD_MEMBERS` privileged intent.

#### Type references

**Guild Object** (key fields): `id`, `name` (2–100 chars), `icon?`, `splash?`, `discovery_splash?`, `owner` (only via `/users/@me/guilds`), `owner_id`, `permissions` (requesting user, excludes overwrites), `afk_channel_id?`, `afk_timeout`, `verification_level`, `default_message_notifications`, `explicit_content_filter`, `roles` (array of Role), `emojis`, `stickers`, `features`, `mfa_level`, `application_id?`, `system_channel_id?`, `system_channel_flags`, `rules_channel_id?`, `max_presences?`, `max_members`, `vanity_url_code?`, `description?`, `banner?`, `premium_tier`, `premium_subscription_count`, `preferred_locale`, `public_updates_channel_id?`, `nsfw_level`, `premium_progress_bar_enabled`, `safety_alerts_channel_id?`, `incidents_data?`, `approximate_member_count?`, `approximate_presence_count?`.

| Enum | Values |
|---|---|
| Default Message Notifications | 0 ALL_MESSAGES, 1 ONLY_MENTIONS |
| Explicit Content Filter | 0 DISABLED, 1 MEMBERS_WITHOUT_ROLES, 2 ALL_MEMBERS |
| MFA Level | 0 NONE, 1 ELEVATED |
| Verification Level | 0 NONE, 1 LOW, 2 MEDIUM, 3 HIGH, 4 VERY_HIGH |
| Premium Tier | 0 NONE, 1 TIER_1, 2 TIER_2, 3 TIER_3 |
| NSFW Level | 0 DEFAULT, 1 EXPLICIT, 2 SAFE, 3 AGE_RESTRICTED |
| System Channel Flags | 1<<0 SUPPRESS_JOIN_NOTIFICATIONS, 1<<1 SUPPRESS_PREMIUM_SUBSCRIPTIONS, 1<<2 SUPPRESS_GUILD_REMINDER_NOTIFICATIONS, 1<<3 SUPPRESS_JOIN_NOTIFICATION_REPLIES |

**Guild Member Object** (key fields): `user`, `nick?`, `avatar?`, `banner?`, `roles` (array of snowflake role ids), `joined_at`, `premium_since?`, `deaf`, `mute`, `flags`, `pending?`, `permissions` (with overwrites, in interactions), `communication_disabled_until?`.

**Guild Features** (string list, selected): `ANIMATED_BANNER`, `ANIMATED_ICON`, `AUTO_MODERATION`, `BANNER`, `COMMUNITY`, `CREATOR_STORE_PAGE`, `DISCOVERABLE`, `FEATURABLE`, `INVITES_DISABLED`, `INVITE_SPLASH`, `MEMBER_VERIFICATION_GATE_ENABLED`, `NEWS`, `PARTNERED`, `PREVIEW_ENABLED`, `RAID_ALERTS_DISABLED`, `ROLE_ICONS`, `ROLE_SUBSCRIPTIONS_ENABLED`, `SOUNDBOARD`, `TICKETED_EVENTS_ENABLED`, `VANITY_URL`, `VERIFIED`, `VIP_REGIONS`, `WELCOME_SCREEN_ENABLED`, `GUESTS_ENABLED`, `GUILD_TAGS`, `ENHANCED_ROLE_COLORS`.

#### Syntax / Method Signature

```
GET /guilds/{guild.id}
- Description: Full Guild object.
- Query: with_counts (bool, default false).
- Return: 200 Guild.

GET /guilds/{guild.id}/preview
- Description: Guild Preview. If requester is not in the guild, the guild MUST be DISCOVERABLE.
- Return: 200 Guild Preview.

PATCH /guilds/{guild.id}
- Description: Modify guild settings. Requires MANAGE_GUILD; COMMUNITY/DISCOVERABLE changes require ADMINISTRATOR.
- Body (all optional): name, verification_level, default_message_notifications, explicit_content_filter, afk_channel_id, afk_timeout (60/300/900/1800/3600), icon, splash, discovery_splash, banner, system_channel_id, system_channel_flags, rules_channel_id, public_updates_channel_id, preferred_locale, features, description, premium_progress_bar_enabled, safety_alerts_channel_id.
- Return: 200 updated Guild. Fires Guild Update.

GET /guilds/{guild.id}/channels
- Description: List guild channels (excludes threads). Return: 200 array of Channel.

POST /guilds/{guild.id}/channels
- Description: Create channel. Requires MANAGE_CHANNELS. All params optional except name.
- Body: name, type, topic, bitrate, user_limit, rate_limit_per_user (0-21600), position, permission_overwrites, parent_id, nsfw, rtc_region, video_quality_mode, default_auto_archive_duration, default_reaction_emoji, available_tags, default_sort_order, default_forum_layout, default_thread_rate_limit_per_user, flags.
- Return: 200 new Channel. Fires Channel Create.

PATCH /guilds/{guild.id}/channels
- Description: Modify channel positions. Requires MANAGE_CHANNELS. At most one entry may change parent_id.
- Body: JSON array of { id, position?, lock_permissions?, parent_id?, flags? }.
- Return: 204. Fires Channel Update events.

GET /guilds/{guild.id}/threads/active
- Description: All active threads (public + private), ordered by id descending.
- Return: 200 { threads, members }.

GET /guilds/{guild.id}/members/{user.id}
- Description: Get guild member. Return: 200 Guild Member.

GET /guilds/{guild.id}/members
- Description: List guild members. REQUIRES GUILD_MEMBERS intent.
- Query: limit (1-1000, default 1), after (snowflake, default 0).
- Return: 200 array of Guild Member.

GET /guilds/{guild.id}/members/search
- Description: Search members whose username/nickname starts with `query`.
- Query: query, limit (1-1000, default 1).
- Return: 200 array of Guild Member.

PUT /guilds/{guild.id}/members/{user.id}
- Description: Add user to guild. Requires OAuth2 token with guilds.join scope; bot must be in the guild with CREATE_INSTANT_INVITE. Only access_token required.
- Body: access_token, nick (MANAGE_NICKNAMES), roles (MANAGE_ROLES), mute (MUTE_MEMBERS), deaf (DEAFEN_MEMBERS).
- Return: 201 Guild Member (204 if already a member). Fires Guild Member Add.

PATCH /guilds/{guild.id}/members/{user.id}
- Description: Modify member attributes. All params optional.
- Body: nick (MANAGE_NICKNAMES), roles (MANAGE_ROLES), mute (MUTE_MEMBERS), deaf (DEAFEN_MEMBERS), channel_id (MOVE_MEMBERS; null disconnects), communication_disabled_until (MODERATE_MEMBERS; max 28 days), flags (MANAGE_GUILD or MANAGE_ROLES or MODERATE+KICK+BAN).
- Return: 200 Guild Member. Fires Guild Member Update.

DELETE /guilds/{guild.id}/members/{user.id}
- Description: Remove member (kick). Requires KICK_MEMBERS. Return: 204. Fires Guild Member Remove.

GET /guilds/{guild.id}/bans
- Description: List bans. Requires BAN_MEMBERS.
- Query: limit (default 1000), before?, after?.
- Return: 200 array of Ban.

PUT /guilds/{guild.id}/bans/{user.id}
- Description: Create ban. Requires BAN_MEMBERS.
- Body: delete_message_days (0-7, deprecated), delete_message_seconds (0-604800, default 0).
- Return: 204. Fires Guild Ban Add.

DELETE /guilds/{guild.id}/bans/{user.id}
- Description: Remove ban. Requires BAN_MEMBERS. Return: 204. Fires Guild Ban Remove.

POST /guilds/{guild.id}/bulk-ban
- Description: Ban up to 200 users. Requires BAN_MEMBERS AND MANAGE_GUILD.
- Body: user_ids (max 200), delete_message_seconds (0-604800, default 0).
- Return: 200 { banned_users: [snowflake], failed_users: [snowflake] }.

POST /guilds/{guild.id}/roles
- Description: Create role. Requires MANAGE_ROLES. All params optional.
- Body: name (max 100, default "new role"), permissions, color (deprecated), colors, hoist, icon, unicode_emoji, mentionable.
- Return: 200 new Role. Fires Guild Role Create.

PATCH /guilds/{guild.id}/roles
- Description: Modify role positions. Requires MANAGE_ROLES.
- Body: JSON array of { id, position? }.
- Return: 200 list of all guild Roles.

PATCH /guilds/{guild.id}/roles/{role.id}
- Description: Modify role. Requires MANAGE_ROLES.
- Return: 200 updated Role. Fires Guild Role Update.

DELETE /guilds/{guild.id}/roles/{role.id}
- Description: Delete role. Requires MANAGE_ROLES. Return: 204. Fires Guild Role Delete.

GET /guilds/{guild.id}/prune
- Description: Prune count preview. Requires MANAGE_GUILD AND KICK_MEMBERS.
- Query: days (1-30, default 7), include_roles.
- Return: 200 { pruned }.

POST /guilds/{guild.id}/prune
- Description: Begin prune. Requires MANAGE_GUILD AND KICK_MEMBERS.
- Return: 200 { pruned }. Fires Guild Member Remove events.

GET /guilds/{guild.id}/invites
- Description: List invites. Requires MANAGE_GUILD or VIEW_AUDIT_LOG.
- Return: 200 array of Invite.

GET /guilds/{guild.id}/vanity-url
- Description: Vanity invite code. Requires MANAGE_GUILD.
- Return: 200 partial Invite { code, uses }.

GET /guilds/{guild.id}/welcome-screen
- Description: Welcome Screen. MANAGE_GUILD required if the screen is disabled.

PATCH /guilds/{guild.id}/welcome-screen
- Description: Modify welcome screen. Requires MANAGE_GUILD.
- Body: enabled, welcome_channels, description.
- Return: 200 updated.

GET/PUT /guilds/{guild.id}/onboarding
- Description: Get/modify Guild Onboarding. PUT requires MANAGE_GUILD AND MANAGE_ROLES.
- Return: 200 Guild Onboarding object.

GET /guilds/{guild.id}/integrations / DELETE /guilds/{guild.id}/integrations/{integration.id}
- Description: List (max 50) / delete integrations. Requires MANAGE_GUILD.
- Return: 200 array / 204.
```

#### Return values

Guild and related objects as documented; `200`/`201`/`204` on success.

#### Side effects

Mutating endpoints fire the corresponding Gateway events (Guild Update, Channel Create, Guild Member Add, etc.) and support the `X-Audit-Log-Reason` header.

#### References

https://docs.discord.com/developers/resources/guild.md

### 3.9 Channel

#### Overview

Represents a guild or DM channel. Channel types include guild text/voice/announcement, DMs, group DMs, categories, threads, stage, directory, forum, and media channels.

#### Prerequisites & Requirements

- Modify operations require `MANAGE_CHANNELS` (or `MANAGE_ROLES` when modifying permission overwrites).
- Thread-specific modifications require `MANAGE_THREADS` or the thread creator in some cases.

#### Type references

**Channel Types**

| Type | Value |
|---|---|
| GUILD_TEXT | 0 |
| DM | 1 |
| GUILD_VOICE | 2 |
| GROUP_DM | 3 |
| GUILD_CATEGORY | 4 |
| GUILD_ANNOUNCEMENT | 5 |
| ANNOUNCEMENT_THREAD | 10 |
| PUBLIC_THREAD | 11 |
| PRIVATE_THREAD | 12 |
| GUILD_STAGE_VOICE | 13 |
| GUILD_DIRECTORY | 14 |
| GUILD_FORUM | 15 |
| GUILD_MEDIA | 16 |

**Channel Object** (key fields): `id`, `type`, `guild_id?`, `position?`, `permission_overwrites?` (array of Overwrite), `name?` (1–100 chars), `topic?`, `nsfw?`, `last_message_id?`, `bitrate?`, `user_limit?`, `rate_limit_per_user?` (0–21600), `recipients?`, `icon?`, `owner_id?`, `application_id?`, `parent_id?`, `last_pin_timestamp?`, `rtc_region?`, `video_quality_mode?`, `message_count?`, `member_count?`, `thread_metadata?`, `member?`, `default_auto_archive_duration?`, `permissions?`, `flags?`, `total_message_sent?`, `available_tags?`, `applied_tags?`, `default_reaction_emoji?`, `default_sort_order?`, `default_forum_layout?`.

**Overwrite Structure**: `id` (snowflake), `type` (0 = role, 1 = member), `allow` (string), `deny` (string).

**Thread Metadata**: `archived`, `auto_archive_duration` (60/1440/4320/10080), `archive_timestamp`, `locked`, `invitable?`, `create_timestamp?`.

**Channel Flags**: `PINNED` `1 << 1`, `REQUIRE_TAG` `1 << 4`, `HIDE_MEDIA_DOWNLOAD_OPTIONS` `1 << 15`, `IS_SPOILER_CHANNEL` `1 << 21`.

#### Syntax / Method Signature

```
GET /channels/{channel.id}
- Description: Get a channel by ID.
- Return: 200 Channel object.

PATCH /channels/{channel.id}
- Description: Update a channel's settings. All params optional.
- Body: name, type (text<->announcement), position, topic, nsfw, rate_limit_per_user, bitrate, user_limit, permission_overwrites, parent_id, rtc_region, video_quality_mode, default_auto_archive_duration, flags, available_tags (max 20), default_reaction_emoji, default_thread_rate_limit_per_user, default_sort_order, default_forum_layout.
- Return: 200 Channel. Fires Channel Update. 400 on invalid parameters.

DELETE /channels/{channel.id}
- Description: Delete a channel or close a DM. Requires MANAGE_CHANNELS (MANAGE_THREADS if a thread). Cannot be undone for guild channels.
- Return: 200 Channel. Fires Channel Delete / Thread Delete.

PUT /channels/{channel.id}/permissions/{overwrite.id}
- Description: Edit permission overwrites. Requires MANAGE_ROLES.
- Body: allow (default "0"), deny (default "0"), type (0=role, 1=member).
- Return: 204. Fires Channel Update.

DELETE /channels/{channel.id}/permissions/{overwrite.id}
- Description: Delete a permission overwrite. Requires MANAGE_ROLES. Return: 204.

GET /channels/{channel.id}/invites
- Description: List invites. Requires MANAGE_CHANNELS. Return: 200 array of Invite (with metadata).

POST /channels/{channel.id}/invites
- Description: Create an invite. Requires CREATE_INSTANT_INVITE. Body optional but MUST send `{}` if empty.
- Body: max_age (0-604800, default 86400), max_uses (0-100, default 0), temporary, unique, target_type, target_user_id, target_application_id, role_ids (requires MANAGE_ROLES).
- Return: 200 Invite. Fires Invite Create.

POST /channels/{channel.id}/followers
- Description: Follow an Announcement Channel. Requires MANAGE_WEBHOOKS in the target channel.
- Body: webhook_channel_id.
- Return: 200 Followed Channel. Fires Webhooks Update.

POST /channels/{channel.id}/typing
- Description: Post a typing indicator (expires after 10 seconds). Bots generally SHOULD NOT use this.
- Return: 204. Fires Typing Start.

POST /channels/{channel.id}/messages/{message.id}/threads
- Description: Start a thread from an existing message. Creates PUBLIC_THREAD (GUILD_TEXT) or ANNOUNCEMENT_THREAD (GUILD_ANNOUNCEMENT). Thread id equals the source message id.
- Body: name (1-100), auto_archive_duration?, rate_limit_per_user?.
- Return: 200 Channel. Fires Thread Create and Message Update.

POST /channels/{channel.id}/threads
- Description: Start a thread without a message (PRIVATE_THREAD by default).
- Body: name, auto_archive_duration?, type?, invitable?, rate_limit_per_user?.
- Return: 200 Channel. Fires Thread Create.

POST /channels/{channel.id}/threads  [forum/media variant]
- Description: Start a thread in a forum/media channel and send the first message. Current user MUST have SEND_MESSAGES. MUST provide at least one of content/embeds/sticker_ids/components/files[n].
- Body: name, auto_archive_duration?, rate_limit_per_user?, message, applied_tags?, files[n]?, payload_json?.
- Return: 200 Channel with nested Message. Fires Thread Create and Message Create.

PUT /channels/{channel.id}/thread-members/@me
- Description: Join a thread (must not be archived). Return: 204.

PUT /channels/{channel.id}/thread-members/{user.id}
- Description: Add another member to a thread. Return: 204.

DELETE /channels/{channel.id}/thread-members/@me
- Description: Leave a thread (must not be archived). Return: 204.

DELETE /channels/{channel.id}/thread-members/{user.id}
- Description: Remove another member. Requires MANAGE_THREADS (or thread creator for PRIVATE_THREAD). Return: 204.

GET /channels/{channel.id}/thread-members
- Description: List thread members. Restricted by GUILD_MEMBERS privileged intent.
- Query: with_member?, after?, limit? (1-100, default 100).
- Return: 200 array of Thread Member.

GET /channels/{channel.id}/threads/archived/public
- Description: List public archived threads, ordered by archive_timestamp descending. Requires READ_MESSAGE_HISTORY.
- Query: before? (ISO8601), limit?.
- Return: 200 { threads, members, has_more }.

GET /channels/{channel.id}/threads/archived/private
- Description: List private archived threads. Requires READ_MESSAGE_HISTORY and MANAGE_THREADS.
- Return: 200 { threads, members, has_more }.

GET /channels/{channel.id}/users/@me/threads/archived/private
- Description: List private archived threads the current user joined. Requires READ_MESSAGE_HISTORY.
- Return: 200 { threads, members, has_more }.
```

#### Return values

Channel objects, arrays, or `204` empty responses as shown.

#### Side effects

Write operations fire Channel Update, Thread Create/Delete, Thread Members Update, Typing Start, Invite Create, and Webhooks Update events, and support `X-Audit-Log-Reason`.

#### References

https://docs.discord.com/developers/resources/channel.md

### 3.10 Message

#### Overview

Represents a message sent in a channel. Covers message retrieval, creation, editing, deletion, reactions, pins, and search.

#### Prerequisites & Requirements

- Without the `MESSAGE_CONTENT` privileged intent, apps receive empty `content`, `embeds`, `attachments`, and `components` and the `poll` field is omitted.
- You MUST provide at least one of `content`, `embeds`, `sticker_ids`, `components`, `files[n]`, `poll`, or `shared_client_theme` when creating a message.
- Reactions: the emoji MUST be URL-encoded, or the request fails with `10014` (Unknown Emoji). Custom emojis encode as `name:id`.

#### Type references

**Message Object** (key fields): `id`, `channel_id`, `author`, `content`, `timestamp`, `edited_timestamp?`, `tts`, `mention_everyone`, `mentions`, `mention_roles`, `mention_channels?`, `attachments`, `embeds`, `reactions?`, `nonce?`, `pinned`, `webhook_id?`, `type`, `application_id?`, `flags?`, `message_reference?`, `referenced_message?`, `interaction_metadata?`, `thread?`, `components?`, `sticker_items?`, `poll?`.

**Message Types** (selected): DEFAULT 0, RECIPIENT_ADD 1, RECIPIENT_REMOVE 2, CHANNEL_PINNED_MESSAGE 6, REPLY 19, CHAT_INPUT_COMMAND 20, THREAD_STARTER_MESSAGE 21, CONTEXT_MENU_COMMAND 23, AUTO_MODERATION_ACTION 24.

**Message Flags**: `CROSSPOSTED` `1<<0`, `IS_CROSSPOST` `1<<1`, `SUPPRESS_EMBEDS` `1<<2`, `SOURCE_MESSAGE_DELETED` `1<<3`, `URGENT` `1<<4`, `HAS_THREAD` `1<<5`, `EPHEMERAL` `1<<6`, `LOADING` `1<<7`, `SUPPRESS_NOTIFICATIONS` `1<<12`, `IS_VOICE_MESSAGE` `1<<13`, `HAS_SNAPSHOT` `1<<14`, `IS_COMPONENTS_V2` `1<<15` (once set, cannot be removed).

**Embed Limits**: title ≤256 chars, description ≤4096, ≤25 fields, field name ≤256, field value ≤1024, footer text ≤2048, author name ≤256. The combined sum across all embeds in a message MUST NOT exceed 6000 characters. Embeds are deduplicated by URL.

**Allowed Mentions**: `parse?` (array of `roles`/`users`/`everyone`), `roles?` (max 100), `users?` (max 100), `replied_user?` (default false). `parse` is mutually exclusive with `roles`/`users`. Default when unset: regular messages parse all types; interactions and webhooks parse only `["users"]`.

**Attachment Structure**: `id`, `filename`, `title?`, `description?` (max 1024), `content_type?`, `size`, `url`, `proxy_url`, `height?`, `width?`, `ephemeral?`, `duration_secs?`, `waveform?`, `flags?`.

#### Syntax / Method Signature

```
GET /channels/{channel.id}/messages
- Description: Retrieves messages, newest to oldest. Requires VIEW_CHANNEL (plus CONNECT for voice).
- Query: around?/before?/after? (mutually exclusive), limit? (1-100, default 50).
- Return: 200 array of Message.

GET /guilds/{guild.id}/messages/search
- Description: Search messages in a guild. Requires READ_MESSAGE_HISTORY. Restricted by MESSAGE_CONTENT privileged intent.
- Query: limit? (1-25, default 25), offset? (max 9975), content? (max 1024), channel_id?, author_id?, mentions?, pinned?, has?, sort_by?, sort_order?, include_nsfw?, and more.
- Return: 200 search result object { total_results, messages, threads?, members? }.

GET /channels/{channel.id}/messages/{message.id}
- Description: Get a specific message. Requires VIEW_CHANNEL and READ_MESSAGE_HISTORY.
- Return: 200 Message.

POST /channels/{channel.id}/messages
- Description: Post a message to a guild text or DM channel. Requires SEND_MESSAGES; tts requires SEND_TTS_MESSAGES. Max request size 25 MiB.
- Body: content (<=2000), nonce (<=25 chars), tts, embeds (up to 10, <=6000 chars), allowed_mentions, message_reference, components, sticker_ids (up to 3), files[n], payload_json, attachments, flags, poll, shared_client_theme.
- Return: 200 Message. Fires Message Create.

POST /channels/{channel.id}/messages/{message.id}/crosspost
- Description: Crosspost a message in an Announcement Channel. Requires SEND_MESSAGES (or MANAGE_MESSAGES if not the author).
- Return: 200 Message. Fires Message Update.

PUT /channels/{channel.id}/messages/{message.id}/reactions/{emoji.id}/@me
- Description: Create a reaction. Requires READ_MESSAGE_HISTORY, plus ADD_REACTIONS if nobody else has reacted with this emoji.
- Return: 204. Fires Message Reaction Add.

DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji.id}/@me
- Description: Delete own reaction. Return: 204. Fires Message Reaction Remove.

DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji.id}/{user.id}
- Description: Delete another user's reaction. Requires MANAGE_MESSAGES. Return: 204.

GET /channels/{channel.id}/messages/{message.id}/reactions/{emoji.id}
- Description: Get users that reacted with this emoji.
- Query: type? (0 NORMAL, 1 BURST), after?, limit? (1-100, default 25).
- Return: 200 array of User.

DELETE /channels/{channel.id}/messages/{message.id}/reactions
- Description: Delete all reactions. Requires MANAGE_MESSAGES.

DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji.id}
- Description: Delete all reactions for a given emoji. Requires MANAGE_MESSAGES.

PATCH /channels/{channel.id}/messages/{message.id}
- Description: Edit a previously sent message. Original author may edit content/embeds/flags/components; others may edit only flags with MANAGE_MESSAGES. When editing flags, MUST include all previously set flags plus modifications. All params optional and nullable.
- Body: content (<=2000), embeds (up to 10), flags, allowed_mentions, components, files[n], payload_json, attachments.
- Return: 200 Message. Fires Message Update.

DELETE /channels/{channel.id}/messages/{message.id}
- Description: Delete a message. Requires MANAGE_MESSAGES if not the author.
- Return: 204. Fires Message Delete.

POST /channels/{channel.id}/messages/bulk-delete
- Description: Delete multiple messages (guild channels only). Requires MANAGE_MESSAGES. Messages older than 2 weeks are not deleted; any provided old or duplicate ID causes a 400.
- Body: messages (2-100 snowflakes).
- Return: 204. Fires Message Delete Bulk.

GET /channels/{channel.id}/messages/pins
- Description: Get channel pins. Requires VIEW_CHANNEL.
- Query: before?, limit? (1-50, default 50).
- Return: 200 { items, has_more }.

PUT /channels/{channel.id}/messages/pins/{message.id}
- Description: Pin a message. Requires PIN_MESSAGES. Fires Channel Pins Update.

DELETE /channels/{channel.id}/messages/pins/{message.id}
- Description: Unpin a message. Requires PIN_MESSAGES. Return: 204. Fires Channel Pins Update.
```

#### Return values

Message objects or `204` empty responses as shown.

#### Side effects

Write operations fire Message Create, Message Update, Message Delete, Message Reaction Add/Remove, and Channel Pins Update events. `PATCH` edits reconstruct mentions from scratch using the request `allowed_mentions`.

#### References

https://docs.discord.com/developers/resources/message.md

### 3.11 Webhook

#### Overview

Webhooks are a low-effort way to post messages to Discord channels without a bot user or authentication.

#### Prerequisites & Requirements

- Create/modify/delete of webhooks (with the exception of token-authenticated variants) requires `MANAGE_WEBHOOKS`.
- A webhook name MUST NOT contain the substrings `clyde` or `discord` (case-insensitive) and MUST be 1–80 characters.
- Executing a webhook MUST provide at least one of `content`, `embeds`, `components`, `file`, or `poll`.

#### Type references

**Webhook Object** (key fields): `id`, `type`, `guild_id?`, `channel_id?`, `user?`, `name?`, `avatar?`, `token?`, `application_id?`, `source_guild?`, `source_channel?`, `url?`.

**Webhook Types**: 1 Incoming, 2 Channel Follower, 3 Application.

#### Syntax / Method Signature

```
POST /channels/{channel.id}/webhooks
- Description: Create a webhook. Requires MANAGE_WEBHOOKS. Fires Webhooks Update.
- Body: name (1-80), avatar? (?image data).
- Return: 200 Webhook.

GET /channels/{channel.id}/webhooks
- Description: Get channel webhooks. Requires MANAGE_WEBHOOKS. Return: 200 array of Webhook.

GET /guilds/{guild.id}/webhooks
- Description: Get guild webhooks. Requires MANAGE_WEBHOOKS. Return: 200 array of Webhook.

GET /webhooks/{webhook.id}
- Description: Get a webhook. Requires MANAGE_WEBHOOKS unless the app owns it. Return: 200 Webhook.

GET /webhooks/{webhook.id}/{webhook.token}
- Description: Get a webhook with no authentication; returns no `user`. Return: 200 Webhook.

PATCH /webhooks/{webhook.id}
- Description: Modify a webhook. Requires MANAGE_WEBHOOKS. All params optional.
- Body: name, avatar, channel_id (new channel to move to).
- Return: 200 updated Webhook. Fires Webhooks Update.

PATCH /webhooks/{webhook.id}/{webhook.token}
- Description: Modify with no authentication; does NOT accept `channel_id`. Return: 200 updated Webhook.

DELETE /webhooks/{webhook.id}
- Description: Delete permanently. Requires MANAGE_WEBHOOKS. Return: 204.

DELETE /webhooks/{webhook.id}/{webhook.token}
- Description: Delete with no authentication. Return: 204.

POST /webhooks/{webhook.id}/{webhook.token}
- Description: Execute a webhook (send a message). Forum/media channel: MUST provide `thread_id` or `thread_name`.
- Query: wait? (bool, default false; returns created message when true), thread_id?, with_components? (default false).
- Body: content (<=2000), username (override), avatar_url (override), tts, embeds (up to 10), allowed_mentions, components, files[n], payload_json, attachments, flags, thread_name (forum/media only), applied_tags (forum/media only), poll.
- Return: 200 Message or 204 No Content depending on `wait`.

POST /webhooks/{webhook.id}/{webhook.token}/slack
- Description: Slack-compatible execution. Query: thread_id?, wait? (default true).

POST /webhooks/{webhook.id}/{webhook.token}/github
- Description: GitHub-compatible execution. Query: thread_id?, wait? (default true).

GET /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}
- Description: Get a previously-sent webhook message (same token). Query: thread_id?. Return: 200 Message.

PATCH /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}
- Description: Edit a previously-sent webhook message. All params optional and nullable.
- Query: thread_id?, with_components?.
- Body: content, embeds, flags, allowed_mentions, components, files[n], payload_json, attachments, poll.
- Return: 200 Message.

DELETE /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}
- Description: Delete a message created by the webhook. Query: thread_id?. Return: 204.
```

#### Return values

Webhook objects, arrays, Message objects, or `204` empty responses as shown.

#### Side effects

Create/modify/delete operations fire the Webhooks Update event and support `X-Audit-Log-Reason`.

#### References

https://docs.discord.com/developers/resources/webhook.md

### 3.12 Interactions

#### Overview

Interactions let apps respond to user-initiated events (commands, components, modals, autocomplete) over HTTP.

#### Prerequisites & Requirements

- You MUST send the initial response within 3 seconds of receiving the event, or the interaction token is invalidated.
- The interaction `token` is valid for 15 minutes for followups.
- Interaction endpoints are NOT bound to the bot's global rate limit.

#### Type references

**Interaction Object** (key fields): `id`, `application_id`, `type`, `data?`, `guild_id?`, `channel_id?`, `member` (guild) or `user` (DM), `token`, `version` (always `1`), `message?`, `app_permissions`, `locale`, `guild_locale?`, `entitlements`, `context`.

**Interaction Types**: PING `1`, APPLICATION_COMMAND `2`, MESSAGE_COMPONENT `3`, APPLICATION_COMMAND_AUTOCOMPLETE `4`, MODAL_SUBMIT `5`.

**Interaction Context Types**: GUILD `0`, BOT_DM `1`, PRIVATE_CHANNEL `2`.

**Interaction Callback Types**: PONG `1`, CHANNEL_MESSAGE_WITH_SOURCE `4`, DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE `5`, DEFERRED_UPDATE_MESSAGE `6`, UPDATE_MESSAGE `7`, APPLICATION_COMMAND_AUTOCOMPLETE_RESULT `8`, MODAL `9`, PREMIUM_REQUIRED `10` (deprecated), LAUNCH_ACTIVITY `12`.

**Callback Data Message fields**: `tts`, `content`, `embeds` (up to 10), `allowed_mentions`, `flags` (only `SUPPRESS_EMBEDS`, `EPHEMERAL`, `IS_COMPONENTS_V2`, `IS_VOICE_MESSAGE`, `SUPPRESS_NOTIFICATIONS` settable), `components`, `attachments`, `poll`. For `DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE`, the only valid flag is `EPHEMERAL`. Autocomplete: `choices` (max 25). Modal: `custom_id` (1–100 chars), `title` (max 45), `components` (1–5).

#### Syntax / Method Signature

```
POST /interactions/{interaction.id}/{interaction.token}/callback
- Description: Create interaction response.
- Body: interaction response object { type, data? }.
- Return: 204, or 200 with callback response when `with_response=true`.

GET /webhooks/{application.id}/{interaction.token}/messages/@original
- Description: Get original interaction response. Return: 200 Message.

PATCH /webhooks/{application.id}/{interaction.token}/messages/@original
- Description: Edit original interaction response. Return: 200 Message.

DELETE /webhooks/{application.id}/{interaction.token}/messages/@original
- Description: Delete original interaction response. Return: 204.

POST /webhooks/{application.id}/{interaction.token}
- Description: Create followup message. Functions like Execute Webhook with `wait` always true. Use EPHEMERAL flag `1 << 6` for user-only messages. Apps limited to 5 followups per interaction when initiated from a user-installed app not installed in the server.
- Return: 200 Message.

GET|PATCH|DELETE /webhooks/{application.id}/{interaction.token}/messages/{message.id}
- Description: Get/edit/delete a followup message. Return: 200 Message / 204 on DELETE.
```

#### Return values

`204` or Message objects as shown.

#### Side effects

Responding is always over HTTP; responses are not sent as commands over the gateway.

#### References

https://docs.discord.com/developers/interactions/receiving-and-responding.md

### 3.13 Application Commands

#### Overview

Application commands create slash commands, user context menu commands, and message context menu commands.

#### Prerequisites & Requirements

- Commands register ONLY over HTTP. You MUST use a bot token or a client-credentials token.
- Creating commands in a guild requires the `applications.commands` OAuth2 scope (auto-included with `bot`).
- CHAT_INPUT command and option names MUST match `^[-_'\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]{1,32}$`; use lowercase variants where they exist.
- Required options MUST be listed before optional ones.

#### Type references

**Command Object** (key fields): `id`, `type` (default 1), `application_id`, `guild_id?`, `name` (1–32 chars), `name_localizations`, `description` (1–100 for CHAT_INPUT; empty for USER/MESSAGE), `description_localizations`, `options` (max 25), `default_member_permissions`, `dm_permission` (deprecated), `nsfw` (default false), `integration_types`, `contexts`, `version`, `handler`.

**Command Types**: CHAT_INPUT `1`, USER `2`, MESSAGE `3`, PRIMARY_ENTRY_POINT `4`.

**Option Types**: SUB_COMMAND `1`, SUB_COMMAND_GROUP `2`, STRING `3`, INTEGER `4`, BOOLEAN `5`, USER `6`, CHANNEL `7`, ROLE `8`, MENTIONABLE `9`, NUMBER `10`, ATTACHMENT `11`.

**Limits**: 100 global CHAT_INPUT, 15 global USER, 15 global MESSAGE, 1 global PRIMARY_ENTRY_POINT; same counts per guild. Rate limit of 200 command creates per day per guild. Command total size max 8000 characters (name/description/value across options and choices; with localizations, only the longest per field counts).

**Permissions**: enable/disable commands for up to 100 users, roles, channels per guild. `default_member_permissions` is a bitwise OR-ed permission string; `"0"` restricts to admins/overrides only. Permission types: ROLE `1`, USER `2`, CHANNEL `3`. `@everyone` = `guild_id`, All Channels = `guild_id - 1`. Administrator members can use all commands.

#### Syntax / Method Signature

```
GET /applications/{application.id}/commands
- Description: List global commands. Query: with_localizations?.
- Return: 200 array of Command.

POST /applications/{application.id}/commands
- Description: Create a global command. Same-name commands overwrite the existing one.
- Return: 201 new / 200 overwrite.

GET/PATCH/DELETE /applications/{application.id}/commands/{command.id}
- Description: Get / edit / delete a global command. PATCH is all-fields-optional (provided fields fully overwritten).
- Return: 200 Command / 204 on DELETE.

PUT /applications/{application.id}/commands
- Description: Bulk overwrite global commands (all types). New commands count toward daily create limits.
- Return: 200 array of Command.

GET/POST /applications/{application.id}/guilds/{guild.id}/commands
- Description: List / create guild commands. Guild commands update instantly (recommended for testing).
- Return: 200 array / 201.

GET/PATCH/DELETE /applications/{application.id}/guilds/{guild.id}/commands/{command.id}
- Description: Get / edit / delete a guild command.

PUT /applications/{application.id}/guilds/{guild.id}/commands
- Description: Bulk overwrite guild commands.

GET /applications/{application.id}/guilds/{guild.id}/commands/permissions
- Description: List command permissions for the guild.

GET/PUT /applications/{application.id}/guilds/{guild.id}/commands/{command.id}/permissions
- Description: Get / overwrite permissions for a command (up to 100 overwrites). Requires a Bearer token with `applications.commands.permissions.update`, from a user with Manage Guild + Manage Roles.
- Return: 200 array of Command Permission.
```

#### Return values

Command objects, arrays, or `204` as shown.

#### Side effects

Deleting or renaming a command deletes its permissions. Fires Application Command Permissions Update.

#### References

https://docs.discord.com/developers/interactions/application-commands.md

### 3.14 Threads

#### Overview

Threads are temporary sub-channels inside an existing channel used to organize conversation. Threads are available only in API v9+.

#### Prerequisites & Requirements

- `SEND_MESSAGES` has no effect in threads; users MUST have `SEND_MESSAGES_IN_THREADS`.
- Deleting a thread requires `MANAGE_THREADS`.
- Only active threads can be manipulated; sending a message auto-unarchives a thread unless it is locked.
- `LIST_THREAD_MEMBERS` (via gateway) and `GET /channels/{id}/thread-members` require the `GUILD_MEMBERS` intent.

#### Type references

**Thread object**: reuses Channel object fields. `owner_id` = user who started the thread; `parent_id` = the text/announcement channel it was created in. Thread-only fields: `member_count` (approx, stops at 50), `message_count` (decrements on delete), `total_message_sent` (never decrements), `thread_metadata`.

**Thread Metadata**: `archived`, `archive_timestamp`, `auto_archive_duration`, `locked`.

**Thread Member**: `id`, `user_id`, `join_timestamp`, `flags`.

#### Syntax / Method Signature

```
POST /channels/{channel_id}/threads
- Description: Start a thread (forum/media, from message, or standalone). Returns a Channel object.

PUT /channels/{channel_id}/thread-members/@me
- Description: Join a thread. Return: 204.

DELETE /channels/{channel_id}/thread-members/@me
- Description: Leave a thread. Return: 204.

GET /channels/{channel_id}/thread-members
- Description: List thread members (requires GUILD_MEMBERS intent).

GET /guilds/{guild_id}/threads/active
- Description: List active guild threads the user can access.

GET /channels/{channel_id}/threads/archived/public
- Description: List archived public threads, ordered by archive timestamp descending.

GET /channels/{channel_id}/threads/archived/private
- Description: List archived private threads.

GET /channels/{channel_id}/users/@me/threads/archived/private
- Description: List archived private threads the current user is a member of.

PATCH /channels/{channel_id}
- Description: Edit a thread (same endpoint as channels). Changing name/archived/auto_archive_duration requires MANAGE_THREADS or the thread creator.

DELETE /channels/{channel_id}
- Description: Delete/close a thread (requires MANAGE_THREADS).
```

#### Return values

Channel objects or `204` empty responses as shown.

#### Side effects

Thread lifecycle is driven by Gateway events: `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`, `THREAD_LIST_SYNC`, `THREAD_MEMBER_UPDATE`, `THREAD_MEMBERS_UPDATE`. Webhooks can post to threads via the `thread_id` query parameter on Execute Webhook.

#### References

https://docs.discord.com/developers/topics/threads.md

### 3.15 Emoji

#### Overview

Emoji objects represent custom emojis, which can be created, modified, and deleted.

#### Prerequisites & Requirements

- Creating emojis requires `CREATE_GUILD_EXPRESSIONS`; modifying/deleting emoji created by others requires `MANAGE_GUILD_EXPRESSIONS`.
- Static and animated emoji files MUST be at most 256 KiB. Upload as JPEG, PNG, GIF, WebP, or AVIF; all are served as WebP.
- Emoji routes do NOT follow normal rate-limit conventions; they are limited per guild.

#### Type references

**Emoji Object**: `id` (?snowflake), `name` (?string), `roles` (array of snowflake), `user?` (creator), `require_colons`, `managed`, `animated`, `available`.

Premium emoji count toward a separate limit of 25. An app can own up to 2000 emojis, usable only by that app (no `USE_EXTERNAL_EMOJIS` required).

#### Syntax / Method Signature

```
GET /guilds/{guild.id}/emojis
- Description: List guild emojis. Fires Guild Emojis Update.
- Return: 200 array of Emoji.

GET /guilds/{guild.id}/emojis/{emoji.id}
- Description: Get a guild emoji. Return: 200 Emoji.

POST /guilds/{guild.id}/emojis
- Description: Create an emoji. Requires CREATE_GUILD_EXPRESSIONS.
- Body: name, image (128x128 image data), roles.
- Return: 200 Emoji.

PATCH /guilds/{guild.id}/emojis/{emoji.id}
- Description: Modify an emoji. Body: name, roles (all optional). Return: 200 Emoji.

DELETE /guilds/{guild.id}/emojis/{emoji.id}
- Description: Delete an emoji. Return: 204 No Content.

GET /applications/{application.id}/emojis
- Description: List application-owned emojis. Return: 200 { items }.

POST /applications/{application.id}/emojis
- Description: Create an application-owned emoji. Body: name, image. Return: 200 Emoji.

GET|PATCH|DELETE /applications/{application.id}/emojis/{emoji.id}
- Description: Get / modify (name) / delete an application-owned emoji. Return: 200 / 204.
```

#### Return values

Emoji objects, arrays, or `204` as shown.

#### Side effects

Guild emoji endpoints fire the Guild Emojis Update Gateway event and support `X-Audit-Log-Reason`.

#### References

https://docs.discord.com/developers/resources/emoji.md

### 3.16 Invite

#### Overview

Invites are codes that allow users to join guilds, group DMs, or friend requests.

#### Prerequisites & Requirements

- Deleting an invite requires `MANAGE_CHANNELS` on the invite's channel, or `MANAGE_GUILD` to remove any invite guild-wide.

#### Type references

**Invite Object**: `type`, `code`, `guild` (partial), `channel` (?partial), `inviter`, `target_type?`, `target_user?`, `target_application?`, `approximate_presence_count?`, `approximate_member_count?`, `expires_at?`, `guild_scheduled_event?`, `flags`.

**Invite Types**: GUILD `0`, GROUP_DM `1`, FRIEND `2`. **Invite Target Types**: STREAM `1`, EMBEDDED_APPLICATION `2`.

**InviteMetadata**: `uses`, `max_uses`, `max_age`, `temporary`, `created_at`.

#### Syntax / Method Signature

```
GET /invites/{invite.code}
- Description: Get an invite.
- Query: with_counts?, guild_scheduled_event_id?.
- Return: 200 Invite.

DELETE /invites/{invite.code}
- Description: Delete an invite. Requires MANAGE_CHANNELS (or MANAGE_GUILD).
- Return: 200 Invite. Fires Invite Delete.

GET /invites/{invite.code}/target-users
- Description: Get users allowed to see/accept the invite. Returns CSV. Requires the inviter, MANAGE_GUILD, or VIEW_AUDIT_LOG.

PUT /invites/{invite.code}/target-users
- Description: Update allowed users. Form param: target_users_file (CSV of user IDs). Requires the inviter or MANAGE_GUILD.

GET /invites/{invite.code}/target-users/job-status
- Description: Async processing status. Status: 0 UNSPECIFIED, 1 PROCESSING, 2 COMPLETED, 3 FAILED.
```

#### Return values

Invite objects, CSV content, or job status as shown.

#### Side effects

Deleting fires the Invite Delete event and supports `X-Audit-Log-Reason`. Get/Delete on channel and guild invite endpoints live on the Channel and Guild resource pages.

#### References

https://docs.discord.com/developers/resources/invite.md

## 4. Configuration Reference

### 4.1 Developer Portal

| Setting | Description |
|---|---|
| Bot token | Authentication for bot users; found on the Bot page |
| Privileged Gateway Intents | MUST be enabled for `GUILD_PRESENCES`, `GUILD_MEMBERS`, `MESSAGE_CONTENT`; verified apps MUST be approved |
| Public Bot | Unchecked: only the owner can add the bot to guilds |
| Require OAuth2 Code Grant | Enables full OAuth2 authorization for bot adds |
| Interactions Endpoint URL | URL for receiving interactions over HTTP (outgoing webhooks) |
| Default Install Settings | Scopes/permissions used when no `scope`/`integration_type` is specified |
| Verification | Apps over 10,000 users require privileged intent review (annual reapply) |

### 4.2 OAuth2 Configuration

- Redirect URIs MUST be registered when creating the application.
- Redirect URIs MUST use `https` (or `localhost` during development).
- Client id and client secret MUST be kept confidential.

### 4.3 Gateway Configuration

- Gateway URL query params: `v` (API version), `encoding` (`json` | `etf`), `compress?` (`zlib-stream` | `zstd-stream`).
- `GET /gateway/bot` returns recommended shard count and `session_start_limit`.

## 5. Error Handling

### 5.1 HTTP Response Codes

| Code | Meaning |
|---|---|
| 200 (OK) | Request completed successfully |
| 201 (CREATED) | Entity created successfully |
| 204 (NO CONTENT) | Success, no content returned |
| 304 (NOT MODIFIED) | Entity not modified |
| 400 (BAD REQUEST) | Malformed request / server could not understand |
| 401 (UNAUTHORIZED) | `Authorization` header missing or invalid |
| 403 (FORBIDDEN) | Token lacks permission to the resource |
| 404 (NOT FOUND) | Resource does not exist |
| 405 (METHOD NOT ALLOWED) | Method invalid for the location |
| 429 (TOO MANY REQUESTS) | Rate limited |
| 502 (GATEWAY UNAVAILABLE) | No gateway available; wait and retry |
| 5xx (SERVER ERROR) | Server error (rare) |

### 5.2 JSON Error Body

Starting in API v8, form error responses include the offending JSON key, an error code, and a human-readable message. Body shape:

```json
{
  "code": 50035,
  "errors": {
    "access_token": {
      "_errors": [
        { "code": "BASE_TYPE_REQUIRED", "message": "This field is required" }
      ]
    }
  },
  "message": "Invalid Form Body"
}
```

The complete error list changes frequently; you MUST handle unknown error codes gracefully.

### 5.3 JSON Error Codes (highlights)

- **0** — General error (malformed request body).
- **10001–10071** — Unknown-resource family: Unknown account `10001`, application `10002`, channel `10003`, guild `10004`, invite `10006`, member `10007`, message `10008`, role `10011`, token `10012`, user `10013`, emoji `10014`, webhook `10015`, interaction `10062`, application command `10063`.
- **20001** — Bots cannot use this endpoint. **20012** — Not authorized for this action on this application. **20016** — Blocked by slowmode.
- **30001** — Max guilds (100). **30003** — Max pins (250). **30005** — Max roles (250). **30007** — Max webhooks (15). **30010** — Max reactions (20). **30013** — Max channels (500). **30015** — Max attachments (10).
- **40001** — Unauthorized; provide a valid token. **40002** — Account must be verified. **40004** — Send messages temporarily disabled. **40005** — Request entity too large. **40007** — User banned from guild. **40043** — Interaction failed to send. **40060** — Interaction already acknowledged. **40067** — Tag required to create a forum post. **40333** — Cloudflare blocking; set a proper User-Agent.
- **50001** — Missing access. **50005** — Cannot edit a message authored by another user. **50006** — Cannot send an empty message. **50008** — Cannot send messages in a non-text channel. **50013** — You lack permissions. **50014** — Invalid authentication token. **50016** — Must delete 2–99 messages. **50019** — Message can only be pinned to its channel. **50021** — Cannot execute on a system message. **50024** — Cannot execute on this channel type. **50025** — Invalid OAuth2 access token. **50026** — Missing OAuth2 scope. **50027** — Invalid webhook token. **50034** — Message too old to bulk delete. **50035** — Invalid form body / Content-Type. **50045** — File exceeds max size. **50083** — Operation on an archived thread.
- **60003** — Two-factor required. **90001** — Reaction blocked. **130000** — API overloaded, retry later. **160002** — Cannot reply without read-message-history permission. **160005** — Thread is locked. **200000** — Message blocked by auto-mod. **220001** — Webhook to forum channel needs `thread_name` or `thread_id`. **500000** — Failed to ban users.

### 5.4 Rate Limit Errors

On HTTP 429, the body includes `message`, `retry_after` (float seconds), `global` (boolean), and optional `code`. You MUST wait `retry_after` seconds (or honor the `Retry-After` header) before retrying.

### 5.5 Gateway and Voice Close Codes

Gateway close codes are listed in [3.5 Gateway](#35-gateway-websocket). Voice close codes include `4001` Unknown opcode, `4004` Authentication failed, `4006` Session no longer valid, `4009` Session timeout, `4011` Server not found, `4014` Disconnected (should not reconnect), `4015` Voice server crashed (resume), `4021` Rate limited (should not reconnect).

## 6. References

- llms.txt index: https://docs.discord.com/llms.txt
- API Reference: https://docs.discord.com/developers/reference
- All source pages are linked at the top of this document under "Source Specifications".
