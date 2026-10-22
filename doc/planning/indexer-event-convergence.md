# Indexer & Event-Pipeline Convergence (REQ-301..452)

> Status: **planned** — not started. Todo list derived from `src/app/SPEC.md`
> (Concurrent Workspace Indexer Pipeline, System state, Live Workspace File
> System Watcher, Tag/Directory/File management, PDF Support sections).
> Aspirational work only; the live state of the spec is in `src/app/SPEC.md`,
> which must not be edited here ([RUST-043]).
>
> Spec sync 2026-10-20: the owner re-published this section — REQ-306 replaces
> the old REQ-457 progress item, file management is now **REQ-440/441/442**
> (duplicate REQ-43x numbering resolved), and REQ-320/330 now key event
> emission on the crawler *enumerating* an entry (the previous
> "not previously known" gating is gone, so Q1 is settled: producers emit
> raw, consumers dedupe). REQ-451A/452 remain in the spec.
>
> Spec sync #2 2026-10-20: REQ-332/333/334 re-worded — the `[FileChanged]`
> event has a **single event-level `[Kind]`** ([REQ-332]: kind-homogeneous
> events; mixed-kind situations such as watch-rename fan out into multiple
> events), and per-file properties are `Path` ([REQ-333]) and `FileName`
> ([REQ-334]). This matches the existing `FileEvent { kind, paths }` shape:
> the per-file-Kind question is closed (option b); only the per-file
> `FileName` property remains to implement ([REQ-334]).

## Context

Current implementation (as-verified by code review, 2026-10-20):

- Crawler: `src/app/background/indexer.rs` (`scan_libraries`, `spawn_workers`)
- Events: `src/app/bus/events/file.rs` (`FileEventKind`) and
  `src/app/bus/events/typed.rs` (`FsEvent`)
- Managers: `src/app/workspace/tags.rs`, `workspace/watcher/directory_tracker.rs`,
  `workspace/watcher/file_processor.rs`
- Watcher: `src/app/workspace/watcher/file_watcher.rs`
- PDF: `src/app/background/pdf_converter.rs`, `src/app/bus/router/bus_router.rs`
- Event vocabulary → literally implemented on 2026-10-20 (variant names
  retired):
  - `FileChanged { kind: Added, files: [FileEntry{path, file_name}] }` =
    `[FileChanged][Added]` ([REQ-330..334])
  - `FileChanged { kind: Deleted }`  = `[FileChanged][Deleted]`
  - `FileChanged { kind: Changed }`  = `[FileChanged][Changed]`
  - `FileEvent::DirectoryChanged(DirectoryChanged{path, kind: Added})` =
    `[DirectoryChanged][Added]` ([REQ-320..322])
  - `FileEvent::DirectoryChanged(DirectoryChanged{path, kind: Deleted})` =
    `[DirectoryChanged][Deleted]`
  - `FsEvent::FileParsed` / `FsEvent::DirParsed` — worker→UI results channel
    (post-tag-parse), distinct from the spec's `[FileChanged]`

## Spec-hygiene items (no code)

- [ ] **T0.1 — Minor: REQ-331 is intentionally duplicated** (event-level
      `[Files]` list + per-file properties). Semantically clear — no action
      beyond confirming the owner meant both lines.
- [ ] **T0.2 — Branch.** Implementation must run on a fresh
      `feature/indexer-event-convergence` branch (current branch is
      `chore/iced-migration-plan` with an unrelated dirty file).

## Open decision questions

1. **Tag-manager routing ([REQ-421]/[REQ-422]).** Tags are pre-extracted by
   workers and ride `FileParsed`/`FileModified` payloads; the spec wants the
   tag manager itself to read the YAML header on Added/Changed. Keep
   pre-extraction as an approved optimization (recommended, protects
   [FMD-061]) or move reads to the manager.
2. **PDF routing ([REQ-450]).** Scan-time PDFs go straight to `tx_pdf`
   (`indexer.rs:141-147`) bypassing `[FileChanged]`; `BusRouter` fans bus
   events only post-scan. Converge through the bus, or document the direct
   channel as an approved deviation (recommended for now).

## Phase 1 — Event vocabulary (System state: REQ-320..333)

- [x] **T1.1 — `FileName` property per file ([REQ-334]).** DONE
      2026-10-20 (revised same day to the literal spec shape): the
      `[Files]` list entries materialize per-file `path` ([REQ-333]) and
      `file_name` ([REQ-334]) as `FileEntry` — no derived helper. Files:
      `bus/events/file.rs`.
