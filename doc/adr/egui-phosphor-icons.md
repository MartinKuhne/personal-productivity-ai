# Use egui-phosphor for UI Icons

Status: accepted
Date: 2026-08-26

## Context

The directory tree UI previously used plain text Unicode arrows (`▼` and `▶`, and later `▾` and `▸`) to indicate whether a directory was expanded or collapsed. 

Because `egui` defaults to `Ubuntu-Light` or `Hack` which have limited glyph coverage, rendering these Unicode arrows relies heavily on the operating system's font fallback mechanisms. On Windows, this fallback is notoriously fragile and often fails, resulting in the arrows rendering as unprintable "tofu" boxes (empty squares) or mojibake.

To fix this, we want to use an icon font like Phosphor. The `egui-phosphor` crate provides these icons. However, `egui-phosphor 0.13.0` currently compiles against `egui 0.35.0`, while our application (`fastmd`) uses `eframe/egui 0.36.0`. This causes a compile-time trait/type mismatch if we try to pass our `egui 0.36.0::FontDefinitions` into `egui_phosphor::add_to_fonts`.

## Decision

Initially, the `egui-phosphor` crate was adopted to provide the TTF font and icon codepoints. However, `egui-phosphor 0.13.0` was pinned to `egui 0.35.0`, which pulled in an entire duplicate egui/epaint/font-shaping tree (~40 crates).

To eliminate this massive dependency duplication while retaining the exact same visual glyphs:
1. `Phosphor-Regular.ttf` is embedded directly into the binary via `include_bytes!("../../../assets/fonts/Phosphor-Regular.ttf")` in `src/app/ui/fonts.rs`.
2. The required Phosphor icon codepoints are defined as string constants in `src/app/ui/strings.rs` (`ICON_CARET_DOWN`, `ICON_CARET_RIGHT`, `ICON_MAGNIFYING_GLASS`, `ICON_X`, `ICON_COPY`, `ICON_LIST`, `ICON_STOP`, `ICON_ROBOT`, `ICON_LIGHTNING`).
3. The `egui-phosphor` dependency has been completely removed from `Cargo.toml`.

## Consequences

- Completely eliminates the duplicate `egui 0.35` / `epaint 0.35` tree, resolving 10 duplicate crate versions across font parsers and egui sub-crates.
- Zero external runtime or build-time dependency on third-party icon crates.
- Retains 100% glyph visual compatibility and zero platform-fallback fragility.
