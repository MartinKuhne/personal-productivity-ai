# FastMD Agent Tools Reference

The following table summarizes all 27 tools available to the LLM agent, categorized by functionality:

| Category | Tool Name | Description | Configuration Required |
| --- | --- | --- | --- |
| **Core Workspace** | `grep` | Search term case-insensitively across markdown files. | None |
| **Core Workspace** | `read_tags` | List all unique tags from markdown front-matter. | None |
| **Core Workspace** | `list_files_by_tag` | List markdown files containing a specific tag (paginated; default page size 20; returns a JSON array; every response includes total). | None |
| **Core Workspace** | `list_files` | List all markdown files in a directory (paginated; default page size 20; returns a JSON array; every response includes total). | None |
| **Core Workspace** | `read_file` | Read the entire text contents of a file at the specified path. | None |
| **Core Workspace** | `read_file_lines` | Read specific lines from a file (1-indexed). | None |
| **Core Workspace** | `create_file` | Create a new file with specified content. | None |
| **Core Workspace** | `insert_lines` | Insert lines at a specific 1-indexed position. | None |
| **Core Workspace** | `delete_lines` | Delete specific lines from a file. | None |
| **Core Workspace** | `replace_text` | Replace exact occurrences of old_string with new_string in a file. | None |
| **Core Workspace** | `read_yaml_header` | Parse a YAML header from a markdown file and return its content representation. | None |
| **Core Workspace** | `write_yaml_header` | Write or update data in a YAML header to a markdown file. | None |
| **Web Integration** | `web_fetch` | Fetch a URL and convert its HTML body to markdown. | None |
| **Web Integration** | `web_search` | Search the web using SearXNG. | `searxng_url` |
| **Web Integration** | `web_delegate` | Delegate complex web research to a sub-agent with web_search/web_fetch tools. | None |
| **JMAP Productivity**| `search_calendar` | Search calendar events by keyword. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `get_calendar` | Retrieve calendar events by ISO date range. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `get_calendar_item`| Retrieve a specific calendar event by ID. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `add_calendar_item`| Create a new calendar event using a JSON object. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `update_calendar_item`| Update a calendar event using a JSON patch object. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `delete_calendar_item`| Delete a calendar event by ID. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `search_email` | Search emails by any combination of keyword, folder, date range, sender, recipient, unread, or flagged status. All filters are combined with AND. Results are paginated (default page size 10); every response includes total so the caller can drive follow-up page requests. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `get_email_by_id` | Get email by ID (full content). | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `send_email` | Send an email to a recipient. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `search_contact` | Search contacts by keyword. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `get_contact` | Retrieve a specific contact's details by ID. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `add_contact` | Create a new contact using a JSON object. | `jmap_url`, `jmap_token` |

### Detailed Tool Schema and Arguments

Here is the exact schema and argument breakdown for each tool as declared in the codebase.

All tool responses follow the same envelope:

```json
// Success
{ "status": "success", "data": { <response fields> } }

// Error
{ "status": "error", "message": "Description of what went wrong." }
```

---

#### 1. Core Workspace Tools

##### `grep`
* **Description:** Search for a query string case-insensitively across all Markdown files in the workspace.
* **Request:**
  ```json
  { "query": "search term" }
  ```
* **Response (`data`):**
  ```json
  { "matches": "lib/file.md:42 - line content\nlib/other.md:10 - another line" }
  ```

##### `read_tags`
* **Description:** Get all unique tags defined in front-matter headers of all Markdown files in the workspace.
* **Request:**
  ```json
  {}
  ```
* **Response (`data`):**
  ```json
  { "tags": ["meeting", "notes", "todo"] }
  ```

##### `list_files_by_tag`
* **Description:** List Markdown files that contain a specific tag in their front-matter. Results are returned as a JSON array, paginated across all configured libraries (default page size 20). Every response includes the total number of matching files so the caller can drive follow-up page requests.
* **Request:**
  ```json
  { "tag": "meeting", "page": 1, "page_size": 20 }
  ```
  * `page` — 1-indexed page number. Defaults to `1` if omitted. Values `< 1` are normalised to `1`.
  * `page_size` — files per page. Defaults to `20` if omitted. Values `< 1` are normalised to `1`.
