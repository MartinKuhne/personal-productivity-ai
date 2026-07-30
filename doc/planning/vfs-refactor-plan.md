# Plan: Refactor Virtual File System into `app/vfs/`

Status: proposal
Date: 2026-07-29
Owner: TBD
Supersedes: `spec-split-plan.md` §4.7 (which proposed dropping REQ-700..708 into `tools/SPEC.md`; the VFS concern is a domain, not a tool, so it gets its own home under `app/`)

## 1. Goal

Promote the Virtual File System (VFS) from a "feature spread across `config/`, `tools/`, and the root `SPEC.md`" into a single, focused subsystem under `src/desktop/src/app/vfs/`, with its own spec at `src/desktop/src/app/vfs.md`.

Today, VFS concerns live in five places:

- **Spec** — REQ-700..708 in `src/desktop/SPEC.md` (`## Libraries`), and CONFIG-009 in `src/desktop/src/config/SPEC.md`.
- **Data shape** — `ContentLibrary` struct + its `resolve` / `is_writable` / `contains_path` / `root_path` / `display_label` methods, all in `src/desktop/src/config.rs`.
- **Path parsing & resolution** — `VirtualPath`, `VirtualPathError`, `parse`, `resolve`, `is_writable` in `src/desktop/src/config/virtual_path.rs`.
- **Real-path → virtual-label reverse mapping** — `library_display_label` free function in `src/desktop/src/config.rs`.
- **Tool-facing resolver** — `ToolContext::resolve_virtual_path` and `resolve_writable` in `src/desktop/src/tools/context.rs`.

The VFS is a domain concern, not a config concern. Moving it to `app/vfs/` puts the code where the spec lives, lets us test it as a unit, and removes the awkward "config module owns domain behaviour" coupling.

This plan complements `doc/planning/spec-split-plan.md` (which moves the rest of the SPEC into per-module files). The VFS section is the first "domain" subsystem to leave `config/`; if it works, the same shape can be reused for other domain concerns that have accumulated there.

## 2. Target structure

```
src/desktop/src/app/
├── dialog_manager.rs
├── messages.rs
├── panel_layout.rs
├── persisted.rs
├── selection_manager.rs
├── tab_manager.rs
├── tag_manager.rs
├── text_buffer.rs
├── watcher/
└── vfs/                          # NEW
    ├── mod.rs                    # //! module doc; pub use the pieces
    ├── virtual_path.rs           # MOVED from config/virtual_path.rs
    ├── library.rs                # NEW: ContentLibrary behaviour + library_display_label
    └── resolve.rs                # NEW: pure resolve_virtual_path() function
└── vfs.md                        # NEW: authoritative VFS spec
```

`ContentLibrary` **stays in `config/`** as a data type (it is loaded from `config.yaml`; the `AppConfig` field `content_libraries: Vec<ContentLibrary>` is a config schema concern). `app/vfs/library.rs` re-imports it and adds the domain behaviour that today is awkwardly attached as `impl ContentLibrary` in `config.rs`. `library_display_label` moves to `app/vfs/library.rs`.

`ToolContext::resolve_virtual_path` becomes a thin wrapper that calls the pure function `app::vfs::resolve::resolve(vpath, allow_write, &config.content_libraries) -> Result<Option<(PathBuf, bool)>, String>`. The pure function is the testable unit; the wrapper is a one-liner.

## 3. File-by-file edits

