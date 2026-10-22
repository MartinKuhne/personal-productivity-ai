//! Pure placement solver for floating cards (`UIF-CMENU-001`,
//! `UIF-FMENU-001`, `UIF-INP-001`, geometry half of `UIF-VER-002`).
//!
//! Given **what the card is anchored to**, **how big the card is** and
//! **what the viewport looks like**, [`solve`] returns where the card
//! goes. It also returns *why* it went there. That second part is the
//! reason [`Placement`] is a struct rather than a [`Point`]: the solver
//! computes four independent facts on every call — which side of the
//! anchor each axis chose, whether the roomier-side fallback engaged, and
//! whether the viewport clamped the result — and a bare origin throws all
//! of them away. Callers need them: the menu view needs `scrollable` to
//! decide whether to wrap the card in a scroll container, the cascade
//! controller needs the flip flags to place a submenu against a card that
//! has already moved, and a caller deciding whether to re-measure needs
//! to know the origin came from the anchor rule or from a clamp.
//!
//! There is no widget, no renderer and no mutable state in this module:
//! it is arithmetic over [`Point`], [`Size`] and [`Rectangle`] data,
//! which is what makes the `UIF-VER-002` boundary gate checkable down to
//! `640 × 480` without a window.
//!
//! # One root coordinate space
//!
//! Every input lives in the single root viewport coordinate space of
//! `UIF-WIN-002`: origin `(0, 0)` at the top-left of the Main Window
//! content area, `+x` right, `+y` down. The viewport is taken as a
//! [`Rectangle`] rather than a bare [`Size`] so a content area that does
//! not start at the origin still solves correctly; every rule below is
//! written against the rectangle's four inset edges, and for the
//! canonical `Rectangle::with_size(size)` the two forms agree exactly.
//!
//! ```text
//!   viewport.x ───────────────────────────────────── viewport.x + width
//!     │                                                    │
//!     │   near ────────────────────────────────── far      │   near = edge + space.sm
//!     │     │                                    │         │   far  = edge − space.sm
//!     │     │        the inset: where the card     │         │
//!     │     │        is allowed to touch          │         │
//! ```
//!
//! # The three anchors
//!
//! [`Anchor::Pointer`] — a context menu at the cursor (`UIF-CMENU-001`).
//! The card opens `+2, +2` from the pointer (`space.2xs`, so the cursor
//! rests just outside the card's near corner rather than on its first
//! row). It flips when the preferred side cannot hold the card:
//!
//! ```text
//!   horizontal:  x + 2 + w > viewport.right − 8   →  x − w − 2
//!   vertical:    y + 2 + h > viewport.bottom − 8  →  y − h − 2
//! ```
//!
//! [`Anchor::Control`] — a fixed menu on a trigger's control rectangle
//! (`UIF-FMENU-001`), and the submenu cascade (`UIF-FMENU-002`). Same
//! fit-and-flip machinery, but the preferred and mirrored origins come
//! from the [`ControlAlignment`] rather than from a cursor offset: below
//! the trigger with a `4px` gap (`space.xs`), right-edge aligned for a
//! navbar-right trigger, or hung beside a parent card and aligned to the
//! parent row's top edge for a submenu.
//!
//! [`Anchor::CursorProximal`] — an input dialog (`UIF-INP-001`). This is
//! the one anchor that **does not flip**. Pointer-invoked it opens
//! `+2, +2` from the point and is clamped to the margins; the dialog is
//! small enough that flipping it away from the cursor would be worse than
//! sliding it along. Keyboard-invoked it centres in the viewport and the
//! point is ignored — a dialog summoned without a pointer has no pointer
//! to be near.
//!
//! # Flipping is per-axis and independent
//!
//! Each axis is solved on its own, so a menu can flip horizontally
//! without flipping vertically, and both at once. There is no "quadrant"
//! chosen wholesale: the horizontal decision never looks at the vertical
//! one.
//!
//! # The scrollable fallback
//!
//! When neither side of the anchor fits on an axis, the solver picks the
//! **roomier** side, clamps to the `8px` margins and reports the card as
//! truncated. Roominess is the space available on each side of the anchor
//! *before* the card is placed — it does not depend on the card's size:
//!
//! ```text
//!   room on the preferred side = far − preferred_origin
//!   room on the mirrored side  = mirrored_origin + extent − near
//! ```
//!
//! **Tie-break: the preferred side wins.** An exact tie means the anchor
//! sits dead centre of the inset, and `UIF-CMENU-001` states the
//! preference as "Right and Below", so keeping the preferred direction is
//! the spec's own answer rather than a coin flip. It also means the
//! common case — a card centred in its own viewport — resolves without a
//! flip flag.
//!
//! `scrollable` is computed from the *resolved* geometry, not from the
//! decision that produced it: it is true exactly when the card's bottom
//! edge still sits past the bottom margin after clamping. A card that
//! neither side of the anchor could hold but that the clamp pulled fully
//! inside reports `scrollable == false`, because there is genuinely
//! nothing to scroll.
//!
//! # Degenerate inputs are defined, never panicked
//!
//! | Input | What it returns |
//! |---|---|
//! | card larger than the viewport inset | Leading edge pinned to the near margin, trailing edge overflows; `clamped_*` is `true` and `scrollable` is `true` vertically. The card is never pushed off-screen. |
//! | zero-size card | Resolves to the preferred origin, unclamped and unscrollable — a point card always "fits". |
//! | anchor outside the viewport | Origin clamped back inside the margins. The flip flags still follow the anchor's roominess, so a point far off the right edge reports the horizontal flip it asked for even though the clamp moved it. |
//! | viewport smaller than `2 × margin` | The inset is empty and no placement can satisfy an `8px` margin on both sides. The origin is pinned to `edge + margin` on that axis and the card is reported truncated. |
//! | non-finite input (`NaN`, `±∞`) | Sanitised to `0.0` before solving. The solver never returns `NaN`, which iced would propagate into collapsed layout. |
//! | both sides exactly equally roomy | Preferred side wins, no flip (see the tie-break above). |
//!
//! Unit tests live in the sibling `anchor_tests.rs` sidecar.

