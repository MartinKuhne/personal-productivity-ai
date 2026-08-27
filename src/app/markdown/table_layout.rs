//! Markdown table layout — computes per-cell geometry (width/height/position)
//! from measured cell content using the FTWA column-width solver.
//!
//! Unit tests live in the sibling `table_layout_tests.rs` sidecar.
use crate::markdown::model::InlineElem;
use crate::markdown::table_width::{CellTokens, DeficitStrategy, compute_column_breakpoints, ftwa};

/// A trait for measuring text widths in a UI-agnostic way.
pub trait TextMeasurer {
    /// Measure a cell's contents and return `(max_content_width, min_content_width, cell_tokens)`
    fn measure_cell(&self, cell: &[InlineElem]) -> (f32, f32, CellTokens);
    /// Return the width of a single space character.
    fn space_width(&self) -> f32;
    /// Return the height of a single line of text.
    fn line_height(&self) -> f32;
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutCell {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub content: Vec<InlineElem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableLayout {
    pub rows: Vec<Vec<LayoutCell>>,
    pub total_width: f32,
    pub total_height: f32,
    pub needs_horizontal_scroll: bool,
}

pub struct TableLayoutBuilder<'a, M: TextMeasurer> {
    measurer: &'a M,
    strategy: DeficitStrategy,
    available_width: f32,
    padding_h: f32,
    padding_v: f32,
    col_spacing: f32,
    row_spacing: f32,
}

impl<'a, M: TextMeasurer> TableLayoutBuilder<'a, M> {
    pub fn new(measurer: &'a M, available_width: f32, strategy: DeficitStrategy) -> Self {
        Self {
            measurer,
            strategy,
            available_width,
            padding_h: 0.0,
            padding_v: 0.0,
            col_spacing: 10.0,
            row_spacing: 2.0,
        }
    }

    pub fn with_padding(mut self, padding_h: f32, padding_v: f32) -> Self {
        self.padding_h = padding_h;
        self.padding_v = padding_v;
        self
    }

    pub fn with_spacing(mut self, col_spacing: f32, row_spacing: f32) -> Self {
        self.col_spacing = col_spacing;
        self.row_spacing = row_spacing;
        self
    }

    pub fn build(&self, ast: &[Vec<Vec<InlineElem>>]) -> TableLayout {
        let n_cols = ast.iter().map(|row| row.len()).max().unwrap_or(0);
        if n_cols == 0 {
            return TableLayout {
                rows: Vec::new(),
                total_width: 0.0,
                total_height: 0.0,
                needs_horizontal_scroll: false,
            };
        }

        let mut max_w = vec![0.0_f32; n_cols];
        let mut min_w = vec![0.0_f32; n_cols];
        let mut cell_tokens_per_col: Vec<Vec<CellTokens>> = vec![Vec::new(); n_cols];

        // Measure all cells
        for row in ast {
            for (j, cell) in row.iter().enumerate() {
                let (cell_max, cell_min, tokens) = self.measurer.measure_cell(cell);
                if cell_max > max_w[j] {
                    max_w[j] = cell_max;
                }
                if cell_min > min_w[j] {
                    min_w[j] = cell_min;
                }
                cell_tokens_per_col[j].push(tokens);
            }
        }

        for w in &mut max_w {
            if *w <= 0.0 {
                *w = 1.0;
            }
            *w += self.padding_h;
        }
        for w in &mut min_w {
            if *w <= 0.0 {
                *w = 1.0;
            }
            *w += self.padding_h;
        }

        let space_w = self.measurer.space_width();
        let breakpoints = cell_tokens_per_col
            .iter()
            .map(|col_tokens| compute_column_breakpoints(col_tokens, space_w))
            .collect::<Vec<_>>();

        let content_avail =
            (self.available_width - (n_cols as f32 - 1.0) * self.col_spacing).max(0.0);
        let decision = ftwa(&max_w, &min_w, &breakpoints, content_avail, self.strategy);

        let col_widths = if decision.needs_horizontal_scroll {
            max_w.clone() // fallback to max widths if scrolling
        } else {
            decision.widths.clone()
        };

        // Layout rows and cells
        let mut rows = Vec::with_capacity(ast.len());
        let mut current_y = 0.0;
        let line_h = self.measurer.line_height();

        for row in ast {
            let mut current_x = 0.0;
            let mut row_max_h = line_h; // minimum 1 line height

            // First pass: compute row height
            for (j, cell) in row.iter().enumerate() {
                let col_w = col_widths.get(j).copied().unwrap_or(0.0);
                let inner_w = (col_w - self.padding_h).max(0.0);

                let tokens = self.measurer.measure_cell(cell).2.token_widths;
                let lines = greedy_line_count(&tokens, space_w, inner_w);
                let cell_h = (lines as f32) * line_h + self.padding_v;

                if cell_h > row_max_h {
                    row_max_h = cell_h;
                }
            }

            // Second pass: build layout cells
            let mut layout_cells = Vec::with_capacity(n_cols);
            for (j, cell) in row.iter().enumerate() {
                let col_w = col_widths.get(j).copied().unwrap_or(0.0);
                layout_cells.push(LayoutCell {
                    x: current_x,
                    y: current_y,
                    width: col_w,
                    height: row_max_h,
                    content: cell.clone(),
                });
                current_x += col_w + self.col_spacing;
            }

            rows.push(layout_cells);
            current_y += row_max_h + self.row_spacing;
        }

        let total_w = col_widths.iter().sum::<f32>() + (n_cols as f32 - 1.0) * self.col_spacing;
        let total_h = (current_y - self.row_spacing).max(0.0);

        TableLayout {
            rows,
            total_width: total_w,
            total_height: total_h,
            needs_horizontal_scroll: decision.needs_horizontal_scroll,
        }
    }
}

fn greedy_line_count(token_widths: &[f32], space_width: f32, col_width: f32) -> i32 {
    if token_widths.is_empty() {
        return 1;
    }
    let mut lines = 1;
    let mut line_w = 0.0;
    for (i, &tw) in token_widths.iter().enumerate() {
        let token_w = if i == 0 { tw } else { space_width + tw };
        if line_w + token_w > col_width + 1e-6 {
            lines += 1;
            line_w = tw;
        } else {
            line_w += token_w;
        }
    }
    lines
}

#[cfg(test)]
#[path = "table_layout_tests.rs"]
mod table_layout_tests;
