//! Iced pane-grid layout of the FASTMD three-band geometry (UI-001).
//!
//! The view is a vertical `column![]` of three full-width bands:
//! the top band stub, the middle band, and the bottom band stub. The
//! middle band is an `iced::widget::pane_grid` split horizontally into the
//! left (file tree stub), center (content pane) and right (table-of-contents
//! stub) panes:
//!
//! - the split arrangement is expressed as a [`Configuration`] tree with
//!   [`pane_grid::Axis::Vertical`] walls (left/right — `iced_widget-0.14.2/src/
//!   pane_grid/axis.rs:48-71`); the ids assigned by
//!   `Internal::from_configuration` (`iced_widget-0.14.2/src/pane_grid/
//!   state.rs:365-408`) are derived from the built [`pane_grid::State`]
//!   rather than hardcoded, and the shape contract is pinned by tests;
//! - the right pane is gated by [`should_show_panel`] — the pure ToC rule,
//!   owned by [`crate::ui_iced::panels::right`] and re-exported here so the
//!   public path stays stable ([RUST-051]: one iced-side home for
//!   the gate);
//! - pane-grid resize drag events ([`pane_grid::ResizeEvent`] fields
//!   `split`/`ratio` — `iced_widget-0.14.2/src/pane_grid.rs:1226-1237`, fed
//!   through `PaneGrid::on_resize(leeway, f)` — `pane_grid.rs:267-275`)
//!   reduce 1:1 onto `crate::ui::PanelLayout` widths (`ui/panel_layout.rs:
//!   26-30`), clamped by the UI-016 constants re-verified at
//!   `ui/panel_layout.rs:5-23`;
//! - the widget runs with a real intra-pane gutter ([`PANE_SPACING_PIXELS`],
//!   defect #12), so a split no longer sits at `ratio * viewport_width`: iced
//!   centres a `spacing`-wide gap on the split line, which means the leading
//!   pane claims `ratio * region_width - spacing / 2` and the trailing pane
//!   claims the mirror image (`iced_widget-0.14.2/src/pane_grid/axis.rs:48-71`).
//!   Every pixel↔ratio conversion therefore carries the half-spacing term —
//!   [`split_ratio_for_width`] and [`pane_width_from_ratio`] are the only two
//!   places that arithmetic lives — and the nested (centre | right) region is
//!   [`nested_region_width`], i.e. the viewport minus the left pane minus the
//!   gutter the left split consumed.
//! - the gutter is painted [`pane_gutter_fill`] (`color.border.subtle`, the
//!   framework's designated pane-split-line colour) by the container that
//!   backs the grid, so the seam reads as a divider rather than as the panes'
//!   own canvas showing through (`UIF-PRIN-006` visible-divider clause).
//!
//! Widget construction is exercised by the running app; the pieces here are
//! the unit-testable state reductions.
//!
//! Unit tests live in the sibling `layout_tests.rs` sidecar.

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;

use crate::ui::panel_layout::{
    LEFT_PANEL_MAX_WIDTH_FRACTION, LEFT_PANEL_MIN_WIDTH, PanelLayout,
    RIGHT_PANEL_MAX_WIDTH_FRACTION, RIGHT_PANEL_MIN_WIDTH,
};
use crate::ui::theme::Palette;
use iced::Background;
use iced::Length;
use iced::widget::container::Style as ContainerStyle;
use iced::widget::pane_grid::{self, Configuration, Pane, Split};