use crate::token::space;
use iced::{Point, Rectangle, Size};

/// Minimum margin every floating card keeps from every viewport edge
/// (`space.sm`, shared by `UIF-CMENU-001` and `UIF-INP-001`).
const VIEWPORT_MARGIN: f32 = space::SM;

/// Offset from the cursor to the card's near corner (`space.2xs`), so the
/// pointer rests just outside the card instead of on top of its first row.
const POINTER_OFFSET: f32 = space::XXS;

/// Gap between a trigger's control rectangle and the card it opens
/// (`space.xs`, `UIF-FMENU-001`).
const TRIGGER_GAP: f32 = space::XS;

/// Gap between a parent card and the submenu card hung beside it
/// (`space.2xs`, `UIF-FMENU-002`). The spec says only "adjacent";
/// `space.2xs` is the rung defined as the micro gap between an anchor and
/// a floating menu, which is exactly this relationship.
const SUBMENU_GAP: f32 = space::XXS;

/// Where a floating card is anchored, and therefore which placement rule
/// applies to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// A context menu hung off the pointer (`UIF-CMENU-001`).
    ///
    /// Opens `+2, +2` from `point` and flips to the other side of the
    /// cursor on either axis when the preferred side cannot hold the card.
    Pointer {
        /// Root-space pointer coordinate the menu is anchored to.
        point: Point,
    },
    /// A fixed menu hung off a trigger's control rectangle
    /// (`UIF-FMENU-001`), including a submenu hung off its parent card
    /// (`UIF-FMENU-002`).
    Control {
        /// Bounding rectangle of the trigger — or of the parent card, for
        /// a submenu.
        rect: Rectangle,
        /// Which side of that rectangle the card prefers, and how it lines
        /// up against it.
        alignment: ControlAlignment,
    },
    /// A cursor-proximal input dialog (`UIF-INP-001`).
    ///
    /// Never flips: the card is small, and sliding it along the viewport
    /// keeps it nearer the cursor than throwing it to the other side
    /// would.
    CursorProximal {
        /// Root-space point the dialog opens near, when pointer-invoked.
        point: Point,
        /// `true` when the dialog was summoned by a keyboard shortcut, in
        /// which case the dialog centres in the viewport and `point` is
        /// ignored.
        keyboard_invoked: bool,
    },
}

/// Vertical relationship between a fixed-menu card and its control
/// rectangle (`UIF-FMENU-001`, `UIF-FMENU-002`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VerticalAlign {
    /// Card below the trigger, separated by the `4px` trigger gap
    /// (`space.xs`). The spec's default for a fixed menu.
    #[default]
    Below,
    /// Card above the trigger, separated by the `4px` trigger gap.
    Above,
    /// Card's top edge level with the anchor's top edge — a submenu aligned
    /// to the parent row's top (`UIF-FMENU-002`).
    Top,
    /// Card's bottom edge level with the anchor's bottom edge.
    Bottom,
    /// Card centred vertically on the anchor. Has no mirrored
    /// alternative, so it can only ever be clamped, never flipped.
    Middle,
}

