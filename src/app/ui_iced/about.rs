//! About window — application identity, build metadata, the MIT license
//! and the entry point into the third-party attributions list.
//!
//! A **pure view over a plain snapshot**: [`about_card`] renders an
//! [`AboutViewModel`] into an [`iced::Element`] of [`Message`] and touches
//! neither the bus, the disk nor `FastMdApp`. The reducer owns the
//! open/close state ([`AboutWindowState`]) and performs the two side
//! effects the card only requests.
//!
//! # The framework owns the box, the app owns the answer
//!
//! As of integration increment INT-5a this module is a consumer of
//! [`iced_pro::surface::modal`] rather than a re-implementation of
//! §4.6.2. Nothing here carries a card measurement of its own:
//!
//! | Concern | Owner |
//! |---|---|
//! | Card width, gutter clamp | [`ModalSizing::about`] via [`about_card_width`] |
//! | Card surface, border, radius, padding | [`ModalFrame::resolve`] through [`crate::ui_iced::overlay::modal_card`] |
//! | Section divider colour and weight | [`crate::ui_iced::overlay::modal_divider`] (`color.border.subtle`) |
//! | Type sizes, gaps, glyph size | `iced_pro::token::{typography, space, size, shape}` |
//! | Footer / teaser button geometry and states | [`iced_pro::widget::button_widget::Button`] |
//! | Which window is open, what dismisses it | [`AboutWindowState`] + `OverlayState::close_topmost` |
//!
//! [`iced_pro::surface::modal::ModalHost`] is deliberately **not** used,
//! for the same reason [`crate::ui_iced::dialogs`] did not take it: the
//! host self-registers its surface in a
//! [`iced_pro::surface::zorder::SurfaceRegistry`] and pushes its own
//! focus trap, while this app already owns `SURFACE_ID_ABOUT` in
//! [`crate::ui_iced::app::OverlayState::sync_surfaces`] and owns
//! dismissal in [`crate::ui_iced::app::OverlayState::close_topmost`].
//! Two owners of one surface id is the collision INT-4 settled; the app
//! keeps both. See that module's docs for the full reasoning.
//!
//! # What that choice does NOT buy (`UIF-MOD-001..003`)
//!
//! Stated plainly, because the framework docs describe a host and this
//! card is not wired to it:
//!
//! * **`UIF-MOD-001` (focus trap + restoration) — NOT provided.** The
//!   app captures only `Escape` off its keyboard stream
//!   (`ui_iced/subscription.rs`) and leaves `Tab` to iced's built-in
//!   router, which knows nothing about a trap. Nothing would feed
//!   `FocusState::push_trap` the `Tab` events it needs, so pushing one
//!   here would be a trap on paper only. Focus can still reach the
//!   panes under the scrim.
//! * **`UIF-MOD-002` (unsaved-edit / busy guard) — NOT provided, and
//!   nothing is lost.** The About card has no editable field and no
//!   in-flight action, so `dirty` and `busy` are permanently `false` and
//!   the guard could never refuse a dismissal.
//! * **`UIF-MOD-003` (structured error banner) — NOT provided.** This
//!   card has no failing action: every field is compile-time data and the
//!   two effects it requests (clipboard write, open credits) are not
//!   modelled as failures. `ModalError` therefore never appears here.
//!
//! What the card *does* provide is the two dismissal paths the app owns
//! (`Escape` and scrim click, both routed to
//! [`AboutMessage::CloseRequested`]) plus the card-boundary absorption
//! [`AboutMessage::Absorbed`] that stops a press on the padding ring or
//! a dead control from reaching the scrim.
//!
//! # Data sources (all compile-time — no render-time I/O)
//!
//! | Field | Source |
//! | --- | --- |
//! | App name / copyright / description | `crate::ui::strings` |
//! | Version | [`APP_VERSION`] — the `fastmd` package version constant (`env!("CARGO_PKG_VERSION")`), owned here |
//! | Branch / commit / build date | the root `build.rs` `cargo:rustc-env` vars, read with `env!` |
//! | License body | the shared license reflow [`license_display_text`] over `include_str!("../../../LICENSE")` |
//! | Attribution count | [`crate::ui::attributions::DIRECT_DEPENDENCIES`] — the curated catalog also rendered by [`crate::ui_iced::attributions`] |
//!
//! # Banner: a glyph, not the PNG
//!
//! `Cargo.toml` enables iced's `wgpu`, `tiny-skia`, `fira-sans`,
//! `advanced-shaping` and `markdown` features only, so
//! `iced::widget::image` and `iced::widget::svg` **do not exist** in this
//! build. The compiled-in `assets/icon-32.png`
//! ([`crate::utils::app_icon::APP_ICON_PNG`]) therefore cannot be shown;
//! the banner is the shared "Document + Bolt" stand-in from
//! [`crate::ui_iced::logo`] — the same mark the top bar renders. That
//! module records the raster / SVG / `canvas` trade-off; enabling any of
//! those features is a build-level decision, not this module's.
//!
//! # Message channel
//!
//! These windows ride their **own** [`Message::About`] channel rather than
//! the [`crate::ui_iced::dialogs`] one. `DialogMessage`'s vocabulary is
//! name-entry shaped (`NameChanged`, `DestinationSelected`, `Submitted`)
//! and none of it applies here, while the About vocabulary
//! ([`AboutMessage`]) has nothing in common with it; sharing one channel
//! would force two unrelated state models into `DialogState`. Opening the
//! dialog itself stays user intent on the command channel — the hamburger
//! already emits `UserCommand::OpenAboutDialog`
//! (`ui_iced/panels/top.rs:439`), which the reducer maps to
//! [`AboutWindowState::open_about`].
//!
//! # Design notes
//!
//! * **Attributions live elsewhere.** The card keeps only the count plus a
//!   [`strings::ABOUT_VIEW_ATTRIBUTIONS_BUTTON`] that opens
//!   [`crate::ui_iced::attributions`], so the credits list gets a
//!   dedicated, wider window.
//! * **Copy-to-clipboard is deferred intent.** iced exposes the clipboard
//!   only to `update` (`iced_core-0.14.0/src/clipboard.rs`), so the
//!   commit cell emits [`AboutMessage::CopyCommitHashRequested`] and the
//!   reducer performs the write.
//! * **No hover tooltip.** iced 0.14 has no built-in tooltip widget and
//!   `iced_aw` is not a dependency, so the full-hash reveal cannot be a
//!   tooltip. [`AboutViewModel::commit_tooltip`] still produces the text
//!   for the reducer to surface another way.
//! * **Unknown build metadata stays inert** (spec FR-007): the commit cell
//!   is always allocated, it simply gets no press handler — see
//!   [`commit_copy_press`].
//!
//! Unit tests live in the sibling `about_tests.rs` sidecar.

