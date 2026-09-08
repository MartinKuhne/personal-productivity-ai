# Interface Contract: `web_fetch` Tool

**Feature**: Chrome DevTools Integration for Web Fetch  
**Contract Version**: 1.1  
**Tool Name**: `web_fetch`  
**Tool Group**: `Web`  
**Safety**: `ReadOnly`  

---

## 1. Description

Fetches content from a URL and converts its HTML body to Markdown. If Google Chrome or a compatible Chromium browser is installed on the host system, the tool automatically uses headless browser DevTools execution to render client-side JavaScript before extracting the DOM. If Chrome is not installed or encounters an error, the tool seamlessly falls back to standard HTTP retrieval. The response is paginated (up to 64 lines per call) and cached for 30 minutes.

---

## 2. Input Parameters (`WebFetchInput`)

| Parameter | Type | Required | Default | Description |
| :--- | :--- | :--- | :--- | :--- |
| `url` | `String` | **Yes** | — | The HTTP or HTTPS URL to fetch. |
| `cursor` | `Option<String>` | No | `null` | Opaque pagination cursor returned by a previous call to fetch the next 64 lines of Markdown. |
| `force_refetch` | `bool` | No | `false` | When `true`, bypasses and invalidates the 30-minute cache and forces a fresh network/browser fetch. |
| `headers` | `bool` | No | `false` | When `true`, includes the HTTP response headers in the output. |

### JSON Schema Snippet
```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "description": "The URL to fetch and convert to Markdown."
    },
    "cursor": {
      "type": ["string", "null"],
      "description": "Cursor for paginating through large Markdown documents."
    },
    "force_refetch": {
      "type": "boolean",
      "default": false,
      "description": "Force a fresh fetch even if content is cached."
    },
    "headers": {
      "type": "boolean",
      "default": false,
      "description": "Include HTTP response headers in the response."
    }
  },
  "required": ["url"]
}
```

---

## 3. Output Response (`WebFetchResponse`)

| Field | Type | Description |
| :--- | :--- | :--- |
| `content` | `String` | The Markdown content of the current page (up to 64 lines). |
| `total_lines` | `usize` | Total number of lines across all pages in the full document. |
| `cursor` | `Option<String>` | Opaque cursor token to retrieve the next page, or `null` if exhausted. |
| `hint` | `Option<String>` | Pagination status hint (e.g. `"Showing lines 1 to 64 of 180. Pass cursor to get more."`). |
| `response_headers` | `Option<HashMap<String, String>>` | HTTP response headers (present only if `headers = true`). |
| `from_cache` | `bool` | `true` if content was served from the local 30-minute cache. |

### Example Response (Success)
```json
{
  "content": "# Documentation Title\n\nThis content was rendered via client-side JavaScript...\n",
  "total_lines": 128,
  "cursor": "c3Vyc3ByaXNlX2N1cnNvcg==",
  "hint": "Showing lines 1 to 64 of 128. Pass cursor 'c3Vyc3ByaXNlX2N1cnNvcg==' to view the next page.",
  "response_headers": {
    "content-type": "text/html; charset=utf-8",
    "server": "cloudflare"
  },
  "from_cache": false
}
```

---

## 4. Error Modes & Fallback Semantics

| Scenario | Behavior | User/Agent Facing Outcome |
| :--- | :--- | :--- |
| Chrome installed and page renders OK | Headless render -> DOM extracted -> converted to Markdown. | Success with rich JS-rendered Markdown. |
| Chrome NOT installed | Automatic fallback to `reqwest` HTTP GET. | Success with static HTML Markdown; zero error. |
| Chrome execution crash or non-zero exit | Warning logged; automatic fallback to `reqwest` HTTP GET. | Success with static HTML Markdown; zero error. |
| Chrome times out (>15 seconds) | Process killed; warning logged; fallback to `reqwest` HTTP GET. | Success with static HTML Markdown; zero error. |
| Network error (both Chrome and HTTP fail) | Error returned to caller. | Error string: `"Failed to fetch URL: <cause>"`. |
| Invalid URL scheme (e.g. `file://` or malformed) | Error returned immediately. | Error string: `"Invalid URL: <cause>"`. |
