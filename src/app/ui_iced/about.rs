//! About window — application identity, build metadata, the MIT license
//! and the third-party attributions list itself.
//!
//! A **pure view over a plain snapshot**: [`about_card`] renders an
//! [`AboutViewModel`] plus an
//! [`AttributionsViewModel`]
//! into an [`iced::Element`] of [`Message`] and touches neither the bus, the
//! disk nor `FastMdApp`. The reducer owns the open/close state
//! ([`AboutWindowState`]) and performs the two side effects the card only
//! requests.
//!
//! # The framework owns the box, the app owns the answer
//!
//! As of integration increment INT-5a this module is a consumer of
//! [`hoarfrost::surface::modal`] rather than a re-implementation of
//! §4.6.2. What it takes from the framework:
//!
//! | Concern | Owner |
//! |---|---|
//! | Card width / height policy parts, gutter clamp | [`ABOUT_SIZING`] assembled from [`AxisPolicy`] / [`Desired`] / [`Cap`], resolved through [`about_card_size`] |
//! | Card surface, border, radius, padding | [`ModalFrame::resolve`] through [`crate::ui_iced::overlay::modal_card`] |
//! | Section divider colour and weight | [`crate::ui_iced::overlay::modal_divider`] (`color.border.subtle`) |
//! | Type sizes, gaps, glyph size | `hoarfrost::token::{typography, space, size, shape}` |
//! | Footer button geometry and states | [`hoarfrost::widget::button_widget::Button`] |
//! | Which window is open, what dismisses it | [`AboutWindowState`] + `OverlayState::close_topmost` |
//! | Credits-list sub-region solve and cap | [`hoarfrost::surface::modal::subregion_height`] via [`credits_list_height`] |
//!
//! [`hoarfrost::surface::modal::ModalHost`] is deliberately **not** used,
//! for the same reason [`crate::ui_iced::dialogs`] did not take it: the
//! host self-registers its surface in a
//! [`hoarfrost::surface::zorder::SurfaceRegistry`] and pushes its own
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
//!   two effects it requests (clipboard write, open a repository URL) are
//!   not modelled as failures. `ModalError` therefore never appears here.
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
//! | Credits rows and count line | [`crate::ui::attributions::DIRECT_DEPENDENCIES`], snapshotted once as [`AttributionsViewModel`] and rendered by [`crate::ui_iced::attributions`]: the card paints that snapshot's own [`summary`](crate::ui_iced::attributions::AttributionsViewModel::summary), so the count line counts the rows it sits above and the empty catalog gets its own line |
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
//! * **The credits list is inline, not a sub-dialog.** The card renders the
//!   header, the count line and the scrollable table itself
//!   (`attributions_section`); there is no button that opens a second
//!   window. The owner's ruling removed the teaser and the separate
//!   attributions card, so [`crate::ui_iced::attributions`] is now a
//!   renderer this card hosts rather than a window of its own.
//! * **The card is viewport-sized, not content-sized.** [`about_card`]
//!   mounts the frame at [`about_card_size`], so the box is
//!   [`ABOUT_VIEWPORT_FILL`] of the window and does **not** grow with its
//!   content. Everything that can get long — the licence body and the
//!   credits table — is a scroll region inside it, and the credits region
//!   is solved from the card's own height ([`credits_list_height`]) rather
//!   than given a number of its own.
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

use hoarfrost::surface::modal::{
    ABOUT_WIDTH, ATTRIBUTIONS_LIST_MAX_HEIGHT, AxisPolicy, Cap, Desired, ModalFrame, ModalSizing,
    SubRegionBands, subregion_height,
};
use hoarfrost::token::{shape, size, space, typography};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::attributions::{AttributionsViewModel, attribution_table, credits_list};
use crate::ui_iced::logo;
use crate::ui_iced::message::{AboutMessage, Message};
use crate::ui_iced::overlay::{modal_card, modal_divider};

#[cfg(test)]
#[path = "about_tests.rs"]
mod tests;

/// The fraction of the viewport the About card fills on **both** axes
/// (owner ruling: "the about window should take up more of the available
/// screen real estate").
///
/// Named rather than inlined ([RUST-021]): the one ratio drives the width
/// policy, the height policy, and every test that pins the measured card.
/// It is the same figure the Inline Document Editor
/// ([`hoarfrost::surface::modal::EDITOR_VIEWPORT_FRACTION`]) and the Tools
/// card ([`crate::ui_iced::tools_dialog::VIEWPORT_FILL`]) already take, so
/// the app has one answer to "how big is a big modal".
pub const ABOUT_VIEWPORT_FILL: f32 = 0.8;

