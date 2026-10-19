//! Iced port of the third-party attributions window — the credits list
//! behind the About dialog's attributions section ([TASK-017b2]).
//!
//! A **pure view over a plain snapshot**: [`attributions_view`] renders an
//! [`AttributionsViewModel`] into an [`iced::Element`] of [`Message`] and
//! performs no I/O. The catalog itself is **reused unchanged** from the
//! the legacy UI build — `crate::ui::attributions::DIRECT_DEPENDENCIES`, the
//! curated `&'static [Attribution]` constant in
//! `src/app/ui/attributions.rs` (57 direct third-party crates across all
//! four workspace members, alphabetically sorted, no duplicates). There
//! is no generated `cargo-about` file and no runtime shell-out anywhere in
//! either build; the data is compiled in.
//!
//! # What the catalog actually carries
//!
//! `Attribution { name, authors, github_url }` — that is the whole row.
//! The catalog has **no per-crate license or version field**, so this
//! window shows Crate / Authors / Repository exactly as the legacy UI dialog
//! did (`src/app/ui/about_dialog.rs:200-220`), not a license column.
//! The only license text in the app is FastMD's own MIT license, shown in
//! the About card ([`crate::ui_iced::about`]). Adding real per-crate
//! license data means a new compile-time pipeline (e.g. a `cargo-about`
//! generated file) — a data-source decision for the orchestrator, not
//! something this port invents.
//!
//! Note also that the catalog is *curated*, not derived from the resolved
//! dependency graph: `the legacy_extras` is listed at
//! `src/app/ui/attributions.rs:76` purely to credit its authors even
//! though the crate is not a real consumer. That curation is preserved
//! verbatim.
//!
//! # Layout
//!
//! the legacy UI laid the rows out as plain `ui.horizontal` triples inside a
//! `ScrollArea` — no table widget. This port uses
//! `iced::widget::table` (`iced_widget-0.14.2/src/table.rs:18-49`) so the
//! three columns the legacy UI header already names actually line up, per the
//! migration map's `Table` → `iced::widget::table` mapping
//! (`doc/planning/iced-migration-map.md` A5).
//!
//! ```text
//! Third-Party Attributions
//! 57 direct third-party dependencies
//! ────────────────────────────────────────────────────────────
//! Crate   │ Authors             │ Repository
//! ─────────┼───────────────────┼──────────────────────────────
//! anyhow  │ David Tolnay      │ https://github.com/dtolnay/…
//! ```
//!
//! The repository cell is clickable: iced has no `hyperlink_to`, so the
//! row emits the deferred intent [`AboutMessage::RepositoryOpened`] and
//! the [TASK-022]/[TASK-023] reducer opens the URL (the legacy UI build
//! pushed `OutputCommand::OpenUrl` from inside the click).
//!
//! The empty catalog renders the same widget tree with an empty table and
//! an empty-state summary line, so the shape never changes (AGENTS §6).
//!
//! Unit tests live in the sibling `attributions_tests.rs` sidecar.

use iced::widget::{Column, Row, container, mouse_area, rule, scrollable, space, table, text};
use iced::{Alignment, Element, Length};

use crate::ui::attributions::{Attribution, DIRECT_DEPENDENCIES};
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::about::{
    ABOUT_BODY_SIZE, ABOUT_SECTION_SPACING, SEPARATOR_WIDTH, close_button,
};
use crate::ui_iced::message::{AboutMessage, Message};

#[cfg(test)]
#[path = "attributions_tests.rs"]
mod tests;

/// Width of the attributions card — same card width as the About dialog
/// ([`crate::ui_iced::about::ABOUT_CARD_WIDTH`]).
pub const ATTRIBUTIONS_CARD_WIDTH: f32 = 620.0;

/// Logical point size of the window title.
pub const ATTRIBUTIONS_TITLE_SIZE: f32 = 18.0;

/// Logical point size of a table header cell.
pub const ATTRIBUTIONS_HEADER_SIZE: f32 = 14.0;