/// Intra-pane gutter width (logical pixels) the [`pane_grid::PaneGrid`] is
/// built with (defect #12, "there is no visual separation between the three
/// window panes").
///
/// Bound to the framework rung `space.2xs` (`hoarfrost::token::space::XXS`)
/// rather than a literal ([RUST-021]). `space.2xs` is the rung hoarfrost
/// itself uses for a visible separation gap between adjacent sibling
/// surfaces — `UIF-PRIN-006` puts a `space.2xs` gap between sibling tab
/// cards on the recessed track — and it is the rung nearest the `1px`
/// pane-split stroke `UIF-PANE-001` describes, so the seam stays a divider
/// rather than turning into a margin.
///
/// The gutter is not free: iced centres it on the split line, so every
/// pixel↔ratio conversion carries a half-spacing term (see
/// [`split_ratio_for_width`]) and the resize leeway shrinks by exactly this
/// much ([`RESIZE_LEEWAY_PIXELS`]).
pub const PANE_SPACING_PIXELS: f32 = hoarfrost::token::space::XXS;

/// Grab-leeway (logical pixels) around each split line for resize drags,
/// passed to `PaneGrid::on_resize` (`iced_widget-0.14.2/src/pane_grid.rs:
/// 258-266`: the grab area length is `spacing + leeway`, centred on the
/// split line).
///
/// `UIF-PANE-001` mandates a `10px` grab band reaching `5px` to each side of
/// the divider line. iced reaches that total as `spacing + leeway`, so the
/// leeway is the framework token `size.splitter.grab` (`UIF-TOK-002`) MINUS
/// the gutter the grid already paints ([`PANE_SPACING_PIXELS`]). The two
/// together stay at `10.0`, and the app still owns no second number for the
/// same measurement ([RUST-021]).
pub const RESIZE_LEEWAY_PIXELS: f32 = hoarfrost::token::size::SPLITTER_GRAB - PANE_SPACING_PIXELS;

/// The fill the inter-pane gutter shows through with — `color.border.subtle`,
/// the framework's designated colour for pane split lines (`UIF-TOK-005`,
/// `UIF-PANE-001`).
///
/// The colour of the seam between two adjacent panes.
///
/// `UIF-PRIN-006`: adjacent containers must differ in surface token or carry
/// a visible divider. The gutter is that divider, so its fill must not be the
/// canvas the panes sit on — an identical fill is exactly defect #12.
/// Resolved from the palette; no literal here.
///
/// **This only reads as a divider because [`pane_canvas_fill`] makes every pane
/// opaque.** iced's pane bodies are transparent containers, so a grid backed
/// with this colour and panes that paint nothing shows this colour across the
/// whole middle band rather than in the 2px seams. The two functions are a
/// pair and must be changed together — see
/// `defect_12_followup_the_panes_occlude_the_gutter_backing`.
#[must_use]
pub fn pane_gutter_fill() -> iced::Color {
    Palette::fastmd_dark().border
}

/// The colour every pane paints itself with.
///
/// This is the window canvas token (`color.surface.base`, the same value
/// `ui_iced::theme` installs as iced's `background` role), so an opaque pane is
/// visually identical to a transparent one over the window — the app looked like
/// this before defect #12 and must keep looking like it.
///
/// The pane has to paint it *deliberately*, though: iced gives a pane no
/// background of its own, so once the grid is backed with
/// [`pane_gutter_fill`] an unpainted pane lets the divider colour through.
/// That is the regression this function exists to prevent.
#[must_use]
pub fn pane_canvas_fill() -> iced::Color {
    Palette::fastmd_dark().surface
}

/// Full-on-screen height of the stub top/bottom bands, in logical points —
/// `size.control.lg` (`UIF-TOK-002`), the rung the live navbar is built
/// at.
///
/// **Movement: 28 → 36**, following the navbar band (see
/// [`crate::ui_iced::panels::top::BAR_HEIGHT_POINTS`]).
///
/// **These stubs are dev scaffolding and nothing mounts them.**
/// [`top_band`] and [`bottom_band`] are `pub` but have no caller anywhere
/// in the crate — the live view builds the real navbar and command band.
/// They are kept on the token rather than at the old literal so no
/// off-scale `28` survives in the tree, but note that one fixed height
/// cannot represent both real bands: §4.1.1 makes the top band 36px and
/// the bottom band `Shrink 44px..120px`. Removing the stubs outright is
/// the cleaner end state and is left as a stated follow-on rather than
/// folded into a token-alignment increment.
pub const STUB_BAND_HEIGHT_POINTS: f32 = hoarfrost::token::size::CONTROL_LG;

