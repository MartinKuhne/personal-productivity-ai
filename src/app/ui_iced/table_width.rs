//! Iced adapter for the Fair Table Width Algorithm ([TASK-015],
//! `doc/planning/iced.md`).
//!
//! The solver itself — `crate::markdown::table_width::ftwa` plus the
//! `crate::markdown::table_layout::TableLayoutBuilder` that drives it — is
//! pure `f32` math and is **reused unchanged** ([RUST-051]). What the solver
//! cannot do is measure text: it consumes `max_content` / `min_content`
//! widths and a `TextMeasurer` that the egui build supplied from
//! `egui::Context` text shaping (`src/app/ui/table_width/mod.rs`). This
//! module is the iced-side replacement for exactly that bridging layer.
//!
//! # The contract, preserved
//!
//! Same three stages the egui adapter ran, in the same order:
//!
//! 1. **available width → solver width.** The iced `Table` paints
//!    [`CELL_PADDING_X`] outside every cell on both sides, so the width the
//!    solver may spend is [`solver_available_width`] = available minus the
//!    two outer padding bands. The egui adapter had no outer frame padding,
//!    so this is the one deliberate numeric difference ([RUST-041] note in
//!    the TASK-015 verification log).
//! 2. **solver inputs.** [`IcedTextMeasurer`] implements the pure
//!    [`TextMeasurer`] trait: per-cell `max_content` (single-line width of
//!    the whole cell), `min_content` (longest unbreakable token) and the
//!    ordered token widths the breakpoint curve is built from. Horizontal
//!    padding is folded in by the *builder*, not by the measurer — exactly
//!    as `EguiTextMeasurer` left it (`TBL-033` accounting).
//! 3. **solver output → iced widths.** [`column_widths`] reads the solved
//!    per-column widths back out of the layout, and [`column_length`] turns
//!    one into the `Length::Fixed` a `table::Column` is pinned to.
//!    `TableLayout::needs_horizontal_scroll` selects the §3.6
//!    horizontal-scroll fallback, mirroring
//!    `src/app/ui/render/table/configured.rs:122-144`.
//!
//! # Column geometry
//!
//! iced's `Table` derives its inter-cell gap as
//! `spacing = 2 * padding + separator`
//! (`iced_widget-0.14.2/src/table.rs:258-259`) and starts each row at
//! `x = padding_x` (`table.rs:263`). Setting `padding_x = CELL_PADDING_X`
//! with `separator_x = 0` therefore yields a [`COLUMN_SPACING`] of 10 px —
//! the same gutter the egui table used (`item_spacing.x = 10.0`,
//! `configured.rs:86`) — without painting a separator line through it.
//! Vertically the same rule gives [`ROW_SPACING`] = 2 px
//! (`egui: item_spacing.y = 2.0`, `configured.rs:75`).
//!
//! # Measurement without a renderer
//!
//! iced only hands a text renderer to `Widget::layout`
//! (`iced_graphics-0.14.0/src/text/renderer.rs`); `view()` has none, so a
//! pixel-exact `measure_width` is not reachable from the render path. The
//! measurer therefore takes its width function as an injected `&dyn Fn`
//! ([`IcedTextMeasurer::new`]) and ships a deterministic
//! average-advance approximation ([`heuristic_width`]) as the default.
//! [TASK-022]/[TASK-023] can pass a renderer-backed closure with no
//! change to this module. The approximation is uniform per font, so every
//! FTWA regime transition (surplus / deficit / fallback) still fires on
//! the same *relative* column shapes the renderer-backed path produces.
//!
//! # Relocated helper ([TASK-029])
//!
//! [`TablePadding`] used to be `use`d from `crate::ui::table_width`, a file
//! [TASK-029b] deletes. It is copied into this module verbatim and is
//! **egui-free** (four `f32` fields, four pure methods). The egui module's
//! `TableRenderConfig` is likewise egui-free but has no `ui_iced` consumer,
//! so it is not copied.
//!
//! Unit tests live in the sibling `table_width_tests.rs` sidecar.

