## UI/UX Design Review: FastMD Agent (Personal Productivity AI)

---

### 0. General UI/UX Fundamentals

**Rating: Adequate**

**Evidence:**
- **Consistency**: The app has reasonable internal consistency — dark theme throughout, consistent use of `RichText` for styled labels, uniform button patterns in context menus. However, visual hierarchy is flat: headings, indicators, and body text all use similar sizing and lack a clear typographic scale.
- **Feedback**: File indexing status is visible in the top bar with file count and a spinner. The agent status line shows "Initializing...", "Waiting for LLM...", etc. But clicks in the file tree (selecting a file, expanding a folder) produce no visual feedback beyond the selection highlight.
- **Visibility of system status**: The indexing indicator turns green when done. The agent status line is present. However, there is no persistent indicator of *which documents* are open in tabs unless you look at the tab bar — tabs are horizontally laid out with no overflow handling.
- **Error prevention**: Confirmation dialogs are absent for destructive actions. "Delete" on a file/directory in the tree context menu sends it to trash with no confirmation (`tree.rs:103-113`, `tree.rs:167-179`).
- **Recognition rather than recall**: No keyboard shortcuts are advertised. Users must know that `Enter` submits the command, `Shift+Enter` adds a newline. Context menus are the primary interaction, which are discoverable but not visible without right-clicking.
- **Aesthetic and minimalist design**: The UI is sparse but functional. The dark theme (`rgb(9,9,11)` background) is visually clean. Empty states have helpful text ("Select a markdown file from the left pane..."). However, the bottom panel command input occupies the full width with minimal chrome, which is good.

**Anti-patterns identified:**
- **No confirmation for delete**: Deleting files sends them to trash without any "Are you sure?" — this violates error prevention. **(Severity: Major)**
- **No undo mechanism**: No undo for file moves, renames, or deletions. Once an action is committed through a modal, it is irreversible within the app. **(Severity: Critical)**
- **Tab overflow**: If many tabs are opened, they overflow horizontally with no scroll or collapse mechanism (`center.rs:136-197`). This will break the layout. **(Severity: Major)**

**Recommendations:**
1. Add "Are you sure?" confirmation for Delete operations, even if sending to trash.
2. Add a simple undo toast or state history for recent file operations.
3. Implement horizontal scroll or dropdown overflow for tabs when they exceed available width.

---

### 1. Transparency & Reasoning Visibility

**Rating: Adequate**

**Evidence:**
- **Planning visibility**: The agent does NOT show a plan before execution. It immediately begins working — the user sees "Initializing agent..." and then tool calls stream in. There is no "intended action plan" stage.
- **Tool-use disclosure**: Tool calls ARE shown in the response, formatted as `> **Executing tool `grep`**` with quoted arguments, followed by result summaries. This is good — it surfaces tool usage inline.
- **Reasoning at decision points**: The agent's "thinking" is shown in a collapsible `Thinking Process` section (`center.rs:105-113`). This uses the `🤔...🤔` delimiter from the LLM response. However, it appears to only capture the model's chain-of-thought reasoning text and not the *evaluation* of alternatives or tradeoffs.
- **Source attribution**: When the agent fetches web content or reads files, the tool call shows arguments including file paths. But the result content is summarized (e.g., "Result: 42 line(s) read") rather than shown inline for verification.
- **Memory surfacing**: The agent system prompt includes user info (name, address, age, gender) from config (`agent.rs:79-127`), but the user has no visibility into what the agent "remembers" about them.

**Anti-patterns identified:**
- **No plan stage before execution**: The agent jumps directly to tool execution with no human review of the plan. **(Severity: Major)**
- **Tool results summarized, not shown**: Users cannot verify the content the agent read — they only see line counts and result sizes. **(Severity: Major)**
- **No memory/context disclosure**: Users have no way to see what the agent knows about them (name, preferences, history). **(Severity: Minor)**

**Recommendations:**
1. Add a "Review Plan" stage where the agent shows its intended steps before executing, with Approve/Modify/Reject buttons.
2. For read operations, include a collapsible preview of the content that was read, not just a line count.
3. Add a "What I know about you" command or button that reveals the agent's context.