| # | File | Action |
| --- | --- | --- |
| 1 | `src/desktop/src/app/mod.rs` | Add `pub mod vfs;` and `pub mod vfs_spec;` (re-export of the markdown file is impossible — see §6 — so we instead add a pointer comment in `mod.rs`.) |
| 2 | `src/desktop/src/app/vfs/mod.rs` | **New.** Module doc `//!` summary + `pub use` of `virtual_path::{VirtualPath, VirtualPathError}`, `library`, `resolve`. |
| 3 | `src/desktop/src/app/vfs/virtual_path.rs` | **New.** Verbatim copy of `config/virtual_path.rs` (including all `#[cfg(test)] mod tests`). Imports updated to `use crate::app::vfs::library::ContentLibraryExt;` etc. as needed. |
| 4 | `src/desktop/src/app/vfs/library.rs` | **New.** `ContentLibraryExt` trait with `resolve`, `is_writable`, `contains_path`, `root_path`, `display_label` (moved from `impl ContentLibrary` in `config.rs`). Free function `library_display_label(libraries, path)` (moved from `config.rs`). All tests for these methods move here. |
| 5 | `src/desktop/src/app/vfs/resolve.rs` | **New.** `pub fn resolve(vpath, allow_write, libraries) -> Result<Option<(PathBuf, bool)>, String>`. Body is the current `ToolContext::resolve_virtual_path` body, with `self.config.content_libraries` replaced by the `libraries` parameter. All `#[cfg(test)] mod tests` for VFS resolution move here (8 tests from `tools/context.rs`). |
| 6 | `src/desktop/src/config.rs` | **Edit.** Remove `pub mod virtual_path;`. Remove `pub use virtual_path::{VirtualPath, VirtualPathError};` (the re-export moves to `lib.rs`). Remove `impl ContentLibrary { ... }` block and `pub fn library_display_label` (move to `app/vfs/library.rs`). Add a one-line `pub use crate::app::vfs::{VirtualPath, VirtualPathError};` re-export so `crate::config::VirtualPath` keeps resolving for any code that still imports it that way (currently only `tools/context.rs` and `lib.rs`; both will be updated in the same PR but the re-export costs nothing and prevents surprise breakage in untested paths). Keep `pub struct ContentLibrary { ... }` and `pub content_libraries: Vec<ContentLibrary>` field on `AppConfig`. |
| 7 | `src/desktop/src/config/virtual_path.rs` | **Delete.** |
| 8 | `src/desktop/src/tools/context.rs` | **Edit.** `use crate::app::vfs::{self, VirtualPath, VirtualPathError};`. Replace the body of `ToolContext::resolve_virtual_path` and `resolve_writable` with `vfs::resolve::resolve(...)` / `vfs::resolve::resolve_writable(...)` calls. Move the 8 `#[cfg(test)]` tests for VFS resolution out of this file and into `app/vfs/resolve.rs` (the harness builds an `AppConfig` directly, so the test bodies need only swap the call site). |
| 9 | `src/desktop/src/lib.rs` | **Edit.** `pub use app::vfs::{VirtualPath, VirtualPathError};` (replaces the line `pub use config::{..., VirtualPath, VirtualPathError, ...}`). Everything else stays. |
| 10 | `src/desktop/src/app/vfs.md` | **New.** Authoritative VFS spec. See §4 for layout. |
| 11 | `src/desktop/SPEC.md` | **Edit.** Replace the `### Libraries` section (REQ-700..708, lines ~145-156) with a one-line pointer: `> Moved to [`app/vfs.md`](src/app/vfs.md) (REQ-700..708).` Update the inline cross-references in `tools/SPEC.md` and `agent/SPEC.md` (AGENT-013 / AGENT-014) to point to `app/vfs.md` for the virtual-path semantics they currently cite by REQ number. |
| 12 | `src/desktop/src/config/SPEC.md` | **Edit.** Delete the `### Virtual File System` subsection (CONFIG-009). Add `> Virtual path resolution rules: see [`src/app/vfs.md`](../../app/vfs.md) (REQ-702, REQ-708). CONFIG-009 superseded.` Keep the cross-cutting reference at the bottom of the file but redirect it to `app/vfs.md`. |
| 13 | `src/desktop/src/agent/SPEC.md` | **Edit.** AGENT-013 / AGENT-014 currently say "send the full virtual path of that file" without citing the VFS spec. Add a parenthetical: `… the full virtual path (see `app/vfs.md`, REQ-702).` |
| 14 | `src/desktop/src/tools/SPEC.md` | **Edit.** If any TOOL-xxx requirement cites a REQ-700..708 in its body, redirect the citation to `app/vfs.md`. (None do today, but the existing `tools/filesystem.rs` `///` comments that mention "virtual" get updated to add a `REQ-702` citation pointing at the new home.) |
| 15 | `src/desktop/AGENTS.md` | **Edit.** §5 "Folder structure" block: replace the `├── config/                 # AppConfig + client structs, loader, secrets, VirtualPath` line with `├── config/                 # AppConfig + client structs, loader, secrets (data shapes only)`. Add `├── app/vfs/                # Virtual File System — parser, library behaviour, resolver (app/vfs.md)`. |
| 16 | `doc/technical-context/ARCHITECTURE_C4.md` | **Edit.** Update the "Folder Layout" block to match the new tree, and update the `config` container's responsibilities to drop the "virtual path resolution" bullet (it moves to a new `app/vfs` container). The C4 Component diagram for the VFS domain gets a new component node `app/vfs/` with sub-components `VirtualPath`, `ContentLibraryExt`, `resolve`. |
| 17 | `doc/planning/spec-split-plan.md` | **Edit.** §4.7 (the `tools/SPEC.md` section) — change the VFS paragraph to point at `app/vfs.md` instead of `tools/SPEC.md`. This is the only spec-split plan section that needs to be redirected. |

