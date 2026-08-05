# Virtual File System Specification

> Part of [`SPEC.md`](../../../SPEC.md) (FastMD crate). See the
> [Requirements Index](../../../SPEC.md#requirements-index) for the full
> REQ-xxx → file map.
>
> Owns VFS-001..VFS-009. (CONFIG-009 superseded; see "Superseded
> requirements" below.) The previous REQ-700..708 numbering is
> retired; this file is the canonical home and the only one new
> VFS work should cite.
>
> Code: `src/desktop/src/app/vfs/`. Egui-free.

## Scope

The VFS subsystem owns the library model (`ContentLibrary` data type
+ behaviour), the virtual-path parser (`VirtualPath` /
`VirtualPathError`), the resolution rules (path-traversal protection,
library-name validation, read-only enforcement), and the tool-facing
resolver that turns a virtual path into an absolute filesystem path.
The data type `ContentLibrary` is defined in `config/` (it is part of
the YAML schema) but its behaviour lives here.

## Requirements

> Format: EARS. VFS IDs replace the retired REQ-700..708 range and
> cover the same ground plus the priority field that was previously
> a non-numbered sub-rule of REQ-700.

### Libraries (model)

* [VFS-001] The system shall support multiple content libraries. The libraries have `[root_folder, name, kind, readonly (optional, default true), priority (optional, default 0)]` attributes. (Replaces REQ-700.)
* [VFS-002] The system shall support a content library of kind `text`. The behaviours throughout this document apply to this type. The tools are Markdown focused. (Replaces REQ-701.)
* [VFS-003] The system shall support a content library of kind `image`. The image library stores image files that are not directly exposed to the UI or tools. Instead, the system performs vision analysis on images (REQ-470 through REQ-478) and generates corresponding Markdown files that are indexed as text content. (Replaces REQ-701b.)
* [VFS-008] The `ContentLibrary::priority` field (default 0) orders grep search across libraries: libraries with higher priority are searched first. (Extracted from REQ-707.)

### Virtual Paths (parser + resolution)

* [VFS-004] The system shall support a virtual file system. The virtual paths are composed of the library name, then the files and directories present at the configured `root_folder`. Path traversal (`..` components) shall be rejected. (Replaces REQ-702.)
* [VFS-009] Virtual path resolution shall reject paths containing parent directory (`..`) components and validate the library name exists. (Replaces REQ-708.)

### Tool integration (contract for downstream consumers)

* [VFS-005] The Directory tree pane shall display the content library name for each library as the top-level node. (Replaces REQ-703.)
* [VFS-006] The file-based tools shall take virtual paths as arguments, and shall resolve these paths to fully qualified file names for the underlying operating system. (Replaces REQ-704.)
* [VFS-007] The `grep` tool shall search all libraries in priority order (highest first), and return a concatenated result. When the `list_files` tool is invoked with the `/` or `.` argument alone, it shall enumerate the list of libraries, enabling the LLM to continue the folder search for the virtual library subfolders. (Replaces REQ-705 + REQ-706.)

## Architecture

```
                  ┌──────────────────────────┐
   AppConfig ────►│  app::vfs::behaviour      │
                  │  (resolve + display       │
                  │   label + library ext)    │
                  └─────────────┬────────────┘
                                │
                  ┌─────────────▼────────────┐
                  │  app::vfs::virtual_path   │
                  │  (parser + errors)        │
                  └──────────────────────────┘
```

`ToolContext::resolve_virtual_path` in
[`src/tools/context.rs`](../../tools/context.rs) is a one-line shim over
`app::vfs::behaviour::resolve`. Callers that need a VFS resolution
without a `ToolContext` (e.g. background indexing, the prompt builder)
call `app::vfs::behaviour::resolve` directly with
`&config.content_libraries`.

## Cross-cutting references

- **VFS-005** — UI tree rendering lives in [`src/ui/SPEC.md`](../../ui/SPEC.md).
- **VFS-006** — Tool contract: filesystem tools implement `tools::Tool` and call `app::vfs::behaviour::resolve` via `ToolContext`. See [`src/tools/SPEC.md`](../../tools/SPEC.md).
- **VFS-007** — Grep ordering and list-files enumeration are implemented in [`src/tools/filesystem.rs`](../../tools/filesystem.rs) and registered in [`src/tools/registry.rs`](../../tools/registry.rs).
- **VFS-008** — The priority field is read by the indexer in [`src/background/SPEC.md`](../../background/SPEC.md) and by the tool registry's `grep` dispatch.
- **VFS-004, VFS-009** — Path-traversal protection is implemented in [`src/app/vfs/virtual_path.rs`](virtual_path.rs) and applied uniformly by `behaviour::resolve`.

## Superseded requirements

- **CONFIG-009** (formerly `config/SPEC.md`, Virtual Path Resolution) is superseded by **VFS-004** + **VFS-009**. The CONFIG-009 text is fully covered by those two VFS requirements. The `config/SPEC.md` cross-reference redirects here.
- **REQ-700..708** (formerly `src/desktop/SPEC.md`, "Libraries" section) is superseded by **VFS-001..VFS-009** in this file. The `src/desktop/SPEC.md` section has been replaced with a one-line pointer to this file.

## Quality gate (this subsystem)

- `cargo nextest run -p fastmd app::vfs` — all 33 unit tests pass (17 in `virtual_path`, 7 in `library`, 9 in `resolve`).
- `cargo nextest run -p fastmd tools::context` — `ToolContext` shim test (none directly; the shim is exercised by every tool test).
- No `eframe::egui` import anywhere under `app/vfs/` (per
  `src/desktop/AGENTS.md` §5 "`app/` is egui-free").
- `cargo doc --no-deps` renders `app::vfs` with no warnings; every
  `pub` item has a `///` doc.
