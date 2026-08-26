# Use egui-phosphor for UI Icons

Status: accepted
Date: 2026-08-26

## Context

The directory tree UI previously used plain text Unicode arrows (`▼` and `▶`, and later `▾` and `▸`) to indicate whether a directory was expanded or collapsed. 

Because `egui` defaults to `Ubuntu-Light` or `Hack` which have limited glyph coverage, rendering these Unicode arrows relies heavily on the operating system's font fallback mechanisms. On Windows, this fallback is notoriously fragile and often fails, resulting in the arrows rendering as unprintable "tofu" boxes (empty squares) or mojibake.

To fix this, we want to use an icon font like Phosphor. The `egui-phosphor` crate provides these icons. However, `egui-phosphor 0.13.0` currently compiles against `egui 0.35.0`, while our application (`fastmd`) uses `eframe/egui 0.36.0`. This causes a compile-time trait/type mismatch if we try to pass our `egui 0.36.0::FontDefinitions` into `egui_phosphor::add_to_fonts`.

## Decision

We have adopted the `egui-phosphor` crate as a dependency but bypassed its helper functions.

Instead of calling `egui_phosphor::add_to_fonts`, we manually read the `&'static [u8]` TTF byte slice via `egui_phosphor::Variant::Regular.font_bytes()`. We then wrap those bytes in an `std::sync::Arc<egui::FontData>` and inject them into our `egui 0.36.0` `FontDefinitions` directly during app initialization.

For the directory tree arrows, we are now using `egui_phosphor::regular::CARET_DOWN` and `egui_phosphor::regular::CARET_RIGHT`.

### Alternatives considered

| Option | Outcome |
|--------|---------|
| Rely on default OS font fallback | Failed on many Windows machines, resulting in bug reports of unprintable checkboxes. |
| Use ASCII (`[+]`, `[-]`) | 100% reliable but visually dated and inconsistent with modern UI aesthetics. |
| Use `egui::CollapsingHeader` custom polygon | Robust, but requires replacing `ui.selectable_label` with a custom `ui.horizontal` layout everywhere a tree node is drawn. Harder to maintain. |
| **Inject Phosphor TTF Bytes (chosen)** | Guaranteed to render perfectly on all OSes. Allows us to use `egui-phosphor` constants immediately without waiting for a crate update to `0.36.0`. |

## Consequences

- The application binary size increases slightly due to the embedded Phosphor TTF font.
- We have access to the entire Phosphor icon library for future UI components.
- The directory tree UI is completely immune to missing-glyph bugs on Windows.
- We have a small piece of manual font-loading boilerplate in `init.rs` that can be removed and replaced with `egui_phosphor::add_to_fonts` once `egui-phosphor` updates to `egui 0.36.0`.