use iced::widget::{Column, Row, Space, mouse_area, scrollable, text};
use iced::{Alignment, Element, Length, Size};

use iced_pro::surface::modal::{ModalFrame, ModalSizing};
use iced_pro::token::{size, space, typography};
use iced_pro::widget::button::ButtonVariant;
use iced_pro::widget::button_widget::Button;

use crate::ui::attributions::DIRECT_DEPENDENCIES;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::logo;
use crate::ui_iced::message::{AboutMessage, Message};
use crate::ui_iced::overlay::{modal_card, modal_divider};

#[cfg(test)]
#[path = "about_tests.rs"]
mod tests;

/// §4.6.2 — the About card's sizing policy: 440px wide, content-fit.
///
/// Held as the policy object rather than a number, so the card states
/// *which* modal it is and the framework does the clamping.
pub const ABOUT_SIZING: ModalSizing = ModalSizing::about();

/// The About card's **outer** width for a viewport width (§4.6.2).
///
/// 440px on any viewport wide enough to hold it, clamped to
/// `viewport.width - VIEWPORT_GUTTER` (`space::XXL`, 32px) below that.
/// The card's padding is *inside* this figure — see
/// [`crate::ui_iced::overlay::modal_card`].
#[must_use]
pub fn about_card_width(viewport_width: f32) -> f32 {
    ABOUT_SIZING.width.resolve(viewport_width, 0.0)
}

/// The frame this card is drawn with (§4.6.1).
///
/// Resolved from the app's framework palette on each call rather than
/// cached: [`ModalFrame::resolve`] is a `const fn` over a `Copy` value,
/// and reading it at the call site keeps the frame visibly owned by the
/// framework instead of copied into this module's own constants.
#[must_use]
pub fn modal_frame() -> ModalFrame {
    ModalFrame::resolve(&Palette::pro())
}