## 4. `src/desktop/src/app/vfs.md` layout

Follow the established subsystem-`SPEC.md` shape (cf. `tools/SPEC.md`, `config/SPEC.md`, `agent/SPEC.md`):

```markdown
# Virtual File System Specification

> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate). See the
> [Requirements Index](../../SPEC.md#requirements-index) for the full
> REQ-xxx → file map.
>
> Owns VFS-REQ-700..708. (CONFIG-009 superseded; see §"Superseded
> requirements" below.)
>
> Code: `src/desktop/src/app/vfs/`. Egui-free.

## Scope

The VFS subsystem owns the library model (`ContentLibrary` data type +
behaviour), the virtual-path parser (`VirtualPath` / `VirtualPathError`),
the resolution rules (path-traversal protection, library-name validation,
read-only enforcement), and the tool-facing resolver that turns a
virtual path into an absolute filesystem path. The data type
`ContentLibrary` is defined in `config/` (it is part of the YAML
schema) but its behaviour lives here.

## Requirements

> Format: EARS. REQ IDs are inherited from `src/desktop/SPEC.md` and
> are stable; they are *moved*, not renumbered.

### Libraries

* [REQ-700] The system shall support multiple content libraries. The libraries have [root_folder, name, kind, readonly (optional, default true), priority (optional, default 0)] attributes
* [REQ-701] The system shall support a content library 'text'. ...
* [REQ-701b] The system shall support a content library 'image'. ...
* [REQ-707] ContentLibrary priority field (default 0): grep searches libraries in descending priority order

### Virtual Paths

* [REQ-702] The system shall support a virtual file system. The virtual paths are composed of the library name, then the files and directories present at the configured root_folder. Path traversal (.. components) shall be rejected.
* [REQ-708] Virtual path resolution shall reject paths containing parent directory (..) components and validate the library name exists

### Tool Integration

* [REQ-703] The Directory tree pane shall display the content library name for each library as the top level node
* [REQ-704] The file based tools shall take virtual paths as arguments, and shall resolve these paths to fully qualified file names for the underlying operating system.
* [REQ-705] The [grep] tool shall search all libraries in priority order (highest first), and return a concatenated result
* [REQ-706] When the [list_files] tool is invoked with the '/' or '.' argument alone, it shall enumerate the list of libraries, enabling the LLM to continue the folder search for the virtual library subfolders

## Architecture

```
                  ┌──────────────────────────┐
   AppConfig ────►│  app::vfs::resolve       │
                  │  (pure function)         │
                  └─────────────┬────────────┘
                                │
                  ┌─────────────▼────────────┐
                  │  app::vfs::virtual_path   │
                  │  (parser + errors)        │
                  └─────────────┬────────────┘
                                │
                  ┌─────────────▼────────────┐
   ContentLibrary►│  app::vfs::library       │
   (from config)  │  (behaviour + label map) │
                  └──────────────────────────┘
```

`ToolContext::resolve_virtual_path` in `tools/context.rs` is a one-line
shim over `app::vfs::resolve::resolve`. Callers that need a VFS
resolution without a `ToolContext` (e.g. background indexing) call
`app::vfs::resolve::resolve` directly with `&config.content_libraries`.

## Cross-cutting references

- REQ-703 — UI tree rendering lives in [`src/ui/SPEC.md`](../ui/SPEC.md).
- REQ-704..706 — Tool implementations live in [`src/tools/SPEC.md`](../tools/SPEC.md). The tool registry calls `app::vfs::resolve::resolve` via `ToolContext`.
- REQ-707 — Indexer / search ordering lives in [`src/background/SPEC.md`](../background/SPEC.md).

## Superseded requirements

- **CONFIG-009** (config/SPEC.md, Virtual Path Resolution) is superseded by REQ-702 + REQ-708. The CONFIG-009 text is fully covered by those two REQs. The `config/SPEC.md` cross-reference redirects here.

## Quality gate (this subsystem)

- `cargo nextest run -p fastmd app::vfs` — all unit tests pass.
- `cargo nextest run -p fastmd tools::context` — ToolContext shim tests pass.
- No `eframe::egui` import anywhere under `app/vfs/` (per
  `src/desktop/AGENTS.md` §5 "`app/` is egui-free").
- `cargo doc --no-deps` renders `app::vfs` with no warnings; every
  `pub` item has a `///` doc.
