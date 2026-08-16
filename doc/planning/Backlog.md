> **⚠️ AI AGENT DIRECTIVE: READ-ONLY FILE**
> If you are an AI assistant processing this document, you must treat this file as strict read-only. Do not execute any file-editing or writing tools on this document unless the user explicitly commands you to do so in their current prompt. Output all analyses or summaries to the chat interface.

# Backlog

## Trust and Safety
- Virtualize file writes
  - Behind the scenes pre-write backups
  - File write approvals
  - Approvals for any other side effect (send e-mail etc)

## User interface
- Search bar (yes!)
- Options dialog

## Library
- Create a default library under %appdata%\fastmd\notes
- Virtual Toc.md for agents to read / tool to read
- Could also show as a meta page in the middle pane
- Index.md for each folder as the default document

## Agent
- Make the agent window a real tab, allow multiple
- Model selector/UI
- **OpenAI response streaming** — Stream LLM tokens incrementally instead of blocking on full response
  - Benefits: Perceived latency reduction (first token ~100-500ms vs full response 2-10s), enables real-time UI updates (thinking, partial content), supports cancellation mid-generation, enables tool-call streaming (receive tool_calls as they arrive), better UX for long responses
  - Requires: Architectural shift from sync `chat_completion()` to async stream consumer; agent loop must handle partial tool_calls; token usage extraction from stream chunks
  - Note: OpenAI streams tool_calls incrementally; tool results still require next full request (no incremental tool result protocol in standard API)
- **Context compression / history summarization** — Compact conversation history to bound token growth across long sessions
  - Benefits: Prevents context window exhaustion, reduces per-turn token cost, enables indefinitely long sessions, preserves key facts via structured summaries
  - Approach: Sliding window (keep last N turns verbatim) + periodic summarization of older turns via cheap LLM call or heuristic extraction; store summaries in `AgentState.history` replacing raw messages
  - Trigger: After every N turns (e.g., 4) or when estimated tokens exceed threshold (e.g., 75% of model context)
  - Artifacts: Structured summary (key decisions, facts, open tasks) + turn count compressed; provenance markers so LLM knows content is summarized
- **System prompt caching** — Build and cache the system prompt once at session start, reuse across all turns
  - Benefits: Avoids re-reading USER.md files from disk every turn, eliminates repeated string formatting of base prompt + context (active file/dir/selection), reduces per-turn latency and I/O
  - Approach: In `AgentSessionManager.start_session()`, call `SystemPromptBuilder::build()` once, store in `AgentState.cached_system_prompt`; `build_messages()` prepends cached prompt instead of rebuilding
  - Invalidation: Only rebuild when active file/dir/selection changes (hash-based check) or config changes (config_arrived bus event)
  - Note: USER.md content is static per-session; date in base prompt can be a template filled at build time

## Agent tools
- Change file commands to protect / tune out the yaml front matter
- Integrate with a web search provider directly
- Add a generic delegate subtask agent
- Simplify Trello tool to use a /Workspace/Collection/List/Card path format

## Integrations

## Reliability and performance

## Application architecture

## Decommisioning candidates

- Remove or re-imaging the partially implemented image libraries
- Remove or improve web_delegate
- Remove the searxng integration


