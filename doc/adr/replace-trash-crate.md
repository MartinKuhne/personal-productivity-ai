# Replace `trash` crate with inline Recycle Bin helper

Status: accepted
Date: 2026-08-08

## Context

The desktop app (`fastmd`) switched its eframe rendering backend from
`glow` (OpenGL) to `wgpu` (Vulkan / DX12 / Metal). The `wgpu` 30.x
dependency tree pulls in `windows 0.62` (via `wgpu-hal` →
`gpu-allocator`). Meanwhile the `trash 5.2.6` crate depends on
`windows 0.56`.

Having both `windows 0.56` and `windows 0.62` in the same binary
causes a compile-time trait mismatch inside `wgpu-hal`'s DX12
suballocation code: the two `windows-core` versions define duplicate
`Interface` and `CanInto` traits that are incompatible at monomorphisation.

```
error[E0277]: the trait bound `&ID3D12Heap: Param<ID3D12Heap, InterfaceType>`
              is not satisfied
  --> wgpu-hal-30.0.0/src/dx12/suballocation.rs:384:17
note: there are multiple different versions of crate `windows_core`
      in the dependency graph
```

`trash 5.2.6` is the latest published version compatible with
Rust 1.97.1 (the project MSRV) and there is no newer release that
moves to `windows 0.62`.

## Decision

Replace the `trash` crate with a minimal inline module at
[`src/desktop/src/utils/recycle_bin.rs`](../../src/desktop/src/utils/recycle_bin.rs).

The new module calls the same Windows COM API that `trash` uses
internally (`IFileOperation` with `FOF_ALLOWUNDO`), so the behaviour
is identical: files and directories are moved to the Recycle Bin
rather than permanently deleted.

Only one function from `trash` was used across the entire codebase —
`trash::delete(path)` — at three call sites in
[`ui/tree/render.rs`](../../src/desktop/src/ui/tree/render.rs).

### Alternatives considered

| Option | Outcome |
|--------|---------|
| Wait for `trash` to update to `windows 0.62` | No release available; blocked on upstream. |
| Patch `trash` via `[patch.crates-io]` | Fragile; would need to maintain a fork. |
| Disable the DX12 wgpu backend | Would degrade performance on Windows and reduce driver compatibility. |
| Use a different recycle-bin crate (`win_desktop_utils`, `ai-trash`) | These either wrap `trash` themselves or have the same `windows`-version issue. |
| **Inline the COM call (chosen)** | ~160 lines, zero new dependencies (reuses the `windows 0.62` already in the tree from wgpu). |

## Consequences

- The `trash` crate and `windows 0.56` are removed from the
  dependency graph, eliminating the version conflict.
- The `windows 0.62` crate (already present via wgpu) is added as a
  direct dependency with `Win32_System_Com` and `Win32_UI_Shell`
  features.
- The new `recycle_bin` module is Windows-only. If the project ever
  targets macOS or Linux natively, a platform-specific implementation
  (or re-adoption of `trash`) would be needed behind `cfg` gates.
- If `trash` releases a version compatible with `windows 0.62` in
  the future, the inline module can be replaced by re-adding the
  crate dependency.
