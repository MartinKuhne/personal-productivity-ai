//! Iced port of the About window — application identity, build metadata,
//! the MIT license and the entry point into the third-party attributions
//! list ([TASK-017b2]).
//!
//! Ports `src/app/ui/about_dialog.rs`. Like every other `ui_iced` view
//! this is a **pure view over a plain snapshot**: [`about_card`] renders an
//! [`AboutViewModel`] into an [`iced::Element`] of [`Message`] and touches
//! neither the bus, the disk nor `FastMdApp`. The [TASK-022]/[TASK-023]
//! reducer owns the open/close state ([`AboutWindowState`]) and the two
//! side effects the egui dialog performed inside its click handler.
//!
//! # Data sources (all compile-time — no render-time I/O)
//!
//! | Field | Source |
//! | --- | --- |
//! | App name / copyright / description | `crate::ui::strings` ([GUD-002]) |
//! | Version | the `fastmd` package version constant in `src/app/ui/about_dialog.rs` (`env!("CARGO_PKG_VERSION")`), reused unchanged |
//! | Branch / commit / build date | the root `build.rs` `cargo:rustc-env` vars, read with `env!` — the same values the egui dialog's private consts read |
//! | License body | the shared license reflow in `src/app/ui/about_dialog.rs` over `include_str!("../../../LICENSE")` |
//! | Attribution count | [`crate::ui::attributions::DIRECT_DEPENDENCIES`] — the same curated catalog the egui dialog renders (see [`crate::ui_iced::attributions`]) |
//!
//! # Banner: a glyph, not the PNG
//!
//! `Cargo.toml` enables iced's `wgpu`, `tiny-skia`, `fira-sans`,
//! `advanced-shaping` and `markdown` features only (DEC-011 — minimal
//! features), so `iced::widget::image` and `iced::widget::svg` **do not
//! exist** in this build. The compiled-in `assets/icon-32.png`
//! ([`crate::utils::app_icon::APP_ICON_PNG`]) therefore cannot be shown;
//! the banner is the shared "Document + Bolt" stand-in from
//! [`crate::ui_iced::logo`] — the same mark the top bar renders
//! ([TASK-011], [TASK-020]). That module records the raster / SVG /
//! `canvas` trade-off; enabling any of those features is a build-level
//! decision for the orchestrator, not this task.
//!
//! # Message channel
//!
//! These windows ride their **own** [`Message::About`] channel rather than
//! the [TASK-017b1] [`crate::ui_iced::dialogs`] one. `DialogMessage`'s
//! vocabulary is name-entry shaped (`NameChanged`, `DestinationSelected`,
//! `Submitted`) and none of it applies here, while the About vocabulary
//! ([`AboutMessage`]) has nothing in common with it; sharing one channel
//! would force two unrelated state models into `DialogState`. Opening the
//! dialog itself stays user intent on the command channel — the hamburger
//! already emits `UserCommand::OpenAboutDialog`
//! (`ui_iced/panels/top.rs:439`), which the reducer maps to
//! [`AboutWindowState::open_about`].
//!
//! # Deliberate deviations from the egui dialog
//!
//! * **Attributions moved out.** egui renders the whole catalog inline in
//!   the About window (`src/app/ui/about_dialog.rs:200-220`); this port
//!   keeps only the count plus a
//!   [`strings::ABOUT_VIEW_ATTRIBUTIONS_BUTTON`] that opens
//!   [`crate::ui_iced::attributions`], so the credits list gets the
//!   dedicated, wider window the brief asks for.
//! * **A version row is new** — the egui dialog never displayed
//!   `CARGO_PKG_VERSION` even though it owns the constant.
//! * **Copy-to-clipboard is deferred intent.** egui called
//!   `ctx.copy_text` inside the click handler
//!   (`src/app/ui/about_dialog.rs:68-77`); iced exposes the clipboard
//!   only to `update` (`iced_core-0.14.0/src/clipboard.rs`), so the
//!   commit cell emits [`AboutMessage::CopyCommitHashRequested`] and the
//!   reducer performs the write.
//! * **No hover tooltip.** iced 0.14 has no built-in tooltip widget and
//!   DEC-005 rules out `iced_aw`, so the full-hash reveal cannot follow
//!   the egui shape. [`AboutViewModel::commit_tooltip`] still produces
//!   the same text for the reducer to surface another way.
//! * **Unknown build metadata stays inert** (spec FR-007): the commit cell
//!   is always allocated, it simply gets no press handler — see
//!   [`commit_copy_press`].
//!
//! Unit tests live in the sibling `about_tests.rs` sidecar.

use iced::widget::{Column, Row, button, mouse_area, rule, scrollable, space, text};
use iced::{Alignment, Element, Length};

use crate::ui::attributions::DIRECT_DEPENDENCIES;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::logo;
use crate::ui_iced::message::{AboutMessage, Message};
use crate::ui_iced::theme::to_iced_color;

#[cfg(test)]
#[path = "about_tests.rs"]
mod tests;