/// Horizontal relationship between a fixed-menu card and its control
/// rectangle (`UIF-FMENU-001`, `UIF-FMENU-002`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HorizontalAlign {
    /// Card's left edge level with the anchor's left edge.
    #[default]
    Start,
    /// Card's right edge level with the anchor's right edge — the navbar's
    /// right-side menus, whose card hangs down and left from the trigger.
    End,
    /// Card centred horizontally on the anchor. Has no mirrored
    /// alternative, so it can only ever be clamped, never flipped.
    Center,
    /// Card hung immediately right of the anchor across the submenu gap —
    /// a submenu opening away from its parent card.
    Trailing,
    /// Card hung immediately left of the anchor across the submenu gap —
    /// what `Trailing` flips to near the right viewport edge.
    Leading,
}

/// How a fixed-menu card prefers to sit against its control rectangle.
///
/// The preference is only a preference: when the preferred side cannot
/// hold the card, the solver mirrors this alignment to its opposite
/// (`Below` ↔ `Above`, `Start` ↔ `End`, `Trailing` ↔ `Leading`) before
/// it reaches for the fallback. `Center` and `Middle` have no opposite,
/// so they are clamped rather than flipped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlAlignment {
    /// Preferred vertical relationship.
    pub vertical: VerticalAlign,
    /// Preferred horizontal relationship.
    pub horizontal: HorizontalAlign,
}

impl ControlAlignment {
    /// Alignment with the given vertical and horizontal preferences.
    #[must_use]
    pub const fn new(vertical: VerticalAlign, horizontal: HorizontalAlign) -> Self {
        Self {
            vertical,
            horizontal,
        }
    }

    /// The spec's fixed-menu default: open below the trigger, left edges
    /// level (`UIF-FMENU-001`).
    #[must_use]
    pub const fn below_start() -> Self {
        Self::new(VerticalAlign::Below, HorizontalAlign::Start)
    }

    /// A navbar-right menu: open below the trigger with the card's right
    /// edge level with the trigger's right edge (`UIF-FMENU-001`).
    #[must_use]
    pub const fn below_end() -> Self {
        Self::new(VerticalAlign::Below, HorizontalAlign::End)
    }

    /// A submenu opening to the right of its parent card, aligned to the
    /// parent row's top edge (`UIF-FMENU-002`).
    #[must_use]
    pub const fn submenu() -> Self {
        Self::new(VerticalAlign::Top, HorizontalAlign::Trailing)
    }
}

/// The resolved placement of a card, plus the reasons it resolved there.
///
/// The origin alone is not an answer: two different anchors can produce
/// the same origin through different rules, and the caller has to know
/// which. A card clamped to the left margin and a card deliberately
/// flipped to the left of its cursor sit at the same `x` and need
/// different treatment downstream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Top-left corner of the card in the root coordinate space.
    pub origin: Point,
    /// The card size this placement was solved for, sanitised the same way
    /// [`solve`] sanitises its input. Carried so [`Placement::rect`] needs
    /// no second argument and so an assertion can name the card it is
    /// talking about.
    pub size: Size,
    /// The card opened on the mirrored side of the anchor horizontally —
    /// left of the cursor for a context menu, `Leading` instead of
    /// `Trailing` for a submenu.
    pub flipped_horizontal: bool,
    /// The card opened on the mirrored side of the anchor vertically —
    /// above the cursor for a context menu, `Above` instead of `Below` for
    /// a fixed menu.
    pub flipped_vertical: bool,
    /// The card's bottom edge cannot be brought inside the bottom margin,
    /// so the card must scroll vertically inside itself
    /// (`UIF-CMENU-001` scrollable fallback).
    pub scrollable: bool,
    /// The viewport constrained the horizontal axis: the origin was pushed
    /// to a margin, or the card is wider than the inset and overflows.
    pub clamped_horizontal: bool,
    /// The viewport constrained the vertical axis: the origin was pushed to
    /// a margin, or the card is taller than the inset and overflows.
    pub clamped_vertical: bool,
}

impl Placement {
    /// The full card rectangle at the resolved origin.
    #[must_use]
    pub fn rect(&self) -> Rectangle {
        Rectangle::new(self.origin, self.size)
    }