/// The About card's sizing policy: [`ABOUT_VIEWPORT_FILL`] of the window on
/// **both** axes.
///
/// Held as the policy object rather than a set of numbers, so the card
/// states what it wants and the framework does the clamping.
///
/// **This is an app-side override of §4.6.2, not a restatement of it.**
/// The framework's [`ModalSizing::about`] still describes the spec table
/// verbatim — 440px wide, content-fit tall — and the owner's ruling
/// replaces that with a viewport-driven box. The override is assembled
/// here from the framework's own [`AxisPolicy`] parts rather than by
/// editing the spec table in `hoarfrost`, so the framework keeps saying
/// what the spec says while this card says what its owner asked for. It is
/// the same move [`crate::ui_iced::tools_dialog::TOOLS_SIZING`] made for
/// the Tools card.
///
/// * **Width** — 80% of the window, floored at [`ABOUT_WIDTH`]. Unlike the
///   Tools card, this card has content that was laid out and *measured* at
///   the retired 440px figure: the Branch / Commit / Built row is a single
///   unwrapping `Row` of six cells, and the credits table is a three-column
///   `table` whose repository column is its only fluid one. Below a ~550px
///   window 80% is narrower than the width both were proven in, so the
///   retired spec width stays as a floor rather than being discarded.
/// * **Height** — 80% of the window, **no floor**. §4.6.2 gives About a
///   content-fit height with no ceiling, so there is no framework figure to
///   defend, and a card-local floor would only be a second number competing
///   with the viewport. A short window is absorbed by the scroll regions
///   inside the card — the licence region compresses first, measured in the
///   sidecar — which is what a viewport-sized card with internal scrolling
///   is for.
/// * **Both axes** — capped at the same ratio and then clamped by the
///   framework's `VIEWPORT_GUTTER`, which is applied last and always wins,
///   so neither the width floor nor the 80% fill can overflow a tiny
///   viewport.
pub const ABOUT_SIZING: ModalSizing = ModalSizing::new(
    AxisPolicy {
        desired: Desired::Viewport(ABOUT_VIEWPORT_FILL),
        min: ABOUT_WIDTH,
        max: Cap::Viewport(ABOUT_VIEWPORT_FILL),
    },
    AxisPolicy {
        desired: Desired::Viewport(ABOUT_VIEWPORT_FILL),
        min: 0.0,
        max: Cap::Viewport(ABOUT_VIEWPORT_FILL),
    },
);

/// The About card's **outer** width for a viewport width.
///
/// [`ABOUT_VIEWPORT_FILL`] — 80% — of the window, held up by the
/// [`ABOUT_WIDTH`] floor and clamped to `viewport.width - VIEWPORT_GUTTER`
/// (`space::XXL`, 32px) on a window too narrow to hold it. The card's
/// padding is *inside* this figure — see
/// [`crate::ui_iced::overlay::modal_card`].
#[must_use]
pub fn about_card_width(viewport_width: f32) -> f32 {
    // `Desired::Viewport` ignores the content extent; `0.0` is passed so
    // the policy's floor and gutter are the only things that can move it.
    ABOUT_SIZING.width.resolve(viewport_width, 0.0)
}

/// The About card's **outer** height for a viewport height.
///
/// [`ABOUT_VIEWPORT_FILL`] of the window, clamped last by the framework's
/// `VIEWPORT_GUTTER`. The card no longer fits its content: the licence body
/// and the credits table are scroll regions inside it, which is the whole
/// point of the decision — a two-row catalog and a sixty-row one get the
/// same card, and both lists scroll.
#[must_use]
pub fn about_card_height(viewport_height: f32) -> f32 {
    ABOUT_SIZING.height.resolve(viewport_height, 0.0)
}

