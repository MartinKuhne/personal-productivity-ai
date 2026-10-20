//! Iced adapter for the Fair Table Width Algorithm.
//!
//! The solver itself — `crate::markdown::table_width::ftwa` plus the
//! `crate::markdown::table_layout::TableLayoutBuilder` that drives it — is
//! pure `f32` math and is **reused unchanged** ([RUST-051]). What the solver
//! cannot do is measure text: it consumes `max_content` / `min_content`
//! widths and a `TextMeasurer`. This module is the iced-side bridging layer
//! that supplies them.
//!
//! # The pipeline
//!
//! Three stages, in this order:
//!
//! 1. **available width → solver width.** The iced `Table` paints
//!    [`CELL_PADDING_X`] outside every cell on both sides, so the width the
//!    solver may spend is [`solver_available_width`] = available minus the
//!    two outer padding bands.
//! 2. **solver inputs.** [`IcedTextMeasurer`] implements the pure
//!    [`TextMeasurer`] trait: per-cell `max_content` (single-line width of
//!    the whole cell), `min_content` (longest unbreakable token) and the
//!    ordered token widths the breakpoint curve is built from. Horizontal
//!    padding is folded in by the *builder*, not by the measurer — the
//!    `TBL-033` accounting.
//! 3. **solver output → iced widths.** [`column_widths`] reads the solved
//!    per-column widths back out of the layout, and [`column_length`] turns
//!    one into the `Length::Fixed` a `table::Column` is pinned to.
//!    `TableLayout::needs_horizontal_scroll` selects the
//!    horizontal-scroll fallback.
//!
//! # Column geometry
//!
//! iced's `Table` derives its inter-cell gap as
//! `spacing = 2 * padding + separator`
//! (`iced_widget-0.14.2/src/table.rs:258-259`) and starts each row at
//! `x = padding_x` (`table.rs:263`). Setting `padding_x = CELL_PADDING_X`
//! with `separator_x = 0` therefore yields a [`COLUMN_SPACING`] of 10 px
//! without painting a separator line through the gutter. Vertically the
//! same rule gives [`ROW_SPACING`] = 2 px.
//!
//! # Measurement without a renderer
//!
//! iced only hands a text renderer to `Widget::layout`
//! (`iced_graphics-0.14.0/src/text/renderer.rs`), `view()` has none, and
//! the reducer never sees one. A renderer is not the only route to real
//! metrics: [`fira_sans_width`] shapes the string with `cosmic-text` over
//! `iced_graphics::text::FIRA_SANS_REGULAR` — the very bytes iced loads
//! as its Fira face — and returns true glyph advances with no renderer, no
//! window, and no handle on iced's own font engine.
//!
//! The seam is an injected `&dyn Fn` ([`IcedTextMeasurer::new`]), and
//! [`heuristic_width`] is the default ([`IcedTextMeasurer::heuristic`]).
//! Two properties of that split matter:
//!
//! * Real metrics are emphatically **not** uniform per font: `"iiii"` and
//!   `"MMMM"` measure differently at the same size, which is precisely the
//!   difference a flat 0.5 em model cannot see.
//! * The heuristic is **not** the general-purpose width function. It
//!   survives as the **fallback** for any face this build holds no bytes
//!   for — `Font::MONOSPACE` above all, since measuring Fira Sans in a
//!   monospace slot would be measuring the wrong face.
//!
//! One caveat travels with the real metrics and is documented in full on
//! [`fira_sans_width`]: `cosmic-text` loads the host's installed fonts
//! into every engine, and iced pins Fira Sans as the default face only on
//! `wasm32`, so the app must pin `default_font` itself for the measured
//! face to be the painted face.
//!
//! # `TablePadding`
//!
//! [`TablePadding`] is **framework-free** — four `f32` fields and four pure
//! methods — so it lives here, alongside the adapter that consumes it.
//!
//! Unit tests live in the sibling `table_width_tests.rs` sidecar.

use crate::markdown::model::InlineElem;
use crate::markdown::table_layout::{TableLayout, TableLayoutBuilder, TextMeasurer};
use crate::markdown::table_width::DeficitStrategy;
use iced::Length;
use iced::font::{Family, Font};
use std::sync::{Arc, Mutex, OnceLock};

/// Per-cell padding in logical pixels applied around cell content.
///
/// Padding is resolved per cell by taking the most-specific non-`None` layer
/// (per-cell override > per-column override > global default), then sanitised
/// (negative components clamped to `0.0`). Resolved horizontal padding is
/// factored into column width accounting (`TBL-033`): every returned
/// `max_content`/`min_content` is increased by `padding.horizontal()`.
/// Vertical padding is applied as inner frame margin in the cell renderer
/// and increases the rendered row height.
///
/// The type is **framework-free by construction** — four `f32` fields and
/// four pure methods, no framework type anywhere. The iced render path
/// carries it as
/// [`RenderContext::table_padding`](crate::ui_iced::render::RenderContext).
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

    /// Clamp each negative component to `0.0` (malformed-input branch).
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