/// Width of the About card — egui parity (`default_size: [620.0, 780.0]`,
/// `src/app/ui/about_dialog.rs:135`).
pub const ABOUT_CARD_WIDTH: f32 = 620.0;

/// Logical point size of the app-name heading.
pub const ABOUT_HEADING_SIZE: f32 = 22.0;

/// Logical point size of the banner glyph.
pub const ABOUT_BANNER_SIZE: f32 = 48.0;

/// Logical point size of a section header.
pub const ABOUT_HEADER_SIZE: f32 = 16.0;

/// Logical point size of body, fact and metadata rows.
pub const ABOUT_BODY_SIZE: f32 = 14.0;

/// Vertical gap between card sections and between inline labels.
pub const ABOUT_SECTION_SPACING: f32 = 8.0;

/// Maximum height of the license scroll area — egui parity
/// (`ScrollArea::max_height(200.0)`, `src/app/ui/about_dialog.rs:191`).
pub const LICENSE_MAX_HEIGHT: f32 = 200.0;

/// Width of the horizontal rules separating card sections.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// The sentinel `build.rs` writes when git metadata is unavailable; the
/// egui dialog tests the same value (`src/app/ui/about_dialog.rs:93`).
pub const UNKNOWN_BUILD_METADATA: &str = "unknown";

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
            version: crate::ui::about_dialog::APP_VERSION,
            description: strings::ABOUT_DESCRIPTION,
            copyright: strings::ABOUT_COPYRIGHT,
            branch: env!("BUILD_BRANCH"),
            commit_hash: env!("BUILD_COMMIT_HASH"),
            commit_short_hash: env!("BUILD_COMMIT_SHORT_HASH"),
            build_date: env!("BUILD_DATE"),
            license_text: crate::ui::about_dialog::license_display_text(),
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

    /// The full-hash reveal line the egui dialog showed on hover
    /// (`src/app/ui/about_dialog.rs:42-49`). iced 0.14 has no tooltip
    /// widget, so this is text the reducer may surface elsewhere — it is
    /// not rendered by [`about_card`].
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

    /// Renders this snapshot as the About card ([`about_card`]).
    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        about_card(self)
    }
}

/// Renders the About card ([TASK-017b2]).
///
/// Layout, mirroring the egui dialog body (`src/app/ui/about_dialog.rs:147-221`)
/// with the attributions section replaced by a teaser button:
///
/// ```text
/// [⚡]  FastMD Viewer
///       Copyright (c) 2026 Martin Kuhne
/// Version: 0.2.0
/// <description>
/// ─────────────────────────────────────────────
/// Branch: chore/x   Commit: 1a2b3c4d   Built: 2026-10-18
/// ─────────────────────────────────────────────
/// License
/// ┌ MIT License … (scrollable, 200px cap) ─────┐
/// ─────────────────────────────────────────────
/// Third-Party Attributions
/// 57 direct third-party dependencies
/// [ View third-party attributions... ]
/// ─────────────────────────────────────────────
///                                   [ Close ]
/// ```
///
/// Emitted messages:
/// * commit cell → [`AboutMessage::CopyCommitHashRequested`] (only when
///   the hash is known — see [`commit_copy_press`])
/// * the teaser button → [`AboutMessage::AttributionsRequested`]
/// * the footer → [`AboutMessage::CloseRequested`]
///
/// The card is the *content* only: the root view mounts it through
/// [`crate::ui_iced::overlay::modal`] with
/// `Message::About(AboutMessage::CloseRequested)` as the dismiss message
/// so a scrim click or Escape closes it.
///
/// Every row is always allocated — the unknown-build and empty-catalog
/// states change text and press handlers, never the widget count (AGENTS
/// §6).
///
/// Purity: pure view over `view_model`.
#[must_use]
pub fn about_card(view_model: &AboutViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let identity = Column::with_children(vec![
        text(view_model.app_name)
            .size(ABOUT_HEADING_SIZE)
            .color(to_iced_color(palette.text_strong))
            .into(),
        text(view_model.copyright)
            .size(ABOUT_BODY_SIZE)
            .color(to_iced_color(palette.text_muted))
            .into(),
    ])
    .spacing(2.0);

    let header = Row::with_children(vec![banner_mark(), identity.into()])
        .spacing(ABOUT_SECTION_SPACING * 2.0)
        .align_y(Alignment::Center);

    let facts = Column::with_children(vec![
        text(view_model.version_line())
            .size(ABOUT_BODY_SIZE)
            .color(to_iced_color(palette.text_value))
            .into(),
        text(view_model.description)
            .size(ABOUT_BODY_SIZE)
            .color(to_iced_color(palette.text))
            .into(),
    ])
    .spacing(2.0);

    Column::with_children(vec![
        header.into(),
        facts.into(),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        build_metadata_row(view_model),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        license_section(view_model),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        attributions_teaser(view_model),
        rule::horizontal(SEPARATOR_WIDTH).into(),
        about_footer(),
    ])
    .spacing(ABOUT_SECTION_SPACING)
    .padding(ABOUT_SECTION_SPACING)
    .width(Length::Fixed(ABOUT_CARD_WIDTH))
    .into()
}