/// The About card's **outer** size for a viewport.
///
/// Both axes come from [`ABOUT_SIZING`]; the card is centred over the scrim
/// by [`crate::ui_iced::overlay::modal_centered`] when it is mounted.
#[must_use]
pub fn about_card_size(viewport: Size) -> Size {
    Size::new(
        about_card_width(viewport.width),
        about_card_height(viewport.height),
    )
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
/// **A token gap, reported rather than papered over.** The framework has no
/// token for "how tall one scroll region inside a card may get" —
/// [`hoarfrost::surface::modal::CONTENT_HEIGHT_MAX`] is the tallest a
/// *whole* content-fit card body may be (480px), which is a statement about
/// a different thing. So the cap stays an app-owned value: a layout decision
/// about one region of one card, not a design token, and deliberately not
/// stretched to a framework constant that means something else.
///
/// **What the viewport-sized card changed is which region absorbs a short
/// window, not this number.** The cap is still what the licence region is
/// *given* whenever the card has 200px to give it; when the card is shorter
/// than its own furniture, iced hands the region whatever is left and it
/// scrolls anyway (measured at a 400px-tall window in the sidecar). The
/// credits region below it is solved rather than capped-and-hoped — see
/// [`credits_list_height`].
pub const LICENSE_MAX_HEIGHT: f32 = 200.0;

/// Everything the About card stacks **above** the credits list, including
/// the dividers and the `space::SM` gaps those rows sit on (`UIF-MOD-004`).
///
/// This is the header band the credits sub-region is solved against, so it
/// has to be the card's real furniture and not a guess: identity header row,
/// version + description, divider, build-metadata row, divider, the licence
/// section (its own header, gap and capped scroll area), divider, the
/// credits header and the count line, each pair separated by `space::SM`.
///
/// Every term is a token or a constant this module already owns. The
/// composition is **this card's** layout, not the framework's: the framework
/// supplies the solver and the frame's padding, and only the host knows it
/// renders a banner, two fact rows, three dividers and a licence block
/// above the list. Keeping it here is what stops app layout knowledge
/// leaking into `surface::modal`.
///
/// **Literal-free on purpose.** A coefficient such as `3.0 * space::SM` is a
/// numeric literal, and the app's token gate treats any literal in a
/// visual-named constant as a violation unless the constant is registered;
/// writing the repeated gaps out as repeated terms keeps the composition
/// `TokenBound` on its own merits rather than by exemption, and the terms
/// are countable by eye against the `Column` in [`about_card`].
///
/// **`_BAND_HEIGHT`, not `_BAND`.** The gate only inspects a constant whose
/// name reads as a measurement
/// ([`VISUAL_NAME_MARKERS`](hoarfrost::token_lint::VISUAL_NAME_MARKERS)
/// matches `HEIGHT`), so the suffix is what makes these two compositions
/// auditable at all — the same reason the framework names its cap
/// `ATTRIBUTIONS_LIST_MAX_HEIGHT` rather than the spec's `_MAX_H`. They
/// replace the two band constants the retired standalone attributions card
/// carried, so the gate's census of token-bound app visuals is unchanged.
///
/// **The banner is deliberately not a term.** The header `Row` is
/// centre-aligned, so its height is the taller of its two children, and the
/// identity column (`TITLE_MD` line + `space::XXS` + `BODY` line, 50px)
/// towers over the `size::ICON_LG` glyph (20px on iced's default leading).
/// The sidecar measures the header row against this band's first three
/// terms, so if a future banner ever does become the taller child the
/// arithmetic is caught rather than assumed.
pub const ABOUT_CREDITS_HEADER_BAND_HEIGHT: f32 = typography::TITLE_MD.line_height
    + space::XXS
    + typography::BODY.line_height
    + space::SM
    + typography::BODY.line_height
    + space::XXS
    + typography::BODY.line_height
    + space::SM
    + shape::BORDER_HAIRLINE
    + space::SM
    + typography::BODY.line_height
    + space::SM
    + shape::BORDER_HAIRLINE
    + space::SM
    + typography::TITLE_SM.line_height
    + space::SM
    + LICENSE_MAX_HEIGHT
    + space::SM
    + shape::BORDER_HAIRLINE
    + space::SM
    + typography::TITLE_SM.line_height
    + space::SM
    + typography::BODY.line_height
    + space::SM;

/// Everything the About card stacks **below** the credits list — the divider
/// above the footer, the footer control row, and the two `space::SM` gaps
/// they sit on (`UIF-MOD-004`).
///
/// Literal-free for the same reason [`ABOUT_CREDITS_HEADER_BAND_HEIGHT`] is: one
/// `BORDER_HAIRLINE`, a `size.control.md` footer row, and two `space::SM`
/// gaps written out as two terms.
pub const ABOUT_CREDITS_FOOTER_BAND_HEIGHT: f32 =
    shape::BORDER_HAIRLINE + size::CONTROL_MD + space::SM + space::SM;

/// The height the credits list is actually given, for a card of
/// `card_height` (`UIF-MOD-004`).
///
/// Solved from the card's **solved** bounds through the framework's own
/// solver, then held down by the §2.2 cap. It is never the card's outer
/// constant, and it never exceeds what the card has left:
///
/// ```text
/// min( card_height - (header + 2 × padding + footer), ATTRIBUTIONS_LIST_MAX_HEIGHT )
/// ```
///
/// The cap is kept because §2.2 states it, not because the card needs it:
/// on a tall window the remainder passes 200 and the cap binds, leaving
/// unused card below the list; on an ordinary window the remainder is what
/// is left after the licence region spent its own 200, and the list gets
/// that instead.
///
/// Purity: pure function of `card_height`.
#[must_use]
pub fn credits_list_height(card_height: f32) -> f32 {
    let bands = SubRegionBands::from_frame(&modal_frame())
        .with_header(ABOUT_CREDITS_HEADER_BAND_HEIGHT)
        .with_footer(ABOUT_CREDITS_FOOTER_BAND_HEIGHT);
    subregion_height(card_height, bands, ATTRIBUTIONS_LIST_MAX_HEIGHT)
}

/// The [`Length`] the credits list is rendered at, for a card of
/// `card_height` (`UIF-MOD-004`).
///
/// `Fixed(solved)` whenever the solver found room. **The zero case falls
/// back to `Fill`, and that is not a hedge — it is a measured iced
/// behaviour.** `Length::Fixed(0.0)` is *void* in iced 0.14
/// (`Size::is_void()` is true when either axis is `Fixed(0.0)`), and a void
/// child is silently dropped from the widget tree. Measured in the sidecar:
/// on a viewport whose card has no room left the solved height is `0`, and
/// with `Fixed(0.0)` the scrollable is not in the tree at all
/// (`SelectorNotFound`), while with `Fill` it is present and measures `0.0`.
///
/// Losing the widget would break AGENTS §5 —
/// [`ATTRIBUTIONS_LIST_ID`](crate::ui_iced::attributions::ATTRIBUTIONS_LIST_ID)
/// is a stateful scrollable whose scroll position must survive a rebuild, and
/// a widget that vanishes and returns under the same id starts fresh. It would
/// also break §6's "do not let the layout collapse silently".
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
        }
    }

    /// Whether `build.rs` could not read git and fell back to
    /// [`UNKNOWN_BUILD_METADATA`] (spec FR-007) — the commit cell then
    /// renders without a copy affordance.
    #[must_use]
    pub fn is_build_metadata_unknown(&self) -> bool {
        self.commit_hash == UNKNOWN_BUILD_METADATA
    }

    /// The version row text: `Version: 0.3.0`.
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

    /// Renders this snapshot together with the credits snapshot as the
    /// framed About card ([`about_card`]).
    ///
    /// The credits list is a separate snapshot on purpose: the card owns its
    /// own identity/build fields, while the rows and the count line above
    /// them come from one [`AttributionsViewModel`] so the two cannot
    /// disagree ([`crate::ui_iced::attributions`]).
    #[must_use]
    pub fn view<'a>(
        &'a self,
        attributions: &'a AttributionsViewModel,
        viewport: Size,
    ) -> Element<'a, Message> {
        about_card(self, attributions, viewport)
    }
}