/// Horizontal gap between adjacent columns, matching the pure builder's own
/// default (`src/app/markdown/table_layout.rs:53`).
pub const COLUMN_SPACING: f32 = 10.0;

/// Vertical gap between adjacent rows.
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

/// Renderer-free text width approximation.
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

/// Family name under which iced registers the Fira Sans bytes.
///
/// `iced_graphics` loads `FIRA_SANS_REGULAR` into its font database under
/// this family, and `iced_test` normalises `Font::DEFAULT` to
/// `Font::with_name("Fira Sans")`
/// (`iced_test-0.14.0/src/simulator.rs:66-69`), so asking for the face by
/// name is asking for what the renderer actually paints.
const FIRA_SANS_FAMILY_NAME: &str = "Fira Sans";

/// Process-wide `cosmic-text` engine seeded from iced's own Fira Sans bytes.
///
/// Built lazily with the same
/// `FontSystem::new_with_fonts([fontdb::Source::Binary(..)])` construction
/// iced uses (`iced_graphics-0.14.0/src/text.rs:118-131`), fed from
/// `iced_graphics::text::FIRA_SANS_REGULAR` rather than a second copy of
/// the font, so the measured face and the painted face cannot drift.
///
/// iced's own engine is reachable only through the global
/// `iced_graphics::text::font_system()` `RwLock`, whose write lock the
/// renderer holds while painting; taking it per row would put the reducer
/// behind the renderer. This module therefore owns a second engine built
/// from the same bytes.
///
/// `new_with_fonts` calls `db.load_system_fonts()` **before** adding the
/// explicit source (`cosmic-text-0.15.0/src/font/system.rs:396-404`), so
/// this engine — exactly like iced's own — also carries every font
/// installed on the host (517 faces on the machine this was measured on).
/// Fira Sans sits on top of that collection, which is why every
/// measurement asks for the face by name instead of by generic family.
///
/// The `Mutex` is required, not decorative: `cosmic_text::FontSystem`
/// mutates its shaping and font caches in place, so it is `Send` but not
/// `Sync` — the `static` below would not compile without it. A poisoned
/// lock is treated as "no measurement available" and degrades to
/// [`heuristic_width`] rather than reusing a possibly half-finished cache.
fn fira_sans_font_system() -> &'static Mutex<cosmic_text::FontSystem> {
    static FONT_SYSTEM: OnceLock<Mutex<cosmic_text::FontSystem>> = OnceLock::new();
    FONT_SYSTEM.get_or_init(|| {
        Mutex::new(cosmic_text::FontSystem::new_with_fonts([
            cosmic_text::fontdb::Source::Binary(Arc::new(iced_graphics::text::FIRA_SANS_REGULAR)),
        ]))
    })
}

/// Whether the Fira Sans face this module measures with is loaded and
/// resolvable.
///
/// Returns `false` if the shared engine could not be built, its lock was
/// poisoned, or the family is missing from the database — every one of
/// which means [`fira_sans_width`] is answering with
/// [`heuristic_width`] rather than real metrics.
#[must_use]
pub fn fira_sans_available() -> bool {
    let Ok(system) = fira_sans_font_system().lock() else {
        return false;
    };
    system
        .db()
        .query(&cosmic_text::fontdb::Query {
            families: &[cosmic_text::fontdb::Family::Name(FIRA_SANS_FAMILY_NAME)],
            ..Default::default()
        })
        .is_some()
}

/// Accept a raw measurement only when it is finite and strictly positive;
/// otherwise return `fallback`.
///
/// The single home of the "do not trust this number" rule, so the rule can
/// be tested directly instead of depending on what the shaper happens to
/// produce.
fn usable_width(measured: f32, fallback: f32) -> f32 {
    if measured.is_finite() && measured > 0.0 {
        measured
    } else {
        fallback
    }
}

/// Shape `text` in Fira Sans and return the summed advance of every laid
/// out line.
///
/// `size` must already be finite and positive — `cosmic-text` asserts a
/// non-zero line height, so [`fira_sans_width`] validates it first. The
/// return value is raw: it may be `0.0` (nothing shaped, or a poisoned
/// lock) or non-finite, and callers must pass it through [`usable_width`].
fn measured_fira_sans_width(text: &str, size: f32) -> f32 {
    let Ok(mut font_system) = fira_sans_font_system().lock() else {
        return 0.0;
    };

    // `Buffer::new_empty` leaves the wrap width unset, so nothing is
    // wrapped: every paragraph stays on one line and `shape_until_scroll`
    // shapes the whole buffer rather than a viewport slice.
    let mut buffer = cosmic_text::Buffer::new_empty(cosmic_text::Metrics::new(size, size));
    let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Name(FIRA_SANS_FAMILY_NAME));
    buffer.set_text(
        &mut font_system,
        text,
        &attrs,
        cosmic_text::Shaping::Advanced,
        None,
    );

    buffer.layout_runs().map(|run| run.line_w).sum::<f32>()
}