- [x] **T1.2 — Event-level `Kind` values ([REQ-322], [REQ-332]).** DONE
      2026-10-20: crawler batches already kind-homogeneous; watcher mapping
      fixed (create/rename-target → `FileChanged[Added]`, confirmed
      `Modify(Any)` + `Modify(Name(From|To))` delivery on Windows) with test
      `test_watcher_rename_is_delete_then_create` in `file_watcher.rs`.
- [x] **T1.3 — "emits on enumeration" ([REQ-320], [REQ-330]).** DONE
      2026-10-20: producers emit raw events at enumeration time (crawler
      publishes one `FileChanged[Added]` batch per parent directory and one
      `DirectoryChanged[Added]` per directory — singular `Path` per
      [REQ-321]; watcher emits per notify event), consumers dedupe via set
      inserts (unchanged). Regression test
      `test_scan_libraries_publishes_dir_discovered_on_bus`.

## Phase 2 — Concurrent Workspace Indexer Pipeline (REQ-301..306)

- [ ] **T2.1 — REQ-301 audit.** Crawler covers `.md`, `.markdown`, `.txt`
      (markdown+text) and `.pdf` across **all configured content libraries**.
      Already implemented (`indexer.rs:107-169`); add a narrow integration test
      (`tests/`) asserting a library containing `sample.md`, `note.txt`,
      `spec.pdf` yields the three corresponding bus/queue emissions.
- [ ] **T2.2 — REQ-302 audit.** Worker pool is exactly `4`
      (`indexer.rs:192` in `background/task.rs`) with the existing
      `test_spawn_workers_*` tests. Confirm "up to four" is config-independent
      or make the constant spec-visible; add test pinning the default.
- [ ] **T2.3 — REQ-303 spinner + live file count.** `FastMdApp` has
      `file_processor.indexing_finished` but the top bar must show: (a) a
      loading spinner while any indexing is active, (b) the *current* count
      (grows as `FileParsed` arrives). Implement in `ui_iced` top bar + strings
      constants ([RUST-021], UI-string rules). Test: tier-3 UI test that
      `files_parsed`-derived view-model number renders while
      `indexing_finished == false`.
- [ ] **T2.4 — REQ-304 completion UX.** On `FsEvent::Finished`
      (`orchestrator.rs:468-475`): spinner → total file count; tag combobox
      populated (already `tags.rebuild()` → `all_tags`). Verify combobox
      binding to `Tags::all_tags` end-to-end; add view-model test.
- [ ] **T2.5 — REQ-305 left-column width.** Compute the widest
      file/dir-name combination among discovered entries when indexing
      finishes and adjust the left panel width. New behaviour; needs a pure
      helper (max display width over `all_files`/`all_dirs` display names) +
      unit tests, then a reducer application on `Finished`.
- [ ] **T2.6 — REQ-306 progress message completeness.** The 500-files/5-s
      cadence exists (`indexer.rs:179-194`) but the message lacks
      "conversions queued **and completed**" — completed counts must come from
      `PdfConverterWorker` (it publishes its own bus event; aggregate into the
      progress log entry). Tests: fake converter emitting N completions
      reflected in the next progress message.

## Phase 3 — Live Workspace File System Watcher (REQ-401..407)

- [ ] **T3.1 — REQ-401 watcher-per-library audit.** Verify `FileWatcher`
      maintains a watcher receiving OS notifications **for each content
      library** (recursive mode on each `root_folder`). If a single combined
      watcher exists, either split or reconcile with the spec owner.
- [x] **T3.2 — REQ-402/403 emission.** DONE 2026-10-20: watcher publishes
      `[DirectoryChanged]` ([REQ-402]; notify Folder kind, `Any` + gone
      extension-less heuristic) and `[FileChanged]` ([REQ-403]; Added/
      Changed/Deleted) — tests in `file_watcher.rs`.
- [x] **T3.3 — REQ-404 rename decomposition.** DONE 2026-10-20: both delivery
      modes handled — `Modify(Name(From|To))` split events and the
      pair-shaped `Name(Both)` notification — each surfaces as
      `Removed(old)` + `Discovered(new)` ([RUST-005] test first).
- [ ] **T3.4 — REQ-405 hot reload.** Modified active doc → invalidate
      `tabs.loaded_path` only when the editor is not open, then reload/redraw.
      Add orchestrator unit test simulating `FileModified` on the loaded path
      ([RUST-005]: write failing test first).
- [ ] **T3.5 — REQ-406 reset viewer pane.** Deleted active doc → tabs close
      (`close_tabs_for_removed_files`, `orchestrator.rs:200-214`), selection
      falls back, content cleared. Add unit test.
