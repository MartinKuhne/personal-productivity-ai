//! Third-party attributions — the credits list the About card renders inline.
//!
//! A **pure view over a plain snapshot**: [`attribution_table`] renders the
//! catalog into an [`iced::Element`] of [`Message`] and performs no I/O. The
//! catalog itself is **reused unchanged** —
//! `crate::ui::attributions::DIRECT_DEPENDENCIES`, the curated
//! `&'static [Attribution]` constant in `src/app/ui/attributions.rs` (61
//! direct third-party crates across all four workspace members,
//! alphabetically sorted, no duplicates). There is no generated
//! `cargo-about` file and no runtime shell-out anywhere; the data is
//! compiled in.
//!
//! # Who owns which line
//!
//! The list is a **region of the About card**, so most of what a reader would
//! call "this window's design" belongs to another module:
//!
//! | Concern | Owner |
//! |---|---|
//! | Rows, columns, header captions, link cells | this module ([`attribution_table`], `header_cell`, [`repository_cell`]) |
//! | The scroll area and its stable id | [`credits_list`] / [`ATTRIBUTIONS_LIST_ID`] here |
//! | How tall that scroll area is | [`crate::ui_iced::about::credits_list_height`] — the bands above and below it are About's furniture, so the arithmetic lives there |
//! | The card that hosts the list: surface, border, radius, padding, size | [`crate::ui_iced::about`] through [`crate::ui_iced::overlay::modal_card`] |
//! | Section divider colour and weight | [`crate::ui_iced::overlay::modal_divider`] (`color.border.subtle`) |
//! | Cell type sizes and padding | `hoarfrost::token::{typography, space, shape}` |
//! | Link colour, underline, hit height | `hoarfrost::widget::hyperlink` resolver (`UIF-ATOM-001d`) |
//! | Which window is open, what dismisses it | `AboutWindowState` + `OverlayState::close_topmost`, both about the **About** card only |
//!
//! The list is not given a hand-picked height either. It is **solved** from
//! the About card's own height, so the card stays the size the viewport says
//! and the credits region is *whatever the card has left* after the identity
//! header, the build-metadata row, the licence region and the footer — and it
//! is the same card height whether the catalog has two rows or two thousand.
//!
//! Because there is no separate window any more, there is no surface id for
//! it in `crate::ui_iced::app::OverlayState::sync_surfaces` and no entry in
//! `crate::ui_iced::app::OverlayState::close_topmost`: `Esc` over the credits
//! list closes the About card, which is what the list is inside. The
//! card-boundary absorption ([`AboutMessage::Absorbed`]) that keeps a press
//! on the card's padding or a dead row from reaching the scrim is therefore
//! About's, not this module's.
//!
//! # What the catalog actually carries
//!
//! `Attribution { name, authors, github_url }` — that is the whole row.
//! The catalog has **no per-crate license or version field**, so the list
//! shows Crate / Authors / Repository and no license column.
//! The only license text in the app is FastMD's own MIT license, shown in
//! the About card above this list ([`crate::ui_iced::about`]). Adding real per-crate
//! license data means a new compile-time pipeline (e.g. a `cargo-about`
//! generated file) — a data-source decision, not something this view
//! invents.
//!
//! The catalog is *curated*, not derived from the resolved dependency
//! graph: entries are chosen by hand and listed verbatim.
//!
//! # Layout
//!
//! The rows render through `iced::widget::table`
//! (`iced_widget-0.14.2/src/table.rs:18-49`) so the three columns the
//! header names actually line up.
//!
//! ```text
//! │ Third-Party Attributions                                    │
//! │ 61 direct third-party dependencies                          │
//! │ ─────────────────────────────────────────────────────────── │
//! │ Crate   │ Authors             │ Repository                  │
//! │ ────────┼─────────────────────┼─────────────────────────────│
//! │ anyhow  │ David Tolnay        │ github.com/dtolnay/anyhow   │
//! │         │                     │  ↑ accent_text, underlined,  │
//! │         │                     │    24px hit target           │
//! │         …  the solved height — scrolls …                    │
//! ```
//!
//! The count line, the two rules around the list and everything above and
//! below them are the **About card's** — see
//! [`crate::ui_iced::about::about_card`] for the whole card and
//! [`crate::ui_iced::about::credits_list_height`] for how the list's height
//! is arrived at.
//!
//! The repository cell is a hyperlink. iced has no `hyperlink_to` and
//! `hoarfrost` ships the hyperlink as a **resolver, not a rendered
//! widget** (`widget/hyperlink.rs` has `resolve_style` and the geometry
//! but no `Widget` impl), so the cell composes them: the resolver's
//! resting `foreground` (`color.accent.text`) and `underlined` flag drive
//! the label, `hyperlink::MIN_HEIGHT` (`size.hit_target.min`) drives the
//! hit box, and a [`mouse_area`] carries the press. The row emits the
//! deferred intent [`AboutMessage::RepositoryOpened`] and the reducer
//! opens the URL.
//!
//! The empty catalog renders the same widget tree with an empty table, and
//! [`AttributionsViewModel::summary`] swaps the count line for the empty-state
//! text, so the shape never changes (AGENTS §6).
//!
//! Unit tests live in the sibling `attributions_tests.rs` sidecar.

