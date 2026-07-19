# FastMD - Hardware-Accelerated Markdown Viewer

## Summary
FastMD is a high-performance, GPU-accelerated Markdown viewer for Windows built using Rust and the `egui` framework. It parses documents in the background, extracts YAML metadata tags, and provides dynamic tag-based filesystem filtering and table rendering.

## Background / Context
Markdown document viewing is historically handled by CPU-bound rendering engines or resource-heavy Electron shells. FastMD solves this by providing a lightweight native binary that leverages hardware acceleration (via DirectX on Windows) to render text and layout elements with minimal CPU and memory footprint, making it ideal for large note vaults or documentation trees.

## Detailed Analysis / Usage Guide

### Key Features
1. **GPU Acceleration**: Fluid rendering and layout, bypassing large web browser dependencies.
2. **Dynamic File Tree**: The left panel builds a tree of all markdown files and directories in real-time, showing folder expansion states. Double-clicking a file opens it in your Windows default external application.
3. **YAML Front-Matter Parsing**: Extracts front-matter metadata and displays it in a dedicated table format with a distinct container style.
4. **Concurrent Parser Pipeline**: On startup, a dedicated directory enumerator thread scans the filesystem and enqueues markdown file paths into a shared internal work queue. A pool of 4 parallel worker threads pulls files from this queue, parses their YAML front-matter, and updates the GUI in real-time, significantly boosting loading speed on multi-core processors.
5. **Live Directory Watcher**: Schedules a background thread using the Windows directory change API (`notify` crate) to listen to file creations, modifications, and removals. The file tree, tag lists, and currently open document reload instantly on save.
6. **Hierarchical Table of Contents (ToC)**: When a document contains H1 to H3 headers, a right panel is automatically displayed showing a ToC. Clicking a ToC item scrolls the markdown view directly to that header.
7. **Command Prompt Panel**: Displays a prompt panel at the bottom of the window for entering commands. Includes built-in AI agent slash commands: `/models` to list available LLM models and `/model <alias>` to switch the active model on the fly.

### Command-Line Usage
By default, FastMD opens and scans the current working directory. You can specify a custom directory to scan by passing it as the first argument:

```cmd
fastmd.exe C:\path\to\your\markdown\notes
```

If the provided directory does not exist, the application will fallback to scanning the current working directory.

### Configuration
FastMD uses a YAML configuration file for its internal settings, AI agent configurations, and external API integrations (such as JMAP and SearXNG). 

The application searches for the configuration file in the following order:
1. `%APPDATA%\fastmd\config.yaml`
2. `%USERPROFILE%\.fastmd.yaml`
3. `.fastmd.yaml` (in the current working directory)

If no configuration file is found, a default one will be automatically created at the first available location.

The `config.yaml` file supports the following options:

| Option | Type | Default Value | Description |
|--------|------|---------------|-------------|
| `api_url` | String | `https://openrouter.ai/api/v1` | The base URL for the LLM API endpoint. |
| `model` | String | `google/gemini-2.5-flash:free` | The specific LLM model ID to use for the AI agent. |
| `api_key` | String | `your-api-key-here` | The API key for authenticating with the LLM provider. |
| `models` | HashMap (Optional) | `{}` | A mapping of model aliases to specific LLM configurations (`model`, `api_url`, `api_key`). Use `/models` to list and `/model <alias>` to switch models. |
| `searxng_url` | String (Optional) | `http://localhost:8090` | The URL for a SearXNG instance to enable the `web_search` tool. Leave null to disable. |
| `jmap_clients` | HashMap (Optional) | `{}` | A mapping of account names to JMAP configuration objects (`url`, `token`) for email/contact tools. |
| `caldav_clients` | HashMap (Optional) | `{}` | A mapping of account names to CalDAV configuration objects (`url`, `username`, `password`) for calendar tools. |

Example `config.yaml` with models and clients configured:
```yaml
content_libraries:
  - name: "Workspace"
    root_folder: "C:\\path\\to\\your\\workspace"
    kind: "text"
    readonly: false
  - name: "Reference"
    root_folder: "C:\\path\\to\\reference\\docs"
    kind: "text"
    readonly: true
models:
  gpt4:
    model: "openai/gpt-4"
    api_url: "https://api.openai.com/v1"
    api_key: "your-openai-key"
  claude:
    model: "anthropic/claude-3-opus"
    api_url: "https://api.anthropic.com/v1"
    api_key: "your-anthropic-key"
jmap_clients:
  work:
    url: "https://api.fastmail.com/jmap/api"
    token: "your-fastmail-token"
caldav_clients:
  personal:
    url: "https://caldav.fastmail.com/"
    username: "you@fastmail.com"
    password: "app-password"
```

### Build and Deployment
To build the application in release mode and deploy it directly to `C:\tools\fastmd.exe`, run the following custom deployment target:

```bash
cargo run --bin deploy
```

This will:
1. Build `fastmd.exe` in release mode.
2. Create the `C:\tools` directory if it does not exist.
3. Deploy the compiled executable to `C:\tools\fastmd.exe` automatically.

If you wish to only build the executable without deploying:
```bash
cargo build --release
```
The resulting executable will be built at `target/release/fastmd.exe`.

---

## Key Findings / Recommendations
- **Deployment Location**: It is recommended to deploy the binary to a directory in your system `PATH` (such as `C:\tools`) so that you can invoke it from any terminal.
- **YAML Front-Matter Format**: Ensure tags are formatted using standard YAML lists or scalar strings for the background indexer to detect them:
  ```yaml
  ---
  title: Document Title
  tags: [vacation, cruise]
  ---
  ```

---

## Sources
- Rust Programming Language: [rust-lang.org](https://www.rust-lang.org)
- Egui GUI Library: [github.com/ocornut/egui](https://github.com/ocornut/egui)
- Pulldown-Cmark Parser: [github.com/pulldown-cmark/pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark)
- Walkdir Library: [github.com/BurntSushi/walkdir](https://github.com/BurntSushi/walkdir)