```

## 5. Migration order (small iterations)

Per the root `AGENTS.md` ("Small Iterations" + "Test-Driven Changes"), each step is independently green. **No step deletes code before a step that proves the new home works.**

1. **Step 1 — Create `app/vfs/virtual_path.rs` as a verbatim copy of `config/virtual_path.rs`.** Update its `use` paths to import from itself (`use super::library` will fail because `library.rs` doesn't exist yet, so use a direct `use crate::config::ContentLibrary` for now and clean it up in step 3). Re-export `VirtualPath` and `VirtualPathError` from `app::vfs`. Add `pub mod vfs;` to `app/mod.rs`. Add `pub use crate::app::vfs::{VirtualPath, VirtualPathError};` to `config.rs` (keeps both paths working). **Run `cargo nextest run -p fastmd config` and `cargo nextest run -p fastmd tools::context` — all 20+ existing tests in `config/virtual_path.rs` pass from the new location.**
2. **Step 2 — Create `app/vfs/library.rs` with the `ContentLibraryExt` trait and `library_display_label` free function.** Copy the relevant `impl ContentLibrary` block + `library_display_label` body from `config.rs` into `library.rs` as trait methods + a free function. Add `impl ContentLibraryExt for ContentLibrary {}` blanket-ish (or just use UFCS). Re-export from `app::vfs`. Add a temporary `pub use crate::app::vfs::library::ContentLibraryExt;` to `config.rs`. **Move the 6 `#[cfg(test)]` tests for `ContentLibrary` methods and `library_display_label` to `library.rs`. Run `cargo nextest run -p fastmd config app::vfs` — all 6 tests pass from the new location.**
3. **Step 3 — Update `app/vfs/virtual_path.rs` to use `ContentLibraryExt` instead of directly calling `lib.resolve(&self.sub_path)`.** Update the `library.rs` to import `ContentLibrary` from `config` and provide the `resolve` method. **All 20+ tests in `virtual_path.rs` continue to pass.**
4. **Step 4 — Create `app/vfs/resolve.rs` with the pure `resolve` function.** Body is the current `ToolContext::resolve_virtual_path` body, parameterised on `&[ContentLibrary]`. Add `resolve_writable` as a sibling function (or inline it; current code has it as a method on `ToolContext`). Re-export from `app::vfs`. **Move the 8 `#[cfg(test)]` tests from `tools/context.rs` to `resolve.rs`; rewrite them to call the free function directly instead of through `ToolContext::new`. Run `cargo nextest run -p fastmd app::vfs` — all 8 tests pass.**
5. **Step 5 — Make `ToolContext::resolve_virtual_path` and `resolve_writable` one-line shims over `app::vfs::resolve`.** **Run the full `cargo nextest run -p fastmd` — every test in the crate passes; no test was deleted, no test was weakened.**
6. **Step 6 — Create `src/desktop/src/app/vfs.md` with the new spec content.** No code changes. (Spec lands alongside the code that implements it.)
7. **Step 7 — Update cross-references in `src/desktop/SPEC.md`, `config/SPEC.md`, `agent/SPEC.md`, `tools/SPEC.md`, and `src/desktop/AGENTS.md` §5 folder tree.** No code changes; only doc edits.
8. **Step 8 — Update `doc/technical-context/ARCHITECTURE_C4.md` folder layout + C4 Component diagram.** No code changes.
9. **Step 9 — Cleanup.** Delete `src/desktop/src/config/virtual_path.rs`. Remove the `pub use crate::app::vfs::...` re-exports from `config.rs` (callers in `lib.rs` and `tools/context.rs` are already updated in step 5; verify with `rg -n 'crate::config::virtual_path' src` and `rg -n 'config::VirtualPath' src` before removing). Update `doc/planning/spec-split-plan.md` §4.7 to redirect to `app/vfs.md`.
10. **Step 10 — Final quality gate.** Run the full §6 gate from `src/desktop/`.

## 6. Acceptance criteria

The refactor is "done" when **all** of these are true:

