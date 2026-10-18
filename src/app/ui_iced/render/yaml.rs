//! YAML front-matter mapping and widget ([TASK-015]).
//!
//! The egui renderer (`src/app/ui/render/yaml_table.rs`) painted the
//! front-matter mapping as a two-column table with a fixed key column, a
//! value column that takes the remaining width, and alternating row
//! backgrounds. The presentation is ported here as themed iced rows; the
//! parsing stays where it is — [`crate::markdown::parse_yaml_to_pairs`]
//! ([RUST-051]).
//!
//! What the egui build cached in `egui`'s temp data (parsed pairs and row
//! heights, `yaml_table.rs:26-35`, `:44-57`) is **not** ported: the iced
//! port is a pure function of its inputs, and the [TASK-022] state layer
//! is where a cache would live if one proves necessary.
//!
//! Non-mapping front matter (a bare string, a sequence, a null document)
//! yields no rows, exactly as the egui fall-through did — the front-matter
//! editor only understands key/value pairs.
//!
//! Unit tests live in the sibling `yaml_tests.rs` sidecar.

use crate::markdown::parse_yaml_to_pairs;
use crate::ui::theme::Palette;
use crate::ui_iced::message::Message;
use crate::ui_iced::theme::to_iced_color;
use iced::widget::{Column, container, text};
use iced::{Alignment, Background, Element, Length};

/// Preferred width of the key column.
///
/// Ported from `YAML_KEY_COLUMN_WIDTH` (`src/app/ui/render/yaml_table.rs:24`).
pub const YAML_KEY_COLUMN_WIDTH: f32 = 110.0;

/// Horizontal gap between the key and value columns.
///
/// The 12 px inter-column gap the egui table used
/// (`yaml_table.rs:23`, `:68`).
pub const YAML_COLUMN_GAP: f32 = 12.0;

/// Vertical gap between front-matter rows.
pub const YAML_ROW_SPACING: f32 = 4.0;

/// Floor for the key column when the viewport is narrow.
///
/// Ported from the `.max(40.0)` clamp on `key_col_width`
/// (`src/app/ui/render/yaml_table.rs:62`).
pub const YAML_MIN_KEY_WIDTH: f32 = 40.0;

/// Floor for the value column.
///
/// Ported from the `.max(40.0)` clamp on `value_col_width`
/// (`src/app/ui/render/yaml_table.rs:63`).
pub const YAML_MIN_VALUE_WIDTH: f32 = 40.0;

/// Chrome subtracted from the available width before the key column is
/// clamped — the egui table's frame allowance (`yaml_table.rs:62`).
pub const YAML_WIDTH_CHROME: f32 = 20.0;

/// Space left under the whole front-matter block.
///
/// Ported from the `ui.add_space(8.0)` that closed `render_yaml_table`
/// (`src/app/ui/render/yaml_table.rs:157`).
pub const YAML_BOTTOM_SPACING: f32 = 8.0;

/// Row-index parity that receives the striped background.
pub const YAML_STRIPE_PARITY: usize = 1;

/// One front-matter row with its solved column widths.
#[derive(Clone, Debug, PartialEq)]
pub struct YamlRowModel {
    /// The mapping key.
    pub key: String,
    /// The value rendered as text (sequences and nested scalars are
    /// stringified by `parse_yaml_to_pairs`).
    pub value: String,
    /// Solved width of the key cell.
    pub key_width: f32,
    /// Solved width of the value cell.
    pub value_width: f32,
    /// Whether this row paints the stripe.
    pub striped: bool,
}

/// Whether the stripe paints on a front-matter row.
#[must_use]
pub fn is_striped(index: usize) -> bool {
    index % 2 == YAML_STRIPE_PARITY
}

/// Solves the key/value column widths for one viewport width.
///
/// Reproduces the egui clamp chain exactly
/// (`src/app/ui/render/yaml_table.rs:62-63`): the key column is the
/// preferred width capped by what is left after the frame chrome, floored
/// at [`YAML_MIN_KEY_WIDTH`]; the value column takes the remainder,
/// floored at [`YAML_MIN_VALUE_WIDTH`].
#[must_use]
pub fn yaml_column_widths(available_width: f32) -> (f32, f32) {
    let key_width =
        YAML_KEY_COLUMN_WIDTH.min((available_width - YAML_WIDTH_CHROME).max(YAML_MIN_KEY_WIDTH));
    let value_width = (available_width - key_width - YAML_COLUMN_GAP).max(YAML_MIN_VALUE_WIDTH);
    (key_width, value_width)
}

/// Maps parsed front-matter pairs onto row models.
#[must_use]
pub fn yaml_rows(pairs: &[(String, String)], available_width: f32) -> Vec<YamlRowModel> {
    let (key_width, value_width) = yaml_column_widths(available_width);
    pairs
        .iter()
        .enumerate()
        .map(|(index, (key, value))| YamlRowModel {
            key: key.clone(),
            value: value.clone(),
            key_width,
            value_width,
            striped: is_striped(index),
        })
        .collect()
}

/// Maps a YAML document onto row models, dropping non-mapping documents.
#[must_use]
pub fn yaml_rows_from_value(yaml: &serde_norway::Value, available_width: f32) -> Vec<YamlRowModel> {
    parse_yaml_to_pairs(yaml)
        .map(|pairs| yaml_rows(&pairs, available_width))
        .unwrap_or_default()
}

/// Renders the front-matter block.
///
/// An empty row list renders nothing, matching the egui fall-through for a
/// non-mapping document.
#[must_use]
pub fn yaml_block(rows: &[YamlRowModel]) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();

    let body = rows.iter().fold(
        Column::<Message>::new().spacing(YAML_ROW_SPACING),
        |body, row_model| body.push(yaml_row(row_model)),
    );

    container(body)
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(to_iced_color(palette.table_surface))),
            ..container::Style::default()
        })
        .into()
}

/// Renders one front-matter row: strong key cell, wrapping value cell.
#[must_use]
pub fn yaml_row(model: &YamlRowModel) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();

    let key = text(model.key.clone())
        .size(14.0)
        .color(to_iced_color(palette.accent_text_soft))
        .width(Length::Fixed(model.key_width.max(0.0)));

    let value = text(model.value.clone())
        .size(14.0)
        .color(to_iced_color(palette.text_value))
        .width(Length::Fixed(model.value_width.max(0.0)))
        .wrapping(iced::widget::text::Wrapping::Word);

    let cells = iced::widget::Row::with_children(vec![key.into(), value.into()])
        .spacing(YAML_COLUMN_GAP)
        .align_y(Alignment::Start);

    // Hoisted so the style closure captures a `bool`, not the borrowed
    // row — that is what keeps the returned element `'static`.
    let striped = model.striped;

    container(cells)
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: striped.then(|| Background::Color(to_iced_color(palette.surface_variant))),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
#[path = "yaml_tests.rs"]
mod tests;