    /// Did either axis flip?
    #[must_use]
    pub fn was_flipped(&self) -> bool {
        self.flipped_horizontal || self.flipped_vertical
    }

    /// Did the viewport constrain either axis?
    #[must_use]
    pub fn was_clamped(&self) -> bool {
        self.clamped_horizontal || self.clamped_vertical
    }

    /// How much of the card's height is inside the viewport's inset
    /// bottom edge — the height a scroll container inside the card gets.
    ///
    /// `0.0` when the card sits entirely below the inset, and the card's
    /// own height when it fits. This is what `scrollable` costs: the
    /// difference between the two is the content the user scrolls to
    /// reach.
    #[must_use]
    pub fn visible_height(&self, viewport: Rectangle) -> f32 {
        let far = viewport.y + viewport.height - VIEWPORT_MARGIN;
        (far - self.origin.y).clamp(0.0, self.size.height)
    }
}

/// Solve where `card_size` anchored at `anchor` lands inside `viewport`
/// (`UIF-CMENU-001`, `UIF-FMENU-001`, `UIF-INP-001`).
///
/// Pure: the same three inputs always produce the same [`Placement`], and
/// nothing outside the return value is touched. See the [module docs]
/// (self) for the rules each anchor kind follows and for what every
/// degenerate input returns.
#[must_use]
pub fn solve(anchor: Anchor, card_size: Size, viewport: Rectangle) -> Placement {
    let card = sanitize_size(card_size);
    let view = sanitize_rect(viewport);

    let near_x = view.x + VIEWPORT_MARGIN;
    let far_x = view.x + view.width - VIEWPORT_MARGIN;
    let near_y = view.y + VIEWPORT_MARGIN;
    let far_y = view.y + view.height - VIEWPORT_MARGIN;

    match anchor {
        Anchor::Pointer { point } => {
            let point = sanitize_point(point);
            let x = solve_axis(
                point.x + POINTER_OFFSET,
                Some(point.x - card.width - POINTER_OFFSET),
                card.width,
                near_x,
                far_x,
            );
            let y = solve_axis(
                point.y + POINTER_OFFSET,
                Some(point.y - card.height - POINTER_OFFSET),
                card.height,
                near_y,
                far_y,
            );
            assemble(x, y, card)
        }
        Anchor::Control { rect, alignment } => {
            let rect = sanitize_rect(rect);
            let (preferred_x, mirror_x) = horizontal_rule(alignment.horizontal, rect, card.width);
            let (preferred_y, mirror_y) = vertical_rule(alignment.vertical, rect, card.height);
            let x = solve_axis(preferred_x, mirror_x, card.width, near_x, far_x);
            let y = solve_axis(preferred_y, mirror_y, card.height, near_y, far_y);
            assemble(x, y, card)
        }
        Anchor::CursorProximal {
            point,
            keyboard_invoked,
        } => {
            let point = sanitize_point(point);
            // Keyboard-invoked dialogs centre; pointer-invoked ones open at
            // the cursor offset. Neither flips — `UIF-INP-001` clamps.
            let (preferred_x, preferred_y) = if keyboard_invoked {
                (
                    view.x + (view.width - card.width) / 2.0,
                    view.y + (view.height - card.height) / 2.0,
                )
            } else {
                (point.x + POINTER_OFFSET, point.y + POINTER_OFFSET)
            };
            let x = solve_axis(preferred_x, None, card.width, near_x, far_x);
            let y = solve_axis(preferred_y, None, card.height, near_y, far_y);
            assemble(x, y, card)
        }
    }
}

/// The preferred and mirrored origins for one horizontal relationship.
///
/// `None` for the mirrored origin means the relationship has no opposite
/// to fall back to, so the axis can only be clamped.
fn horizontal_rule(alignment: HorizontalAlign, rect: Rectangle, width: f32) -> (f32, Option<f32>) {
    match alignment {
        HorizontalAlign::Start => (rect.x, Some(rect.x + rect.width - width)),
        HorizontalAlign::End => (rect.x + rect.width - width, Some(rect.x)),
        HorizontalAlign::Center => (rect.x + (rect.width - width) / 2.0, None),
        HorizontalAlign::Trailing => (
            rect.x + rect.width + SUBMENU_GAP,
            Some(rect.x - width - SUBMENU_GAP),
        ),
        HorizontalAlign::Leading => (
            rect.x - width - SUBMENU_GAP,
            Some(rect.x + rect.width + SUBMENU_GAP),
        ),
    }
}