use crate::markdown::model::InlineElem;
use crate::markdown::table_layout::{TableLayout, TableLayoutBuilder, TextMeasurer};
use crate::markdown::table_width::DeficitStrategy;
use iced::Length;
use iced::font::{Family, Font};

/// Per-cell padding in logical pixels applied around cell content.
///
/// Padding is resolved per cell by taking the most-specific non-`None` layer
/// (per-cell override > per-column override > global default), then sanitised
/// (negative components clamped to `0.0`, satisfying `TBL-050`). Resolved
/// horizontal padding is factored into column width accounting (`TBL-033`):
/// every returned `max_content`/`min_content` is increased by
/// `padding.horizontal()`. Vertical padding is applied as inner frame margin
/// in the cell renderer and increases the rendered row height.
///
/// See [contracts/table-renderer.md](../../../specs/002-table-layout-renderer/contracts/table-renderer.md) Part C.
///
/// # Relocated ([TASK-029])
///
/// Copied verbatim from `src/app/ui/table_width/mod.rs:29-86`, which used to
/// be `use`d across the boundary and is deleted by [TASK-029b]. The type is
/// **egui-free by construction** — four `f32` fields and four pure methods,
/// no `egui`/`eframe` type anywhere — so the copy is exact.
///
/// The egui module's `TableRenderConfig` is **also** egui-free but is not
/// copied: no `ui_iced` code reads it (the iced render path carries
/// [`RenderContext::table_padding`](crate::ui_iced::render::RenderContext)
/// instead). The egui module keeps both until Phase 5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TablePadding {
    /// Top padding in logical pixels.
    pub top: f32,
    /// Bottom padding in logical pixels.
    pub bottom: f32,
    /// Left padding in logical pixels.
    pub left: f32,
    /// Right padding in logical pixels.
    pub right: f32,
}

impl TablePadding {
    /// Zero padding on every side — the no-arg default.
    pub const ZERO: Self = Self {
        top: 0.0,
        bottom: 0.0,
        left: 0.0,
        right: 0.0,
    };

    /// Clamp each negative component to `0.0` (`TBL-050` malformed-input branch).
    #[must_use]
    pub fn sanitised(self) -> Self {
        Self {
            top: self.top.max(0.0),
            bottom: self.bottom.max(0.0),
            left: self.left.max(0.0),
            right: self.right.max(0.0),
        }
    }