* **Response (`data`):**
  ```json
  { "files": ["work/file_000.md", "work/file_001.md", "..."], "total": 50 }
  ```
  * `files` — JSON array of virtual paths for the requested page. Empty array when the tag has no matches or when the requested `page` is past the end (in which case `hint` is also set).
  * `total` — total number of files matching the tag across all libraries. Returned on every response (including the "no matches" and "past end" cases).
  * `hint` *(optional)* — present only when the result is empty for a structural reason. Two forms:
    * `No matching tagged files found.` — when no file in any library carries the tag.
    * `No tagged files on page N (showing 0 of M total, page_size: S).` — when the requested page is past the end.

##### `list_files`
* **Description:** List Markdown files in a directory (non-recursive). Results are returned as a JSON array, paginated (default page size 20). Every response includes the total number of files in the directory so the caller can drive follow-up page requests. With `path` set to `"/"` or `"."` returns the configured content libraries.
* **Request:**
  ```json
  { "path": "MyLib", "page": 1, "page_size": 20 }
  ```
  * `page` — 1-indexed page number. Defaults to `1` if omitted. Values `< 1` are normalised to `1`.
  * `page_size` — files per page. Defaults to `20` if omitted. Values `< 1` are normalised to `1`.
* **Response (`data`):**
  ```json
  { "files": ["MyLib/notes.md", "MyLib/diary.md"], "total": 2 }
  ```
  * `files` — JSON array of virtual paths for the requested page. Empty array when the directory has no Markdown files or when the requested `page` is past the end (in which case `hint` is also set).
  * `total` — total number of Markdown files in the directory (across all pages). Returned on every response.
  * `hint` *(optional)* — present only when the result is empty for a structural reason. Forms use `"file"` / `"files"` for a normal directory, or `"library"` / `"libraries"` for the root listing:
    * `No matching files found.` (or `No matching libraries found.` for the root).
    * `No files on page N (showing 0 of M total, page_size: S).` (or the same with `"libraries"`).

##### `read_file`
* **Description:** Read the entire text contents of a file at the specified path.
* **Request:**
  ```json
  { "path": "MyLib/doc.md" }
  ```
* **Response (`data`):**
  ```json
  { "content": "# Title\n\nFull file content..." }
  ```

##### `read_file_lines`
* **Description:** Read specific lines from a file (1-indexed).
* **Request:**
  ```json
  { "path": "MyLib/doc.md", "start_line": 5, "end_line": 10 }
  ```
* **Response (`data`):**
  ```json
  { "content": "line 5\nline 6\nline 7\nline 8\nline 9\nline 10" }
  ```

##### `create_file`
* **Description:** Create a new file at the specified path with the provided content.
* **Request:**
  ```json
  { "path": "MyLib/new.md", "content": "---\ntitle: New Doc\n---\n# Hello" }
  ```
* **Response (`data`):**
  ```json
  { "result": "File created successfully.", "size_bytes": 52 }
  ```

##### `insert_lines`
* **Description:** Insert lines into a file at a specific 1-indexed line index (lines are inserted before the given line).
* **Request:**
  ```json
  { "path": "MyLib/doc.md", "line_index": 3, "lines": ["new line A", "new line B"] }
  ```
* **Response (`data`):**
  ```json
  { "result": "Lines inserted successfully." }
  ```

##### `delete_lines`
* **Description:** Delete specific lines from a file (1-indexed, inclusive).
* **Request:**
  ```json
  { "path": "MyLib/doc.md", "start_line": 5, "end_line": 7 }
  ```
* **Response (`data`):**
  ```json
  { "result": "Lines deleted successfully." }
  ```

##### `replace_text`
* **Description:** Replace exact occurrences of `old_string` with `new_string` in a file.
* **Request:**
  ```json
  { "path": "MyLib/doc.md", "old_string": "foo", "new_string": "bar" }
  ```
* **Response (`data`):**
  ```json
  { "result": "Text replaced successfully." }
  ```

##### `read_yaml_header`
* **Description:** Parse a YAML header from a markdown file and return its content representation.
* **Request:**
  ```json
  { "path": "MyLib/doc.md" }
  ```