---

### 2. User Control & Intervention

**Rating: Needs Improvement**

**Evidence:**
- **Step-level intervention**: The user CANNOT pause, modify, or skip individual steps. The agent runs its loop autonomously.
- **Approval gates**: There are NO approval gates for high-risk actions. The agent classifies tools as "safe" (read-only, like `grep`, `read_file`, `web_fetch`) vs. "unsafe" (write operations like `create_file`, `edit_file`, `delete_file`). But BOTH are executed without user approval — unsafe tools just run sequentially after safe ones complete in parallel (`agent.rs:388-437`).
- **Undo/override**: The only override mechanism is the "⏹ Stop" button (`bottom.rs:128-138`), which sets a cancel flag. This is binary — stop everything or let it run.
- **Control granularity**: No granular control. It's approve-all (by doing nothing) or abort-all (stop button).
- **Resume after interruption**: If the user stops the agent, there is no resume capability. The session is abandoned.

**Anti-patterns identified:**
- **Binary control only**: Stop or let run — no review-approve-skip per step. **(Severity: Critical)**
- **No approval gates for unsafe operations**: The agent can create/edit/delete files without asking the user. **(Severity: Critical)**
- **No resume after interruption**: Aborting the agent loses all progress. **(Severity: Major)**

**Recommendations:**
1. Implement approval gates for write/destructive tool calls (create_file, edit_file, delete_file, rename_file) — show the proposed action and require user confirmation before executing.
2. Allow users to "Pause After This Step" rather than only a hard abort.
3. Add a "Resume" mechanism that preserves agent state across interruptions.

---

### 3. Trust Calibration

**Rating: Needs Improvement**

**Evidence:**
- **Confidence signaling**: There is NO confidence signaling. The agent outputs responses as authoritative text with no indication of certainty.
- **Transparency depth**: The collapsible "Thinking Process" section is a good progressive disclosure pattern, but it only contains the model's chain-of-thought — not confidence levels, alternatives considered, or limitations.
- **Honest capability boundaries**: No onboarding screen or help text explains what the agent can and cannot do. The agent is simply invoked by typing in the command bar. Capabilities are implied by the tool list but not explained to the user.
- **Trust-building**: The system earns no trust progressively — it has full tool access from the first query.

**Anti-patterns identified:**
- **No confidence indicators**: Users cannot distinguish between a confident answer and speculation. **(Severity: Major)**
- **No capability disclosure**: The opening "⚡ FastMD Viewer" header gives no hint about what the AI can do. The empty state text only says "Select a markdown file". **(Severity: Minor)**
- **Full autonomy from the start**: No graduated autonomy — trust is demanded, not earned. **(Severity: Major)**

**Recommendations:**
1. Add confidence levels to agent responses (e.g., a subtle "High / Medium / Low" badge on responses).
2. Add a brief onboarding/dismissable hint on first use: "Try: 'Find all documents tagged work', 'Summarize my emails from last week', etc."
3. Consider a "capability level" setting that starts at read-only and graduates to full autonomy after confirmed reliability.

---

### 4. Information Architecture

**Rating: Adequate**

**Evidence:**
- **Activity panel separation**: The center panel serves dual duty — it either shows the file/viewer OR the agent session, never both simultaneously. This is simple but creates a context switch: when the agent runs, the document disappears and is replaced by the agent thread.
- **Conversation thread purpose**: The agent session IS the conversation thread and also acts as the activity log. Agent status, thinking, tool calls, and responses are all interleaved in a single scrollable area in the center panel.
- **Workflow tracking**: There is NO timeline view. The agent session shows incremental output but no persistent "workflow" state that survives across sessions or interruptions.
- **Multi-step visibility**: The interface is chat-scroll — it does not scale beyond a linear conversation. For long-horizon tasks, there is no tabular/dashboard view of steps.
- **Persistent context**: When the agent finishes, you can click "Back to Document" (`center.rs:86-88`) but this clears all agent state (`clear_agent_session_state`).