/// Real, renderer-matching single-line text width for the proportional UI
/// font, measured with `cosmic-text`.
///
/// Drop-in replacement for [`heuristic_width`] at the
/// `Fn(&str, Font, f32) -> f32` seam ([`IcedTextMeasurer::new`]): it
/// returns the pixel advance of `text` at `size` using actual glyph
/// advances from the same Fira Sans bytes iced renders with, kerning
/// included. Narrow and wide glyphs therefore come apart — `"iiii"` is
/// genuinely shorter than `"MMMM"` — which the flat per-character model
/// could never express.
///
/// # Fallbacks
///
/// Every path below lands on a finite number, and all but the size guard on
/// [`heuristic_width`], so the function is total:
///
/// * [`Font::MONOSPACE`] — this build ships **no** monospace bytes, so
///   shaping mono text with Fira Sans would be measuring the wrong face.
///   Mono widths stay on the heuristic's `MONOSPACE_AVG_ADVANCE` until a
///   real mono face is vendored.
/// * A measurement that is non-finite, or zero for non-empty text — the
///   FTWA solver panics on non-finite input
///   (`crate::markdown::table_width::ftwa`), and a zero-width column that
///   is not really zero-width would collapse the layout, so both are
///   refused in favour of the heuristic's guaranteed-positive answer.
/// * A non-finite or non-positive `size` — nothing can be shaped, and the
///   heuristic would just propagate the bad value, so this returns `0.0`.
///
/// Any other family (`SansSerif`, `Serif`, `Cursive`, `Fantasy`, or a
/// `Family::Name`) is deliberately measured as Fira Sans **by name**, which
/// is what makes the answer identical on every machine. The generic
/// families are not usable here: `db.query(SansSerif)` and
/// `db.query(Monospace)` both return `None` on a host without cosmic-text's
/// hardcoded generic defaults ("Open Sans" / "Noto Sans Mono") installed,
/// so measuring through a generic family would drift with whatever the user
/// happens to have installed.
///
/// # Renderer parity caveat ([RUST-041])
///
/// iced rewrites its default font to `Font::with_name("Fira Sans")` only
/// under `target_arch = "wasm32"`
/// (`iced_graphics-0.14.0/src/settings.rs:38-49`). On native Windows
/// `Font::DEFAULT` stays the generic `SansSerif`, which cosmic-text
/// resolves out of the host collection rather than from Fira Sans. Exact
/// painted-equals-measured parity therefore requires the app to pin
/// `default_font = Font::with_name("Fira Sans")` in its iced settings;
/// until it does, this is a stable, Fira-shaped measurement of the intended
/// face rather than a byte-exact match of whatever the host paints.
///
/// # Guarantees
///
/// * Empty input is exactly `0.0`.
/// * The result is always finite and never negative.
/// * Non-decreasing in the text: `width(prefix) <= width(prefix + rest)`,
///   which is what keeps the solver's `max_content >= min_content`
///   invariant intact.
/// * Never panics — multi-byte UTF-8, emoji, whitespace-only and
///   pathologically long inputs are all shaped and returned as-is.
#[must_use]
pub fn fira_sans_width(text: &str, font: Font, size: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    if !size.is_finite() || size <= 0.0 {
        // A zero, negative or non-finite size cannot be shaped (`cosmic-text`
        // asserts a non-zero line height) and would hand the heuristic's
        // multiply straight back as `NaN`/`inf` — which the FTWA solver
        // panics on. Answer zero and stay finite.
        return 0.0;
    }
    if font.family == Family::Monospace {
        return heuristic_width(text, font, size);
    }
    usable_width(
        measured_fira_sans_width(text, size),
        heuristic_width(text, font, size),
    )
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
    pub const FASTMD: TextMetrics = TextMetrics {
        body_size: 14.0,
        mono_size: 13.0,
        line_height: 18.0,
    };
}

/// A [`TextMeasurer`] for the pure FTWA builder, driven by iced fonts.
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
    /// Monospace only for `code` runs; links, HTML and the image
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
    /// accumulation crosses element boundaries — a word split between two
    /// `Text` runs keeps its identity — so the solver sees the same break
    /// opportunities wherever the text was split.
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
                    // A single-part run that ends with whitespace closes the
                    // pending token immediately, measured with the
                    // *previous* run's font.
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

/// Whether the solved table needs the horizontal-scroll fallback.
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