use iced::widget::{container, mouse_area, rich_text, scrollable, table, text, text::Span};
use iced::{Alignment, Element, Length, Padding};

use hoarfrost::state::{BaseState, VisualState};
use hoarfrost::token::{shape, space, typography};
use hoarfrost::widget::hyperlink;

use crate::ui::attributions::{Attribution, DIRECT_DEPENDENCIES};
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::about::modal_frame;
use crate::ui_iced::message::{AboutMessage, Message};

#[cfg(test)]
#[path = "attributions_tests.rs"]
mod tests;

/// Stable id for the credits scroll area, so the reducer can address it
/// and the scroll position survives a tree rebuild (AGENTS §5).
pub const ATTRIBUTIONS_LIST_ID: &str = "attributions_list_scroll";

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

    /// The summary line the host card renders under the credits header: the
    /// count, or the empty-state line when there is nothing to credit.
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
}

/// Paints `value` on a type rung at its resting weight.
///
/// The single place this window turns a rung into a `text` widget, so no
/// call site restates a point size ([RUST-021], `UIF-TOK-004`).
///
/// `line_height` is passed as `iced::Pixels`, never as a bare `f32`:
/// `From<f32> for iced::LineHeight` yields `Relative(factor)`, a
/// multiple of the font size, so a bare `20.0` would mean twenty lines
/// of leading. See `about::rung` for the measured evidence.
fn rung(
    value: impl Into<String>,
    rung: typography::TypeRung,
    color: iced::Color,
) -> text::Text<'static> {
    text(value.into())
        .size(rung.size)
        .line_height(iced::Pixels(rung.line_height))
        .color(color)
}

/// The credits list, given the [`Length`] the host card says it gets.
///
/// The [`Length`] is solved by the host — see
/// [`crate::ui_iced::about::credits_list_length`] — because only the card
/// knows what it has already spent above and below this scroll area. This
/// module supplies the widget and its stable id; the host supplies the height.
///
/// **`Length::Fixed(solved)`, not `Length::Fill` — and the reason is the
/// §2.2 cap, not a dislike of `Fill`.** `Fill` was the right mechanism for
/// the hazard the spec names: a remainder structurally cannot borrow the
/// card's outer constant, so it cannot overflow past the header and footer
/// bands. It is the wrong mechanism for the *positive* ask, because a
/// remainder has no ceiling — in the retired 480px attributions card it
/// measured **330px** against the spec's **200px** cap.
///
/// iced 0.14 cannot express "fill, but not beyond N": `Length` is
/// `Fill` / `FillPortion` / `Shrink` / `Fixed`, `Container` has no
/// `max_height`, and there is no constraint widget. The cap therefore has
/// to be a **solved fixed height**, which is exactly what `UIF-MOD-004`
/// asks for.
///
/// **Overflow safety is preserved by construction, not by luck.** The
/// solved value is `min(remainder, cap)`, and the remainder is computed
/// from the same band terms the host's `Column` actually consumes, so
/// `solved + bands == the card's inner height` — the list lands exactly
/// where `Fill` would have put it, never further. The safety property that
/// `Fill` gave for free is now carried by the solver, and the sidecar
/// re-measures it at several viewports rather than trusting the argument.
/// **The property is now conditional on the band terms being exact:** an
/// understated band would over-solve and overflow where `Fill` could not
/// have. That is the risk this change introduces, and it is why the layout
/// tests assert the measured list bottom against the measured card bottom.
#[must_use]
pub fn credits_list<'a>(
    table: Element<'a, Message>,
    length: Length,
) -> iced::widget::Scrollable<'a, Message> {
    scrollable(table).id(ATTRIBUTIONS_LIST_ID).height(length)
}