**Anti-patterns identified:**
- **Conflating conversation thread with activity log**: The agent's thinking, tool calls, and responses are all mixed in one chat-like view. **(Severity: Minor)**
- **No persistent workflow timeline**: Long-running multi-step processes have no summary view or checkpoint system. **(Severity: Major)**
- **Agent session replaces document**: Users cannot refer to the document while the agent runs. **(Severity: Major)**

**Recommendations:**
1. Implement a split-view mode where the agent session occupies the bottom half while the document remains visible above.
2. Add a session summary timeline showing each tool call, its duration, and result in a structured format.
3. Allow agent results to persist in a sidebar or expandable section rather than fully replacing the document view.

---

### 5. Status Communication

**Rating: Strong**

**Evidence:**
- **Proactive status**: The agent clearly communicates its state: "Initializing agent...", "Waiting for LLM completions...", tool execution messages, and the status line always shows the current phase. This is excellent.
- **Processing indicators**: The status line is specific (e.g., "Waiting for LLM completions...") and a spinner appears when the agent is running. However, it does NOT say things like "Searching 3 databases" — it only shows generic status strings from the LLM loop.
- **Streaming output**: Tool inputs and outputs ARE shown as they happen (streaming via `AgentResponse` messages pushed through the channel). The response area updates incrementally as each tool call result is appended.
- **Silence avoidance**: The agent sends periodic status updates. Long waits during LLM calls show "Waiting for LLM completions..." with a spinner, reducing the "is it broken?" anxiety.

**Anti-patterns identified:**
- Status messages are single-status only (one line), not multi-threaded or parallel progress. **(Severity: Minor)**
- Background indexing shows "Indexing workspace (found X files)..." but only during startup — if re-indexing triggers later, there's no visible progress. **(Severity: Minor)**

**Recommendations:**
1. Enhance status to show parallel progress: "Executing 3 safe tools in parallel..."
2. Add progress dots or a time-elapsed counter during LLM API calls to further reassure the user.

---

### 6. Error Recovery

**Rating: Adequate**

**Evidence:**
- **Three-part error messages**: The agent error messages are relatively good. An API failure shows: "HTTP Request failed with status 400: bad request" — but does NOT include "what to try next." Status 500, network errors, invalid JSON all produce different messages, but the actionable guidance is limited.
- **Error categorization**: The agent distinguishes between:
  - Missing API key → "API key not set. Please either configure your API key..."
  - HTTP status errors → "HTTP Request failed with status {code}: {body}"
  - Network errors → "HTTP Request failed: {error}"
  - Invalid JSON → "Failed to parse JSON response: {error}"
  - Missing choices → "Invalid response schema"
  - Missing message → "No message returned in choices"
  This is excellent error categorization.
- **Recovery routing**: There is no smart recovery routing. All errors just set the status to "Error: {msg}" and stop the agent.
- **Graceful degradation**: If the agent fails mid-task, the user cannot resume from where it left off. The partial output remains visible but the session is dead.

**Anti-patterns identified:**
- **No retry/repair suggestions**: Errors tell what happened and why, but not what the user can do next. **(Severity: Major)**
- **No partial failure recovery**: If 2 of 5 tool calls succeed and 3 fail, the whole agent process stops with no partial results preserved. **(Severity: Major)**
- **No retry buttons**: All errors are terminal — there's no "Try Again" or "Retry with different settings." **(Severity: Minor)**

**Recommendations:**
1. Add "what to try next" to every error message: e.g., "Check your API key in Settings, or use /models to switch to a different provider."
2. For partial failures, show which tool calls succeeded and which failed, allowing user to retry only the failed ones.
3. Add a "Retry" button on error states that re-runs the last prompt.

---

### 7. Conversational Design

**Rating: Adequate**