/// Maximum height of the license scroll area, in logical pixels.
///
/// **A token gap, reported rather than papered over.** §4.6.2 gives the
/// About card a *content-fit* height policy with no ceiling, and the
/// license body is the only unbounded thing in it: reflowed at the
/// card's 440px width the MIT text runs to roughly 26 lines, so left
/// uncapped the card would grow past 800px tall. The framework has no
/// token for "how tall one scroll region inside a card may get" —
/// [`iced_pro::surface::modal::CONTENT_HEIGHT_MAX`] is the tallest a
/// *whole* content-fit card body may be (480px), which would still make
/// this card ~710px tall.
///
/// So the cap stays an app-owned value. It is a layout decision about
/// one region of one card, not a design token, and it is deliberately
/// not stretched to a framework constant that means something else.
pub const LICENSE_MAX_HEIGHT: f32 = 200.0;

/// Stable id of the About card box itself, so a layout test (and the
/// reducer, if it ever needs to) can address the rendered card rather
/// than only its contents (AGENTS §5).
pub const ABOUT_CARD_ID: &str = "about_card_frame";

/// Stable id for the license scroll area, so the reducer can address it
/// and the scroll position survives a tree rebuild (AGENTS §5).
pub const LICENSE_SCROLL_ID: &str = "about_license_scroll";

/// The sentinel `build.rs` writes when git metadata is unavailable.
pub const UNKNOWN_BUILD_METADATA: &str = "unknown";

/// Current application version, from the `fastmd` package manifest
/// ([`env!`] `CARGO_PKG_VERSION`). Owned by this module.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Full MIT license text bundled at compile time.
const LICENSE_TEXT: &str = include_str!("../../../LICENSE");

/// Reflows the hard-wrapped `LICENSE_TEXT` (fixed ~78-char lines) into
/// flowing paragraphs so the label wraps at the available width and the
/// license section fills the card like every other section. Single
/// newlines become spaces; blank lines stay as paragraph breaks; the word
/// sequence is preserved verbatim.
///
/// Pure; owned by this module.
#[must_use]
pub fn license_display_text() -> String {
    LICENSE_TEXT
        .split("\n\n")
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Decides whether the About dialog should open automatically at startup
/// (spec FR-016): `None` (fresh state, or state written before the flag
/// existed) or a version mismatch auto-shows; a match stays quiet.
#[must_use]
pub fn should_auto_show_about(recorded: Option<&str>, current: &str) -> bool {
    match recorded {
        None => true,
        Some(version) => version != current,
    }
}

/// Applies the first-run auto-show decision to startup state (spec FR-016).
///
/// When [`should_auto_show_about`] holds for the recorded version, opens
/// the dialog and records `current` so later starts stay quiet.
/// Returns `true` when the dialog was auto-opened.
pub fn apply_first_run_auto_show(
    persisted: &mut crate::ui::PersistedUiState,
    dialogs: &mut crate::ui::Dialogs,
    current: &str,
) -> bool {
    if should_auto_show_about(persisted.about_shown_for_version.as_deref(), current) {
        dialogs.about_dialog_open = true;
        persisted.about_shown_for_version = Some(current.to_owned());
        true
    } else {
        false
    }
}

/// The snapshot the About card renders — application identity plus build
/// metadata plus the license body.
///
/// Plain data with public fields so a caller (or a test) can build a card
/// around any metadata it likes; [`AboutViewModel::from_build`] is the
/// production snapshot taken from compile-time constants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AboutViewModel {
    /// Application name ([`strings::ABOUT_APP_NAME`]).
    pub app_name: &'static str,
    /// Package version, from `env!("CARGO_PKG_VERSION")`.
    pub version: &'static str,
    /// One-line product description ([`strings::ABOUT_DESCRIPTION`]).
    pub description: &'static str,
    /// Copyright notice ([`strings::ABOUT_COPYRIGHT`]).
    pub copyright: &'static str,
    /// Git branch captured by `build.rs`, or [`UNKNOWN_BUILD_METADATA`].
    pub branch: &'static str,
    /// Full commit hash, or [`UNKNOWN_BUILD_METADATA`].
    pub commit_hash: &'static str,
    /// Short (7-8 char) commit hash for display, or
    /// [`UNKNOWN_BUILD_METADATA`].
    pub commit_short_hash: &'static str,
    /// Build date in `YYYY-MM-DD` form, or [`UNKNOWN_BUILD_METADATA`].
    pub build_date: &'static str,
    /// The MIT license body, re-flowed into flowing paragraphs so the
    /// label wraps at the card width instead of the file's hard wrap.
    pub license_text: String,
    /// How many crates the third-party attribution catalog carries.
    pub attribution_count: usize,
}

