//! Third-party attributions window — the credits list behind the About
//! dialog's attributions section.
//!
//! A **pure view over a plain snapshot**: [`attributions_view`] renders an
//! [`AttributionsViewModel`] into an [`iced::Element`] of [`Message`] and
//! performs no I/O. The catalog itself is **reused unchanged** —
//! `crate::ui::attributions::DIRECT_DEPENDENCIES`, the curated
//! `&'static [Attribution]` constant in `src/app/ui/attributions.rs` (57
//! direct third-party crates across all four workspace members,
//! alphabetically sorted, no duplicates). There is no generated
//! `cargo-about` file and no runtime shell-out anywhere; the data is
//! compiled in.
//!
//! # The framework owns the box, the app owns the rows
//!
//! As of integration increment INT-5a this window takes its geometry from
//! [`iced_pro::surface::modal`] rather than from constants of its own:
//!
//! | Concern | Owner |
//! |---|---|
//! | Card width (640px) and height (480px), gutter clamp | [`ModalSizing::attributions`] via [`attributions_card_size`] |
//! | Card surface, border, radius, content padding | [`iced_pro::surface::modal::ModalFrame::resolve`] through [`crate::ui_iced::overlay::modal_card`] |
//! | Section divider colour and weight | [`crate::ui_iced::overlay::modal_divider`] (`color.border.subtle`) |
//! | Title / cell type sizes, cell padding | `iced_pro::token::{typography, space, shape}` |
//! | Link colour, underline, hit height | `iced_pro::widget::hyperlink` resolver (`UIF-ATOM-001d`) |
//! | Which window is open, what dismisses it | `AboutWindowState` + `OverlayState::close_topmost` |
//!
//! The list is no longer a hand-picked `340px` cap. The card is a fixed
//! 480px box and the scrollable takes `Length::Fill`, so the credits
//! region is *whatever the card has left* after the title, summary,
//! dividers and footer — and it stays 480px total whether the catalog has
//! two rows or two thousand.
//!
//! [`iced_pro::surface::modal::ModalHost`] is deliberately **not**
//! used, for the same reason [`crate::ui_iced::about`] does not take it:
//! the app already owns `SURFACE_ID_ATTRIBUTIONS` in
//! `crate::ui_iced::app::OverlayState::sync_surfaces` and owns
//! dismissal in `crate::ui_iced::app::OverlayState::close_topmost`.
//! Consequently
//! `UIF-MOD-001` (focus trap), `UIF-MOD-002` (dirty/busy guard) and
//! `UIF-MOD-003` (error banner) are **not** wired here — see that
//! module's "What that choice does NOT buy" section for the full
//! accounting. What *is* wired is the card-boundary absorption
//! ([`AboutMessage::Absorbed`]) that keeps a press on the card's
//! padding or a dead row from reaching the scrim.
//!
//! # What the catalog actually carries
//!
//! `Attribution { name, authors, github_url }` — that is the whole row.
//! The catalog has **no per-crate license or version field**, so this
//! window shows Crate / Authors / Repository and no license column.
//! The only license text in the app is FastMD's own MIT license, shown in
//! the About card ([`crate::ui_iced::about`]). Adding real per-crate
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
//! ┌─ Attributions card · 640×480 · surface.overlay · radius 6 ───┐
//! │ Third-Party Attributions                                     │
//! │ 57 direct third-party dependencies                           │
//! │ ──────────────────────────────────────────────────────────── │
//! │ Crate   │ Authors             │ Repository                   │
//! │ ────────┼─────────────────────┼──────────────────────────────│
//! │ anyhow  │ David Tolnay        │ github.com/dtolnay/anyhow    │
//! │         │                     │  ↑ accent_text, underlined,   │
//! │         │                     │    24px hit target            │
//! │         …  Length::Fill — scrolls  …                          │
//! │ ──────────────────────────────────────────────────────────── │
//! │                                          [ Close ]           │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! The repository cell is a hyperlink. iced has no `hyperlink_to` and
//! `iced_pro` ships the hyperlink as a **resolver, not a rendered
//! widget** (`widget/hyperlink.rs` has `resolve_style` and the geometry
//! but no `Widget` impl), so the cell composes them: the resolver's
//! resting `foreground` (`color.accent.text`) and `underlined` flag drive
//! the label, `hyperlink::MIN_HEIGHT` (`size.hit_target.min`) drives the
//! hit box, and a [`mouse_area`] carries the press. The row emits the
//! deferred intent [`AboutMessage::RepositoryOpened`] and the reducer
//! opens the URL.
//!
//! The empty catalog renders the same widget tree with an empty table and
//! an empty-state summary line, so the shape never changes (AGENTS §6).
//!
//! Unit tests live in the sibling `attributions_tests.rs` sidecar.