/// Renders the framed About card at [`about_card_size`] — [`ABOUT_VIEWPORT_FILL`]
/// of the window on both axes — with the third-party credits list rendered
/// **inline**.
///
/// Layout:
///
/// ```text
/// ┌─ About card · 80% of the viewport, gutter-clamped · radius 6 ────────┐
/// │ [⚡]  FastMD AI                                                      │
/// │       Copyright (c) 2026 Martin Kuhne                                │
/// │ Version: 0.3.0                                                       │
/// │ <description>                                                        │
/// │ ──────────────────────────────────────────────────────────────────── │
/// │ Branch: chore/x   Commit: 1a2b3c4d   Built: 2026-10-18               │
/// │ ──────────────────────────────────────────────────────────────────── │
/// │ License                                                              │
/// │ ┌ MIT License … (scrollable, 200px cap) ────────────────────────────┐│
/// │ ──────────────────────────────────────────────────────────────────── │
/// │ Third-Party Attributions                                             │
/// │ 61 direct third-party dependencies                                   │
/// │ ┌ Crate │ Authors │ Repository … (scrollable, solved remainder) ───┐│
/// │ ──────────────────────────────────────────────────────────────────── │
/// │                                        [ Close ]                     │
/// └───────────────────────────────────────────────────────────────────────┘
/// ```
///
/// Emitted messages:
/// * commit cell → [`AboutMessage::CopyCommitHashRequested`] (only when
///   the hash is known — see [`commit_copy_press`])
/// * a repository row → [`AboutMessage::RepositoryOpened`], through
///   [`crate::ui_iced::attributions::repository_press`]
/// * the footer → [`AboutMessage::CloseRequested`]
///
/// The card is **framed but not mounted**: it carries its own §4.6.1
/// chrome and its solved outer size, and the root view centres it over the
/// scrim with [`crate::ui_iced::overlay::modal_centered`], passing
/// [`AboutMessage::Absorbed`] as the card-boundary message and
/// [`AboutMessage::CloseRequested`] as the dismiss message.
///
/// Every row is always allocated — the unknown-build and empty-catalog
/// states change text and press handlers, never the widget count (AGENTS
/// §6).
///
/// Purity: pure view over `view_model`, `attributions` and `viewport`.
#[must_use]
pub fn about_card<'a>(
    view_model: &'a AboutViewModel,
    attributions: &'a AttributionsViewModel,
    viewport: Size,
) -> Element<'a, Message> {
    let frame = modal_frame();
    let card_size = about_card_size(viewport);

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
        attributions_section(attributions, card_size.height),
        modal_divider(frame).into(),
        about_footer(),
    ])
    .spacing(space::SM);

    modal_card(
        body,
        frame,
        ABOUT_CARD_ID,
        Length::Fixed(card_size.width),
        Length::Fixed(card_size.height),
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

/// The Close button the About card's footer renders.
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

/// Stable focus id of the About card's `Close` button.
pub const ABOUT_CLOSE_BUTTON_ID: &str = "about_close";

/// A footer-width framework button carrying `label`.
///
/// The label is mounted in a fixed
/// [`BUTTON_MIN_WIDTH`](hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH)
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
            hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH,
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

/// The credits header, the count line and the scrollable credits table
/// itself — the section the owner asked to see **inside** this card instead
/// of behind a button and a second window.
///
/// The list's height is **solved, not picked**: [`credits_list_length`]
/// hands the scrollable the framework solver's answer for what this card
/// has left after [`ABOUT_CREDITS_HEADER_BAND_HEIGHT`] and
/// [`ABOUT_CREDITS_FOOTER_BAND_HEIGHT`] ([`credits_list_height`],
/// `UIF-MOD-004`), held down by the §2.2
/// [`ATTRIBUTIONS_LIST_MAX_HEIGHT`](hoarfrost::surface::modal::ATTRIBUTIONS_LIST_MAX_HEIGHT)
/// cap. When the card is too short to have anything left the solver returns
/// zero and the length falls back to `Fill` rather than `Length::Fixed(0.0)`
/// — a zero-length child is *void* in iced 0.14 and would silently drop the
/// scrollable, and with it `ATTRIBUTIONS_LIST_ID` (AGENTS §5).
///
/// The count line and the rows both come from the one `attributions`
/// snapshot, so the line above the table counts exactly the rows below it,
/// and an empty catalog renders the header-only table rather than
/// collapsing the section (AGENTS §6).
fn attributions_section(
    attributions: &AttributionsViewModel,
    card_height: f32,
) -> Element<'_, Message> {
    let header = rung(
        strings::ABOUT_ATTRIBUTIONS_HEADER,
        typography::TITLE_SM,
        Palette::fastmd_dark().text_strong,
    );

    let summary = rung(
        attributions.summary(),
        typography::BODY,
        Palette::fastmd_dark().text_muted,
    );

    let list = credits_list(
        attribution_table(attributions.entries()),
        credits_list_length(card_height),
    );

    Column::with_children(vec![header.into(), summary.into(), list.into()])
        .spacing(space::SM)
        .into()
}

/// Which identity window is showing.
///
/// There is only one. The owner's ruling put the third-party credits list
/// **inside** [`about_card`], so the enum that used to choose between the
/// About card and a separate credits window is now a single-variant marker.
/// It stays a type rather than collapsing to a `bool` because
/// [`AboutWindowState`] reports *which* window it opened and closed, and the
/// overlay layer and the registry surface id name the same thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutWindow {
    /// The About card ([`about_card`]), credits list included.
    About,
}

impl AboutWindow {
    /// The window title, from `crate::ui::strings`.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::About => strings::ABOUT_DIALOG_TITLE,
        }
    }

    /// The sizing policy this window's card is mounted with.
    ///
    /// This is the app-side [`ABOUT_SIZING`] override, **not** the
    /// framework's §4.6.2 row ([`ModalSizing::about`]) — see that constant's
    /// doc for why the card states its own answer.
    #[must_use]
    pub const fn sizing(&self) -> ModalSizing {
        match self {
            Self::About => ABOUT_SIZING,
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
            AboutMessage::CloseRequested => self.close(),
            AboutMessage::CopyCommitHashRequested
            | AboutMessage::RepositoryOpened { .. }
            | AboutMessage::Absorbed => false,
        }
    }
}
