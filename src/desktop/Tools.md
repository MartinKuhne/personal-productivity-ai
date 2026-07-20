# FastMD Agent Tools Reference

The following table summarizes all 28 tools available to the LLM agent, categorized by functionality:

| Category | Tool Name | Description | Configuration Required |
| --- | --- | --- | --- |
| **Core Workspace** | `grep` | Search term case-insensitively across markdown files. | None |
| **Core Workspace** | `read_tags` | List all unique tags from markdown front-matter. | None |
| **Core Workspace** | `list_files_by_tag` | List markdown files containing a specific tag. | None |
| **Core Workspace** | `list_files` | List all markdown files in the workspace. | None |
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
| **JMAP Productivity**| `search_email` | Search emails by keyword. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `get_email` | Retrieve email details including body and metadata. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `get_email_by_id` | Get email by ID (full content). | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `send_email` | Send an email to a recipient. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `search_contact` | Search contacts by keyword. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `get_contact` | Retrieve a specific contact's details by ID. | `jmap_url`, `jmap_token` |
| **JMAP Productivity**| `add_contact` | Create a new contact using a JSON object. | `jmap_url`, `jmap_token` |

### Detailed Tool Schema and Arguments

Here is the exact schema and argument breakdown for each tool as declared in the codebase:

#### 1. Core Workspace Tools

##### `grep`
* **Description:** Search for a query string case-insensitively across all Markdown files in the workspace.
* **Arguments:**
  * `query` (string, **required**): The search term.

##### `read_tags`
* **Description:** Get all unique tags defined in front-matter headers of all Markdown files in the workspace.
* **Arguments:**
  * None.

##### `list_files_by_tag`
* **Description:** List all Markdown files that contain a specific tag in their front-matter.
* **Arguments:**
  * `tag` (string, **required**): The tag to filter by.

##### `list_files`
* **Description:** List all Markdown files in the workspace.
* **Arguments:**
  * None.

##### `read_file`
* **Description:** Read the entire text contents of a file at the specified path.
* **Arguments:**
  * `path` (string, **required**): The path to the file.
* **Implementation:** `tool_read_file` in `src/llm.rs`

##### `read_file_lines`
* **Description:** Read specific lines from a file (1-indexed).
* **Arguments:**
  * `path` (string, **required**): The path to the file.
  * `start_line` (integer, **required**): The start line index (inclusive, 1-indexed).
  * `end_line` (integer, **required**): The end line index (inclusive, 1-indexed).
* **Implementation:** `tool_read_file_lines` in `src/llm.rs`

##### `create_file`
* **Description:** Create a new file at the specified path with the provided content.
* **Arguments:**
  * `path` (string, **required**): The path of the file to create.
  * `content` (string, **required**): The content to write to the file.

##### `insert_lines`
* **Description:** Insert lines into a file at a specific 1-indexed line index.
* **Arguments:**
  * `path` (string, **required**): The path to the file.
  * `line_index` (integer, **required**): The 1-indexed position to insert lines at (the lines will be inserted right before this line).
  * `lines` (array of strings, **required**): The lines of text to insert.

##### `delete_lines`
* **Description:** Delete specific lines from a file (1-indexed, inclusive).
* **Arguments:**
  * `path` (string, **required**): The path to the file.
  * `start_line` (integer, **required**): The start line (1-indexed).
  * `end_line` (integer, **required**): The end line (1-indexed).

##### `replace_text`
* **Description:** Replace exact occurrences of old_string with new_string in a file.
* **Arguments:**
  * `path` (string, **required**): The path to the file.
  * `old_string` (string, **required**): The exact string to replace.
  * `new_string` (string, **required**): The replacement string.

##### `read_yaml_header`
* **Description:** Parse a YAML header from a markdown file and return its content representation.
* **Arguments:**
  * `path` (string, **required**): The path to the markdown file.

##### `write_yaml_header`
* **Description:** Write or update data in a YAML header to a markdown file.
* **Arguments:**
  * `path` (string, **required**): The path to the file.
  * `title` (string, **optional**): A brief title.
  * `summary` (string, **optional**): A three sentence summary of the contents.
  * `tags` (array of strings, **optional**): Array of tags.
  * `header-date` (string, **optional**): RFC 3339 timestamp.

---

#### 2. Web Integration Tools

##### `web_fetch`
* **Description:** Fetch content from a URL and convert it to Markdown.
* **Arguments:**
  * `url` (string, **required**): The URL to fetch.

##### `web_search`
* **Description:** Search the web using SearXNG (enabled only if `searxng_url` is configured in `config.yaml`).
* **Arguments:**
  * `query` (string, **required**): The search query.

##### `web_delegate`
* **Description:** Delegate complex web research to a sub-agent with web_search and web_fetch tools. Returns a summarized answer.
* **Arguments:**
  * `instruction` (string, **required**): The research task instructions for the sub-agent.

---

#### 3. JMAP Productivity Tools

##### `search_calendar`
* **Description:** Search the JMAP calendar by keyword.
* **Arguments:**
  * `keyword` (string, **required**): The search keyword.

##### `get_calendar`
* **Description:** Get calendar items by date range.
* **Arguments:**
  * `start_date` (string, **required**): Start date in ISO format.
  * `end_date` (string, **required**): End date in ISO format.

##### `get_calendar_item`
* **Description:** Get a specific calendar item by id.
* **Arguments:**
  * `id` (string, **required**): The calendar item ID.

##### `add_calendar_item`
* **Description:** Add a new calendar item.
* **Arguments:**
  * `item_json` (string, **required**): JSON representation of the calendar item.

##### `update_calendar_item`
* **Description:** Update a calendar item.
* **Arguments:**
  * `id` (string, **required**): The calendar item ID.
  * `update_json` (string, **required**): JSON patch object.

##### `delete_calendar_item`
* **Description:** Delete a calendar item.
* **Arguments:**
  * `id` (string, **required**): The calendar item ID.

##### `search_email`
* **Description:** Search email by keyword, optionally within a specific folder, date range, or by sender/recipient.
* **Arguments:**
  * `keyword` (string, *optional*): The search keyword. At least one filter field must be provided.
  * `folder` (string, *optional*): Folder/mailbox name to search within (e.g., "Inbox", "Sent"). Resolved to a JMAP mailbox ID automatically.
  * `start_date` (string, *optional*): Only return emails received on or after this date (e.g., "2026-01-01" or ISO 8601).
  * `end_date` (string, *optional*): Only return emails received before or on this date (e.g., "2026-12-31" or ISO 8601).
  * `from` (string, *optional*): Filter by sender name or email address.
  * `to` (string, *optional*): Filter by recipient name or email address.

##### `get_email`
* **Description:** Get email details by id.
* **Arguments:**
  * `id` (string, **required**): The email ID.

##### `send_email`
* **Description:** Send an email.
* **Arguments:**
  * `to` (string, **required**): Recipient email address.
  * `subject` (string, **required**): Email subject line.
  * `body` (string, **required**): Email content body.

##### `search_contact`
* **Description:** Search contacts by keyword.
* **Arguments:**
  * `keyword` (string, **required**): The search keyword.

##### `get_contact`
* **Description:** Get contact details by id.
* **Arguments:**
  * `id` (string, **required**): The contact ID.

##### `add_contact`
* **Description:** Add a new contact.
* **Arguments:**
  * `contact_json` (string, **required**): JSON representation of the contact.