/// Text rendered inside every stub region: a dev-scaffolding label, not
/// user-facing copy — kept in exactly one place per [RUST-021].
pub const STUB_TEXT: &str = "stub";

/// Text rendered inside the right (TOC) pane when
/// [`should_show_panel`] passes; see [`STUB_TEXT`].
pub const TOC_STUB_TEXT: &str = "toc";

/// Pure visibility gate for the right (TOC) pane.
///
/// Owned by [`crate::ui_iced::panels::right`] and re-exported here so the
/// [`layout`](crate::ui_iced::layout) public path stays stable and
/// [`view`](crate::ui_iced::view) can resolve it from either name.
pub use crate::ui_iced::panels::right::should_show_panel;

/// Contents of each middle-band pane — [RUST-011]: pub item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaneKind {
    /// The left (file tree) pane.
    Left,
    /// The central content pane.
    Center,
    /// The right (table-of-contents) pane.
    Right,
}

/// The `Pane`/`Split` identifiers of the three-band middle area.
///
/// Ids are discovered from the built [`pane_grid::State`] because the
/// wrapper ints of `pane_grid::{Pane, Split}` are `pub(super)` and cannot be
/// constructed by foreign code (`iced_widget-0.14.2/src/pane_grid/pane.rs:
/// 4-5`, `split.rs:4-5`). The pane order (`BTreeMap` ascending — `state.rs:
/// 29,186`) and the root-first `Node::splits()` walk (`node.rs:59-76`,
/// yields the outer split before the nested one for our tree shape) make
/// the mapping deterministic; `layout_tests.rs` pins it.
///
/// [RUST-011]: pub item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MiddleBandPanes {
    /// The left (file tree) pane id.
    pub left: Pane,
    /// The center content pane id.
    pub center: Pane,
    /// The right (TOC) pane id.
    pub right: Pane,
    /// The split line between the left pane and the center group — resizing
    /// it moves the LEFT panel border ([UI-016]).
    pub left_split: Split,
    /// The split line between the center pane and the right pane — resizing
    /// it moves the RIGHT panel border ([UI-016]).
    pub right_split: Split,
}

/// Root `(left | centre-group)` split ratio the middle band is seeded with.
///
/// Only a seed: [`apply_pane_widths`] overrides it with the remembered width
/// whenever one exists, and a user drag overrides it after that. Named rather
/// than inlined so the startup replay can compute the region the nested split
/// will actually get when no left width has been remembered ([RUST-021]).
pub const DEFAULT_ROOT_SPLIT_RATIO: f32 = 0.25;

/// Nested `(centre | right)` split ratio the middle band is seeded with.
///
/// See [`DEFAULT_ROOT_SPLIT_RATIO`].
pub const DEFAULT_NESTED_SPLIT_RATIO: f32 = 0.5;

/// Builds the middle-band pane tree: a vertical split of (left | (center |
/// right)) (UI-001).
///
/// Tree construction ([RUST-020]): pure — a fresh [`Configuration`] per
/// call, so callers own the mutable [`pane_grid::State`].
pub fn middle_band_configuration() -> Configuration<PaneKind> {
    Configuration::Split {
        // Axis::Vertical divides left/right (`axis.rs:48-71`); a: left,
        // b: the nested center|right split (`node.rs:11-27`).
        axis: pane_grid::Axis::Vertical,
        ratio: DEFAULT_ROOT_SPLIT_RATIO,
        a: Box::new(Configuration::Pane(PaneKind::Left)),
        b: Box::new(Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: DEFAULT_NESTED_SPLIT_RATIO,
            a: Box::new(Configuration::Pane(PaneKind::Center)),
            b: Box::new(Configuration::Pane(PaneKind::Right)),
        }),
    }
}