**Evidence:**
- **Informative onboarding**: The app has NO conversational onboarding. The first time a user sees the chat bar, there are no suggested prompts or capability hints. The "Quick Tasks" menu has only "Format Markdown" — a single option.
- **Suggested prompts**: There are no suggested prompts at all. The user must know what to type.
- **Context awareness**: The prompt prefix in the bottom bar shows the current directory context (e.g., `Workspace/subdir >`), which is good context signaling.
- **Progressive disclosure**: Agent thinking is in a collapsible section — good pattern. Tool call details are shown inline but could benefit from expand/collapse.
- **Scroll behavior**: The agent uses `stick_to_bottom(true)` for the thinking scroll area, which auto-scrolls during streaming. This is appropriate for real-time output but can be disruptive for reading.
- **Image inclusion**: Not applicable — the agent renders markdown and images would show as `[Image: url]` placeholders (`render.rs:96-97`), not actual images.
- **Save/share**: No save/share functionality for conversation content.
- **Window sizing**: The egui-based app has resizable panels and windows. The logs window is resizable. Good flexibility.
- **Voice input**: Not available.

**Anti-patterns identified:**
- **No capability introduction**: The chat bar is just an empty text field with hint text "Type command (Enter to submit, Shift+Enter for new line)". No suggestions or examples. **(Severity: Major)**
- **No suggested prompts as buttons**: "Quick Tasks" has only one option. No contextual suggestions. **(Severity: Major)**
- **No save/share**: Useful conversations cannot be saved or shared. **(Severity: Minor)**

**Recommendations:**
1. Add a set of suggested prompt buttons on first use, or after the agent finishes a task: "What next? Try asking to... [Summarize] [Search] [Create document]"
2. Include a "Quick Actions" row of buttons above the command input for common operations.
3. Add a "Copy conversation" or "Export as markdown" button to the agent session header.

---

### 8. Generative UI

**Rating: Missing (Not Applicable)**

The interface does not feature agent-generated UI components. The agent communicates through markdown text rendered in a predefined layout. This is appropriate for the application's scope — keeping the agent constrained to text output avoids the complexity and security risks of generative UI.

**Recommendation:** If generative UI is considered in the future, follow a declarative component catalog pattern with a pre-approved set of widgets (tables, charts, action cards) rather than allowing arbitrary HTML/component rendering.

---

### 9. Human-AI Balance

**Rating: Adequate**

**Evidence:**
- **Adjustable autonomy**: No adjustable autonomy. The agent always has full tool access or none (if stopped). Users cannot configure the level of assistance.
- **Co-pilot mode**: The agent does NOT suggest actions — it executes them autonomously. The "Quick Tasks" menu is a very limited form of suggestion (just "Format Markdown"). There's no "What would you like to do?" mode.
- **Contextual help**: The empty state text is helpful but minimal. There are no tooltips on the agent UI, no inline guidance about how to use the chat. The hint text in the input field is the only help.
- **Familiar patterns**: The tab interface and file tree are familiar desktop patterns. The chat input at the bottom mirrors messaging apps. The markdown rendering is standard.

**Anti-patterns identified:**
- **No adjustable autonomy**: The agent is either fully autonomous or stopped. No "suggest mode", "ask before writing", or "read-only mode". **(Severity: Major)**
- **No contextual help**: The agent UI explains nothing about itself. Users must guess or leave the app to learn. **(Severity: Medium)**

**Recommendations:**
1. Add a slider or dropdown for autonomy level: "Read Only / Suggest / Ask Before Writing / Full Autonomy".
2. Add a tooltip or info icon near the command input that shows a brief help card: "I can read, search, create, and edit documents. Try asking me to..."
3. Add confirmation prompts for write operations even in full autonomy mode.

---

### 10. Accessibility & Inclusion

**Rating: Needs Improvement**

**Evidence:**
- **Voice input**: Not available.
- **Clear labeling**: Buttons are text-labeled (not icon-only), which is good. The stop button uses "⏹ Stop" with both icon and text. The tabs use "❌" icon without text label for close — this is the exception.
- **Color cues**: The theme uses a consistent dark palette. Errors are red, success/inactive text is gray/white. However, there is no color-blind accessible alternative. The green status text (`rgb(100,255,100)`) and blue accent colors may be indistinguishable for some users.
- **Reading flow**: The agent auto-scrolls to the bottom during streaming (`stick_to_bottom(true)` and `scroll_to_cursor(BOTTOM)`). Users who want to read from the beginning will be disrupted. No "Pause auto-scroll" toggle is available.

