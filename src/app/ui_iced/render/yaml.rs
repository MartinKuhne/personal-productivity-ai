//! YAML front-matter mapping and widget.
//!
//! The front-matter mapping is a two-column layout: a key column with a
//! preferred width, a value column that takes the remaining width, and
//! alternating row backgrounds. Parsing stays where it is —
//! [`crate::markdown::parse_yaml_to_pairs`] ([RUST-051]).
//!
//! Nothing is cached: the block is a pure function of its inputs, so the
//! parsed pairs and the solved row widths are recomputed per frame. Were a
//! cache ever to prove necessary, it belongs in the state layer, not here.
//!
//! Non-mapping front matter (a bare string, a sequence, a null document)
//! yields no rows — the front-matter view only understands key/value
//! pairs.
//!
//! Unit tests live in the sibling `yaml_tests.rs` sidecar.

use crate::markdown::parse_yaml_to_pairs;
use crate::ui::theme::Palette;
use crate::ui_iced::message::Message;
use iced::widget::{Column, container, text};
use iced::{Alignment, Background, Element, Length};

/// Preferred width of the key column, in logical pixels.
pub const YAML_KEY_COLUMN_WIDTH: f32 = 110.0;

/// Horizontal gap between the key and value columns, in logical pixels.
pub const YAML_COLUMN_GAP: f32 = 12.0;

/// Vertical gap between front-matter rows.
pub const YAML_ROW_SPACING: f32 = 4.0;

/// Floor for the key column when the viewport is narrow.
pub const YAML_MIN_KEY_WIDTH: f32 = 40.0;

/// Floor for the value column.
pub const YAML_MIN_VALUE_WIDTH: f32 = 40.0;

/// Chrome subtracted from the available width before the key column is
/// clamped — the frame allowance around the block.
pub const YAML_WIDTH_CHROME: f32 = 20.0;

/// Space left under the whole front-matter block.
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
/// The key column is the preferred width capped by what is left after the
/// frame chrome, floored at [`YAML_MIN_KEY_WIDTH`]; the value column takes
/// the remainder, floored at [`YAML_MIN_VALUE_WIDTH`].
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
/// An empty row list renders nothing.
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
            background: Some(Background::Color(palette.table_surface)),
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
        .color(palette.accent_text_soft)
        .width(Length::Fixed(model.key_width.max(0.0)));

    let value = text(model.value.clone())
        .size(14.0)
        .color(palette.text_value)
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
            background: striped.then_some(Background::Color(palette.surface_variant)),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
#[path = "yaml_tests.rs"]
mod tests;