/// Builds the middle-band state together with the discovered pane/split ids.
pub fn middle_band_state() -> (pane_grid::State<PaneKind>, MiddleBandPanes) {
    let panes = pane_grid::State::with_configuration(middle_band_configuration());

    let mut pane_ids = panes.panes.keys().copied();
    let left = pane_ids
        .next()
        .expect("middle band configuration always contains the left pane");
    let center = pane_ids
        .next()
        .expect("middle band configuration always contains the center pane");
    let right = pane_ids
        .next()
        .expect("middle band configuration always contains the right pane");

    // Root-first DFS (`node.rs:59-76`): the outer split (left | rest) is
    // yielded before the nested (center | right) split. The `Copy` ids are
    // collected so the borrowed iterator is released before `panes` moves
    // into the returned tuple.
    let split_ids: Vec<Split> = panes.layout().splits().copied().collect();
    let left_split = split_ids
        .first()
        .copied()
        .expect("middle band configuration always contains the left split");
    let right_split = split_ids
        .get(1)
        .copied()
        .expect("middle band configuration always contains the right split");

    (
        panes,
        MiddleBandPanes {
            left,
            center,
            right,
            left_split,
            right_split,
        },
    )
}

/// Which panel border a resize event's [`Split`] moves. [RUST-011]: pub
/// item (shared by the update reduction and tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSide {
    /// The LEFT panel border (left pane | center group).
    Left,
    /// The RIGHT panel border (center pane | right pane).
    Right,
}

/// Classifies a resize event's split against the known middle-band splits.
///
/// [RUST-020]: pure. Returns `None` for any split this layout does not
/// own (defensive; keeps unknown-id handling out of the caller).
pub fn split_side(panes: &MiddleBandPanes, split: Split) -> Option<PaneSide> {
    if split == panes.left_split {
        Some(PaneSide::Left)
    } else if split == panes.right_split {
        Some(PaneSide::Right)
    } else {
        None
    }
}

/// Pixel width the LEADING (left) side of a split claims at `ratio`.
///
/// iced splits a region of width `region_width` by rounding
/// `ratio * region_width - spacing / 2` for the leading side and handing the
/// remainder minus the gutter to the trailing side
/// (`iced_widget-0.14.2/src/pane_grid/axis.rs:48-71`): the `spacing`-wide gap
/// is CENTRED on the split line, so half of it is charged to each neighbour.
///
/// [RUST-020]: pure. This is the forward half of the conversion;
/// [`split_ratio_for_width`] is its exact inverse.
#[must_use]
pub fn pane_width_from_ratio(ratio: f32, region_width: f32, spacing: f32) -> f32 {
    ratio * region_width - spacing / 2.0
}

/// Split ratio that reproduces a leading-side `width` inside `region_width`.
///
/// Inverse of [`pane_width_from_ratio`]: `ratio = (width + spacing / 2) /
/// region_width`. Feeding this to `pane_grid::State::resize` puts the split
/// line at the centre of the gutter that surrounds the requested width.
///
/// [RUST-020]: pure.
#[must_use]
pub fn split_ratio_for_width(width: f32, region_width: f32, spacing: f32) -> f32 {
    (width + spacing / 2.0) / region_width
}

/// Width of the nested (centre | right) region once the left pane and the
/// gutter the left split consumed are subtracted from the viewport.
///
/// The right split does not operate on the viewport: it operates on the region
/// the root split handed it, which is `viewport_width - left_width - spacing`
/// (`axis.rs:54-55`: the trailing region starts `spacing` past the leading
/// one). Treating it as `viewport_width - left_width` — the zero-spacing
/// premise defect #12 removes — overstates the right pane by one gutter.
///
/// [RUST-020]: pure.
#[must_use]
pub fn nested_region_width(viewport_width: f32, left_width: f32, spacing: f32) -> f32 {
    viewport_width - left_width - spacing
}

