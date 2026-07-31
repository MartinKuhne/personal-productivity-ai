# Model Context Protocol – Client-Side Specification

**Source Specifications:**
- Base Protocol: https://modelcontextprotocol.io/specification/2025-11-25/basic
- Lifecycle: https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
- Transports: https://modelcontextprotocol.io/specification/2025-11-25/basic/transports
- Authorization: https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization
- Cancellation: https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation
- Ping: https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/ping
- Progress: https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/progress

## 1. Overview

This specification defines the Model Context Protocol (MCP) from a **client-side** perspective.
It describes what an MCP client MUST, SHOULD, and MAY do to connect to an MCP server,
negotiate capabilities, and exchange messages.

All message exchange MUST follow [JSON-RPC 2.0](https://www.jsonrpc.org/specification).
All JSON-RPC messages MUST be UTF-8 encoded.

### 1.1 Message Formats

| Type | ID | Direction | Response |
|------|----|-----------|----------|
| Request | MUST be string or integer | Client ↔ Server | Required |
| Response | MUST match request ID | Client ↔ Server | N/A |
| Notification | MUST NOT include ID | Client ↔ Server | Not sent |

**Requests** initiate operations.
The ID MUST NOT be null.
The ID MUST be unique within the session.

**Responses** contain either a `result` field or an `error` field, never both.
Error responses MUST include `code` (integer) and `message` (string).

**Notifications** are one-way messages.
The receiver MUST NOT send a response.

#### General Fields

**`_meta`**
Clients MAY attach metadata to requests, responses, or notifications using the `_meta` field.
Certain key names are reserved by MCP.
Applications MUST NOT assume values at reserved keys.

**`icons`**
Servers MAY expose icons for tools, prompts, and resources.
Clients that render icons MUST support `image/png` and `image/jpeg`.
Clients SHOULD also support `image/svg+xml` and `image/webp`.
Clients MUST treat icon URIs as untrusted input.
Clients MUST fetch icons without credentials.

### 1.2 JSON Schema Dialect

Schemas without an explicit `$schema` field default to JSON Schema 2020-12.
Clients and servers MUST support at least JSON Schema 2020-12.
Clients and servers SHOULD document any additional supported dialects.

---

## 2. Lifecycle

### 2.1 Sequence

1. **Initialization** – client opens the transport, sends `initialize` request, waits for response
2. **Operation** – client sends `initialized` notification, then exchanges messages
3. **Shutdown** – client closes the transport

### 2.2 Initialization

#### Client Actions

The client MUST send an `initialize` request first.
The client MUST NOT send other requests (except `ping`) before the server responds.
The client MUST NOT send the `initialized` notification before the `initialize` response arrives.

##### `initialize` Request Fields

| Field | Type | Description |
|-------|------|-------------|
| `protocolVersion` | string | Version supported by the client |
| `capabilities` | object | Optional client capabilities |
| `clientInfo` | object | Client implementation details |

**Client Capabilities**

| Capability | Description |
|------------|-------------|
| `roots` | Client provides filesystem roots |
| `sampling` | Client supports LLM sampling requests |
| `elicitation` | Client supports server elicitation requests |
| `tasks` | Client supports task-augmented requests |

**`clientInfo` Fields**

| Field | Description |
|-------|-------------|
| `name` | Client program name |
| `version` | Client version string |
| `title` | Display name |
| `description` | Short description |
| `icons` | Optional icon array |
| `websiteUrl` | Optional HTTPS URL |

#### Server Response

The server responds with `protocolVersion`, `capabilities`, `serverInfo`, and optional `instructions`.
If the server returns a different `protocolVersion`, the client MUST disconnect if it cannot support it.

#### Version Negotiation

The client sends its latest supported protocol version in `protocolVersion`.
The server returns the same version if supported, or another version it supports.
If the version does not match, the client SHOULD disconnect.

#### Capability Negotiation

Client and server capabilities establish which optional features are active.
The client MUST only use capabilities the server offered.
The client MUST respect the negotiated protocol version for the session.

#### Session Completion

After a successful `initialize` response, the client MUST send an `initialized` notification:

```json
{ "jsonrpc": "2.0", "method": "notifications/initialized" }
```

### 2.3 Operation

During operation, the client:
- Sends requests and notifications
- Receives responses, requests, and notifications
- Honors negotiated capabilities
- Follows the protocol version negotiated during initialization

The client MUST NOT send `initialize` again.

### 2.4 Shutdown

The client SHOULD cleanly terminate the connection.
The method depends on the transport.

#### stdio Shutdown

1. Close input stream to the server process
2. Wait for the server to exit
3. Send `SIGTERM` if the server does not exit within a reasonable time
4. Send `SIGKILL` if the server still does not exit

#### HTTP Shutdown

Close the associated HTTP connection(s).

### 2.5 Timeouts

The client SHOULD establish timeouts for all sent requests.
If a response does not arrive within the timeout, the client SHOULD:
1. Send a `cancelled` notification for the request
2. Stop waiting for a response

SDKs SHOULD allow per-request timeout configuration.
Clients MAY reset the timeout clock when receiving a `progress` notification.
Clients SHOULD always enforce a maximum timeout regardless of progress.

### 2.6 Error Handling

The client SHOULD handle these error cases:
- Protocol version mismatch
- Failure to negotiate required capabilities
- Request timeouts

---

## 3. Transports

### 3.1 Term Selection

The term *transport* is used consistently throughout this specification.
Do not use synonyms such as channel, conduit, or link.

### 3.2 stdio Transport

#### Overview

The client launches the MCP server as a subprocess.
The client reads JSON-RPC messages from the server's stdout.
The client writes JSON-RPC messages to the server's stdin.

#### Client Requirements

- The client MUST NOT write anything to stdin that is not a valid MCP message
- The client MAY capture, forward, or ignore server stderr output
- The client SHOULD NOT assume stderr output indicates an error
- The client MUST NOT write to stdout; stdout is reserved for the server

#### Message Delimiting

Messages are delimited by newlines.
Messages MUST NOT contain embedded newlines.

#### Initialization Note

The client sends the `protocolVersion` string in the `initialize` request.
No separate protocol-version header is needed for stdio.

### 3.3 Streamable HTTP Transport

#### Overview

The server operates as an independent process.
The client sends messages via HTTP POST.
The client MAY open an SSE stream via HTTP GET to receive server messages.

#### Client Requirements

- The client MUST include `MCP-Protocol-Version: <version>` on all HTTP requests after initialization
- The client MUST include `Accept: application/json, text/event-stream` on all POST requests
- The client MUST include `Accept: text/event-stream` on all GET requests
- The client MUST send exactly one JSON-RPC message per POST request body
- The client MUST support receiving either `Content-Type: application/json` or `Content-Type: text/event-stream`

#### Sending Messages

1. Create a new HTTP POST request to the MCP endpoint
2. Set `Accept` header to `application/json, text/event-stream`
3. Set `Content-Type` to `application/json`
4. If using HTTP transport, set `MCP-Protocol-Version` header
5. Set body to a single JSON-RPC message
6. Send the request

##### POST Responses

| Input Type | Server Response | Client Action |
|------------|-----------------|---------------|
| Request | `text/event-stream` | Open SSE stream; wait for response event |
| Request | `application/json` | Parse response object |
| Notification | `202 Accepted` | Continue operation |
| Notification | HTTP error | Handle error |

#### SSE Events

The server MAY send SSE events containing JSON-RPC messages.
The server MAY assign event IDs to SSE events for resumability.
Event IDs MUST be globally unique within the session or per-client.

#### Resuming an SSE Stream

If the SSE stream closes, the client MAY resume it:
1. Issue an HTTP GET to the MCP endpoint
2. Include `Last-Event-ID` header with the last received event ID
3. The server MAY replay missed messages on that stream
4. The server MUST NOT replay messages from a different stream

#### GET Requests

The client MAY send HTTP GET to the MCP endpoint to listen for server messages.
The server responds with `Content-Type: text/event-stream` or `405 Method Not Allowed`.
Disconnection does NOT imply the client cancelled its request.
To cancel, the client MUST send a `cancelled` notification.

#### Multiple Connections

The client MAY maintain multiple SSE streams simultaneously.
The client MUST track which stream received which message.

### 3.4 Session Management

#### Session IDs

The server MAY assign a session ID during initialization by returning an `MCP-Session-Id` HTTP header.
Session IDs MUST contain only visible ASCII characters (0x21 through 0x7E).
The client MUST handle session IDs securely.

#### Session ID Usage

If the server returns an `MCP-Session-Id`, the client MUST include it in the `MCP-Session-Id` header on all subsequent requests.
If the server returns HTTP 404 for a request with a session ID, the client MUST start a new session by sending a new `initialize` request without a session ID.

#### Session Termination

The client SHOULD send HTTP DELETE to the MCP endpoint with the `MCP-Session-Id` header when the session is no longer needed.
The server MAY respond with `405 Method Not Allowed`, indicating the server manages session lifetime.

### 3.5 Backwards Compatibility

When connecting to an older server:
1. Attempt an HTTP POST with the new `Accept` header
2. If it fails with `400`, `404`, or `405`, send an HTTP GET expecting an `endpoint` SSE event
3. If the `endpoint` event arrives, use the old HTTP+SSE transport

### 3.6 Custom Transports

The client MAY implement custom transports.
Custom transports MUST preserve JSON-RPC message format and lifecycle requirements.

---

## 4. Authorization

This section applies only to HTTP-based transports.
Implementation with stdio transport SHOULD NOT follow this section; instead, retrieve credentials from the environment.

### 4.1 Overview

The client acts as an OAuth 2.1 client.
The client makes protected resource requests on behalf of a resource owner.

### 4.2 Requirements

Authorization is OPTIONAL for MCP implementations.
HTTP-based clients SHOULD conform to this specification.
Alternative transport clients MUST follow established security best practices for their protocol.

### 4.3 Discovery

The client MUST implement RFC 9728 OAuth 2.0 Protected Resource Metadata discovery.

1. Send an unauthenticated request to the MCP server
2. If the server returns `401 Unauthorized` with a `WWW-Authenticate` header:
   - Extract the `resource_metadata` URL from the header
   - Request the resource metadata document
3. If no header is present, fall back to well-known URI probing:
   - Probe `/.well-known/oauth-protected-resource/<mcp-path>`
   - Then probe `/.well-known/oauth-protected-resource`

The client MUST extract `authorization_servers` from the resource metadata.

#### Authorization Server Metadata Discovery

For each authorization server URL, the client MUST try endpoints in this order:

**URLs with path components** (e.g., `https://auth.example.com/tenant1`):
1. `https://auth.example.com/.well-known/oauth-authorization-server/tenant1`
2. `https://auth.example.com/.well-known/openid-configuration/tenant1`
3. `https://auth.example.com/tenant1/.well-known/openid-configuration`

**URLs without path components** (e.g., `https://auth.example.com`):
1. `https://auth.example.com/.well-known/oauth-authorization-server`
2. `https://auth.example.com/.well-known/openid-configuration`

### 4.4 Client Registration

The client SHOULD follow this priority order:

1. Use pre-registered client credentials if available
2. Use Client ID Metadata Documents if the authorization server indicates support
3. Use Dynamic Client Registration as a fallback
4. Prompt the user to enter client information

#### Client ID Metadata Documents

If supported:
- The client MUST host a metadata document at an HTTPS URL
- The URL MUST use the `https` scheme and contain a path component
- The document MUST include `client_id`, `client_name`, and `redirect_uris`
- The `client_id` value MUST match the document URL exactly
- The client MAY use `private_key_jwt` for authentication

### 4.5 Scope Selection

The client MUST follow the principle of least privilege.
The client SHOULD select scopes in this priority order:

1. Use the `scope` parameter from the initial `WWW-Authenticate` header
2. If `scope` is absent, use all scopes defined in `scopes_supported` from the resource metadata

The client MUST NOT request scopes beyond what is available.

### 4.6 Authorization Flow

1. Send MCP request without an access token
2. Receive `401 Unauthorized` with `WWW-Authenticate` header
3. Discover the authorization server
4. Obtain authorization server metadata
5. Register the client (if required)
6. Generate PKCE `code_challenge` and `code_verifier`; use `S256`
7. Send an authorization request including the `resource` parameter
8. Receive the authorization code via redirect
9. Exchange the code for an access token; include the `resource` parameter
10. Send MCP requests with the access token in the `Authorization: Bearer <token>` header
11. Retry the original request with the token

#### Resource Parameter

The client MUST include the `resource` parameter in both authorization requests and token requests.
The `resource` parameter MUST identify the MCP server with its canonical URI.

**Valid canonical URIs:**
- `https://mcp.example.com/mcp`
- `https://mcp.example.com`
- `https://mcp.example.com:8443`

The client MUST NOT omit the `resource` parameter.

#### Access Token Usage

- The client MUST include the access token in the `Authorization` header on every request
- The client MUST NOT include access tokens in URI query strings
- The client MUST NOT send tokens from one MCP server to another

```http
Authorization: Bearer <access-token>
```

### 4.7 Step-Up Authorization

If the server returns `403 Forbidden` with `WWW-Authenticate: Bearer error="insufficient_scope"`:
1. Parse the required scopes from the `scope` parameter
2. Initiate a new authorization flow with the additional scopes
3. Retry the original request with the new token
4. Limit retries to avoid infinite loops

Clients SHOULD track scope upgrade attempts.

### 4.8 Error Handling

The client MUST handle these responses:

| Status | Cause | Client Action |
|--------|-------|---------------|
| 401 | Missing or invalid token | Initiate authorization flow |
| 403 | Insufficient scope | Request additional scopes |
| 400 | Malformed request | Do not retry |

### 4.9 Security

The client MUST:
- Implement PKCE with `S256` before proceeding with authorization
- Verify PKCE support in authorization server metadata (`code_challenge_methods_supported`)
- Use only HTTPS URLs for all endpoints
- Use only `localhost` or HTTPS redirect URIs
- Include a `state` parameter in authorization requests and verify it on callback
- Store access tokens securely

The client SHOULD use short-lived tokens when supported.

---

## 5. Utilities

### 5.1 Cancellation

#### Overview

The client MAY cancel an in-progress request using a notification.

#### Notification Format

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/cancelled",
  "params": {
    "requestId": "<id>",
    "reason": "<string>"
  }
}
```

#### Client Requirements

- The notification MUST reference a request the client previously sent
- The notification MUST reference a request still believed to be in-progress
- The client MUST NOT cancel the `initialize` request
- For task-augmented requests, the client SHOULD use the dedicated task cancellation mechanism instead
- After sending a cancellation, the client SHOULD ignore any response that arrives later
- The client SHOULD log cancellation reasons

#### Race Conditions

Cancellation notifications may arrive after the server has already completed the request.
The sender MUST handle these race conditions gracefully.

### 5.2 Ping

#### Overview

The client MAY send a `ping` request to verify the server is responsive.

#### Request Format

```json
{ "jsonrpc": "2.0", "id": "123", "method": "ping" }
```

#### Client Requirements

- The client MUST expect an empty result response: `{ "jsonrpc": "2.0", "id": "123", "result": {} }`
- The client SHOULD treat non-response as a connection failure
- The client SHOULD configure a reasonable timeout
- The client MAY initiate reconnection after multiple failed pings
- The client SHOULD log ping failures
- The client SHOULD avoid excessive pinging

### 5.3 Progress

#### Overview

The client MAY request progress notifications for long-running operations.

#### Requesting Progress

Include a `progressToken` in the request `_meta`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "some_method",
  "params": {
    "_meta": { "progressToken": "abc123" }
  }
}
```

Progress tokens MUST be strings or integers.
Progress tokens MUST be unique across all active requests.

#### Receiving Progress

The server MAY send progress notifications:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/progress",
  "params": {
    "progressToken": "abc123",
    "progress": 50,
    "total": 100,
    "message": "Reticulating splines..."
  }
}
```

#### Client Requirements

- Progress values MUST increase with each notification
- The client MUST stop tracking progress after the operation completes
- For task-augmented requests, the client MUST continue tracking the same `progressToken` until the task reaches a terminal status
- The client SHOULD track active progress tokens
- The client SHOULD implement rate limiting in its UI rendering