    /// Horizontal padding (`left + right`) added to width accounting.
    #[must_use]
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Vertical padding (`top + bottom`) added to row height accounting.
    #[must_use]
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for TablePadding {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Horizontal padding the iced `Table` paints on each side of every cell.
///
/// Chosen so `2 * padding_x + separator_x` equals [`COLUMN_SPACING`]
/// while `separator_x` stays `0` (a 10 px separator would paint a bar
/// through the gutter). The value is *inside* the FTWA-assigned column
/// width, matching the `TBL-033` rule that assigned widths cover
/// content + horizontal padding.
pub const CELL_PADDING_X: f32 = 5.0;

/// Vertical padding the iced `Table` paints above and below every cell.
///
/// Paired with `separator_y = 0` this yields [`ROW_SPACING`].
pub const CELL_PADDING_Y: f32 = 1.0;

/// Horizontal gap between adjacent columns.
///
/// Mirrors the egui table's `item_spacing.x = 10.0`
/// (`src/app/ui/render/table/configured.rs:86`) and the pure builder's
/// own default (`src/app/markdown/table_layout.rs:53`).
pub const COLUMN_SPACING: f32 = 10.0;

/// Vertical gap between adjacent rows.
///
/// Mirrors the egui table's `item_spacing.y = 2.0`
/// (`src/app/ui/render/table/configured.rs:75`).
pub const ROW_SPACING: f32 = 2.0;

/// Average glyph advance of the proportional UI font, in em units.
///
/// The heuristic's only width model: `chars × advance × size`. 0.5 em is
/// the usual average for a humanist sans at body size.
pub const PROPORTIONAL_AVG_ADVANCE: f32 = 0.5;

/// Average glyph advance of the monospace font, in em units.
///
/// Monospaced faces sit near 0.6 em.
pub const MONOSPACE_AVG_ADVANCE: f32 = 0.6;

/// Renderer-free text width used until a real iced renderer is reachable
/// from `view()` ([TASK-022]/[TASK-023]).
///
/// Counts Unicode scalar values, so multi-byte UTF-8 text is measured by
/// character count rather than byte count. Monotone in the character count,
/// which is what keeps the solver's `max_content >= min_content` invariant
/// intact ([`ftwa`]'s panic condition).
///
/// [`ftwa`]: crate::markdown::table_width::ftwa
#[must_use]
pub fn heuristic_width(text: &str, font: Font, size: f32) -> f32 {
    let advance = if font.family == Family::Monospace {
        MONOSPACE_AVG_ADVANCE
    } else {
        PROPORTIONAL_AVG_ADVANCE
    };
    text.chars().count() as f32 * advance * size
}

/// The sizes the adapter measures and lays out at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    /// Point size of proportional (body) cell text.
    pub body_size: f32,
    /// Point size of inline-code (monospace) cell text.
    pub mono_size: f32,
    /// Line height of one body-text row.
    pub line_height: f32,
}

impl TextMetrics {
    /// The FastMD body metrics: 14 pt body, 13 pt mono, 18 px row height.
    ///
    /// Mirrors the egui defaults the table renderer fell back to
    /// (`TextStyle::Body` → `FontId::proportional(14.0)`,
    /// `src/app/ui/render/table/cell.rs:194`) and the 18 px line the egui
    /// markdown renderer used for its height estimates
    /// (`src/app/ui/render/mod.rs:190`).
    pub const FASTMD: TextMetrics = TextMetrics {
        body_size: 14.0,
        mono_size: 13.0,
        line_height: 18.0,
    };
}

/// A [`TextMeasurer`] for the pure FTWA builder, driven by iced fonts.
///
/// Implements the same trait the egui `EguiTextMeasurer` did
/// (`src/app/ui/table_width/mod.rs:280-292`), so `TableLayoutBuilder`
/// cannot tell the two apart.
pub struct IcedTextMeasurer<'a> {
    width_of: &'a dyn Fn(&str, Font, f32) -> f32,
    metrics: TextMetrics,
}

impl<'a> IcedTextMeasurer<'a> {
    /// Builds a measurer over an injected width function.
    ///
    /// `width_of(text, font, size)` must return the single-line pixel width
    /// of `text`. Pass a renderer-backed closure here once one is
    /// available; [`heuristic_width`] is the default.
    #[must_use]
    pub fn new(width_of: &'a dyn Fn(&str, Font, f32) -> f32, metrics: TextMetrics) -> Self {
        Self { width_of, metrics }
    }

    /// Builds the default measurer: the deterministic
    /// [`heuristic_width`] approximation.
    #[must_use]
    pub fn heuristic(metrics: TextMetrics) -> Self {
        Self {
            width_of: &heuristic_width,
            metrics,
        }
    }

    /// Point size a given font renders at in this table.
    #[must_use]
    pub fn size_of(&self, font: Font) -> f32 {
        if font.family == Family::Monospace {
            self.metrics.mono_size
        } else {
            self.metrics.body_size
        }
    }

    /// The body font handle.
    #[must_use]
    pub fn body_font() -> Font {
        Font::DEFAULT
    }

    /// The inline-code font handle ([`Font::MONOSPACE`],
    /// `iced_core-0.14.0/src/font.rs:27-31`).
    #[must_use]
    pub fn mono_font() -> Font {
        Font::MONOSPACE
    }