/// The left-pane width a freshly replayed grid actually holds.
///
/// `PanelLayout.left_panel_width` is `None` until the user drags or the tree
/// auto-fits, and while it is `None` the grid sits at
/// [`DEFAULT_ROOT_SPLIT_RATIO`] — [`apply_pane_widths`] deliberately does not
/// resize a split it has no remembered width for. Deriving the nested region
/// from `LEFT_PANEL_DEFAULT_WIDTH` in that state would therefore size the right
/// split against a region iced never hands it.
///
/// The error is a **ratio**, not a fixed number of pixels, because the two left
/// widths cross over as the window grows: `DEFAULT_ROOT_SPLIT_RATIO` gives
/// `0.25 × viewport − 1`, while the nominal default is a flat 280. So the
/// assumed region is too small at narrow viewports, exactly equal at
/// `viewport = 1124` (`0.25 × 1124 − 1 = 280`), and too large beyond it:
///
/// | viewport | left the grid holds | left assumed | true region / assumed |
/// |----------|--------------------|--------------|-----------------------|
/// | 800      | 199                | 280          | 599 / 518 = +15.6%    |
/// | 1000     | 249                | 280          | 749 / 718 = +4.3%     |
/// | 1200     | 299                | 280          | 899 / 918 = −2.1%     |
///
/// A replayed right pane therefore lands that multiple past the width the
/// reducer stored. Measured, by breaking this rule and reading the round-trip
/// guard `defect_12_a_right_drag_round_trips_before_any_left_width_is_remembered`
/// fail: at its 1000px viewport the reducer stored 214.4px and the grid laid the
/// pane out at 224px — **9.6px** off. The figure is a hair above the 9.3px the
/// region arithmetic alone predicts because iced rounds a laid-out width to a
/// whole pixel, so the number quoted here is the measured one, not the derived
/// one.
///
/// This is the single home of that rule for anything that has to **replay a
/// ratio into the grid** — [`apply_pane_resize`]'s right arm,
/// [`apply_pane_widths`] and the ToC auto-fit
/// ([`crate::ui_iced::panels::right::auto_fit_width`]) all read it
/// ([RUST-021]). A drag is a replay like any other: solving it against a
/// different left width than the grid holds makes the first right-drag land the
/// pane somewhere other than the width the reducer just stored.
///
/// [RUST-020]: pure.
#[must_use]
pub fn current_left_width(layout: &PanelLayout, viewport_width: f32) -> f32 {
    layout.left_panel_width.unwrap_or_else(|| {
        pane_width_from_ratio(
            DEFAULT_ROOT_SPLIT_RATIO,
            viewport_width,
            PANE_SPACING_PIXELS,
        )
    })
}