* **Response (`data`):**
  ```json
  { "content": "title: My Document\nsummary: A brief description\ntags:\n  - meeting\n  - notes\nheader-date: 2026-07-20T12:00:00Z\n" }
  ```

##### `write_yaml_header`
* **Description:** Write or update data in a YAML header to a markdown file.
* **Request:**
  ```json
  {
    "path": "MyLib/doc.md",
    "title": "Updated Title",
    "summary": "New summary.",
    "tags": ["meeting", "notes"],
    "header-date": "2026-07-20T12:00:00Z"
  }
  ```
* **Response (`data`):**
  ```json
  { "result": "YAML header written successfully." }
  ```

---

#### 2. Web Integration Tools

##### `web_fetch`
* **Description:** Fetch content from a URL and convert it to Markdown.
* **Request:**
  ```json
  { "url": "https://example.com/page" }
  ```
* **Response (`data`):**
  ```json
  { "content": "# Page Title\n\nConverted markdown content..." }
  ```

##### `web_search`
* **Description:** Search the web using SearXNG (enabled only if `searxng_url` is configured in `config.yaml`).
* **Request:**
  ```json
  { "query": "latest AI news" }
  ```
* **Response (`data`):**
  ```json
  { "results": "## Result 1\nURL: https://...\nSnippet: ...\n\n## Result 2\n..." }
  ```

##### `web_delegate`
* **Description:** Delegate complex web research to a sub-agent with `web_search` and `web_fetch` tools. Returns a summarized answer.
* **Request:**
  ```json
  { "instruction": "Research the latest developments in Rust async programming" }
  ```
* **Response (`data`):**
  ```json
  { "result": "Summarized research findings..." }
  ```

---

#### 3. JMAP Productivity Tools

##### `search_calendar`
* **Description:** Search the calendar by keyword.
* **Request:**
  ```json
  { "keyword": "meeting" }
  ```
* **Response (`data`):**
  ```json
  { "results": "[{\"title\":\"Team Standup\",\"start\":\"2026-07-20T09:00:00Z\",...}]" }
  ```

##### `get_calendar`
* **Description:** Get calendar items by date range.
* **Request:**
  ```json
  { "start_date": "2026-07-20", "end_date": "2026-07-26" }
  ```
* **Response (`data`):**
  ```json
  { "results": "[{\"title\":\"Sprint Review\",\"start\":\"2026-07-21T14:00:00Z\",...}]" }
  ```

##### `get_calendar_item`
* **Description:** Get a specific calendar item by its full href (the exact `href` returned by `search_calendar` or `get_calendar`).
* **Request:**
  ```json
  { "href": "/dav/calendars/user/martin/default/item-abc123.ics" }
  ```
* **Response (`data`):**
  ```json
  { "result": "{\"title\":\"Team Standup\",\"start\":\"2026-07-20T09:00:00Z\",\"href\":\"/dav/calendars/...\"}" }
  ```

##### `add_calendar_item`
* **Description:** Add a new calendar item.
* **Request:**
  ```json
  { "item_json": "{\"summary\":\"New Event\",\"dtstart\":\"2026-07-21T10:00:00Z\",\"dtend\":\"2026-07-21T11:00:00Z\"}" }
  ```
* **Response (`data`):**
  ```json
  { "result": "Calendar item created." }
  ```

##### `update_calendar_item`
* **Description:** Update a calendar item using a JSON patch.
* **Request:**
  ```json
  { "id": "/dav/calendars/user/martin/default/item-abc.ics", "update_json": "{\"summary\":\"Updated Summary\"}" }
  ```
* **Response (`data`):**
  ```json
  { "result": "Calendar item updated." }
  ```

##### `delete_calendar_item`
* **Description:** Delete a calendar item by ID.
* **Request:**
  ```json
  { "id": "/dav/calendars/user/martin/default/item-abc.ics" }
  ```
* **Response (`data`):**
  ```json
  { "result": "Calendar item deleted." }
  ```

