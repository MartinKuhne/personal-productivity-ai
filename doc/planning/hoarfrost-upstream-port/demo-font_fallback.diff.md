# `font_fallback.rs` — local vs. upstream

> **Status:** Port record — captured 2026-11-01 from the working tree, **not yet
> applied upstream**. Companion to
> [`hoarfrost-external-crate-plan.md`](../hoarfrost-external-crate-plan.md)
> §2.2 and Phase 0.1.

Records the difference between this repo's `src/hoarfrost-demo/src/font_fallback.rs`
and the upstream version, so the change can be re-applied upstream if it is ever
lost. Nothing in this folder was written into `src/hoarfrost-demo/`.

## How this diff was produced

```
git diff --no-index -- src\hoarfrost-demo\src\font_fallback.rs D:\hoarfrost\src\hoarfrost-demo\src\font_fallback.rs
```

`a/` is **this repo** (`D:\ppai2`), `b/` is the **upstream** checkout
(`D:\hoarfrost`). The upstream side is the committed `dd7af16` blob, not
uncommitted work — verified: `git -C D:\hoarfrost rev-parse dd7af16:src/hoarfrost-demo/src/font_fallback.rs`
and `git hash-object` of the working-tree file both give
`c53c046ef7e5cba01180202745bd7cb3fc12e226`, and that file is absent from
`git -C D:\hoarfrost status --porcelain`. This repo's file is
`ed7215e157f50ecb115be93acc86793cd71fbf16`.

## The diff (verbatim)

```diff
diff --git "a/src\\hoarfrost-demo\\src\\font_fallback.rs" "b/D:\\hoarfrost\\src\\hoarfrost-demo\\src\\font_fallback.rs"
index ed7215e..c53c046 100644
--- "a/src\\hoarfrost-demo\\src\\font_fallback.rs"
+++ "b/D:\\hoarfrost\\src\\hoarfrost-demo\\src\\font_fallback.rs"
@@ -16,7 +16,7 @@
 //!   faithful port of `main`'s discovery order, including the
 //!   environment-variable override. The override is renamed
 //!   [`FALLBACK_FONT_ENV`] (`HOARFROST_DEMO_FALLBACK_FONT`) rather than
-//!   reusing `FASTMD_FALLBACK_FONT`: the two binaries run side by side
+//!   reusing the host fallback font variable: the two binaries run side by side
 //!   during the port, and sharing one variable would make a value set for
 //!   the egui app silently redirect the demo.
 //! * **Phosphor is not ported.** It is 98.1% Private-Use-Area and
@@ -55,14 +55,14 @@
 //! Unit tests live in the sibling `font_fallback_tests.rs` sidecar.
 
 use std::borrow::Cow;
-use std::path::{Path, PathBuf};
+use std::path::PathBuf;
 use std::sync::OnceLock;
 
 use iced::advanced::graphics::text::font_system;
 
 /// Environment variable that overrides discovery with one explicit font file.
 ///
-/// Deliberately not `main`'s `FASTMD_FALLBACK_FONT` — see the module doc.
+/// Deliberately not `main`'s fallback font variable — see the module doc.
 pub const FALLBACK_FONT_ENV: &str = "HOARFROST_DEMO_FALLBACK_FONT";
 
 /// Environment variable that suppresses registration entirely.
@@ -172,7 +172,7 @@ fn platform_candidates() -> Vec<PathBuf> {
 #[cfg(windows)]
 fn windows_candidates() -> Vec<PathBuf> {
     let system_root = std::env::var("SYSTEMROOT").unwrap_or_else(|_| String::from("C:\\Windows"));
-    let fonts_dir = Path::new(&system_root).join("Fonts");
+    let fonts_dir = PathBuf::from(system_root).join("Fonts");
     WINDOWS_FONT_FILES
         .iter()
         .map(|file| fonts_dir.join(file))
```

(The `+++` header line above is reproduced as git printed it; git C-style-quotes
the Windows paths, which is why the backslashes appear doubled.)

## Per-hunk summary

| Hunk | Local line | Change |
|---|---|---|
| `@@ -16,7 +16,7 @@` | 19 | Module doc: the branding literal `FASTMD_FALLBACK_FONT` is replaced by the neutral phrase "the host fallback font variable" (part of upstream's branding scrub). |
| `@@ -55,14 +55,14 @@` (import) | 58 | The `use std::path::{Path, PathBuf};` import becomes `use std::path::PathBuf;` — `Path` is no longer referenced once the call site is rewritten. |
| `@@ -55,14 +55,14 @@` (doc) | 65 | `FALLBACK_FONT_ENV`'s doc comment drops the same `FASTMD_FALLBACK_FONT` literal for "the host fallback font variable". |
| `@@ -172,7 +172,7 @@` | 175 | `Path::new(&system_root).join("Fonts")` → `PathBuf::from(system_root).join("Fonts")`, which moves the `String` instead of borrowing it and is what makes the `Path` import unused. |

Three distinct kinds of change across four hunks: the `PathBuf` rewrite, the
now-unused `Path` import that the rewrite forces, and the branding wording, which
appears twice.

## Direction finding — nothing to port

The plan (Phase 0.1 item 1) lists this file as local content that must be
**ported to** upstream. Measured against `dd7af16`, **upstream already contains
all three changes**: it has `PathBuf::from(system_root)`, no unused `Path`
import, and the branding scrub. The local copy is the *older* text.

So there is no fix to carry upstream for this file — the deletion in Phase 2 loses
nothing here. This document is therefore a regression record: if upstream ever
reverts to `Path::new(&system_root)`, the `+` side above is the state to restore,
and note that the two changes are coupled — dropping `PathBuf::from(system_root)`
without putting `Path` back in the import leaves an unused-import warning, which
the `clippy -- -D warnings` gate rejects.