/// Reduces a `PaneGrid` resize event onto the panel widths (UI-016
/// clamping).
///
/// On success returns the `(split, ratio)` pair to replay into
/// `pane_grid::State::resize` (`iced_widget-0.14.2/src/pane_grid/state.rs:
/// 298-310`) so the widget geometry matches the CLAMPED width, not the raw
/// drag position. On `None` the caller must leave both the pane grid and
/// the [`PanelLayout`] untouched.
///
/// Semantics ([RUST-020]: pure apart from the `layout` parameter), with the
/// gutter of [`PANE_SPACING_PIXELS`] carried through every conversion:
/// - LEFT split — the split's region IS the viewport, so the drag asks for
///   `ratio * viewport_width - spacing / 2` pixels of left pane; that is
///   clamped to `LEFT_PANEL_MIN_WIDTH`..=`viewport * LEFT_PANEL_MAX_WIDTH_FRACTION`
///   and replayed as `(width + spacing / 2) / viewport_width`;
/// - RIGHT split — the split's region is [`nested_region_width`]
///   (`viewport_width - left_width - spacing`), and the right pane is the
///   TRAILING side, so the drag asks for `(1 - ratio) * region - spacing / 2`
///   pixels; that is clamped to
///   `RIGHT_PANEL_MIN_WIDTH`..=`viewport * RIGHT_PANEL_MAX_WIDTH_FRACTION` and
///   replayed as `1 - (width + spacing / 2) / region`;
/// - an unpersisted left width falls back to the width the grid actually holds
///   — the seeded [`DEFAULT_ROOT_SPLIT_RATIO`] — via [`current_left_width`]
///   (all constants from the crate-internal `ui::panel_layout`, UI-016);
/// - when the 20%/30% viewport cap is BELOW the panel minimum (tiny
///   viewports), the minimum wins so both side panels stay draggable;
/// - non-positive viewports reject the event (no zero-division).
///
/// Because the replay ratio is the exact inverse of the width conversion, a
/// drag that is not clamped round-trips: the width the reducer stores is the
/// width the grid lays out.
pub fn apply_pane_resize(
    panes: &MiddleBandPanes,
    event: pane_grid::ResizeEvent,
    viewport_width: f32,
    layout: &mut PanelLayout,
) -> Option<(Split, f32)> {
    if viewport_width <= 0.0 {
        return None;
    }

    match split_side(panes, event.split) {
        Some(PaneSide::Left) => {
            let desired = pane_width_from_ratio(event.ratio, viewport_width, PANE_SPACING_PIXELS);
            let width = clamped_width(
                desired,
                LEFT_PANEL_MIN_WIDTH,
                viewport_width * LEFT_PANEL_MAX_WIDTH_FRACTION,
            );
            layout.set_width(Some(width));
            Some((
                event.split,
                split_ratio_for_width(width, viewport_width, PANE_SPACING_PIXELS),
            ))
        }
        Some(PaneSide::Right) => {
            let left_width = current_left_width(layout, viewport_width);
            let region = nested_region_width(viewport_width, left_width, PANE_SPACING_PIXELS);
            if region <= 0.0 {
                return None;
            }
            let desired = pane_width_from_ratio(1.0 - event.ratio, region, PANE_SPACING_PIXELS);
            let width = clamped_width(
                desired,
                RIGHT_PANEL_MIN_WIDTH,
                viewport_width * RIGHT_PANEL_MAX_WIDTH_FRACTION,
            );
            layout.set_right_width(Some(width));
            Some((
                event.split,
                1.0 - split_ratio_for_width(width, region, PANE_SPACING_PIXELS),
            ))
        }
        None => None,
    }
}

/// Replays remembered panel widths into a freshly built pane grid (defect #12,
/// follow-up F-2).
///
/// At startup the grid boots at the hardcoded ratios of
/// [`middle_band_configuration`]; without this the widths restored from the
/// persisted snapshot reach [`PanelLayout`] but never reach the widget, so the
/// first frame always shows the default geometry.
///
/// Only widths that were actually remembered are replayed — a first run with
/// `None` widths keeps the configuration's own ratios. A remembered width is
/// replayed **verbatim**: [`PanelLayout`] is the single owner of the numbers the
/// grid must show (the `PanelLayout` ownership rule of src/app/AGENTS.md §3),
/// and a restore that silently resized the panels would not be the restore the
/// user made. The UI-016 bounds stay where they belong, on the *drag*
/// ([`apply_pane_resize`]) and on the tree auto-fit
/// ([`crate::ui_iced::update`]), never on the replay.
///
/// The nested split is replayed second, from the region the replayed left
/// width leaves behind, because that is the region iced will actually hand it.
///
/// [RUST-020]: mutates only `panes`; `layout` and `band` are read-only.
pub fn apply_pane_widths(
    panes: &mut pane_grid::State<PaneKind>,
    band: &MiddleBandPanes,
    layout: &PanelLayout,
    viewport_width: f32,
) {
    if viewport_width <= 0.0 {
        return;
    }

    // Only a remembered width is replayed; with none the grid keeps its seeded
    // ratio, and `current_left_width` reports THAT ratio's width so the nested
    // region below is the one iced actually hands the right split.
    let left_width = current_left_width(layout, viewport_width);
    if layout.left_panel_width.is_some() {
        panes.resize(
            band.left_split,
            split_ratio_for_width(left_width, viewport_width, PANE_SPACING_PIXELS),
        );
    }

    let region = nested_region_width(viewport_width, left_width, PANE_SPACING_PIXELS);
    if region <= 0.0 {
        return;
    }

    if let Some(width) = layout.right_panel_width {
        let ratio = 1.0 - split_ratio_for_width(width, region, PANE_SPACING_PIXELS);
        panes.resize(band.right_split, ratio.clamp(0.0, 1.0));
    }
}