impl AboutViewModel {
    /// Takes the production snapshot from the compile-time identity,
    /// `build.rs` metadata and the reused license reflow.
    ///
    /// Purity: pure — every field is a compile-time constant or a pure
    /// function of one. No I/O at render time ([RUST-020]).
    #[must_use]
    pub fn from_build() -> Self {
        Self {
            app_name: strings::ABOUT_APP_NAME,
            version: APP_VERSION,
            description: strings::ABOUT_DESCRIPTION,
            copyright: strings::ABOUT_COPYRIGHT,
            branch: env!("BUILD_BRANCH"),
            commit_hash: env!("BUILD_COMMIT_HASH"),
            commit_short_hash: env!("BUILD_COMMIT_SHORT_HASH"),
            build_date: env!("BUILD_DATE"),
            license_text: license_display_text(),
            attribution_count: DIRECT_DEPENDENCIES.len(),
        }
    }

    /// Whether `build.rs` could not read git and fell back to
    /// [`UNKNOWN_BUILD_METADATA`] (spec FR-007) — the commit cell then
    /// renders without a copy affordance.
    #[must_use]
    pub fn is_build_metadata_unknown(&self) -> bool {
        self.commit_hash == UNKNOWN_BUILD_METADATA
    }

    /// The version row text: `Version: 0.2.0`.
    #[must_use]
    pub fn version_line(&self) -> String {
        format!("{} {}", strings::ABOUT_VERSION_LABEL, self.version)
    }

    /// The full-hash reveal line: the full commit hash plus the copy hint.
    /// iced 0.14 has no tooltip widget, so this is text the reducer may
    /// surface elsewhere — it is not rendered by [`about_card`].
    #[must_use]
    pub fn commit_tooltip(&self) -> String {
        format!(
            "{} {}\n{}",
            strings::ABOUT_FULL_COMMIT_PREFIX,
            self.commit_hash,
            strings::ABOUT_COPY_COMMIT_TOOLTIP
        )
    }

    /// The attributions teaser line (`57 direct third-party
    /// dependencies`).
    #[must_use]
    pub fn attributions_summary(&self) -> String {
        strings::format_attribution_count(self.attribution_count)
    }

    /// Renders this snapshot as the framed About card ([`about_card`]).
    #[must_use]
    pub fn view(&self, viewport: Size) -> Element<'_, Message> {
        about_card(self, viewport)
    }
}

