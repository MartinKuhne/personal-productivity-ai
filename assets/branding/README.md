# FastMD Branding — Logo Options

Explorer generated 2026-09-02. Selected option: **4 — Document + Bolt**.

## Palette (shared across options)
- Badge indigo ` #6366F1` — matches `Visuals::selection.bg_fill` (`src/app/ui/app/init.rs:52`)
- Bolt cyan ` #64C8FF` — matches `top.rs` heading `100,200,255`
- Fold ` #C7D2FE`, stroke ` #A5B4FC`, dark outline ` #4338CA`
- Dark bg ` #09090B` — matches `Visuals::panel_fill` (`init.rs:50`)

## Options

| # | Name | Concept | Strength | Trade-off |
|---|------|---------|----------|-----------|
| 1 | FMD Monogram | Tight `FMD` wordmark in indigo badge | Legible at 16×16, ownable | Loses “fast” metaphor |
| 2 | Evolved Lightning | Custom bolt vs Phosphor `\u{E2DE}` | Keeps equity, minimal change | Mush at 16×16 |
| 3 | Hybrid | `F` + bolt + `MD` | Best of both | More complex kerning |
| **4** | **Document + Bolt** | White doc + folded corner + cyan bolt on indigo badge | Descriptive, survives 16px, distinctive | Busy vs pure wordmark — **selected** |
| 5 | Ligature | `F+M` fused with bolt | Premium single-shape | Abstract, needs learning |

## Files
- `options/option*.svg` — vector source for each exploration (32×32 viewBox)
- `../icon.svg` — primary app icon (copy of option 4)
- `../mark.svg` — header-only mark without badge (transparent bg, for dark toolbar)
- `../icon-<size>.png` — rasterized via Pillow (`image 10.4`) for `ViewportBuilder::with_icon`
- `../icon.ico` — multi-res ICO (16/32/48/64/256) for Windows `build.rs` (`winresource`)

## Integration
- `build.rs` embeds `assets/icon.ico` on `cfg(windows)` via `winresource`
- `src/app/ui/logo.rs::load_app_icon()` decodes `icon-32.png` via `image` crate for `ViewportBuilder::with_icon`
- `src/app/ui/logo.rs::paint_logo()` vector-paints the badge for the top toolbar (20×20, no texture dependency)
- `src/app/ui/strings.rs::APP_TITLE` is now plain `FastMD Viewer` — glyph removed
- `src/app/ui/panels/top.rs` allocates 20×20 rect and paints logo before heading