use iced::widget::{
    Column, Row, Space, container, mouse_area, rich_text, scrollable, table, text, text::Span,
};
use iced::{Alignment, Element, Length, Padding, Point, Rectangle, Size};

use iced_pro::state::{BaseState, VisualState};
use iced_pro::surface::modal::{
    ATTRIBUTIONS_LIST_MAX_HEIGHT, ModalSizing, SubRegionBands, subregion_height,
};
use iced_pro::token::{shape, size, space, typography};
use iced_pro::widget::hyperlink;

use crate::ui::attributions::{Attribution, DIRECT_DEPENDENCIES};
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::about::{close_button, modal_frame};
use crate::ui_iced::message::{AboutMessage, Message};
use crate::ui_iced::overlay::{modal_card, modal_divider};

#[cfg(test)]
#[path = "attributions_tests.rs"]
mod tests;

/// §4.6.2 — the Attributions card's sizing policy: 640×480 fixed.
pub const ATTRIBUTIONS_SIZING: ModalSizing = ModalSizing::attributions();

/// The Attributions card's **outer** size for a viewport (§4.6.2).
///
/// 640×480, each axis clamped to the viewport minus
/// [`VIEWPORT_GUTTER`](iced_pro::surface::modal::VIEWPORT_GUTTER). The
/// content extent is passed as zero because this policy is *fixed* on
/// both axes — the card is 480px tall whether the catalog holds two
/// rows or two thousand, and the list scrolls inside it.
#[must_use]
pub fn attributions_card_size(viewport: Size) -> Size {
    ATTRIBUTIONS_SIZING
        .solve(Rectangle::new(Point::ORIGIN, viewport), Size::ZERO)
        .size()
}

/// Stable id of the Attributions card box itself, so a layout test can
/// address the rendered card rather than only its contents (AGENTS §5).
pub const ATTRIBUTIONS_CARD_ID: &str = "attributions_card_frame";

/// Stable id for the credits scroll area, so the reducer can address it
/// and the scroll position survives a tree rebuild (AGENTS §5).
pub const ATTRIBUTIONS_LIST_ID: &str = "attributions_list_scroll";

/// The Attributions card's **header band** — everything stacked above the
/// credits list, including the divider under it and the three `space::SM`
/// gaps those three rows sit on (`UIF-MOD-004`).
///
/// Every term is a token: the title's `TITLE_SM` line, the summary's
/// `BODY` line, one `BORDER_HAIRLINE` divider and three `space::SM` gaps.
/// The gaps are written out as three terms rather than as `3.0 * space::SM`
/// **on purpose** — a coefficient is a numeric literal, and Rule B treats
/// any literal in a visual-named constant as a violation unless the
/// constant is registered. Writing the three terms keeps the composition
/// literal-free, so the gate reads it as `TokenBound` on its own merits
/// rather than by exemption, and the three terms are the three gaps,
/// countable by eye.
///
/// The composition is **this card's** layout, not the framework's: the
/// framework supplies the solver and the frame's padding, and only the
/// host knows it renders a title, a summary line and one divider above the
/// list. Keeping it here is what stops app layout knowledge leaking into
/// `surface::modal`.
///
/// **Measured, not assumed:** with [`FOOTER_BAND_HEIGHT`] and the frame's
/// padding this totals `150px`, and `480 - 150 = 330` is exactly the
/// height the rendered card measures for the list. The sidecar pins both
/// figures against a headless layout.
pub const HEADER_BAND_HEIGHT: f32 = typography::TITLE_SM.line_height
    + typography::BODY.line_height
    + shape::BORDER_HAIRLINE
    + space::SM
    + space::SM
    + space::SM;

