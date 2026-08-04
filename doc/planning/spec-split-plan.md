# Plan: Split `src/desktop/SPEC.md` into per-module specs

Status: proposal
Date: 2026-07-29
Owner: TBD

## 1. Goal

The single `src/desktop/SPEC.md` (~38 KB, 6 top-level sections, ~80 REQs) is
the only place requirements live, but the codebase is already split into
bounded subsystems (`markdown/`, `background/`, `ui/`, `agent/`, `tools/`,
`batch/`, `config/`, `app/`). The two layouts drift: each AGENTS.md and the
C4 architecture treat the modules as the source of truth, while the SPEC is
read end-to-end and a developer working on, say, `background/indexer.rs`
has to grep the whole file to find the relevant REQs.

This plan splits the SPEC by **subsystem** so each module owns the
requirements that drive its code, while preserving:

- every existing `REQ-xxx` identifier (no renumbering)
- the EARS formatting style and RFC 2119 key-words convention
- a single source of truth (the root SPEC becomes an index + cross-cutting
  concerns only)

It follows the existing precedent already in the repo: per-module
specifications already exist as `src/desktop/src/ui/SPEC.md` (the table
renderer uses `TBL-xxx` requirements there). This plan extends that
pattern to the rest of the crate.

## 2. Current state

```
src/desktop/SPEC.md                                 38 KB   6 top-level sections
src/desktop/src/ui/SPEC.md                          3.2 KB  table renderer only (TBL-xxx)
```

Current SPEC.md sections (with REQ ranges and the module they describe):

| Section | REQ range | Touches module(s) |
| --- | --- | --- |
| 1. UI Layout & Styling | 101-103, 149-198, 250-261 | `ui/`, `app/panel_layout.rs`, `app/tab_manager.rs`, `editor.rs` |
| 2. Markdown | 201-216 | `markdown/`, `markdown/table_width/`, `ui/render.rs` |
| 3. Indexer Pipeline | 301-305 | `background/indexer.rs`, `events/`, `app/tag_manager.rs` |
| 4. File System Watcher | 401-407 | `background/watcher.rs`, `app/watcher/`, `events/` |
| 5. PDF support | 450-458 | `background/pdf_converter.rs`, `background/indexer.rs` |
| 6. Background Process Log | 460-465 | `background/*` (producers) + `ui/background_logs.rs` (consumer) |
| 7. Image Support (Vision) | 470-478 | `background/vision_processor.rs`, `background/indexer.rs` |
| 8. CLI & Deployment | 501-504 | `main.rs`, `bin/deploy.rs` |
| 9. LLM Interface & Agent | 601-620, 630, 640 | `agent/`, `config/` |
| 10. Agent Behavior & UI | 613-619, 616 | `agent/`, `ui/panels/bottom.rs`, `ui/panels/center.rs` |
| 11. Libraries (virtual FS) | 700-708 (renumbered VFS-001..VFS-009) | `app/vfs/`, `tools/filesystem.rs`, `ui/tree.rs` (moved by `vfs-refactor-plan.md`) |
| 12. Batch processing | 800-813 | `batch/`, `ui/panels/top.rs` |
| 13. LLM tools table | (no REQ-xxx; tool-name-keyed) | `tools/` (and every tool family) |
| 14. CSV DB tools | 650-653 | `tools/csv_db/` |
| 15. Web Fetch | 660-665 | `tools/web.rs` |
| 16. YAML frontmatter template | (informational) | `markdown/`, `tools/yaml_header.rs` |
| Sources | (informational) | n/a |

Notable existing constraint: per `src/desktop/AGENTS.md` §4, every
user-facing behaviour must map to a `REQ-xxx`. The plan must keep that
invariant intact.

## 3. Target structure

```
src/desktop/
├── SPEC.md                          # trimmed: intro, ASCII diagram, REQ index,
│                                    # cross-cutting only. Becomes a navigation doc.
└── src/
    ├── agent/SPEC.md                # REQ-601..620, 630, 640 (LLM + agent behaviour)
    ├── background/SPEC.md           # REQ-301..305, 401..407, 450..478
    │                                # (indexer, watcher, PDF, vision, bg log producers)
    ├── batch/SPEC.md                # REQ-800..813
    ├── config/SPEC.md               # REQ-603..604c (config schema, models, converter cmd)
    ├── markdown/SPEC.md             # REQ-201..216, YAML frontmatter template
    ├── tools/SPEC.md                # LLM tools table + REQ-650..653, 660..665, 700..708
    ├── ui/SPEC.md                   # REQ-101..103, 149..198, 250..261, 619, 460..465 (UI half),
    │                                # 616 (thinking delimiter). The existing TBL-xxx
    │                                # table-renderer content stays where it is.
    └── editor/SPEC.md (NEW dir)     # REQ-250..261 — see §6.2 for the placement decision
```