/// Renders the framed About card at the §4.6.2 size.
///
/// Layout, with the attributions section rendered as a teaser button:
///
/// ```text
/// ┌─ About card · 440px outer · surface.overlay · radius 6 ─────┐
/// │ [⚡]  FastMD Viewer                                         │
/// │       Copyright (c) 2026 Martin Kuhne                       │
/// │ Version: 0.2.0                                              │
/// │ <description>                                               │
/// │ ─────────────────────────────────────────────────────────── │
/// │ Branch: chore/x   Commit: 1a2b3c4d   Built: 2026-10-18      │
/// │ ─────────────────────────────────────────────────────────── │
/// │ License                                                     │
/// │ ┌ MIT License … (scrollable, 200px cap) ───────────────────┐│
/// │ ─────────────────────────────────────────────────────────── │
/// │ Third-Party Attributions                                    │
/// │ 57 direct third-party dependencies                          │
/// │ [ View third-party attributions... ]                         │
/// │ ─────────────────────────────────────────────────────────── │
/// │                                        [ Close ]            │
/// └─────────────────────────────────────────────────────────────┘
/// ```
///
/// Emitted messages:
/// * commit cell → [`AboutMessage::CopyCommitHashRequested`] (only when
///   the hash is known — see [`commit_copy_press`])
/// * the teaser button → [`AboutMessage::AttributionsRequested`]
/// * the footer → [`AboutMessage::CloseRequested`]
///
/// The card is **framed but not mounted**: it carries its own §4.6.1
/// chrome and outer width, and the root view centres it over the scrim
/// with [`crate::ui_iced::overlay::modal_centered`], passing
/// [`AboutMessage::Absorbed`] as the card-boundary message and
/// [`AboutMessage::CloseRequested`] as the dismiss message.
///
/// Every row is always allocated — the unknown-build and empty-catalog
/// states change text and press handlers, never the widget count (AGENTS
/// §6).
///
/// Purity: pure view over `view_model` and `viewport`.
#[must_use]
pub fn about_card(view_model: &AboutViewModel, viewport: Size) -> Element<'_, Message> {
    let frame = modal_frame();
    let card_width = about_card_width(viewport.width);

    let identity = Column::with_children(vec![
        rung(
            view_model.app_name,
            typography::TITLE_MD,
            Palette::fastmd_dark().text_strong,
        )
        .into(),
        rung(
            view_model.copyright,
            typography::BODY,
            Palette::fastmd_dark().text_muted,
        )
        .into(),
    ])
    .spacing(space::XXS);

    let header = Row::with_children(vec![banner_mark(), identity.into()])
        .spacing(space::LG)
        .align_y(Alignment::Center);

    let facts = Column::with_children(vec![
        rung(
            view_model.version_line(),
            typography::BODY,
            Palette::fastmd_dark().text_value,
        )
        .into(),
        rung(
            view_model.description,
            typography::BODY,
            Palette::fastmd_dark().text,
        )
        .into(),
    ])
    .spacing(space::XXS);

    let body = Column::with_children(vec![
        header.into(),
        facts.into(),
        modal_divider(frame).into(),
        build_metadata_row(view_model),
        modal_divider(frame).into(),
        license_section(view_model),
        modal_divider(frame).into(),
        attributions_teaser(view_model),
        modal_divider(frame).into(),
        about_footer(),
    ])
    .spacing(space::SM);

    modal_card(
        body,
        frame,
        ABOUT_CARD_ID,
        Length::Fixed(card_width),
        Length::Shrink,
    )
    .into()
}

/// Paints `value` on a type rung at its resting weight.
///
/// The single place this card turns a rung into a `text` widget, so no
/// call site restates a point size ([RUST-021], `UIF-TOK-004`).
///
/// **`line_height` must be handed to iced as `Pixels`, not as a bare
/// `f32`.** `impl From<f32> for iced::LineHeight` produces
/// `LineHeight::Relative(factor)` — a *multiple* of the font size
/// (`iced_core-0.14.0/src/text.rs:221-225`) — so `.line_height(20.0)`
/// on a 14px rung means 20 × 14 = **280px per line**, not 20px. Only
/// `From<Pixels>` reaches `LineHeight::Absolute`. Measured in the
/// sidecar: the bare-`f32` form reported 280px for one BODY line, the
/// `Pixels` form reports 20px.
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

/// The glyph the About banner renders.
///
/// Delegated to [`crate::ui_iced::logo::logo_glyph`] — the About card does
/// not own its own mark ([RUST-051]).
#[must_use]
pub fn banner_glyph() -> &'static str {
    logo::logo_glyph()
}

/// The rendered banner mark (top panel of the card).
#[must_use]
pub fn banner_mark() -> Element<'static, Message> {
    logo::logo_mark(banner_size())
}

/// The point size the banner mark renders at.
///
/// `size.icon.lg` ([`size::ICON_LG`]) — the token whose declared role is
/// the brand mark. The value this replaced was 48px, which no token in the
/// icon scale covered; 20px is the nearest real rung, and taking it is
/// what keeps the banner on the scale instead of beside it.
#[must_use]
pub const fn banner_size() -> f32 {
    size::ICON_LG
}

/// The Close button shared by the About and attributions cards.
///
/// A framework [`Button`] in the `Secondary` variant: `Close` dismisses
/// the window without confirming or committing anything, which is the
/// same role the input dialogs' `Cancel` plays, and leaves the About
/// card's primary variant free for the forward action.
#[must_use]
pub fn close_button() -> Element<'static, Message> {
    footer_button(
        ButtonVariant::Secondary,
        strings::CLOSE_BUTTON,
        Message::About(AboutMessage::CloseRequested),
        ABOUT_CLOSE_BUTTON_ID,
    )
}

/// Stable focus id of the About / attributions `Close` button.
pub const ABOUT_CLOSE_BUTTON_ID: &str = "about_close";

/// Stable focus id of the About card's "view attributions" button.
pub const ABOUT_VIEW_ATTRIBUTIONS_BUTTON_ID: &str = "about_view_attributions";

