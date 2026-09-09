# Email Context Efficiency — Plan

> Status: proposal
> Date: 2026-09-09
> Branch: `feature/tool-descriptions-ste-mcp-prefix`

## Context

A real agent trace ("read `Notes/Martin/Shoes.md` and search email for New
Balance purchases") consumed ~400k context tokens. The causal chain:

1. `search_email` with bare `keyword: "New Balance"` (two unquoted tokens:
   match-anywhere AND semantics per RFC 8621 §4.4.1) returned **1939 hits**
   with an irrelevant first page. The cursor was abandoned, so the full
   1939-item server-side result set was materialized for nothing.
2. Three follow-up variant searches (`NewBalance`, `NB`,
   `joesnewbalanceoutlet`) each materialized another full result set plus
   a first page into the transcript.
3. Having failed at precision, the agent compensated with recall: ten
   parallel `get_email_by_id` calls. That tool returns the **full
   HTML→Markdown converted body with no cap**
   (`simplify_email(&mut email, None)` in
   `src/agent/tools/jmap/email.rs`), so ten HTML-heavy order emails —
   displayed innocently as "13 line(s) read" — filled the window. Every
   subsequent turn re-sent them, because the agent loop replays the full
   transcript by design (`process_turn` in `src/agent/agent_impl.rs`).

Transcript resend is architecture, not bug (the chat-completion API is
stateless). Prompt caching would only discount cost/latency, never window
occupancy — and we send no explicit cache directives
(`src/agent/llm_client.rs`). The only lever that shrinks the window is
payload discipline: this plan.

Relevant requirements: `TOOL-033` (cursor pagination incl. `search_email`),
`TOOL-026a` (32-item email pages), `TOOL-036` (`window_note`
offset/limit convention to mirror), `TOOL-007` (cross-page `total`).
All in `src/agent/tools/SPEC.md`.

Out of scope: LLM summarization at ingest, relevance-ranked search,
attachment content extraction (see Non-goals below).

## Principles

1. **No unbounded payload may enter the transcript.** Every tool response
   kept in history gets a predictable size ceiling.
2. **Staged disclosure.** Each rung must answer "is this worth refining?"
   before the next, larger rung is taken.
3. **Consistency with existing UX.** Email windowing mirrors `window_note`
   (`TOOL-036`: 0-indexed offset/limit), so the LLM transfers its habits.
4. **Server-side work stays server-side.** Ranking, snippet extraction,
   and boilerplate stripping happen in Rust before tokens are minted.

## The ladder

Four rungs, from cheapest to most expensive. Investigations should
terminate as early as possible.

### Rung 1 — Search hits (exists, extend)

`search_email` stays as-is (metadata + 256-char server `preview`, 32/page
per `TOOL-026a`). Extensions:

- Surface `attachment_count` (plus filenames when few) per hit, so the
  LLM sees attachments without fetching.
- Add `subject` and `has_attachment` filter params (JMAP already supports
  both; they are currently unreachable — see prior gap analysis).
- Document phrase quoting on `keyword` (`"New Balance"` for exact
  sequence vs bare tokens for match-anywhere AND). Bare two-word queries
  against common words are what produced the 1939-hit haystack.

### Rung 2 — Match context (new)

Use JMAP `SearchSnippet/get` (RFC 8621 §5) to return the matched regions
with highlights for hits worth inspecting — typically a few hundred
tokens showing the query terms *in context*. Answers "is this hit really
about New Balance shoes or did it match a footer?" for ~1% of a full-body
fetch.

### Rung 3 — Windowed body read (new, the workhorse)

New `read_email_window {id, offset, limit}` mirroring `window_note`:
converted-Markdown body paged by lines (default limit ~80), each response
reporting `lines_shown`, `total_lines`, and whether more remains. Bounded
per call (~3–5k tokens), resumable via offset. Reuses the existing
`simplify_email(max_lines)` path, which is already unit-tested. Expected
to replace full-body fetch in ~95% of cases.

### Rung 4 — Full body (exists, gate it)

Keep `get_email_by_id` but cap the default (first ~200 lines + explicit
truncation marker pointing at `read_email_window`), with an opt-in
`full: true` escape hatch whose description warns about token cost.
Shape-compatible; bounded by default.

## Supporting changes

- **Boilerplate stripping at conversion time.** Order/marketing HTML is
  mostly nav, footer, tracking pixels, and legal text. Strip
  `<header>`/`<footer>`/nav elements, unsubscribe blocks, and sub-1px
  images in `convert_html_in_jmap` before Markdown conversion. Shrinks
  every rung that touches bodies.
- **Token-count metadata.** Every email response reports `approx_tokens`
  (chars/4 suffices) and `total_lines`, so the agent plans ("18k tokens,
  4 windows") instead of discovering size by accident.
- **Rollout order.** Quoting description (staged, uncommitted) → snippet
  + window tools → boilerplate stripping → cap + metadata. Each step is
  independently testable against the existing mock-server harness
  (`src/agent/tools/jmap/mock_server.rs`).

## Non-goals (deliberately excluded)

- **LLM summarization at ingest.** Doubles fetch latency, destroys exact
  facts the agent needs (order numbers, prices), adds a failure mode.
- **Relevance-ranked search.** Servers sort by date, not relevance; we
  cannot outrank without fetching everything first — the anti-pattern.
  Snippets (rung 2) are the cheaper substitute.
- **Attachment content extraction** (PDF invoices, …). Real need, separate
  feature with its own size problems; rung-1 filename metadata unblocks
  the triage decision without solving the whole thing.

## Open questions

- Is ~80 lines the right default window for rung 3?
- Should `read_email_window` pages enter the cursor cache (deduplicating
  repeat reads across turns) or stay live-fetch?