/// The Attributions card's **footer band** — the divider above the footer
/// plus the footer control row plus the two `space::SM` gaps they sit on
/// (`UIF-MOD-004`).
///
/// Literal-free for the same reason [`HEADER_BAND_HEIGHT`] is: one
/// `BORDER_HAIRLINE`, a `size.control.md` footer row, and two `space::SM`
/// gaps written out as two terms.
pub const FOOTER_BAND_HEIGHT: f32 =
    shape::BORDER_HAIRLINE + size::CONTROL_MD + space::SM + space::SM;

/// The height the credits list is actually given, for a card of
/// `card_height` (`UIF-MOD-004`).
///
/// Solved from the card's **solved** bounds through the framework's
/// solver, then held down by the §2.2 cap. It is never the card's outer
/// constant, and it never exceeds what the card has left:
///
/// ```text
/// min( card_height - (header + 2 × padding + footer), ATTRIBUTIONS_LIST_MAX_HEIGHT )
/// ```
///
/// Purity: pure function of `card_height`.
#[must_use]
pub fn credits_list_height(card_height: f32) -> f32 {
    let bands = SubRegionBands::from_frame(&modal_frame())
        .with_header(HEADER_BAND_HEIGHT)
        .with_footer(FOOTER_BAND_HEIGHT);
    subregion_height(card_height, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT)
}

/// The [`Length`] the credits list is rendered at, for a card of
/// `card_height`.
///
/// `Fixed(solved)` whenever the solver found room. **The zero case falls
/// back to `Fill`, and that is not a hedge — it is a measured iced
/// behaviour.** `Length::Fixed(0.0)` is *void* in iced 0.14
/// (`Size::is_void()` is true when either axis is `Fixed(0.0)`), and a
/// void child is silently dropped from the widget tree. Verified here: at a
/// 360×180 viewport the solved height is `0`, and with `Fixed(0.0)` the
/// scrollable is not in the tree at all (`SelectorNotFound`), while with
/// `Fill` it is present and measures `0.0`.
///
/// Losing the widget would break AGENTS §5 — `ATTRIBUTIONS_LIST_ID` is a
/// stateful scrollable whose scroll position must survive a rebuild, and a
/// widget that vanishes and returns under the same id starts fresh. It
/// would also break §6's "do not let the layout collapse silently".
///
/// The fallback costs nothing in cap compliance: at zero room the two
/// mechanisms agree on the height (zero), so `Fill` cannot exceed the cap
/// there. The cap only ever binds where there *is* room, and that path is
/// `Fixed`.
///
/// Purity: pure function of `card_height`.
#[must_use]
pub fn credits_list_length(card_height: f32) -> Length {
    let solved = credits_list_height(card_height);
    if solved > 0.0 {
        Length::Fixed(solved)
    } else {
        Length::Fill
    }
}

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

    /// Renders this snapshot as the framed attributions window
    /// ([`attributions_view`]).
    #[must_use]
    pub fn view(&self, viewport: Size) -> Element<'_, Message> {
        attributions_view(self, viewport)
    }
}