Why this shape:

- Every existing module in the AGENTS.md §5 "Folder structure" map gets a
  SPEC sibling except `app/`, `bin/`, `events/`, `utils/` — those are
  glue/utility layers that don't own requirements on their own. Anything
  that crosses them is owned by the consumer (e.g. the `events/` bus is
  owned by `background/` because that's where its producers live).
- `editor.rs` today is a top-level file (no directory). Promoting it to
  `editor/` is out of scope for *this* plan (that is a separate refactor
  in the AGENTS §5 hierarchy). For now the inline-editor requirements
  live in `ui/SPEC.md` (see §6.2).
- CLI/Deployment (REQ-501..504) is small (3 REQs) and lives in `main.rs`
  and `bin/deploy.rs`; it stays in the root SPEC to avoid creating a
  single-requirement SPEC for a binary entrypoint.

## 4. REQ → module mapping (authoritative)

This is the table the migration uses. Numbers stay exactly as they are
in the current SPEC; only the file each one lives in changes.

### 4.1 `src/desktop/SPEC.md` (root, trimmed)

Keep:

- `# SPEC.md: FastMD Technical Specification` (title)
- `## Summary`
- `## Background / Context`
- The ASCII art layout diagram (it's the only place a top-down picture
  of the four panes lives; it's referenced by `ui/SPEC.md` and
  `ARCHITECTURE_C4.md`)
- `## Requirements` → replaced by `## Requirements Index` (see §5.2)
- `## Sources` (unchanged)
- REQ-501..504 (CLI & Deployment) — see §6.1

Move out: every other section. The `## Requirements` heading is replaced
by the index table described in §5.2.

### 4.2 `src/desktop/src/markdown/SPEC.md` (new)

- `### 2. Markdown` section (REQ-201..216)
- `## YAML frontmatter template` (the example block; cross-referenced
  from `tools/yaml_header.rs` and `markdown/document.rs`)

### 4.3 `src/desktop/src/background/SPEC.md` (new)

- `### 3. Concurrent Workspace Indexer Pipeline` (REQ-301..305)
- `### 4. Live Workspace File System Watcher` (REQ-401..407)
- `### 5. PDF support` (REQ-450..458)
- `### 7. Image Support (Vision)` (REQ-470..478)
- REQ-450..478 (PDF + vision) intentionally sit next to the indexer
  pipeline (3.1) because `background/indexer.rs` is the producer that
  enqueues them; grouping them keeps the discovery flow readable.
- REQ-460..465 (Background Process Log) is split:
  - REQ-460, 462, 463, 464, 465 → `background/SPEC.md` (producers and
    the log buffer itself)
  - REQ-461 (tab behaviour: auto-open, menu re-open) → `ui/SPEC.md`
    (consumed by `ui/background_logs.rs` and `ui/panels/center.rs`)

This split is the one place REQ-460..465 is not intact in a single
file. Mitigation: each side carries a `> See also: REQ-46x in
`background/SPEC.md` / `ui/SPEC.md`` cross-link so the contract is
discoverable from both ends.

### 4.4 `src/desktop/src/ui/SPEC.md` (already exists — extended)

Existing `ui/SPEC.md` keeps its `TBL-xxx` table-renderer content. The
plan **extends** it with the following top-level sections:

- `## User Interface Layout & Styling` (REQ-101..103)
- `## Left Column / Directory Tree` (REQ-149..183)
- `## Middle Column / File Viewer Area` (REQ-190..198)
- `## Inline Text Editor` (REQ-250..261)
- `## Tabbed Document Interface` (REQ-619)
- `## Background Process Log — UI` (REQ-461, plus a reference to
  REQ-460..465 in `background/SPEC.md`)
- `## Thinking Process Section` (REQ-616)

REQ-260, 261 (Cancel / colour scheme) is a UI concern of the inline
editor; the REQ goes to `ui/SPEC.md` even though the editor's parser
uses `markdown/`.

### 4.5 `src/desktop/src/agent/SPEC.md` (new)

- REQ-601..612 (LLM endpoint, config, agent loop, active file/dir
  context)
- REQ-613..619 (model selection, USER.md, history, thinking delimiter,
  quick tasks, tabs) — note REQ-616 (thinking) is also referenced from
  `ui/SPEC.md` because the rendering lives in the centre panel
- REQ-620, 630, 640 (JSON formatting, cancel, model configuration
  examples)

### 4.6 `src/desktop/src/config/SPEC.md` (new)

- REQ-603 (parse YAML config)
- REQ-604 (default template on missing file)
- REQ-604a (multi-use model configuration)
- REQ-604b (PDF converter command configuration)
- REQ-604c (max tokens)
- REQ-640 (model configuration example) — primary copy in `agent/`,
  duplicated here with a `> Schema definition lives here; behaviour in
  `agent/SPEC.md`` note

REQ-604a/640 are intentionally *not* assigned to either `agent/` or
`config/` exclusively: `config/` owns the schema (loader, types, secret
redaction), `agent/` owns the behaviour (use-case routing, cost-based
selection). Both SPECs reference the other.

### 4.7 `src/desktop/src/tools/SPEC.md` (new)

- `## LLM tools` — the full tool table (currently lives in the root
  SPEC as a single markdown table). Each tool row in the table gets a
  stable `ToolId` reference (e.g. `tools::filesystem::ReadFile`) so a
  future per-tool requirement can attach to it without breaking links.
- `## CSV Database Tools` (REQ-650..653)
- `## Web Fetch Pagination & Caching` (REQ-660..665)
- `## Libraries (Virtual File System)` (REQ-700..708)

REQ-700..708 is a borderline case (it touches `config/virtual_path.rs`,
`tools/filesystem.rs`, and `ui/tree.rs`). **Superseded by
`vfs-refactor-plan.md`:** the VFS got its own home at
`src/desktop/src/app/vfs/` and a dedicated spec at
`src/desktop/src/app/vfs/SPEC.md` with the requirements renumbered to
**VFS-001..VFS-009** (the REQ-700..708 IDs are retired). The VFS is a
domain concern, not a tool concern, so it lives next to the code that
implements it rather than in `tools/SPEC.md`.

### 4.8 `src/desktop/src/batch/SPEC.md` (new)

- `## Batch processing` (REQ-800..813) — entire section verbatim except
  for the prompt/mode terminology which already matches the existing
  `batch/` module names (`coordinator`, `discoverer`, `executor`,
  `file_matcher`, `prompts`).

## 5. What the trimmed root SPEC.md looks like

### 5.1 Sections that stay (with the same headings as today)

1. `# SPEC.md: FastMD Technical Specification`
2. `## Summary`
3. `## Background / Context`
4. `### 1. User Interface Layout & Styling` (the ASCII diagram block)
5. `## CLI & Deployment` (REQ-501..504)
6. `## Sources`

### 5.2 New `## Requirements Index` section

A single table that links every REQ-xxx range to the file it now lives
in. Format:

```markdown
## Requirements Index

| REQ range | Topic | Lives in |
| --- | --- | --- |
| 101–103, 149–198, 250–261, 460 (UI half), 461, 616, 619 | UI / panels / inline editor / tabs | `src/ui/SPEC.md` |
| 201–216 | Markdown parsing, rendering, ToC, table layout, YAML front-matter template | `src/markdown/SPEC.md` |
| 301–305 | Indexer pipeline, worker pool, GUI progress | `src/background/SPEC.md` |
| 401–407 | File system watcher, hot reload, new-dir watching | `src/background/SPEC.md` |
| 450–458 | PDF discovery, conversion trigger, converter command | `src/background/SPEC.md` |
| 460 (data half), 462–465 | Background process log buffer, filtering, persistence | `src/background/SPEC.md` |
| 470–478 | Image discovery, vision analysis trigger, result handling | `src/background/SPEC.md` |
| 501–504 | CLI directory input, UNC normalisation, deploy binary | `src/SPEC.md` (this file) |
| 601–612, 613, 615, 618, 620, 630 | LLM endpoint, agent loop, prompt context, cancel | `src/agent/SPEC.md` |
| 604–604c, 640 | Configuration schema (models, converter, max tokens) | `src/config/SPEC.md` |
| 614, 616, 619 | USER.md context, thinking process, tabbed interface (rendering half) | `src/agent/SPEC.md` + `src/ui/SPEC.md` |
| 650–653 | CSV database tools, query, aggregates, location | `src/tools/SPEC.md` |
| 660–665 | Web fetch pagination, headers, cache, force-refetch | `src/tools/SPEC.md` |
| 700–708 (now VFS-001..VFS-009) | Content libraries, virtual paths, priority, grep ordering | `src/app/vfs/SPEC.md` |
| 800–813 | Batch processing: dialog, modes, concurrency, cancel | `src/batch/SPEC.md` |
| (tool table) | `grep`, `read_file`, `web_fetch`, JMAP/CalDAV/CardDAV, CSV DB tools | `src/tools/SPEC.md` |
```

`ARCHITECTURE_C4.md` and the `desktop/AGENTS.md` already cite REQ-xxx;
this index becomes the single place to resolve any "where does REQ-xxx
live now?" question.

### 5.3 Cross-linking convention

Every per-module SPEC.md starts with:

```markdown
# <Module> Specification

> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate). See the
> [Requirements Index](../../SPEC.md#requirements-index) for the full
> REQ-xxx → file map.
>
> Owns REQ-NNN..MMM. Cross-cutting REQs that also touch this module are
> listed at the bottom of this file.
```

Cross-cutting REQs (where a requirement is implemented across two
modules) are listed in **both** SPEC files with the same wording. The
list lives at the bottom of each file under `## Cross-cutting
references` to keep the per-module REQ lists readable.

## 6. Placement decisions worth calling out

### 6.1 Why CLI & Deployment stays at root

REQ-501..504 is 3 short requirements about `main.rs` and `bin/deploy.rs`.
Both files are top-level entrypoints that don't have their own
subsystem directory. Creating a `src/desktop/src/main/SPEC.md` for three
requirements adds navigation overhead without grouping value. Keeping
them in the root SPEC keeps "what does the binary do" discoverable from
the top-level document.

### 6.2 Why the inline editor goes in `ui/SPEC.md` (not a new `editor/SPEC.md`)

`src/desktop/src/editor.rs` is a single top-level file with no
directory. The AGENTS §5 hierarchy treats the inline editor as an egui
widget owned by `ui/` (it has no `markdown/` knowledge of its own;
`editor.rs` calls `markdown::render`). The requirement block
REQ-250..261 is also a pure UI behaviour (modal overlay, monospace
area, status bar, Save/Cancel, validation, file write).

The right long-term move is to give the inline editor its own
`src/editor/` directory (mirroring the AGENTS §5 table-of-contents
shape), but that's a code-organisation change outside the scope of
"split the SPEC." This plan therefore groups REQ-250..261 under
`## Inline Text Editor` inside `ui/SPEC.md`, with a `// see ui/SPEC.md
REQ-250..261` reference at the top of `editor.rs` so the code
co-locates with its spec by file-system convention. If a future PR
extracts `editor/` as a module, the section moves verbatim.

### 6.3 Why REQ-460..465 splits

The Background Process Log is the canonical example of a feature that
crosses a producer boundary (`background/*` workers) and a consumer
boundary (`ui/background_logs.rs`). Putting the whole block in either
file alone leaves the other side without its contract. The split
proposed in §4.3 puts:

- **Producers and buffer** (REQ-460, 462, 463, 464, 465) in
  `background/SPEC.md` because the buffer is owned by
  `background/manager.rs` (or equivalent) and the producers are the
  workers themselves.
- **UI behaviour** (REQ-461 — auto-open on first task, menu re-open) in
  `ui/SPEC.md` because `ui/background_logs.rs` and the top frame menu
  own that surface.

Each side carries a `See also:` line that points at the other file's
REQ number so the contract is discoverable both ways.

### 6.4 Why `editor/SPEC.md` and `events/SPEC.md` are not created

`events/` (Bus, FileEventProcessor, DirectoryTracker) is plumbing.
Adding an events-level SPEC would either be empty (no REQs of its own)
or would duplicate the producer/consumer REQs from `background/` and
`ui/`. The same logic applies to `utils/`. The decision is to keep
SPEC.md at the level of *features*, not infrastructure. If a future
refactor extracts `events/` concerns that earn their own requirements
(e.g. ordering guarantees, exactly-once delivery), a new SPEC.md can be
added without disturbing the existing split.

## 7. Per-module SPEC.md template

To keep the new files consistent, every new SPEC.md uses the same
skeleton. `ui/SPEC.md` and `tools/SPEC.md` already deviate slightly
because they have legacy content; new files use the template below.

```markdown
# <Module> Specification

> Part of [`SPEC.md`](../../SPEC.md). See the [Requirements Index](../../SPEC.md#requirements-index).

## Scope

<one-paragraph statement of what this module owns. Cross-reference the
folder structure in `src/desktop/AGENTS.md` §5.>

## Requirements

<verbatim EARS-formatted REQ list from the root SPEC, in the original
order. REQ-xxx numbers are not changed.>

## Cross-cutting references

- REQ-NNN — <one-line description of how this module participates in
  the cross-cutting requirement, with a link to the other SPEC.md
  that owns the primary contract.>
```

Existing `ui/SPEC.md` is updated in place to add the new top-level
sections after its current `TBL-xxx` content. The table-renderer
content is not disturbed.

## 8. Migration steps (executable as a single PR)

Each step has an explicit acceptance check. The order is chosen so
every step ends in a buildable, testable state.

1. **Add per-module SPEC.md files in a new commit, without removing
   anything from the root.** Each file is a verbatim copy of the
   relevant section, prefixed with the boilerplate from §7. Acceptance:
   `cargo doc --no-deps --quiet` still passes; the root SPEC.md is
   byte-identical to before; the `/// (REQ-NNN)` comments in the code
   still resolve to a REQ in the root file (existing traceability
   preserved).
2. **Add the cross-link header** to each new per-module SPEC.md (the
   `> Part of SPEC.md` block from §5.3). Acceptance: a grep for the
   header string appears in every new file.
3. **Replace the root SPEC.md's `## Requirements` section with the
   `## Requirements Index` table** from §5.2, leaving `Summary`,
   `Background / Context`, the ASCII diagram, REQ-501..504, and
   `Sources` untouched. Acceptance: `rg "REQ-101|REQ-501|REQ-800"` in
   the root file still returns the index table, REQ-501..504, and the
   cross-link from `### 1. UI Layout` to `ui/SPEC.md`; no other
   per-requirement text is left at the root.
4. **Add `/// See src/<module>/SPEC.md REQ-NNN..MMM` cross-references**
   at the top of each module's `mod.rs` (and the top of the existing
   `ui/SPEC.md`'s owning module if not already present). Acceptance:
   every new SPEC.md has a referencing `mod.rs` comment.
5. **Update `src/desktop/AGENTS.md` §4** to point at
   `src/desktop/SPEC.md` for the index and at the per-module SPEC.md
   for traceability. Acceptance: AGENTS.md's "every user-facing
   behaviour maps to a `REQ-xxx` in `SPEC.md`" sentence is reworded to
   "...to a `REQ-xxx` in `src/desktop/SPEC.md` or one of its
   per-module SPEC.md siblings."
6. **Run the quality gate** (`cargo check`, `cargo nextest run`,
   `cargo clippy -- -D warnings`, `cargo doc --no-deps --quiet`,
   `cargo fmt --check`). Acceptance: clean.
7. **Update `ARCHITECTURE_C4.md`** if any module-boundary text refers
   to "see SPEC.md for requirements" — replace with the specific
   per-module SPEC.md reference. Acceptance: every link in
   `ARCHITECTURE_C4.md` resolves to a real file.

Steps 1 and 2 are reversible individually (just delete the new files);
step 3 is the only one that mutates the existing root. Doing step 3
last in its own commit makes the diff easy to review and easy to revert.

## 9. Traceability preservation

Three checks ensure no REQ-xxx is lost or duplicated during the split:

1. **Pre-migration baseline.** Before any change, run
   `rg -o "REQ-[0-9]+[a-z]*" src/desktop/SPEC.md | sort -u > /tmp/before.txt`.
   The count must be 80 (verified from the current file: 78 REQ-xxx
   lines + 2 sub-REQ references like `REQ-211b`). Save the file.
2. **Post-migration check.** After step 3, run the same command across
   `src/desktop/SPEC.md` and all new per-module SPEC.md files. The set
   union must equal `/tmp/before.txt` exactly. Acceptance: `diff
   <(sort -u /tmp/before.txt) <(cat src/desktop/SPEC.md src/*/SPEC.md |
   rg -o "REQ-[0-9]+[a-z]*" | sort -u)` is empty.
3. **Code-side check.** `rg "REQ-[0-9]+[a-z]*" src/desktop/src --type rust`
   must still match the same set of `REQ-xxx` references (the code
   never changed, but this catches accidentally deleted REQs from the
   specs).

## 10. Out of scope (explicit non-goals)

- **No new requirements.** This plan moves text; it does not add,
  remove, or reword any REQ. Any "we should also specify X" ideas go
  in a follow-up plan.
- **No code changes.** `editor.rs`, `ui/`, `background/`, `agent/`,
  `tools/`, `batch/`, `config/` source files are not modified by this
  plan. The only Rust-side change is the optional `/// see SPEC.md`
  breadcrumb in `mod.rs` files (step 4), which is a comment-only diff.
- **No `editor/` directory extraction.** The inline editor stays as a
  single top-level `editor.rs` file. Promoting it to a directory is a
  separate refactor.
- **No `TBL-xxx` renumbering.** The table-renderer requirements in
  `ui/SPEC.md` keep their `TBL-xxx` identifiers. The new
  `ui/SPEC.md` sections use `REQ-xxx` (consistent with the rest of
  the SPEC). The two namespaces coexist because the `TBL-xxx`
  requirements are table-engine concerns and were intentionally scoped
  narrower than user-facing requirements.
- **No automation script.** Step 1 is copy-paste from the root SPEC
  into 5 new files (~12 KB of total content). A script is overkill;
  the diff is reviewable by hand and the cross-link header in step 2
  is a single paragraph.

## 11. Open questions

1. **Should `tools/SPEC.md` introduce per-tool requirement IDs
   (e.g. `TOOL-ReadFile`)?** Currently the tools table has no
   `REQ-xxx` IDs, only the CSV DB and Web Fetch sub-requirements do.
   If we want a per-tool requirement (e.g. "read_file shall never
   return more than 10 MB"), a stable ID is convenient. Proposing:
   add the IDs as part of this split (cheap), but make them advisory
   (not cited from code) until a tool actually needs one. Decision:
   TBD — confirm before step 1.
2. **Should `app/` get a SPEC.md?** `app/` (panel_layout, tab_manager,
   selection_manager, dialog_manager, tag_manager, watcher/) is a
   coordination layer. Most of its behaviour is covered by the per-
   panel REQs in `ui/SPEC.md`. A separate `app/SPEC.md` would be
   cross-references only. Decision: **no** for v1; revisit if
   `app/` accumulates its own requirements.
3. **Markdown frontmatter template block.** It currently lives in the
   root SPEC as an example. The plan moves it to `markdown/SPEC.md`.
   Should `tools/yaml_header.rs` get a `/// see markdown/SPEC.md §
   "YAML frontmatter template"` breadcrumb? Decision: yes, add it
   during step 4.

## 12. Summary of file changes

| File | Action | Size after (approx) |
| --- | --- | --- |
| `src/desktop/SPEC.md` | Edit — replace `## Requirements` with index; keep intro, ASCII diagram, REQ-501..504, Sources | ~7 KB |
| `src/desktop/src/markdown/SPEC.md` | New | ~6 KB |
| `src/desktop/src/background/SPEC.md` | New | ~8 KB |
| `src/desktop/src/ui/SPEC.md` | Edit — add new sections after existing TBL-xxx | ~12 KB |
| `src/desktop/src/agent/SPEC.md` | New | ~5 KB |
| `src/desktop/src/config/SPEC.md` | New | ~2 KB |
| `src/desktop/src/tools/SPEC.md` | New | ~6 KB |
| `src/desktop/src/batch/SPEC.md` | New | ~3 KB |
| `src/desktop/AGENTS.md` | Edit — reword §4 reference | +1 line |
| `src/desktop/src/<module>/mod.rs` (5 files) | Edit — add `/// see SPEC.md` breadcrumb | +1 line each |

No code, no tests, no new requirements. Just a reorganisation of the
existing 38 KB of specification text into 8 files of ~5–8 KB each,
with a single index at the root.