/// Clamps a desired width to `min..=max`, with the minimum winning over the
/// maximum when the cap is itself below `min` (degenerate small viewports —
/// documented in [`apply_pane_resize`]).
///
/// [RUST-020]: pure.
///
/// Shared with the tree auto-fit
/// ([`crate::ui_iced::panels::left::auto_fit_width`]) so every UI-016
/// width in the iced layer obeys one clamping rule.
pub fn clamped_width(desired: f32, min: f32, max: f32) -> f32 {
    if max < min {
        // Degenerate viewport: the 20%/30% cap sits below the panel
        // minimum — the minimum wins so the side panel stays draggable.
        min
    } else {
        desired.clamp(min, max)
    }
}

/// Builds the middle band area: a [`pane_grid::PaneGrid`] over the
/// middle-band pane state whose three pane bodies are supplied by the caller.
///
/// Each body is a builder closure rather than a single `Element` because the
/// pane-grid view callback runs once per pane per frame and `Element` is not
/// `Clone`. The right (TOC) body is only mounted when
/// [`should_show_panel`] passes; the gate is evaluated here so the rule
/// stays in the one place the iced layer defines it
/// ([`crate::ui_iced::panels::right::should_show_panel`]).
///
/// `on_resize` maps a [`pane_grid::ResizeEvent`] into the caller's message
/// type (`on_resize` contract verified at
/// `iced_widget-0.14.2/src/pane_grid.rs:267-275`). The leeway handed to it is
/// [`RESIZE_LEEWAY_PIXELS`], i.e. the `UIF-PANE-001` grab band minus the
/// gutter this grid now paints.
///
/// The grid is backed by a container painted [`pane_gutter_fill`], so the
/// [`PANE_SPACING_PIXELS`] gap iced leaves between panes reads as the
/// pane-split divider instead of as more of the panes' own canvas
/// (defect #12).
///
/// **The two fills are a pair.** iced gives a pane no background, so the
/// panes must paint [`pane_canvas_fill`] themselves (in `fill_pane`) or the
/// gutter backing shows through the whole band instead of only in the seams.
/// That is the regression this branch shipped and fixed;
/// `defect_12_followup_the_middle_band_renders_canvas_with_gutter_seams_only`
/// pins it at pixel level.
pub fn middle_band<'a, Message: 'a>(
    panes: &'a pane_grid::State<PaneKind>,
    left: impl Fn() -> iced::Element<'a, Message> + 'a,
    center: impl Fn() -> iced::Element<'a, Message> + 'a,
    right: impl Fn() -> iced::Element<'a, Message> + 'a,
    has_toc: bool,
    has_selected_file: bool,
    on_resize: impl Fn(pane_grid::ResizeEvent) -> Message + 'a,
) -> iced::Element<'a, Message> {
    let toc_visible = should_show_panel(has_toc, has_selected_file);
    let grid: iced::Element<'a, Message> =
        pane_grid::PaneGrid::new(panes, move |_pane, kind, _is_maximized| {
            pane_grid::Content::new(pane_body(kind, left(), center(), right(), toc_visible))
        })
        .spacing(PANE_SPACING_PIXELS)
        .on_resize(RESIZE_LEEWAY_PIXELS, on_resize)
        .into();

    iced::widget::container(grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(gutter_style)
        .into()
}