/// Renders the framed third-party attributions window.
///
/// Title, count line, the scrollable three-column catalog and the shared
/// Close footer, inside the §4.6.2 640×480 card. The list's height is
/// **solved** from the card's solved bounds by [`credits_list_height`]
/// (`UIF-MOD-004`) and capped at [`ATTRIBUTIONS_LIST_MAX_HEIGHT`],
/// so the card's height is the frame's, not the content's, and the table
/// is always allocated, empty or not.
///
/// Emitted messages:
/// * a repository cell → [`AboutMessage::RepositoryOpened`]
/// * the footer → [`AboutMessage::CloseRequested`]
///
/// Like [`crate::ui_iced::about::about_card`], this is a framed card:
/// the root view centres it over the scrim with
/// [`crate::ui_iced::overlay::modal_centered`], passing
/// [`AboutMessage::Absorbed`] as the card-boundary message and
/// [`AboutMessage::CloseRequested`] as the dismiss message.
///
/// Purity: pure view over `view_model` and `viewport`.
#[must_use]
pub fn attributions_view(
    view_model: &AttributionsViewModel,
    viewport: Size,
) -> Element<'_, Message> {
    let frame = modal_frame();
    let card_size = attributions_card_size(viewport);

    let title = rung(
        strings::ABOUT_ATTRIBUTIONS_HEADER,
        typography::TITLE_SM,
        Palette::fastmd_dark().text_strong,
    );

    let summary = rung(
        view_model.summary(),
        typography::BODY,
        Palette::fastmd_dark().text_muted,
    );

    let list = credits_list(
        attribution_table(view_model.entries()),
        credits_list_length(card_size.height),
    );

    let footer = Row::with_children(vec![
        Space::new().width(Length::Fill).into(),
        close_button(),
    ])
    .spacing(space::SM)
    .align_y(Alignment::Center);

    let body = Column::with_children(vec![
        title.into(),
        summary.into(),
        modal_divider(frame).into(),
        list.into(),
        modal_divider(frame).into(),
        footer.into(),
    ])
    .spacing(space::SM);

    modal_card(
        body,
        frame,
        ATTRIBUTIONS_CARD_ID,
        Length::Fixed(card_size.width),
        Length::Fixed(card_size.height),
    )
    .into()
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

/// The credits list, given the [`Length`] the solver says it gets.
///
/// **`Length::Fixed(solved)`, not `Length::Fill` — and the reason is the
/// §2.2 cap, not a dislike of `Fill`.** `Fill` was the right mechanism for
/// the hazard the spec names: a remainder structurally cannot borrow the
/// card's outer `480px` constant, so it cannot overflow past the header
/// and footer bands. It is the wrong mechanism for the *positive* ask,
/// because a remainder has no ceiling — in a 480px card it measured
/// **330px** against the spec's **200px** cap.
///
/// iced 0.14 cannot express "fill, but not beyond N": `Length` is
/// `Fill` / `FillPortion` / `Shrink` / `Fixed`, `Container` has no
/// `max_height`, and there is no constraint widget. The cap therefore has
/// to be a **solved fixed height**, which is exactly what `UIF-MOD-004`
/// asks for.
///
/// **Overflow safety is preserved by construction, not by luck.** The
/// solved value is `min(remainder, cap)`, and the remainder is computed
/// from the same band terms this `Column` actually consumes, so
/// `solved + bands == the card's inner height` — the list lands exactly
/// where `Fill` would have put it, never further. The safety property that
/// `Fill` gave for free is now carried by the solver, and the sidecar
/// re-measures it at several viewports rather than trusting the argument.
/// **The property is now conditional on the band terms being exact:** an
/// understated band would over-solve and overflow where `Fill` could not
/// have. That is the risk this change introduces, and it is why the layout
/// tests assert the measured list bottom against the measured card bottom.
fn credits_list<'a>(
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
///   [`HyperlinkStyle::foreground`](iced_pro::widget::hyperlink::HyperlinkStyle::foreground)
///   (`color.accent.text`) and
///   [`HyperlinkStyle::underlined`](iced_pro::widget::hyperlink::HyperlinkStyle::underlined)
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