/// A footer-width framework button carrying `label`.
///
/// The label is mounted in a fixed
/// [`BUTTON_MIN_WIDTH`](iced_pro::surface::input_dialog::BUTTON_MIN_WIDTH)
/// box on purpose. `Button::is_icon_only` is a **geometric** test — any
/// content no wider and no taller than `size::CONTROL_SM` (28px) is
/// classed icon-only, and an icon-only button without a tooltip trips a
/// `debug_assert!` in `outer_size` (`button_widget.rs:776`). A short
/// label like `'Close'` sits close enough to that line that a different
/// font measurement would cross it, so the label is given a width the
/// test cannot fail.
fn footer_button(
    variant: ButtonVariant,
    label: &'static str,
    on_press: Message,
    focus_id: &'static str,
) -> Element<'static, Message> {
    let content = Column::new()
        .width(Length::Fixed(
            iced_pro::surface::input_dialog::BUTTON_MIN_WIDTH,
        ))
        .align_x(Alignment::Center)
        .push(rung(
            label,
            typography::BODY,
            Palette::fastmd_dark().text_strong,
        ));

    Button::new(variant, content, on_press, focus_id)
        .palette(Palette::pro())
        .into()
}

/// The card footer: spacer plus right-aligned Close button.
#[must_use]
pub fn about_footer() -> Element<'static, Message> {
    Row::with_children(vec![
        Space::new().width(Length::Fill).into(),
        close_button(),
    ])
    .spacing(space::SM)
    .align_y(Alignment::Center)
    .into()
}

/// The Branch / Commit / Built row.
fn build_metadata_row(view_model: &AboutViewModel) -> Element<'_, Message> {
    let muted = |value: String| rung(value, typography::BODY, Palette::fastmd_dark().text_muted);

    Row::with_children(vec![
        muted(strings::ABOUT_BRANCH_LABEL.to_owned()).into(),
        muted(view_model.branch.to_owned()).into(),
        muted(strings::ABOUT_COMMIT_LABEL.to_owned()).into(),
        commit_cell(view_model),
        muted(strings::ABOUT_DATE_LABEL.to_owned()).into(),
        muted(view_model.build_date.to_owned()).into(),
    ])
    .spacing(space::SM)
    .align_y(Alignment::Center)
    .into()
}

/// The commit-hash cell: always allocated, and pressable only when there
/// is a real hash to copy (spec FR-007 fallback).
fn commit_cell(view_model: &AboutViewModel) -> Element<'_, Message> {
    let cell = rung(
        view_model.commit_short_hash,
        typography::BODY,
        Palette::fastmd_dark().text_value,
    );

    let area = mouse_area(cell);
    match commit_copy_press(view_model.is_build_metadata_unknown()) {
        Some(message) => area.on_press(message).into(),
        None => area.into(),
    }
}

/// The press handler the commit cell gets: `Some` only when the build
/// metadata is known, because an `"unknown"` build has nothing
/// meaningful to copy.
///
/// `MouseArea` has no `on_press_maybe` (`iced_widget-0.14.2/src/mouse_area.rs:38`),
/// so the option is resolved here and the caller attaches the handler only
/// when present — the widget count never changes.
///
/// Purity: pure function of `metadata_unknown`.
#[must_use]
pub fn commit_copy_press(metadata_unknown: bool) -> Option<Message> {
    (!metadata_unknown).then_some(Message::About(AboutMessage::CopyCommitHashRequested))
}

/// The license header plus its capped, scrollable body.
fn license_section(view_model: &AboutViewModel) -> Element<'_, Message> {
    let header = rung(
        strings::ABOUT_LICENSE_HEADER,
        typography::TITLE_SM,
        Palette::fastmd_dark().text_strong,
    );

    let body = scrollable(
        rung(
            &view_model.license_text,
            typography::BODY,
            Palette::fastmd_dark().text,
        )
        .width(Length::Fill),
    )
    .id(LICENSE_SCROLL_ID)
    .height(Length::Fixed(LICENSE_MAX_HEIGHT));

    Column::with_children(vec![header.into(), body.into()])
        .spacing(space::SM)
        .into()
}