##### `search_email`
* **Description:** Search email by any combination of `keyword`, `folder` (mailbox name), `start_date` / `end_date` (ISO `YYYY-MM-DD` or full RFC 3339), `from`, `to`, `is_unread`, and `is_flagged`. All provided filters are combined with AND. At least one filter field must be supplied. Results are paginated (default page size 10); every response includes `total` so the caller can request additional pages via `page` / `page_size`.
* **Request:**
  ```json
  { "keyword": "invoice", "folder": "Inbox", "from": "vendor@example.com", "is_unread": true, "page": 1, "page_size": 10 }
  ```
* **Response (`data`):**
  ```json
  { "results": "[{\"id\":\"abc123\",...}]", "total": 42 }
  ```

##### `get_email_by_id`
* **Description:** Get email by ID (full content including body).
* **Request:**
  ```json
  { "id": "abc123" }
  ```
* **Response (`data`):**
  ```json
  { "result": "{\"id\":\"abc123\",\"subject\":\"Invoice\",\"from\":...,\"body\":\"...\"}" }
  ```

##### `send_email`
* **Description:** Send an email to a recipient.
* **Request:**
  ```json
  { "to": "recipient@example.com", "subject": "Hello", "body": "Email body text." }
  ```
* **Response (`data`):**
  ```json
  { "result": "Email sent successfully." }
  ```

##### `search_contact`
* **Description:** Search contacts by keyword.
* **Request:**
  ```json
  { "keyword": "John" }
  ```
* **Response (`data`):**
  ```json
  { "results": "[{\"id\":\"contact1\",\"displayName\":\"John Doe\",\"email\":\"john@example.com\"}]" }
  ```

##### `get_contact`
* **Description:** Get contact details by ID.
* **Request:**
  ```json
  { "id": "contact1" }
  ```
* **Response (`data`):**
  ```json
  { "result": "{\"id\":\"contact1\",\"displayName\":\"John Doe\",\"email\":\"john@example.com\"}" }
  ```

##### `add_contact`
* **Description:** Add a new contact.
* **Request:**
  ```json
  { "contact_json": "{\"displayName\":\"Jane Doe\",\"email\":\"jane@example.com\"}" }
  ```
* **Response (`data`):**
  ```json
  { "result": "Contact created." }
  ```

---

#### 4. CSV Database Tools

*Conditionally available — only offered to the LLM when the user prompt contains "table", "csv", "database", or a CSV tool name.*

##### `create_csv`
* **Description:** Create a new CSV file database with specified headers.
* **Request:**
  ```json
  { "db_name": "users", "headers": ["id", "name", "age"] }
  ```
* **Response (`data`):**
  ```json
  { "name": "users", "path": "C:\\Users\\...\\db\\users.csv", "headers": ["id", "name", "age"] }
  ```

##### `list_csv`
* **Description:** List all CSV file databases.
* **Request:**
  ```json
  {}
  ```
* **Response (`data`):**
  ```json
  [
    { "name": "users", "path": "...", "headers": ["id", "name", "age"] },
    { "name": "products", "path": "...", "headers": ["sku", "price"] }
  ]
  ```

##### `add_rows`
* **Description:** Add rows to a CSV file database.
* **Request:**
  ```json
  {
    "db_name": "users",
    "rows": [
      { "id": "1", "name": "Alice", "age": "30" },
      { "id": "2", "name": "Bob", "age": "25" }
    ]
  }
  ```
* **Response (`data`):**
  ```json
  { "result": "Added 2 rows" }
  ```

##### `delete_rows`
* **Description:** Delete rows from a CSV file database based on a predicate.
* **Request:**
  ```json
  { "db_name": "users", "predicate": "name == \"Bob\"" }
  ```
* **Response (`data`):**
  ```json
  { "result": "Deleted 1 rows" }
  ```

##### `query`
* **Description:** Query a CSV file database using an evalexpr predicate, supporting sum and average aggregates.
* **Request:**
  ```json
  { "db_name": "users", "predicate": "age > 20" }
  ```
* **Response (`data`):**
  ```json
  {
    "rows": [
      { "id": "1", "name": "Alice", "age": "30" },
      { "id": "2", "name": "Bob", "age": "25" }
    ],
    "aggregate_result": null
  }
  ```