- [ ] `cargo check` in `src/desktop/` — no errors, no warnings.
- [ ] `cargo nextest run -p fastmd` — every test that was green before this refactor is still green. **No test deleted, no test weakened.** (The 20+ tests in `config/virtual_path.rs`, the 6+ tests for `ContentLibrary` methods, and the 8+ tests for `ToolContext::resolve_virtual_path` all live in their new homes and pass.)
- [ ] `cargo clippy -- -D warnings` — clean.
- [ ] `cargo fmt --check` — clean.
- [ ] `cargo doc --no-deps --quiet` — clean; `app::vfs` renders with `///` on every `pub` item.
- [ ] `app/vfs.md` exists at `src/desktop/src/app/vfs.md` and is the authoritative home for REQ-700..708.
- [ ] `src/desktop/SPEC.md` no longer contains the `### Libraries` section's body; it contains a one-line pointer to `app/vfs.md`.
- [ ] `config/SPEC.md` no longer contains CONFIG-009; the cross-cutting reference redirects to `app/vfs.md`.
- [ ] `lib.rs` re-exports `VirtualPath` and `VirtualPathError` from `crate::app::vfs` (not from `crate::config`).
- [ ] `rg -n 'eframe::egui|^\s*use eframe' src/desktop/src/app/vfs` returns nothing (egui-free).
- [ ] `rg -n 'crate::config::virtual_path' src/desktop/src` returns nothing (old module path gone).
- [ ] `ARCHITECTURE_C4.md` "Folder Layout" matches the output of `Get-ChildItem -Recurse src/desktop/src/app`.
- [ ] `doc/planning/spec-split-plan.md` §4.7 redirects to `app/vfs.md`.

## 7. Risks and mitigations

| Risk | Likelihood | Mitigation |
| --- | --- | --- |
| Breaking an untested call site that imports `crate::config::VirtualPath` or `crate::config::virtual_path` directly. | Medium | Step 1 keeps both paths working via `pub use`; step 9 only removes the re-export after `rg` confirms no remaining callers. |
| Moving tests to a different module file breaks a test that depends on `ToolContext::new` or `Bus::new` (e.g. some `tools/context.rs` tests build a `Bus` and a `ToolContext`). | High (already known — 8 tests) | Step 4 rewrites those tests to call the free function directly with a constructed `AppConfig`. The `Bus` becomes unnecessary; the test bodies shrink. |
| The `ContentLibraryExt` trait approach changes call-site syntax (e.g. `lib.resolve(...)` becomes `ContentLibraryExt::resolve(lib, ...)`). | High | The trait method body is identical to the old inherent method, so all *callers* that go through `lib.resolve(&self.sub_path)` keep working — Rust resolves inherent and trait methods transparently. Only direct construction or unusual syntax changes. |
| `cargo doc` warns about missing `///` on a moved `pub` item. | Low | `config/virtual_path.rs` already has `///` on every public item (`VirtualPathError` variants, `VirtualPath` struct, all `impl` methods). Verify with a `cargo doc --no-deps` run in step 1 before moving on. |
| The new `app/vfs/` directory is the first nested directory inside `app/` (current layout is flat). | Low (style) | `app/watcher/` already exists as a nested directory, so the pattern is established. No `AGENTS.md` change needed beyond the §5 folder tree update. |
| Renaming a `///` doc-comment example (e.g. `library.resolve(&vp.sub_path)` → `ContentLibraryExt::resolve(library, &vp.sub_path)`) makes doc-tests break. | Very low | The existing code has no `///` examples with runnable code (only prose). Verify with `cargo test --doc` in step 1. |

## 8. Out of scope

- Behavioural changes to VFS resolution (e.g. supporting relative library paths, symlink handling, globbing). The refactor moves code; it does not change semantics.
- Renumbering REQ-700..708. Per `doc/technical-context/AGENTS.md` §2, REQ IDs are stable; they are *moved*, not renumbered.
- Adopting a third-party VFS crate (e.g. `vfs`, `virtual-fs`). The current hand-rolled parser is small (130 lines) and well-tested; replacing it is a separate decision.
- Splitting `ContentLibrary` into a config-only data type (e.g. moving the struct definition itself out of `config.rs`). The data type is *configured* — it belongs in the config module. Only the *behaviour* (the methods) moves.

## 9. Summary

The VFS is a domain concern masquerading as a config concern. This plan moves its spec to `app/vfs.md` and its code to `app/vfs/`, with backward-compatible re-exports, in nine small steps, each independently green. The first three steps land before any spec is touched, so the code is provably correct before the docs are reorganised. Total blast radius: ~600 lines of Rust moved, ~70 lines of spec reorganised, one new spec file, one new module directory, one C4 diagram update. No test is deleted or weakened.