/// The preferred and mirrored origins for one vertical relationship.
fn vertical_rule(alignment: VerticalAlign, rect: Rectangle, height: f32) -> (f32, Option<f32>) {
    match alignment {
        VerticalAlign::Below => (
            rect.y + rect.height + TRIGGER_GAP,
            Some(rect.y - height - TRIGGER_GAP),
        ),
        VerticalAlign::Above => (
            rect.y - height - TRIGGER_GAP,
            Some(rect.y + rect.height + TRIGGER_GAP),
        ),
        VerticalAlign::Top => (rect.y, Some(rect.y + rect.height - height)),
        VerticalAlign::Bottom => (rect.y + rect.height - height, Some(rect.y)),
        VerticalAlign::Middle => (rect.y + (rect.height - height) / 2.0, None),
    }
}

/// One axis of the placement decision.
struct Axis {
    origin: f32,
    flipped: bool,
    clamped: bool,
    truncated: bool,
}

/// Solve one axis against the viewport's inset edges.
///
/// `preferred` is where the card's leading edge wants to sit; `mirror` is
/// where it would sit if the relationship were mirrored, or `None` when
/// there is no mirrored relationship. `near` and `far` are the inset
/// edges: `viewport_edge + margin` and `viewport_edge + extent − margin`.
///
/// Three outcomes, in order:
///
/// 1. the preferred side fits → take it, no flip;
/// 2. the mirrored side fits → take it, flipped;
/// 3. neither fits → take the roomier side (ties to preferred) and let
///    the clamp pull the card back inside the margins.
fn solve_axis(preferred: f32, mirror: Option<f32>, extent: f32, near: f32, far: f32) -> Axis {
    let fits = |origin: f32| origin >= near && origin + extent <= far;
    // A card larger than the inset leaves no legal range at all; collapse
    // it onto the near margin so the card's leading edge stays on screen
    // rather than letting the range invert.
    let upper = (far - extent).max(near);

    let (candidate, flipped) = if fits(preferred) {
        (preferred, false)
    } else if let Some(mirror) = mirror {
        if fits(mirror) {
            (mirror, true)
        } else {
            let room_preferred = far - preferred;
            let room_mirror = mirror + extent - near;
            if room_mirror > room_preferred {
                (mirror, true)
            } else {
                (preferred, false)
            }
        }
    } else {
        (preferred, false)
    };

    let origin = candidate.clamp(near, upper);
    let truncated = origin + extent > far;
    Axis {
        origin,
        flipped,
        // The viewport constrained this axis either by moving the origin
        // or by being too small to hold the card where the rule wanted it.
        clamped: origin != candidate || truncated,
        truncated,
    }
}

/// Fold the two solved axes into a [`Placement`].
fn assemble(x: Axis, y: Axis, card: Size) -> Placement {
    Placement {
        origin: Point::new(x.origin, y.origin),
        size: card,
        flipped_horizontal: x.flipped,
        flipped_vertical: y.flipped,
        scrollable: y.truncated,
        clamped_horizontal: x.clamped,
        clamped_vertical: y.clamped,
    }
}

/// A non-finite scalar is meaningless as a coordinate; treat it as `0.0`.
///
/// iced does not guarantee finite geometry — an unmeasured widget or a
/// division by a zero scale can hand over `NaN` or `±∞` — and either one
/// propagates through layout silently and collapses whatever it touches.
/// Landing on `0.0` instead keeps the solver a total function whose worst
/// case is a card pinned to the viewport origin.
fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// A negative extent is not a size; treat it as `0.0`.
fn non_negative(value: f32) -> f32 {
    finite(value).max(0.0)
}

/// Sanitise a point. Coordinates may legitimately be negative — an anchor
/// outside the viewport is a defined case — so only non-finite values are
/// rewritten.
fn sanitize_point(point: Point) -> Point {
    Point::new(finite(point.x), finite(point.y))
}

/// Sanitise a size: finite, and neither dimension negative.
fn sanitize_size(size: Size) -> Size {
    Size::new(non_negative(size.width), non_negative(size.height))
}

/// Sanitise a rectangle: position finite, extents non-negative.
fn sanitize_rect(rect: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(finite(rect.x), finite(rect.y)),
        sanitize_size(Size::new(rect.width, rect.height)),
    )
}

#[cfg(test)]
#[path = "anchor_tests.rs"]
mod anchor_tests;