/// The credits table: Crate / Authors / Repository.
///
/// Column widths follow the same rule `ui_iced::logs::log_table` learned
/// the hard way: iced promotes the first all-`Shrink` column to `Fill`
/// (`iced_widget-0.14.2/src/table.rs:129-133`), so the *last* column —
/// the long repository URL — carries `Length::Fill` explicitly and the
/// short name/author columns stay `Shrink`.
///
/// The header row carries the column captions
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
                rung(entry.name, typography::UI_COMPACT, crate_name_color()).into()
            },
        )
        .width(Length::Shrink),
        table::column(
            header_cell(strings::ABOUT_COL_AUTHORS),
            move |entry: Attribution| -> Element<'static, Message> {
                rung(entry.authors, typography::UI_COMPACT, authors_color()).into()
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
        .padding_x(space::XS)
        .padding_y(space::XXS)
        .separator_x(shape::BORDER_HAIRLINE)
        .separator_y(shape::BORDER_HAIRLINE)
        .width(Length::Fill)
        .into()
}

/// A column-header cell.
///
/// `UI_COMPACT` is the dense-table rung (§2.4). The header is
/// distinguished by `text_strong` rather than by weight: the app
/// registers no SemiBold face (see `ui_iced/fonts.rs`), so
/// `UI_COMPACT.emphasis_weight` cannot actually be painted, and colour is
/// the distinction the runtime can honour.
fn header_cell(label: &'static str) -> Element<'static, Message> {
    rung(
        label,
        typography::UI_COMPACT,
        Palette::fastmd_dark().text_strong,
    )
    .into()
}

/// A clickable repository cell, drawn as a `UIF-ATOM-001d` hyperlink.
///
/// The framework ships the hyperlink as a resolver and a geometry, not a
/// rendered widget, so the cell composes them itself:
///
/// * the resting
///   `hoarfrost::widget::hyperlink::HyperlinkStyle::foreground`
///   (`color.accent.text`) and
///   `hoarfrost::widget::hyperlink::HyperlinkStyle::underlined`
///   drive the label — **not** the
///   `accent_fill` colour the cell used before, which measured below the
///   4.5:1 floor against this card's surface;
/// * [`hyperlink::MIN_HEIGHT`] (`size.hit_target.min`) is the hit box,
///   so the cell is a real target rather than a text-height sliver;
/// * [`hyperlink::PADDING_X`] insets the label the way the resolver's
///   `label_bounds` assumes.
///
/// The mount passed to the resolver is the card's own surface, which is
/// what the label is actually drawn on — resolving against a transparent
/// fill would be measuring a colour that is not there.
#[must_use]
pub fn repository_cell(url: &'static str) -> Element<'static, Message> {
    let mount = modal_frame().surface;
    let style = hyperlink::resolve_style(
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
        mount,
        &Palette::pro(),
    );

    let label = rich_text(vec![
        Span::<String>::new(url.to_owned())
            .size(typography::UI_COMPACT.size)
            .color(style.foreground)
            .underline(style.underlined),
    ])
    .width(Length::Fill);

    mouse_area(
        container(label)
            .width(Length::Fill)
            .height(Length::Fixed(hyperlink::MIN_HEIGHT))
            .align_y(Alignment::Center)
            .padding(Padding::ZERO.horizontal(hyperlink::PADDING_X)),
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
/// primary label).
#[must_use]
pub fn crate_name_color() -> iced::Color {
    Palette::fastmd_dark().text_strong
}

/// Palette colour for the authors column (default body `text`).
#[must_use]
pub fn authors_color() -> iced::Color {
    Palette::fastmd_dark().text
}

/// Palette colour for the repository column.
///
/// `color.accent.text`, read through the framework's own hyperlink
/// resolver at rest rather than retyped here — the link's colour is
/// §3.1d's business, and a second copy of it in this module is a second
/// thing that could drift.
#[must_use]
pub fn repository_color() -> iced::Color {
    hyperlink::resolve_style(
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
        modal_frame().surface,
        &Palette::pro(),
    )
    .foreground
}