    /// Measure one token and fold it into the column's `min_content` and
    /// the ordered token-width list.
    fn record_token(&self, token: &str, font: Font, min_w: &mut f32, widths: &mut Vec<f32>) {
        if token.is_empty() {
            return;
        }
        let width = (self.width_of)(token, font, self.size_of(font));
        if width > *min_w {
            *min_w = width;
        }
        widths.push(width);
    }

    /// The text a cell element actually displays, and the font it displays
    /// in.
    ///
    /// Same selection the egui measurer used (`src/app/ui/table_width/mod.rs:440-449`):
    /// monospace only for `code` runs; links, HTML and the image
    /// placeholder all measure in the body font.
    fn displayed(&self, elem: &InlineElem) -> (String, Font) {
        match elem {
            InlineElem::Text(text, style) => {
                let font = if style.code {
                    Self::mono_font()
                } else {
                    Self::body_font()
                };
                (text.clone(), font)
            }
            InlineElem::Link(_, display) => (display.clone(), Self::body_font()),
            InlineElem::Image(url) => (crate::markdown::image_placeholder(url), Self::body_font()),
            InlineElem::Html(html) => (html.clone(), Self::body_font()),
            InlineElem::SoftBreak => (" ".to_string(), Self::body_font()),
        }
    }
}

impl TextMeasurer for IcedTextMeasurer<'_> {
    /// Measures one cell's `(max_content, min_content)` and token widths.
    ///
    /// `max_content` is the sum of every element's single-line width
    /// (elements are laid out flush, zero inter-element spacing);
    /// `min_content` is the widest whitespace-separated token across the
    /// cell, measured with the font that token renders in. Token
    /// accumulation across element boundaries — a word split between two
    /// `Text` runs keeps its identity — is ported verbatim from the egui
    /// measurer (`src/app/ui/table_width/mod.rs:415-510`) so the solver
    /// sees the same break opportunities in both builds.
    fn measure_cell(
        &self,
        cell: &[InlineElem],
    ) -> (f32, f32, crate::markdown::table_width::CellTokens) {
        let mut max_w = 0.0_f32;
        let mut min_w = 0.0_f32;
        let mut token_widths: Vec<f32> = Vec::new();
        let mut current_token = String::new();
        let mut current_font = Self::body_font();

        for elem in cell {
            let (displayed, font) = self.displayed(elem);
            if displayed.is_empty() {
                continue;
            }

            max_w += (self.width_of)(&displayed, font, self.size_of(font));

            let parts: Vec<&str> = displayed.split_whitespace().collect();
            if parts.is_empty() {
                // Whitespace-only element terminates whatever token is open.
                if !current_token.is_empty() {
                    let token = std::mem::take(&mut current_token);
                    self.record_token(&token, current_font, &mut min_w, &mut token_widths);
                }
                continue;
            }

            let starts_ws = displayed.chars().next().is_some_and(char::is_whitespace);
            let ends_ws = displayed.chars().last().is_some_and(char::is_whitespace);
            let last = parts[parts.len() - 1];

            if starts_ws {
                if !current_token.is_empty() {
                    let token = std::mem::take(&mut current_token);
                    self.record_token(&token, current_font, &mut min_w, &mut token_widths);
                }
                for part in &parts[..parts.len() - 1] {
                    self.record_token(part, font, &mut min_w, &mut token_widths);
                }
                if ends_ws {
                    self.record_token(last, font, &mut min_w, &mut token_widths);
                } else {
                    current_token.push_str(last);
                    current_font = font;
                }
            } else {
                current_token.push_str(parts[0]);
                if parts.len() > 1 {
                    let token = std::mem::take(&mut current_token);
                    self.record_token(&token, current_font, &mut min_w, &mut token_widths);
                    for part in &parts[1..parts.len() - 1] {
                        self.record_token(part, font, &mut min_w, &mut token_widths);
                    }
                    if ends_ws {
                        self.record_token(last, font, &mut min_w, &mut token_widths);
                    } else {
                        current_token.push_str(last);
                        current_font = font;
                    }
                } else if ends_ws {
                    // Parity with the egui walker: a single-part run that ends
                    // with whitespace closes the pending token immediately,
                    // measured with the *previous* run's font.
                    let token = std::mem::take(&mut current_token);
                    self.record_token(&token, current_font, &mut min_w, &mut token_widths);
                } else {
                    current_font = font;
                }
            }
        }

        if !current_token.is_empty() {
            let token = std::mem::take(&mut current_token);
            self.record_token(&token, current_font, &mut min_w, &mut token_widths);
        }

        (
            max_w,
            min_w,
            crate::markdown::table_width::CellTokens { token_widths },
        )
    }

    fn space_width(&self) -> f32 {
        (self.width_of)(" ", Self::body_font(), self.metrics.body_size)
    }

    fn line_height(&self) -> f32 {
        self.metrics.line_height
    }
}

