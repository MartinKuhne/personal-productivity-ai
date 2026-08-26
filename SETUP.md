# Setup

## Warnings

- This is an unfinished project, under development. All software has bugs.
- AI and LLMs are new technology
- This is a project for developers and power users

## Installation

### Standard Installation

```bash
cargo install --git https://github.com/MartinKuhne/personal-productivity-ai
```

### Installation with Vector Search (RAG) Support

To enable semantic vector search with Qdrant:

```bash
cargo install --git https://github.com/MartinKuhne/personal-productivity-ai --features vector-search
```

---

## Infrastructure Services (Docker Compose)

FastMD includes an integrated Docker Compose stack in `deploy/` for local infrastructure dependencies:
- **Qdrant** (Vector database for semantic search)
- **SearXNG** (Privacy-respecting metasearch engine)
- **Valkey** (In-memory cache for SearXNG)

### Starting the Services

From the project root:

```bash
docker compose -f deploy/docker-compose.yml up -d
```

- **Qdrant Web Dashboard**: [http://localhost:6333/dashboard](http://localhost:6333/dashboard) (gRPC API on port `6334`)
- **SearXNG Search Web UI & API**: [http://localhost:3001](http://localhost:3001)

An environment template is provided at [`deploy/.env.example`](file:///C:/Users/mkuhn/src/ppai.dev2/deploy/.env.example).

---

## Configuration

FastMD uses a YAML configuration file for internal settings, AI agent configurations, model endpoints, and external service integrations.

On Windows, the preferred location is `%AppData%\fastmd\config.yaml`.

The application searches for the configuration file in the following order:
1. `%APPDATA%\fastmd\config.yaml`
2. `%USERPROFILE%\.fastmd.yaml`
3. `.fastmd.yaml` (in the current working directory)

If no configuration file is found, a default one will be automatically created at the first available location.

### Setting up a Library

FastMD is centered around one or more libraries of markdown formatted notes:

```yaml
content_libraries:
  - name: "Notes"
    root_folder: "C:\\path\\to\\your\\workspace"
    kind: "text"
    readonly: false
  - name: "Reference"
    root_folder: "C:\\path\\to\\reference\\docs"
    kind: "text"
    readonly: true
```

### LLMs and Embeddings

FastMD communicates with OpenAI-compatible chat and embedding endpoints. It does not run LLMs in-process; self-hosting is strongly recommended for privacy (protecting personal PII).

- **Local Runners**: [LM Studio](https://lmstudio.ai/), Ollama, or vLLM.
- **Cloud Providers**: [OpenRouter](https://openrouter.ai/), OpenAI, Anthropic, etc.

Configure your models in `config.yaml`:

```yaml
models:
  chat-model:
    model: "qwen/qwen-2.5-32b-instruct"
    api_url: "http://localhost:1234/v1"
    api_key: "lm-studio"
    cost: 0.0
    use_cases:
      - chat

  embed-model:
    model: "qwen3-embedding-4b" # or text-embedding-3-small
    api_url: "http://localhost:1234/v1"
    api_key: "lm-studio"
    cost: 0.0
    use_cases:
      - embeddings
```

### Vector Search (Qdrant)

When compiled with `--features vector-search` and an embedding model configured above, FastMD automatically indexes your Markdown notes into Qdrant chunks for semantic search.

```yaml
qdrant_url: "http://localhost:6334"
qdrant_collection: "fastmd_chunks"
# qdrant_api_key: "optional-key"
```

#### How to Use Vector Search Effectively

- **Semantic vs. Keyword Queries**: Vector search operates on dense embeddings (meaning, context, concepts) rather than exact word matching.
  - ❌ *Isolated keywords* (e.g. `"invoice"` or `"2007"`): Embeddings for single keywords have weak semantic context against full paragraphs and may be filtered out. For exact terms, IDs, or filenames, use `grep_search`.
  -  *Descriptive / Conceptual queries* (e.g. `"Monthly electricity utility bills from 2024"` or `"Checking account statement and balance history"`): Natural language queries carry strong directional vectors that match relevant paragraphs accurately.
- **Distance Threshold (`max_distance`)**:
  - `0.6` (Default): Standard semantic threshold ($1.0 - \text{cosine\_similarity} \le 0.6$, corresponding to $\ge 0.40$ cosine similarity).
  - `0.3`–`0.5`: High precision / strict matches (only chunks very closely aligned with the query).
  - `0.8`–`1.0`: Broad thematic discovery or shorter search terms.


### Web Search (SearXNG)

FastMD supports SearXNG to perform web searches via its JSON API.

```yaml
searxng_url: "http://localhost:3001"
```

### Email (JMAP)

Connect to a JMAP-compatible email host (e.g. Fastmail):

```yaml
jmap_clients:
  work:
    url: "https://api.fastmail.com/jmap/api"
    token: "your-fastmail-token"
```

### DAV (Calendar & Contacts)

Configure CalDAV and CardDAV synchronization:

```yaml
caldav_clients:
  personal:
    url: "https://caldav.fastmail.com/"
    username: "you@fastmail.com"
    password: "app-password"
```

### To-Do Lists (Trello)

FastMD can integrate with Trello for maintaining tasks and to-do lists:

```yaml
trello_client:
  token: "your-trello-token"
  api_key: "your-trello-key"
```

### PDF Conversion

When provided with a PDF-to-Markdown conversion command, FastMD automatically extracts Markdown representations for PDF documents:

```yaml
pdf_converter_command:
  - marker_single
  - '{input}'
  - --output_dir
  - '{output}'
  - --output_format
  - markdown
  - --disable_image_extraction
```

### Configuration Reference

The `config.yaml` file supports the following top-level options:

| Option | Type | Default Value | Description |
|--------|------|---------------|-------------|
| `system_prompt_extension` | String (Optional) | `null` | Additional text to append to the AI system prompt. |
| `inline_editor_enabled` | Boolean | `false` | Enable the built-in inline text editor. |
| `csv_db_path` | String (Optional) | `null` | Override the default storage location for CSV databases. |
| `searxng_url` | String (Optional) | `http://localhost:8090` | Endpoint for the SearXNG web search engine. |
| `qdrant_url` | String (Optional) | `http://localhost:6334` | gRPC endpoint for the Qdrant vector database. |
| `qdrant_collection` | String (Optional) | `fastmd_chunks` | Collection name for vector embeddings. |
| `qdrant_api_key` | String (Optional) | `null` | Optional API key for authenticated Qdrant instances. |
| `max_tokens` | Integer | `32768` | Maximum tokens for LLM response generation. |
| `table_width_strategy` | String | `hybrid` | Markdown table width deficit layout strategy. |
