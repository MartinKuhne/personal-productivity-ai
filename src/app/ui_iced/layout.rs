//! Iced pane-grid layout of the FASTMD three-band geometry ([TASK-010],
//! UI-001, `doc/planning/iced.md`).
//!
//! The pilot view is a vertical `column![]` of three full-width bands:
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
//!   [TASK-010] public path stays stable ([RUST-051]: one iced-side home for
//!   the gate);
//! - pane-grid resize drag events ([`pane_grid::ResizeEvent`] fields
//!   `split`/`ratio` — `iced_widget-0.14.2/src/pane_grid.rs:1226-1237`, fed
//!   through `PaneGrid::on_resize(leeway, f)` — `pane_grid.rs:267-275`)
//!   reduce 1:1 onto `crate::ui::PanelLayout` widths (`ui/panel_layout.rs:
//!   26-30`), clamped by the UI-016 constants re-verified at
//!   `ui/panel_layout.rs:5-23`;
//! - the widget itself assumes zero intra-pane spacing (the [`pane_grid::PaneGrid`]
//!   default, `pane_grid.rs:202`), so pixel widths convert to split ratios
//!   via `ratio = width / viewport_width` without a spacing term.
//!
//! Widget construction is exercised by the pilot binary only; these
//! unit-testable pieces are the state reductions ([TASK-010] testing rule).
//!
//! Unit tests live in the sibling `layout_tests.rs` sidecar.

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;

use crate::ui::panel_layout::{
    LEFT_PANEL_DEFAULT_WIDTH, LEFT_PANEL_MAX_WIDTH_FRACTION, LEFT_PANEL_MIN_WIDTH, PanelLayout,
    RIGHT_PANEL_MAX_WIDTH_FRACTION, RIGHT_PANEL_MIN_WIDTH,
};
use iced::Length;
use iced::widget::pane_grid::{self, Configuration, Pane, Split};
/// Grab-leeway (logical pixels) around each split line for resize drags,
/// passed to `PaneGrid::on_resize` (`iced_widget-0.14.2/src/pane_grid.rs:
/// 258-266`: the grab area length is `spacing + leeway`).
pub const RESIZE_LEEWAY_PIXELS: f32 = 10.0;

/// Full-on-screen height of the stub top/bottom bands, in logical points.
pub const STUB_BAND_HEIGHT_POINTS: f32 = 28.0;

/// Text rendered inside every stub region ([TASK-010]); dev-scaffolding
/// label, not user-facing copy — promoted from an inline literal per
/// [RUST-021] so the stub wording lives in exactly one place.
pub const STUB_TEXT: &str = "stub";

/// Text rendered inside the right (TOC) pane when
/// [`should_show_panel`] passes; see [`STUB_TEXT`].
pub const TOC_STUB_TEXT: &str = "toc";

/// Pure visibility gate for the right (TOC) pane.
///
/// Relocated into the iced tree at [`crate::ui_iced::panels::right`] for the
/// zero-egui migration ([TASK-029]) and re-exported here, so the
/// [`layout`](crate::ui_iced::layout) public path [TASK-010] established is
/// unchanged and [`view`](crate::ui_iced::view) needs no edit.
pub use crate::ui_iced::panels::right::should_show_panel;

/// Contents of each middle-band pane ([TASK-010]) — [RUST-011]: pub item.
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