- [ ] **T3.6 — REQ-407 recursive watch of new dirs.** On
      `[DirectoryChanged][Added]`, register the new directory for recursive
      watching (notify `RecursiveMode`); unit test with the test watcher
      helper (`file_watcher.rs:329`).

## Phase 4 — Managers (REQ-420..432 ×2, REQ-450..452)

- [ ] **T4.1 — Tag manager event contract ([REQ-420..422]).** Today tags are
      pre-extracted by workers and delivered in `FileParsed`/`FileModified`
      payloads; the spec wants the tag manager to *receive* `[FileChanged]`
      Added/Changed and read the YAML header itself. Converge: have
      `hander_fs_event` read tags via `extract_tags_from_file` for
      Added/Changed events (or keep payload-tags as an approved optimization —
      record the decision). Tests: `Tags` receives Added → tags updated
      (existing tests in `tags.rs` extended for the routing path).
- [ ] **T4.2 — Directory manager ([REQ-430/431/432 dir]).** `DirectoryTracker`
      satisfies the unique-set update (existing `directory_tracker.rs` tests).
      Producer reconcile DONE 2026-10-20: crawler and watcher both publish
      `[DirectoryChanged]` on `Bus<FileEvent>` — the manager's channel ([REQ-431]);
      `FsEvent::DirParsed` is dormant. Remaining: drain-time Lagged-loss
      policy — see T5.2.
- [ ] **T4.3 — File manager ([REQ-440/441/442]).** `FileEventProcessor`
      maintains the file set with O(1) membership and handles
      Added/Deleted/Changed (its `Updated` arm only signals reload). Add tests
      for the full Added/Deleted/Changed triple at the orchestrator
      (`apply_file_event`) level.
- [ ] **T4.4 — PDF manager event contract ([REQ-450..452]).** Spec: PDF
      manager receives `[FileChanged]` events and ignores non-`.pdf`.
      Implementation: during the initial scan the indexer sends PDFs straight
      to `tx_pdf` (`indexer.rs:141-147`); post-scan, `BusRouter` fans
      `Discovered|Updated` pdf paths to `tx_pdf` (`bus_router.rs:59-100`).
      Converge: route scan-time PDF conversion through the event stream (or
      document as approved deviation). Tests: extension filter (REQ-451) and
      same-stem `.md` + timestamp staleness check (`PdfConversionJob::should_convert`)
      against REQ-452.
- [ ] **T4.5 — REQ-451A PDF/tool exposure regression test.** PDFs must never
      reach `search_notes` / `list_notes` / `read_note`: assert
      `AppOrchestrator::is_workspace_file` rejects `.pdf` and that the tool
      descriptors/envelope gate path-based tools on that predicate.

## Phase 5 — Structural cleanup surfaced by the diagnosis (adjacent, small)

- [ ] **T5.1 — Remove inert consumers.** The `FileEventProcessor` bus
      subscription that nobody drains (`ui_iced/app.rs:751`) and the legacy
      `orchestrator.rx` std bridge (`background/task.rs:149-166`) have no REQ
      backing; removing them lowers churn and eliminates the double-delivery
      hazard. Update `ARCHITECTURE_C4.md` if the wiring contract changes
      ([RUST-043] family).
- [ ] **T5.2 — Lag policy per consumer (`FMD-072`).** `BusReader::try_recv`
      maps `Lagged(_)` → `Empty` silently (`bus/core.rs:134-140`), so
      `DirectoryTracker`/file-manager drains violate FMD-072 (loss must be
      reported with count and provide re-sync). Use
      `try_recv_exposing_lag` in the two manager drains and log a warning +
      re-sync (full re-emit of known state on next event).
- [ ] **T5.3 — Scan-time UI responsiveness ([FMD-061]).** Separate follow-up
      tracking the O(N)-rebuild-per-message problem diagnosed on 2026-10-20
      (tree refresh once per frame, cache `note_skill_files`, coalesce
      `tree_dirty` once per batch). This is a prerequisite for verifying
      REQ-303 spinner actually *renders* during the scan on large libraries.

## Quality gate (every task completing code changes)

- [ ] `cargo check --quiet` — no errors or warnings
- [ ] `cargo nextest run -p fastmd` — all app tests pass ([RUST-002])
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo fmt --check`
- [ ] `cargo doc --no-deps --quiet`
- [ ] Every changed requirement covered by tests citing its REQ id in the
      doc comment ([RUST-040]); any behaviour/spec mismatch reported, not
      silently reconciled ([RUST-041]).

## Dependency order

T0.2 → (T1.1..T1.3) → Phase 2 → Phase 3 → Phase 4 → T5.1..T5.3 → quality gate.
T5.3 should land **before** T2.3 manual verification so the spinner is
observable during large scans.
