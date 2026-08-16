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
mod tests {
    use super::*;

    struct MockMeasurer {
        char_width: f32,
        space_width: f32,
        line_height: f32,
    }

    impl TextMeasurer for MockMeasurer {
        fn measure_cell(&self, cell: &[InlineElem]) -> (f32, f32, CellTokens) {
            let mut text = String::new();
            for elem in cell {
                if let InlineElem::Text(t, _) = elem {
                    text.push_str(t);
                }
            }

            if text.is_empty() {
                return (
                    0.0,
                    0.0,
                    CellTokens {
                        token_widths: vec![],
                    },
                );
            }

            let max_w = (text.chars().count() as f32) * self.char_width;
            let tokens: Vec<&str> = text.split_whitespace().collect();
            let mut min_w = 0.0_f32;
            let mut token_widths = Vec::new();

            for t in tokens {
                let w = (t.chars().count() as f32) * self.char_width;
                if w > min_w {
                    min_w = w;
                }
                token_widths.push(w);
            }

            (max_w, min_w, CellTokens { token_widths })
        }

        fn space_width(&self) -> f32 {
            self.space_width
        }

        fn line_height(&self) -> f32 {
            self.line_height
        }
    }

    fn text_elem(s: &str) -> InlineElem {
        InlineElem::Text(s.to_string(), Default::default())
    }

    #[test]
    fn test_table_layout_builder_surplus() {
        let measurer = MockMeasurer {
            char_width: 10.0,
            space_width: 10.0,
            line_height: 20.0,
        };

        let ast = vec![vec![vec![text_elem("Hello World")], vec![text_elem("Foo")]]];

        let builder =
            TableLayoutBuilder::new(&measurer, 500.0, DeficitStrategy::ProportionalToSlack)
                .with_spacing(10.0, 2.0);
        let layout = builder.build(&ast);

        // "Hello World" is 11 chars * 10 = 110 max width
        // "Foo" is 3 chars * 10 = 30 max width
        // Total max = 140, available = 500. So surplus regime.
        assert!(!layout.needs_horizontal_scroll);
        assert_eq!(layout.rows[0][0].width, 110.0);
        assert_eq!(layout.rows[0][1].width, 30.0);
        assert_eq!(layout.rows[0][0].height, 20.0); // 1 line height
        assert_eq!(layout.total_width, 140.0 + 10.0); // +10 spacing
    }

    #[test]
    fn test_table_layout_builder_deficit_wrapping() {
        let measurer = MockMeasurer {
            char_width: 10.0,
            space_width: 10.0,
            line_height: 20.0,
        };

        // "A very long sentence" = 20 chars = 200 max. Min = "sentence" = 80.
        // "Short" = 5 chars = 50 max. Min = 50.
        // Total max = 250.
        let ast = vec![vec![
            vec![text_elem("A very long sentence")],
            vec![text_elem("Short")],
        ]];

        // Available = 150 (minus 10 padding = 140 for content). Deficit regime.
        let builder =
            TableLayoutBuilder::new(&measurer, 150.0, DeficitStrategy::ProportionalToSlack)
                .with_spacing(10.0, 2.0);
        let layout = builder.build(&ast);

        assert!(!layout.needs_horizontal_scroll);

        // "Short" has 0 slack, stays at 50.
        // "A very long sentence" has 120 slack. It shrinks from 200 to 90 to fit the 140 remaining content width.
        assert_eq!(layout.rows[0][0].width, 90.0);
        assert_eq!(layout.rows[0][1].width, 50.0);

        // "A very long sentence" at width 90:
        // Tokens: "A" (10), "very" (40), "long" (40), "sentence" (80)
        // Space = 10
        // Line 1: "A" (10) + space (10) + "very" (40) = 60.
        // Line 2: "long" (40)
        // Line 3: "sentence" (80)
        // Lines = 3. Height = 60.
        assert_eq!(layout.rows[0][0].height, 60.0);
        assert_eq!(layout.total_height, 60.0); // 1 row
    }

    #[test]
    fn test_table_layout_builder_fallback_horizontal_scroll() {
        let measurer = MockMeasurer {
            char_width: 10.0,
            space_width: 10.0,
            line_height: 20.0,
        };

        let ast = vec![vec![
            vec![text_elem("Unbreakable")], // min width 110
            vec![text_elem("AlsoLong")],    // min width 80
        ]];

        // Available = 100, which is less than sum of min_widths + spacing (190 + 10 = 200).
        let builder =
            TableLayoutBuilder::new(&measurer, 100.0, DeficitStrategy::ProportionalToSlack)
                .with_spacing(10.0, 2.0);
        let layout = builder.build(&ast);

        assert!(layout.needs_horizontal_scroll);

        // Should fallback to max content widths (which equal min widths here)
        assert_eq!(layout.rows[0][0].width, 110.0);
        assert_eq!(layout.rows[0][1].width, 80.0);
    }
}