/// Builds the middle-band pane tree: a vertical split of (left | (center |
/// right)) ([TASK-010], UI-001).
///
/// Tree construction ([RUST-020]): pure — a fresh [`Configuration`] per
/// call, so callers own the mutable [`pane_grid::State`].
pub fn middle_band_configuration() -> Configuration<PaneKind> {
    Configuration::Split {
        // Axis::Vertical divides left/right (`axis.rs:48-71`); a: left,
        // b: the nested center|right split (`node.rs:11-27`).
        axis: pane_grid::Axis::Vertical,
        ratio: 0.25,
        a: Box::new(Configuration::Pane(PaneKind::Left)),
        b: Box::new(Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.5,
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
/// [RUST-020]: pure. Returns `None` for any splitting the pilot does not
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

/// Reduces a `PaneGrid` resize event onto the panel widths ([TASK-010],
/// UI-016 clamping).
///
/// On success returns the `(split, ratio)` pair to replay into
/// `pane_grid::State::resize` (`iced_widget-0.14.2/src/pane_grid/state.rs:
/// 298-310`) so the widget geometry matches the CLAMPED width, not the raw
/// drag position. On `None` the caller must leave both the pane grid and
/// the [`PanelLayout`] untouched.
///
/// Semantics ([RUST-020]: pure apart from the `layout` parameter):
/// - LEFT split: `ratio * viewport_width` pixels, clamped to
///   `LEFT_PANEL_MIN_WIDTH`..=`viewport * LEFT_PANEL_MAX_WIDTH_FRACTION`;
/// - RIGHT split: `(1 - ratio) * available` pixels where
///   `available = viewport_width - left_width`, clamped to
///   `RIGHT_PANEL_MIN_WIDTH`..=`viewport * RIGHT_PANEL_MAX_WIDTH_FRACTION`;
///   an unpersisted left width falls back to `LEFT_PANEL_DEFAULT_WIDTH`
///   (all constants from the crate-internal `ui::panel_layout`, UI-016);
/// - when the 20%/30% viewport cap is BELOW the panel minimum (tiny
///   viewports), the minimum wins so both side panels stay draggable;
/// - non-positive viewports reject the event (no zero-division).
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
            let desired = event.ratio * viewport_width;
            let width = clamped_width(
                desired,
                LEFT_PANEL_MIN_WIDTH,
                viewport_width * LEFT_PANEL_MAX_WIDTH_FRACTION,
            );
            layout.set_width(Some(width));
            Some((event.split, width / viewport_width))
        }
        Some(PaneSide::Right) => {
            let left_width = layout.left_panel_width.unwrap_or(LEFT_PANEL_DEFAULT_WIDTH);
            let available = viewport_width - left_width;
            if available <= 0.0 {
                return None;
            }
            let desired = (1.0 - event.ratio) * available;
            let width = clamped_width(
                desired,
                RIGHT_PANEL_MIN_WIDTH,
                viewport_width * RIGHT_PANEL_MAX_WIDTH_FRACTION,
            );
            layout.set_right_width(Some(width));
            Some((event.split, 1.0 - width / available))
        }
        None => None,
    }
}

/// Clamps a desired width to `min..=max`, with the minimum winning over the
/// maximum when the cap is itself below `min` (degenerate small viewports —
/// documented in [`apply_pane_resize`]).
///
/// [RUST-020]: pure.
///
/// Shared with the [TASK-012] tree auto-fit
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

/// Builds the middle band area ([TASK-010], [TASK-023]): a
/// [`pane_grid::PaneGrid`] over the pilot's pane state whose three pane
/// bodies are supplied by the caller.
///
/// Each body is a builder closure rather than a single `Element` because the
/// pane-grid view callback runs once per pane per frame and `Element` is not
/// `Clone`. The right (TOC) body is only mounted when
/// [`should_show_panel`] passes; the gate is evaluated here so the rule
/// stays in the one place the iced layer defines it
/// ([`crate::ui_iced::panels::right::should_show_panel`], [TASK-029]).
///
/// `on_resize` maps a [`pane_grid::ResizeEvent`] into the caller's message
/// type (`on_resize` contract verified at
/// `iced_widget-0.14.2/src/pane_grid.rs:267-275`).
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
    pane_grid::PaneGrid::new(panes, move |_pane, kind, _is_maximized| {
        pane_grid::Content::new(pane_body(kind, left(), center(), right(), toc_visible))
    })
    .on_resize(RESIZE_LEEWAY_PIXELS, on_resize)
    .into()
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
                container(space::vertical())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        }
    }
}

/// Wraps a pane body so it owns the whole pane area.
fn fill_pane<'a, Message: 'a>(body: iced::Element<'a, Message>) -> iced::Element<'a, Message> {
    iced::widget::container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Builds the top band stub ([TASK-010]): a full-width `row![]` bar of
/// fixed height.
pub fn top_band<'a, Message: 'a>() -> iced::Element<'a, Message> {
    band_bar()
}

/// Builds the bottom band stub ([TASK-010]): a full-width `row![]` bar of
/// fixed height.
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