**Anti-patterns identified:**
- **No voice input**: Users with motor impairments or situational limitations cannot use voice. **(Severity: Minor)**
- **No color-blind accessible theme**: The green/red status indicators may not be distinguishable. **(Severity: Major)**
- **Forced auto-scroll during streaming**: Users are scrolled to the bottom of agent output as it streams, which is disorienting if they want to read earlier content. **(Severity: Medium)**

**Recommendations:**
1. Add a "Pause auto-scroll" checkbox or button in the agent session view.
2. Use symbols in addition to color for status (e.g., ✓ for success, ✗ for error, ◌ for in-progress).
3. Add a toggle to switch between auto-scroll-on-update and manual-scroll modes in the agent output.

---

## Executive Summary

FastMD Agent is a functional, technically competent desktop markdown viewer with integrated AI capabilities. Its strengths lie in transparent tool-call logging, clear status communication during agent execution, and a clean, consistent dark theme. However, the interface has several critical gaps in the human-AI interaction layer. Most notably, it offers binary control only (run or stop), lacks approval gates for destructive tool calls, provides no confidence signaling, and has no adjustable autonomy. The conversational UX is bare — no onboarding, no suggested prompts, and no contextual help — which places a high cognitive burden on new users. Trust calibration is weak because the agent has full capability from the first query with no graduated autonomy. These issues are typical of early-stage AI agent interfaces but need addressing before the product can feel safe and confidence-inspiring.

---

## Top 3 Strengths

1. **Transparent tool-call logging**: Every tool invocation is shown inline with its arguments and a structured result summary, giving users visibility into what the agent is doing.
2. **Excellent error categorization**: The agent distinguishes between 6+ distinct failure modes (missing API key, network error, HTTP status, invalid JSON, missing schema, missing message) with specific messages for each.
3. **Clean, consistent visual design**: The dark theme with uniform typography, panel layout, and interaction patterns creates a professional, distraction-free workspace.

---

## Top 3 Critical Issues

1. **No approval gates for destructive operations**: The agent can create, edit, and delete files without user confirmation — this is a trust and safety failure. Binary control (stop the agent entirely or let it do anything) is insufficient.
2. **No undo mechanism**: File moves, renames, and deletions are irreversible within the app. Combined with issue #1, a single erroneous agent action can cause data loss.
3. **No capability onboarding or suggested prompts**: New users are dropped into an empty chat bar with no indication of what the agent can do, placing an unreasonable burden on discovery.

---

## Priority Action Plan

| Priority | Action | Impact | Effort | Dimension |
|----------|--------|--------|--------|-----------|
| 1 | Add approval gates for write/destructive tool calls (create, edit, delete, rename) with inline confirmation UI | Critical | Medium | User Control |
| 2 | Add undo/revert for recent file operations (move, rename, delete) | Critical | Medium | Error Recovery |
| 3 | Add ability to pause after current step (not just hard stop) | High | Medium | User Control |
| 4 | Add confidence badges (High/Medium/Low) to agent responses | High | Small | Trust Calibration |
| 5 | Add suggested prompts as clickable buttons above the command input on first use | High | Small | Conversational Design |
| 6 | Implement split-view so document stays visible during agent session | High | Large | Information Architecture |
| 7 | Add confirmation dialog for all "Delete" operations in the file tree | High | Small | General UI/UX |
| 8 | Add adjustable autonomy selector (Read-only / Suggest / Confirm writes / Full) | Medium | Medium | Human-AI Balance |
| 9 | Add "Show plan before executing" step for agent sessions | Medium | Large | Transparency |
| 10 | Add tab overflow management (horizontal scroll or dropdown) | Medium | Small | General UI/UX |