/// The glyph the About banner renders.
///
/// Delegated to [`crate::ui_iced::logo::logo_glyph`] — the About card does
/// not own its own mark ([TASK-020], [RUST-051]).
#[must_use]
pub fn banner_glyph() -> &'static str {
    logo::logo_glyph()
}

/// The rendered banner mark (top panel of the card).
#[must_use]
pub fn banner_mark() -> Element<'static, Message> {
    logo::logo_mark(ABOUT_BANNER_SIZE)
}

/// The Close button shared by the About and attributions cards.
#[must_use]
pub fn close_button() -> Element<'static, Message> {
    button(text(strings::CLOSE_BUTTON))
        .on_press(Message::About(AboutMessage::CloseRequested))
        .into()
}

/// The card footer: spacer plus right-aligned Close button.
#[must_use]
pub fn about_footer() -> Element<'static, Message> {
    Row::with_children(vec![
        space::Space::new().width(Length::Fill).into(),
        close_button(),
    ])
    .spacing(ABOUT_SECTION_SPACING)
    .align_y(Alignment::Center)
    .into()
}

/// The Branch / Commit / Built row (egui parity,
/// `src/app/ui/about_dialog.rs:154-183`).
fn build_metadata_row(view_model: &AboutViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();
    let muted = |value: &str| {
        text(value.to_owned())
            .size(ABOUT_BODY_SIZE)
            .color(to_iced_color(palette.text_muted))
    };

    Row::with_children(vec![
        muted(strings::ABOUT_BRANCH_LABEL).into(),
        muted(view_model.branch).into(),
        muted(strings::ABOUT_COMMIT_LABEL).into(),
        commit_cell(view_model),
        muted(strings::ABOUT_DATE_LABEL).into(),
        muted(view_model.build_date).into(),
    ])
    .spacing(ABOUT_SECTION_SPACING)
    .align_y(Alignment::Center)
    .into()
}

/// The commit-hash cell: always allocated, and pressable only when there
/// is a real hash to copy (spec FR-007 fallback).
fn commit_cell(view_model: &AboutViewModel) -> Element<'_, Message> {
    let cell = text(view_model.commit_short_hash)
        .size(ABOUT_BODY_SIZE)
        .color(to_iced_color(Palette::fastmd_dark().text_value));

    let area = mouse_area(cell);
    match commit_copy_press(view_model.is_build_metadata_unknown()) {
        Some(message) => area.on_press(message).into(),
        None => area.into(),
    }
}

/// The press handler the commit cell gets: `Some` only when the build
/// metadata is known, because an `"unknown"` build has nothing
/// meaningful to copy (egui parity — `copy_commit_hash_to_output` returns
/// early, `src/app/ui/about_dialog.rs:69-71`).
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
    let palette = Palette::fastmd_dark();

    let header = text(strings::ABOUT_LICENSE_HEADER)
        .size(ABOUT_HEADER_SIZE)
        .color(to_iced_color(palette.text_strong));

    let body = scrollable(
        text(&view_model.license_text)
            .size(ABOUT_BODY_SIZE)
            .width(Length::Fill),
    )
    .height(Length::Fixed(LICENSE_MAX_HEIGHT));

    Column::with_children(vec![header.into(), body.into()])
        .spacing(ABOUT_SECTION_SPACING)
        .into()
}

/// The attributions header, count line and the button that opens the
/// credits window ([`crate::ui_iced::attributions`]).
fn attributions_teaser(view_model: &AboutViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let header = text(strings::ABOUT_ATTRIBUTIONS_HEADER)
        .size(ABOUT_HEADER_SIZE)
        .color(to_iced_color(palette.text_strong));

    let summary = text(view_model.attributions_summary())
        .size(ABOUT_BODY_SIZE)
        .color(to_iced_color(palette.text_muted));

    let open = button(text(strings::ABOUT_VIEW_ATTRIBUTIONS_BUTTON))
        .on_press(Message::About(AboutMessage::AttributionsRequested));

    Column::with_children(vec![header.into(), summary.into(), open.into()])
        .spacing(ABOUT_SECTION_SPACING)
        .into()
}

/// Which of the two identity windows is showing ([TASK-017b2]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutWindow {
    /// The About card ([`about_card`]).
    About,
    /// The third-party attributions list
    /// ([`crate::ui_iced::attributions::attributions_view`]).
    Attributions,
}

impl AboutWindow {
    /// The window title, from `crate::ui::strings` ([GUD-002]).
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::About => strings::ABOUT_DIALOG_TITLE,
            Self::Attributions => strings::ABOUT_ATTRIBUTIONS_HEADER,
        }
    }
}

/// Which identity window is open — the pure state the reducer holds
/// ([TASK-017b2]).
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
    pub fn apply(&mut self, message: AboutMessage) -> bool {
        match message {
            AboutMessage::AttributionsRequested => self.open_attributions(),
            AboutMessage::CloseRequested => self.close(),
            AboutMessage::CopyCommitHashRequested | AboutMessage::RepositoryOpened { .. } => false,
        }
    }
}