/// Styles the container backing the pane grid so the inter-pane gutter paints
/// in [`pane_gutter_fill`].
///
/// **Correction (regression this branch shipped and then fixed).** This
/// function's doc used to claim "panes are drawn on top of it, so only the
/// gutter is ever visible". That was false: iced hands a pane no background of
/// its own, and [`fill_pane`] was an unstyled container. Backing the grid with
/// `color.border.subtle` therefore painted the divider colour across the entire
/// middle band — the canvas went from `surface.base` `RGB(9, 9, 11)` to
/// `border.subtle` `RGB(40, 40, 40)` everywhere the panes sit. The claim is only
/// true once every pane is opaque, which is what [`pane_canvas_style`] does.
fn gutter_style(_theme: &iced::Theme) -> ContainerStyle {
    ContainerStyle {
        background: Some(Background::Color(pane_gutter_fill())),
        ..ContainerStyle::default()
    }
}

/// Styles a pane so it paints the window canvas ([`pane_canvas_fill`]) behind its
/// body, occluding the gutter backing that `gutter_style` paints beneath it.
///
/// Without this the gutter colour reads through every pane and the whole middle
/// band turns gray. With it, the only place the gutter colour can appear is the
/// [`PANE_SPACING_PIXELS`] seam, which is the entire point of the gutter.
///
/// Painting `surface.base` is a visual no-op against the window, which also maps
/// to `surface.base` — the pane looks exactly as it did before defect #12.
pub fn pane_canvas_style(_theme: &iced::Theme) -> ContainerStyle {
    ContainerStyle {
        background: Some(Background::Color(pane_canvas_fill())),
        ..ContainerStyle::default()
    }
}

/// Renders the body of one middle-band pane. Both branches of the TOC gate
/// allocate a container (conditional-shape rule — the pane stays owned by
/// the pane grid either way).
fn pane_body<'a, Message: 'a>(
    kind: &PaneKind,
    left: iced::Element<'a, Message>,
    center: iced::Element<'a, Message>,
    right: iced::Element<'a, Message>,
    toc_visible: bool,
) -> iced::Element<'a, Message> {
    use iced::widget::{container, space};

    match kind {
        PaneKind::Left => fill_pane(left),
        PaneKind::Center => fill_pane(center),
        PaneKind::Right => {
            if toc_visible {
                fill_pane(right)
            } else {
                // Opaque for the same reason `fill_pane` is: with the TOC hidden
                // this area still sits over the gutter backing, and an unpainted
                // container here would show the divider colour instead of the
                // canvas.
                container(space::vertical())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(pane_canvas_style)
                    .into()
            }
        }
    }
}

/// Wraps a pane body so it owns the whole pane area, and makes the pane opaque.
///
/// The opacity is not decoration. iced gives a pane no background, and the grid
/// is backed with the gutter colour (see [`gutter_style`]), so an unstyled pane
/// here would let the divider colour through and gray out the whole middle band
/// — the regression that shipped with defect #12 and is fixed here.
fn fill_pane<'a, Message: 'a>(body: iced::Element<'a, Message>) -> iced::Element<'a, Message> {
    iced::widget::container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(pane_canvas_style)
        .into()
}

/// Builds the top band stub: a full-width `row![]` bar of fixed height.
pub fn top_band<'a, Message: 'a>() -> iced::Element<'a, Message> {
    band_bar()
}

/// Builds the bottom band stub: a full-width `row![]` bar of fixed height.
pub fn bottom_band<'a, Message: 'a>() -> iced::Element<'a, Message> {
    band_bar()
}

fn band_bar<'a, Message: 'a>() -> iced::Element<'a, Message> {
    use iced::widget::{container, row, text};

    container(row![text(STUB_TEXT)])
        .width(Length::Fill)
        .height(Length::Fixed(STUB_BAND_HEIGHT_POINTS))
        .into()
}