/// The attributions header, count line and the button that opens the
/// credits window ([`crate::ui_iced::attributions`]).
fn attributions_teaser(view_model: &AboutViewModel) -> Element<'_, Message> {
    let header = rung(
        strings::ABOUT_ATTRIBUTIONS_HEADER,
        typography::TITLE_SM,
        Palette::fastmd_dark().text_strong,
    );

    let summary = rung(
        view_model.attributions_summary(),
        typography::BODY,
        Palette::fastmd_dark().text_muted,
    );

    let open = footer_button(
        ButtonVariant::Primary,
        strings::ABOUT_VIEW_ATTRIBUTIONS_BUTTON,
        Message::About(AboutMessage::AttributionsRequested),
        ABOUT_VIEW_ATTRIBUTIONS_BUTTON_ID,
    );

    Column::with_children(vec![header.into(), summary.into(), open])
        .spacing(space::SM)
        .into()
}

/// Which of the two identity windows is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutWindow {
    /// The About card ([`about_card`]).
    About,
    /// The third-party attributions list
    /// ([`crate::ui_iced::attributions::attributions_view`]).
    Attributions,
}

impl AboutWindow {
    /// The window title, from `crate::ui::strings`.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::About => strings::ABOUT_DIALOG_TITLE,
            Self::Attributions => strings::ABOUT_ATTRIBUTIONS_HEADER,
        }
    }

    /// The §4.6.2 sizing policy this window's card is mounted with.
    #[must_use]
    pub const fn sizing(&self) -> ModalSizing {
        match self {
            Self::About => ModalSizing::about(),
            Self::Attributions => ModalSizing::attributions(),
        }
    }
}

/// Which identity window is open — the pure state the reducer holds.
///
/// One window at a time, the same one-at-a-time rule
/// [`crate::ui_iced::dialogs::DialogState`] applies: opening replaces
/// whatever was showing, which is also what
/// [`crate::ui_iced::overlay::OverlaySlot::open_modal`] enforces, so a
/// reducer that wants the single-blocking-overlay invariant can mirror
/// this state into `OverlaySlot<AboutWindow>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AboutWindowState {
    open: Option<AboutWindow>,
}

impl AboutWindowState {
    /// A state with neither window open.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether either window is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Whether the About card is showing.
    #[must_use]
    pub fn is_about(&self) -> bool {
        self.open == Some(AboutWindow::About)
    }

    /// Whether the attributions window is showing.
    #[must_use]
    pub fn is_attributions(&self) -> bool {
        self.open == Some(AboutWindow::Attributions)
    }

    /// The open window, if any.
    #[must_use]
    pub fn window(&self) -> Option<AboutWindow> {
        self.open
    }

    /// Shows `window`, replacing whatever was open.
    ///
    /// Returns whether the visible window actually changed, so a caller
    /// can skip a redraw for a redundant open.
    pub fn open(&mut self, window: AboutWindow) -> bool {
        if self.open == Some(window) {
            return false;
        }
        self.open = Some(window);
        true
    }

    /// Shows the About card (`UserCommand::OpenAboutDialog` lands here).
    pub fn open_about(&mut self) -> bool {
        self.open(AboutWindow::About)
    }

    /// Shows the attributions window.
    pub fn open_attributions(&mut self) -> bool {
        self.open(AboutWindow::Attributions)
    }

    /// Hides whichever window is showing.
    ///
    /// Returns whether one was open, the same "was anything consumed"
    /// contract as [`crate::ui_iced::overlay::OverlaySlot::close`].
    pub fn close(&mut self) -> bool {
        let was_open = self.is_open();
        self.open = None;
        was_open
    }

    /// Applies one [`AboutMessage`] to the window state and reports
    /// whether the **visible window** changed.
    ///
    /// [`AboutMessage::CopyCommitHashRequested`] and
    /// [`AboutMessage::RepositoryOpened`] change nothing here — they are
    /// deferred intents the reducer acts on (clipboard write, URL open),
    /// so they return `false` while still requiring that action.
    /// [`AboutMessage::Absorbed`] returns `false` and requires nothing:
    /// its whole job is to have arrived and changed nothing.
    pub fn apply(&mut self, message: AboutMessage) -> bool {
        match message {
            AboutMessage::AttributionsRequested => self.open_attributions(),
            AboutMessage::CloseRequested => self.close(),
            AboutMessage::CopyCommitHashRequested
            | AboutMessage::RepositoryOpened { .. }
            | AboutMessage::Absorbed => false,
        }
    }
}