/// Width the solver may spend, given the viewport width the table gets.
///
/// Subtracts the two outer [`CELL_PADDING_X`] bands the iced `Table`
/// paints around the whole grid (`table.rs:263`, `table.rs:451`). Clamped
/// at zero so a collapsed viewport still solves instead of going negative.
#[must_use]
pub fn solver_available_width(available_width: f32) -> f32 {
    (available_width - 2.0 * CELL_PADDING_X).max(0.0)
}

/// Runs the pure FTWA solver for one markdown table.
///
/// `cells` is the parser's `RenderEvent::Table` payload
/// (`Vec<row> → Vec<cell> → Vec<InlineElem>`, row 0 being the header);
/// `available_width` is the pixel width the table has been given. Ragged
/// rows are tolerated exactly as the pure builder tolerates them, and an
/// empty table solves to an empty layout.
///
/// The returned [`TableLayout`] carries the solved per-cell geometry; use
/// [`column_widths`] for the column view.
#[must_use]
pub fn solve_table_layout(
    cells: &[Vec<Vec<InlineElem>>],
    available_width: f32,
    strategy: DeficitStrategy,
    padding: TablePadding,
    measurer: &IcedTextMeasurer<'_>,
) -> TableLayout {
    let pad = padding.sanitised();
    TableLayoutBuilder::new(measurer, solver_available_width(available_width), strategy)
        .with_padding(pad.horizontal(), pad.vertical())
        .with_spacing(COLUMN_SPACING, ROW_SPACING)
        .build(cells)
}

/// Per-column widths read back out of a solved [`TableLayout`].
///
/// The layout stores widths per *cell*, so a column's width is the maximum
/// its cells report — identical for every cell in a well-formed table, and
/// the max keeps a ragged table from under-sizing a column that only
/// appears in later rows. Empty layout → empty vector.
#[must_use]
pub fn column_widths(layout: &TableLayout) -> Vec<f32> {
    let columns = layout.rows.iter().map(|row| row.len()).max().unwrap_or(0);
    let mut widths = vec![0.0_f32; columns];
    for row in &layout.rows {
        for (index, cell) in row.iter().enumerate() {
            if cell.width > widths[index] {
                widths[index] = cell.width;
            }
        }
    }
    widths
}

/// Maps one solved column width onto the `Length` a iced
/// `table::Column` is pinned to.
///
/// `Length::Fixed` (`iced_core-0.14.0/src/length.rs`) is what keeps the
/// column at exactly the width FTWA assigned: a `Fill` column would let
/// iced re-stretch the table and undo the solver's surplus-regime decision
/// not to grow narrow columns. Clamped at zero so a degenerate solve
/// cannot produce a negative length.
#[must_use]
pub fn column_length(width: f32) -> Length {
    Length::Fixed(width.max(0.0))
}

/// Whether the solved table needs the §3.6 horizontal-scroll fallback.
///
/// Thin accessor so the render layer reads the intent rather than the
/// field, and the fallback rule has one named home in this adapter.
#[must_use]
pub fn needs_horizontal_scroll(layout: &TableLayout) -> bool {
    layout.needs_horizontal_scroll
}

#[cfg(test)]
#[path = "table_width_tests.rs"]
mod tests;