/// Logical point size of a table body cell.
pub const ATTRIBUTIONS_ROW_SIZE: f32 = 14.0;

/// Maximum height of the credits scroll area — legacy parity
/// (`ScrollArea::max_height(340.0)`, `src/app/ui/about_dialog.rs:209`).
pub const ATTRIBUTIONS_LIST_MAX_HEIGHT: f32 = 340.0;

/// Horizontal padding inside a table cell.
pub const CELL_PADDING_X: f32 = 6.0;

/// Vertical padding inside a table cell.
pub const CELL_PADDING_Y: f32 = 2.0;

/// Padding inside the card.
pub const CARD_PADDING: f32 = 12.0;

/// A snapshot of the third-party attribution catalog.
///
/// The entries are copied out of the compile-time catalog once, at
/// snapshot time, so the view holds its own data and cannot be surprised
/// mid-frame. `Attribution`'s fields are all `&'static str`, so the copy
/// is cheap and no allocation outlives the snapshot's data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttributionsViewModel {
    entries: Vec<Attribution>,
}

impl AttributionsViewModel {
    /// Snapshots the reused catalog ([`DIRECT_DEPENDENCIES`]) in its
    /// existing order.
    ///
    /// Purity: pure — reads a `&'static` constant.
    #[must_use]
    pub fn from_catalog() -> Self {
        Self {
            entries: DIRECT_DEPENDENCIES.to_vec(),
        }
    }

    /// Builds a view model over an arbitrary catalog slice — the seam tests
    /// and any future alternative catalog use.
    #[must_use]
    pub fn with_entries(entries: Vec<Attribution>) -> Self {
        Self { entries }
    }

    /// The rows to render, in render order.
    #[must_use]
    pub fn entries(&self) -> &[Attribution] {
        &self.entries
    }

    /// How many crates are listed.
    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Whether the catalog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The summary line under the title: the count, or the empty-state
    /// line when there is nothing to credit.
    ///
    /// Folding the empty state into this line rather than swapping in a
    /// different widget keeps the card's widget tree constant (AGENTS §6).
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_empty() {
            strings::ABOUT_ATTRIBUTIONS_EMPTY.to_string()
        } else {
            strings::format_attribution_count(self.count())
        }
    }

    /// Renders this snapshot as the attributions window
    /// ([`attributions_view`]).
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        attributions_view(self)
    }
}

/// Renders the third-party attributions window ([TASK-017b2]).
///
/// Title, count line, the scrollable three-column catalog and the shared
/// Close footer. The list is capped at
/// [`ATTRIBUTIONS_LIST_MAX_HEIGHT`] (the legacy UI's 340px) and scrolls; the
/// table is always allocated, empty or not.
///
/// Emitted messages:
/// * a repository cell → [`AboutMessage::RepositoryOpened`]
/// * the footer → [`AboutMessage::CloseRequested`]
///
/// Like [`crate::ui_iced::about::about_card`], this is card content: the
/// root view mounts it through
/// [`crate::ui_iced::overlay::modal`] with
/// `Message::About(AboutMessage::CloseRequested)` as the dismiss message.
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn attributions_view(view_model: &AttributionsViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let title = text(strings::ABOUT_ATTRIBUTIONS_HEADER)
        .size(ATTRIBUTIONS_TITLE_SIZE)
        .color(palette.text_strong);

    let summary = text(view_model.summary())
        .size(ABOUT_BODY_SIZE)
        .color(palette.text_muted);

    let list = scrollable(attribution_table(view_model.entries()))
        .height(Length::Fixed(ATTRIBUTIONS_LIST_MAX_HEIGHT));

    let footer = Row::with_children(vec![
        space::Space::new().width(Length::Fill).into(),
        close_button(),
    ])
    .spacing(ABOUT_SECTION_SPACING)
    .align_y(Alignment::Center);

    Column::with_children(vec![
        title.into(),
        summary.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        list.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        footer.into(),
    ])
    .spacing(ABOUT_SECTION_SPACING)
    .padding(CARD_PADDING)
    .width(Length::Fixed(ATTRIBUTIONS_CARD_WIDTH))
    .into()
}

