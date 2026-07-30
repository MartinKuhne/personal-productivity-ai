# Configuration Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.
>
> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate)


## Scope

This module owns the application configuration schema, loading, and persistence. It covers the YAML configuration file structure (`config.yaml`), the configuration types, default template generation, and the virtual path resolution system. The code lives in `src/desktop/src/config/` and `src/desktop/src/config.rs`.

## Requirements

### Inline Editor Configuration

* [CONFIG-001] Inline Editor Toggle: The system shall provide a configuration option `inline_editor_enabled` (default: `false`) in `config.yaml` to enable the built-in inline text editor.
* [CONFIG-002] Edit Behavior Override: When `inline_editor_enabled` is `true`, selecting [Edit] from the file context menu (directory tree or tab bar) shall open the inline editor instead of launching the system default editor.

### PDF Converter Configuration

* [CONFIG-003] Converter Configuration: The system shall provide a configuration option `pdf_converter_command` in `config.yaml` specifying the executable and arguments to convert PDF to Markdown. The command shall receive the PDF file path as the first argument and the output Markdown file path as the second argument. Example: `["pandoc", "-f", "pdf", "-t", "markdown", "-o", "{output}", "{input}"]`.

### Configuration File Loading

* [CONFIG-004] Configuration File: The FastMD Viewer shall parse a YAML configuration file (`config.yaml`) from the standard user configuration path to retrieve the API key, model, endpoint URL, and multi-use model configuration.
* [CONFIG-005] Default Template Config: If the configuration file does not exist, then the FastMD Viewer shall create a default template configuration file.

### Model Configuration

* [CONFIG-006] Multi-Use Model Configuration: The configuration shall support a `models` list where each entry defines `model`, `api_url`, `api_key` (optional, inherits global), `use_case` (array: `chat`, `embeddings`, `vision`), and `cost` (optional integer, default 0, lower = cheaper). The system shall route requests to the appropriate model based on use_case. When multiple models match a use_case, the system shall prefer the model with the lowest `cost`.
* [CONFIG-007] Max Tokens Configuration: The configuration shall support a `max_tokens` field (default: 32768) in `config.yaml` to prevent runaway token generation. The system shall include this value in all LLM API requests as the `max_tokens` parameter.
* [CONFIG-008] Model Configuration: The system shall support a `models` configuration section in `config.yaml` defining multiple models with `use_case` tags: `chat` (default), `embeddings`, `vision`, and an optional `cost` field (integer, default 0, lower = cheaper) used for auto-model selection. Example:
```yaml
models:
  - model: "gpt-4o-mini"
    api_url: "https://api.openai.com/v1"
    use_case: ["chat", "vision"]
    cost: 5
  - model: "text-embedding-3-small"
    api_url: "https://api.openai.com/v1"
    use_case: ["embeddings"]
    cost: 1
  - model: "google/gemini-2.5-flash:free"
    api_url: "https://openrouter.ai/api/v1"
    use_case: ["chat"]
    cost: 0
```

### Virtual File System

> Virtual path resolution rules: see [`src/app/vfs/SPEC.md`](../../app/vfs/SPEC.md) (VFS-004, VFS-009). CONFIG-009 is superseded.

## Cross-cutting references

