# Agent Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.
>
> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate).


## Scope

This module owns the Local LLM Interface, Tool-Call Agent Loop, and Agent Behaviour. It covers the OpenAI-compatible endpoint client, configuration-driven model routing (chat/embeddings/vision), the agent session manager, prompt builder with context injection (active file, active directory, USER.md), the tool executor with safe/unsafe parallelism, response formatting including thinking delimiters, and the quick-tasks menu. The code lives in `src/desktop/src/agent/`.

## Requirements

### 6. Local LLM Interface & Tool Call Agent

* [AGENT-001] OpenAI Compatible Endpoint: The FastMD Viewer shall support connections to an OpenAI compatible chat completions API.
* [AGENT-002] Default Settings: The FastMD Viewer shall default to the OpenRouter endpoint using the free model `google/gemini-2.5-flash:free`.
* [AGENT-003] Configuration File: The FastMD Viewer shall parse a YAML configuration file (`config.yaml`) from the standard user configuration path to retrieve the API key, model, endpoint URL, and multi-use model configuration.
* [AGENT-004] Default Template Config: If the configuration file does not exist, then the FastMD Viewer shall create a default template configuration file.
* [AGENT-005] Multi-Use Model Configuration: The configuration shall support a `models` list where each entry defines `model`, `api_url`, `api_key` (optional, inherits global), `use_case` (array: `chat`, `embeddings`, `vision`), and `cost` (optional integer, default 0, lower = cheaper). The system shall route requests to the appropriate model based on use_case. When multiple models match a use_case, the system shall prefer the model with the lowest `cost`.
* [AGENT-006] PDF Converter Configuration: The configuration shall support `pdf_converter_command` as an array of command and arguments with `{input}` and `{output}` placeholders.
* [AGENT-007] Max Tokens Configuration: The configuration shall support a `max_tokens` field (default: 32768) in `config.yaml` to prevent runaway token generation. The system shall include this value in all LLM API requests as the `max_tokens` parameter.
* [AGENT-008] Monospace Command Prompt: When the bottom panel command entry field is submitted, the FastMD Viewer shall process and execute the command through the Local LLM asynchronously without stalling the user interface.
* [AGENT-009] LLM Tools Library: The LLM Agent shall utilize functional tools as per the [LLM Tools] section in `src/tools/SPEC.md`.
* [AGENT-010] Real-time Stream Output: The FastMD Viewer shall display the LLM's active thinking sequence and render the final Markdown response in real-time inside the Central Panel.
* [AGENT-011] Tool Invocation Logging: The system shall print tool call invocations with their significant parameters to the response window. Tool arguments shall be formatted as pretty-printed JSON.
* [AGENT-012] Agent Loop: The agent shall execute a tool-use loop: (1) call LLM with tools, (2) execute safe tools in parallel, (3) execute unsafe tools sequentially, (4) append results to conversation, (5) repeat until LLM returns no tool calls or max 10 iterations. Safe tools: grep, read_tags, list_files_by_tag, list_files, read_file, read_file_lines, web_fetch, web_search, read_yaml_header, search_calendar, get_calendar, get_calendar_item, search_email, get_email_by_id, get_email, search_contact, get_contact, list_csv, query. Unsafe tools: create_file, insert_lines, delete_lines, replace_text, write_yaml_header, add_calendar_item, update_calendar_item, delete_calendar_item, send_email, add_contact, web_delegate, add_rows, delete_rows, create_csv.
* [AGENT-013] Active File Context: When the user sends an AI prompt and there is a file being displayed in the middle pane, the system shall send the full virtual path of that file (see [`src/app/vfs/SPEC.md`](../app/vfs/SPEC.md), VFS-004) with the system prompt.
* [AGENT-014] Active Directory Context: When the user selects a directory from the left pane, it becomes the directory context for the AI prompt. When the user sends an AI prompt and there is NO file being displayed in the middle pane, the system shall send the full virtual path of the directory context (see [`src/app/vfs/SPEC.md`](../app/vfs/SPEC.md), VFS-004) with the system prompt.
* [AGENT-015] Active Directory Context Display: The AI prompt shall display the directory context, relative to the base directory, with the prompt. Example: 'Users\Martin >'
* [AGENT-016] JSON Tool Argument Formatting: When displaying tool call arguments, format the JSON.
* [AGENT-017] Cancel AI Prompt: While an AI prompt is being executed, the system shall display a stop button. When the user clicks the stop button, the system shall abort the prompt processing.
* [AGENT-018] Model Configuration: The system shall support a `models` configuration section in `config.yaml` defining multiple models with `use_case` tags: `chat` (default), `embeddings`, `vision`, and an optional `cost` field (integer, default 0, lower = cheaper) used for auto-model selection (AGENT-019). Example:
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

### Agent Behavior & UI

* [AGENT-019] Auto-Model Selection: On application startup, if multiple models are configured with the `chat` use_case, the system shall automatically select the model with the lowest `cost` value and persist the selection to the configuration file.
* [AGENT-020] USER.md Context Injection: For each configured content library, if a USER.md file exists at the library root, its contents shall be appended to the system prompt as user context.
* [AGENT-021] Agent Conversation History: The agent shall maintain conversation history across prompts within a session. History is reset when the user clicks "Close" or starts a new session.
* [AGENT-022] Thinking Delimiter: Model reasoning/thinking content wrapped in `🤔...🤔` delimiters shall be extracted and displayed in a collapsible "Thinking Process" section separate from the main response.
* [AGENT-023] Quick Tasks Menu: The bottom panel shall provide a "Quick Tasks" menu with predefined prompts (e.g., "Format Markdown") that inject a structured prompt with YAML front-matter template.
* [AGENT-024] Tools Toolbar Button: The top toolbar shall provide a "Tools..." button alongside the existing "Batch..." button. Clicking the button shall open the Tools dialog (UI-051).
* [AGENT-025] Agent Loop Safety Source: The agent loop's parallel/sequential dispatch (per AGENT-012) shall source tool safety classification uniformly from the centralized tool safety manager for all registered tool names.

## Cross-cutting references

- [`src/config/SPEC.md`](../config/SPEC.md): Configuration