/// The credits table: Crate / Authors / Repository ([TASK-017b2]).
///
/// Column widths follow the same rule `ui_iced::logs::log_table` learned
/// the hard way: iced promotes the first all-`Shrink` column to `Fill`
/// (`iced_widget-0.14.2/src/table.rs:129-133`), so the *last* column —
/// the long repository URL — carries `Length::Fill` explicitly and the
/// short name/author columns stay `Shrink`.
///
/// The header row carries the legacy UI column captions
/// ([`strings::ABOUT_COL_CRATE`] / [`strings::ABOUT_COL_AUTHORS`] /
/// [`strings::ABOUT_COL_REPO`]); with zero rows the header still renders,
/// so the empty state needs no separate widget.
///
/// Purity: pure over `entries`.
#[must_use]
pub fn attribution_table(entries: &[Attribution]) -> Element<'static, Message> {
    let columns = vec![
        table::column(
            header_cell(strings::ABOUT_COL_CRATE),
            move |entry: Attribution| -> Element<'static, Message> {
                text(entry.name)
                    .size(ATTRIBUTIONS_ROW_SIZE)
                    .color(crate_name_color())
                    .into()
            },
        )
        .width(Length::Shrink),
        table::column(
            header_cell(strings::ABOUT_COL_AUTHORS),
            move |entry: Attribution| -> Element<'static, Message> {
                text(entry.authors)
                    .size(ATTRIBUTIONS_ROW_SIZE)
                    .color(authors_color())
                    .into()
            },
        )
        .width(Length::Shrink),
        table::column(
            header_cell(strings::ABOUT_COL_REPO),
            move |entry: Attribution| repository_cell(entry.github_url),
        )
        .width(Length::Fill),
    ];

    table::table(columns, entries.to_vec())
        .padding_x(CELL_PADDING_X)
        .padding_y(CELL_PADDING_Y)
        .separator_x(SEPARATOR_WIDTH)
        .separator_y(SEPARATOR_WIDTH)
        .width(Length::Fill)
        .into()
}

/// A column-header cell.
fn header_cell(label: &'static str) -> Element<'static, Message> {
    text(label)
        .size(ATTRIBUTIONS_HEADER_SIZE)
        .color(Palette::fastmd_dark().text_strong)
        .into()
}

/// A clickable repository cell — the iced re-typing of the legacy UI's
/// `ui.hyperlink_to` (`src/app/ui/about_dialog.rs:216`).
fn repository_cell(url: &'static str) -> Element<'static, Message> {
    mouse_area(
        container(
            text(url)
                .size(ATTRIBUTIONS_ROW_SIZE)
                .color(repository_color()),
        )
        .width(Length::Fill)
        .padding(2.0),
    )
    .on_press(repository_press(url))
    .into()
}

/// The deferred-intent message a repository cell emits.
///
/// The view may not open a browser (no I/O at render time), so the URL
/// travels with the message and the reducer opens it — the same shape the
/// markdown render path uses for document links
/// ([`crate::ui_iced::message::RenderMessage::LinkActivated`]).
///
/// Purity: pure function of `url`.
#[must_use]
pub fn repository_press(url: &str) -> Message {
    Message::About(AboutMessage::RepositoryOpened {
        url: url.to_owned(),
    })
}

/// Palette colour for the crate-name column (`text_strong` — the row's
/// primary label, the legacy UI's `ui.strong(attr.name)`).
#[must_use]
pub fn crate_name_color() -> iced::Color {
    Palette::fastmd_dark().text_strong
}

/// Palette colour for the authors column (default body `text`).
#[must_use]
pub fn authors_color() -> iced::Color {
    Palette::fastmd_dark().text
}

/// Palette colour for the repository column (`accent` — it is a link).
#[must_use]
pub fn repository_color() -> iced::Color {
    Palette::fastmd_dark().accent
}
