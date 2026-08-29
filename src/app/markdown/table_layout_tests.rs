use super::*;
use crate::markdown::table_width::CellTokens;

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

    let builder = TableLayoutBuilder::new(&measurer, 500.0, DeficitStrategy::ProportionalToSlack)
        .with_spacing(10.0, 2.0);
    let layout = builder.build(&ast);

    assert!(!layout.needs_horizontal_scroll);
    assert_eq!(layout.rows[0][0].width, 110.0);
    assert_eq!(layout.rows[0][1].width, 30.0);
    assert_eq!(layout.rows[0][0].height, 20.0);
    assert_eq!(layout.total_width, 140.0 + 10.0);
}

#[test]
fn test_table_layout_builder_deficit_wrapping() {
    let measurer = MockMeasurer {
        char_width: 10.0,
        space_width: 10.0,
        line_height: 20.0,
    };

    let ast = vec![vec![
        vec![text_elem("A very long sentence")],
        vec![text_elem("Short")],
    ]];

    let builder = TableLayoutBuilder::new(&measurer, 150.0, DeficitStrategy::ProportionalToSlack)
        .with_spacing(10.0, 2.0);
    let layout = builder.build(&ast);

    assert!(!layout.needs_horizontal_scroll);
    assert_eq!(layout.rows[0][0].width, 90.0);
    assert_eq!(layout.rows[0][1].width, 50.0);
    assert_eq!(layout.rows[0][0].height, 60.0);
    assert_eq!(layout.total_height, 60.0);
}

#[test]
fn test_table_layout_builder_fallback_horizontal_scroll() {
    let measurer = MockMeasurer {
        char_width: 10.0,
        space_width: 10.0,
        line_height: 20.0,
    };

    let ast = vec![vec![
        vec![text_elem("Unbreakable")],
        vec![text_elem("AlsoLong")],
    ]];

    let builder = TableLayoutBuilder::new(&measurer, 100.0, DeficitStrategy::ProportionalToSlack)
        .with_spacing(10.0, 2.0);
    let layout = builder.build(&ast);

    assert!(layout.needs_horizontal_scroll);
    assert_eq!(layout.rows[0][0].width, 110.0);
    assert_eq!(layout.rows[0][1].width, 80.0);
}

#[test]
fn test_table_layout_builder_empty_ast_returns_empty() {
    let measurer = MockMeasurer {
        char_width: 10.0,
        space_width: 10.0,
        line_height: 20.0,
    };

    let builder = TableLayoutBuilder::new(&measurer, 500.0, DeficitStrategy::ProportionalToSlack)
        .with_spacing(10.0, 2.0);
    let layout = builder.build(&[]);

    assert!(layout.rows.is_empty());
    assert_eq!(layout.total_width, 0.0);
    assert_eq!(layout.total_height, 0.0);
    assert!(!layout.needs_horizontal_scroll);
}

#[test]
fn test_table_layout_builder_ragged_rows_uses_max_columns() {
    let measurer = MockMeasurer {
        char_width: 10.0,
        space_width: 10.0,
        line_height: 20.0,
    };

    // First row has 2 columns, second row has 1 column.
    let ast = vec![
        vec![vec![text_elem("A")], vec![text_elem("B")]],
        vec![vec![text_elem("C")]],
    ];

    let builder = TableLayoutBuilder::new(&measurer, 500.0, DeficitStrategy::ProportionalToSlack)
        .with_spacing(10.0, 2.0);
    let layout = builder.build(&ast);

    assert_eq!(layout.rows.len(), 2);
    assert_eq!(layout.rows[0].len(), 2);
    assert_eq!(layout.rows[1].len(), 1);
}

#[test]
fn test_table_layout_builder_zero_width_cell_clamped_to_one() {
    let measurer = MockMeasurer {
        char_width: 10.0,
        space_width: 10.0,
        line_height: 20.0,
    };

    // An empty cell measures (0.0, 0.0) and must be clamped to a width of 1.0
    // (plus horizontal padding) so it is still visible.
    let ast = vec![vec![vec![], vec![text_elem("X")]]];

    let builder = TableLayoutBuilder::new(&measurer, 500.0, DeficitStrategy::ProportionalToSlack)
        .with_padding(4.0, 0.0)
        .with_spacing(10.0, 2.0);
    let layout = builder.build(&ast);

    assert_eq!(layout.rows[0][0].width, 1.0 + 4.0);
    assert_eq!(layout.rows[0][1].width, 10.0 + 4.0);
    assert_eq!(layout.total_width, (1.0 + 4.0) + (10.0 + 4.0) + 10.0);
}

#[test]
fn test_table_layout_builder_negative_available_width_clamps_to_zero() {
    let measurer = MockMeasurer {
        char_width: 10.0,
        space_width: 10.0,
        line_height: 20.0,
    };

    let ast = vec![vec![vec![text_elem("Foo")]]];

    let builder = TableLayoutBuilder::new(&measurer, -100.0, DeficitStrategy::ProportionalToSlack)
        .with_spacing(10.0, 2.0);
    let layout = builder.build(&ast);

    // content_avail is clamped to >= 0.0, which is below the column min width,
    // so the layout falls back to the max content width and signals scroll.
    assert!(layout.needs_horizontal_scroll);
    assert_eq!(layout.rows.len(), 1);
    assert_eq!(layout.rows[0][0].width, 30.0);
}

#[test]
fn test_table_layout_builder_zero_spacing_no_padding_between_columns() {
    let measurer = MockMeasurer {
        char_width: 10.0,
        space_width: 10.0,
        line_height: 20.0,
    };

    let ast = vec![vec![vec![text_elem("A")], vec![text_elem("B")]]];

    let builder = TableLayoutBuilder::new(&measurer, 500.0, DeficitStrategy::ProportionalToSlack)
        .with_spacing(0.0, 0.0);
    let layout = builder.build(&ast);

    assert_eq!(layout.total_width, 10.0 + 10.0);
    assert_eq!(layout.rows[0][0].height, 20.0);
    assert_eq!(layout.total_height, 20.0);
}
