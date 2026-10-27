//! Reachability clamps for movable overlay cards (`UIF-MOV-002`).
//!
//! # What the clamp protects: the title bar, not the card
//!
//! A movable overlay — the Background Operations Log, the Agent Debug
//! Inspector — is a card the user drags around a window that is often
//! smaller than the card would like to be. The card may legitimately hang
//! off the right edge or the bottom of the viewport: its content scrolls,
//! and losing the bottom of a log view costs nothing a scroll wheel cannot
//! restore. What must never be lost is the `32px` title bar, because the
//! title bar is the only grip the user has to bring the card back. A card
//! whose title bar is off-screen cannot be grabbed, dragged, resized or
//! closed; it is functionally gone until the window is resized or the
//! application restarts.
//!
//! That is why this module is *not* "fit the rectangle inside the
//! viewport". A fit-the-card rule would be too strict — a `680 × 440`
//! card could not exist in a `640 × 480` window at all — and, worse,
//! backwards: it would let the card's *bottom* edge decide where its
//! *top* edge goes.
//!
//! So only the title bar's visibility is constrained, and the two axes
//! are solved independently:
//!
//! * **Horizontal** — at least `120px` of the title bar's width stays
//!   inside `[viewport.x, viewport.x + viewport.width]`. This is a rule
//!   about the *visible* width, so it is measured against the card's own
//!   width rather than assuming the card is wide enough.
//! * **Vertical** — the card's top edge stays within
//!   `[viewport.y + navbar_height, viewport.y + viewport.height − title_bar_height]`:
//!   never under the navbar, never so far down that the title bar itself
//!   leaves the screen. The card's height never enters this bound.
//!
//! ```text
//!      0 ┊←────── viewport.width ──────→ viewport.x + width
//!        ┊                              │
//!   ─────╋──────────────────────────────┼────  viewport.y
//!        ┊   navbar (36px)             │
//!   ─────╋─ top bound ─────────────────┼────  y may not go above this
//!        ┊   ┌───────────────────┐     │
//!        ┊   │ title bar (32px)  │     │   ← the 120px that must show
//!        ┊   ├───────────────────┤     │
//!        ┊   │                   │     │   the body may run past the
//!        ┊   │                   ├─────┼─── bottom bound: y ≤ height−32
//!        ┊   │                   │     │   keeps the title bar on screen;
//!        ┊   └───────────────────┘     │   the card itself may hang off
//!   ─────╋─────────────────────────────┼────  viewport bottom
//! ```
//!
//! # The API
//!
//! * [`clamp_position`] — the drag case: a requested origin plus a card
//!   size, returned as the nearest origin whose title bar is reachable.
//! * [`drag_position`] — the same clamp expressed as a per-frame drag step,
//!   which is where the no-accumulation contract lives (see below).
//! * [`clamp_rect`] — reachability only, for a caller that already has a
//!   sized rectangle and wants its origin fixed. This is the case a caller
//!   usually wants.
//! * [`clamp_size`] — the resize case: the card's size is clamped to the
//!   [`SizePolicy`] first, then the resulting rectangle is made reachable.
//!   The top-left may itself move, because shrinking a card that hangs off
//!   the left can pull its title bar back on screen.
//! * [`resize`] — a resize drag from any of the eight [`ResizeHandle`]
//!   edges and corners, where one edge is held still and the opposite one
//!   follows the pointer.
//! * [`is_reachable`] — the invariant as a predicate, so a caller can
//!   *assert* the title bar survived rather than trusting that it did.
//! * [`title_bar_rect`] and [`visible_title_width`] — the two measurements
//!   the invariant is stated in.
//!
//! # The overshoot contract (`UIF-MOV-001` + `UIF-MOV-002`)
//!
//! A drag must not make the user pay back an overshoot. Drag the card
//! `500px` past the left edge and then `10px` back: the card should move
//! `10px`, not stay pinned until the pointer has re-travelled `500px`.
//!
//! A pure clamp cannot deliver that on its own — it has no memory. The
//! contract is on the caller: **clamp from the last clamped position, never
//! from the drag-start origin plus a running total.**
//!
//! ```text
//!   correct:   pos = clamp_position(pos + frame_delta, …)   each frame
//!   wrong:     pos = clamp_position(drag_start + total_delta, …)
//! ```
//!
//! With the correct form the blocked delta is discarded at the moment it is
//! blocked, because the value it was added to was already sitting on the
//! bound. [`drag_position`] implements exactly that step and clamps its
//! `current` input on the way in, so the contract holds even if the caller
//! lost track. A caller that wants to know whether a frame was blocked
//! compares the returned point with the point it asked for: unequal means
//! the axis absorbed the delta.
//!
//! # Degenerate inputs are defined, never panicked
//!
//! | Input | What it returns |
//! |---|---|
//! | card narrower than the `120px` visible-title requirement | The requirement degrades to the card's own width: the card is clamped **fully visible** horizontally. A `50px` card cannot show `120px` of title bar, so the solver asks for all `50px` it has. |
//! | viewport narrower than the requirement | The requirement degrades to the viewport width: the card is pinned so that it covers the whole viewport, which is the most title bar any position can show. |
//! | viewport shorter than `navbar + title bar` (`68px`) | The vertical interval is empty. The card is pinned to the top bound — "never above the navbar" is the harder half of the rule, so it is the one kept. |
//! | zero-size card | The requirement is `0`; the origin is clamped inside the viewport on both axes. |
//! | card far outside the viewport | Pulled back to the nearest legal position on each axis independently. |
//! | resize below the size-policy minimum | The extent stops at the minimum and the dragged edge stops with it; the held edge never crosses it. |
//! | resize against a maximum | The extent stops at the maximum. A maximum below the minimum is raised to the minimum rather than inverting. |
//! | non-finite maximum size | Treated as unbounded, not as zero. |
//! | resize from an already-unreachable card | The final reachability pass pulls the card back. Reachability outranks the held edge, so that edge may shift by the difference. |
//! | non-finite input (`NaN`, `±∞`) | Sanitised: coordinates to `0.0`, negative extents to `0.0`. Nothing non-finite ever leaves the solver, because iced would propagate it into collapsed layout. |
//!
//! Unit tests live in the sibling `clamp_tests.rs` sidecar.

use crate::token::size;
use iced::{Point, Rectangle, Size, Vector};

/// `UIF-MOV-002` — the width of title bar that must stay inside the
/// viewport.
///
/// No token covers this value. [`crate::token::size`] describes control
/// and icon rungs, not a reachability floor, and this increment may not
/// add one; `120` is a requirement of the invariant rather than a visual
/// size. It is named here so the rule reads as the requirement it is
/// instead of as a bare `120.0`.
const MIN_VISIBLE_TITLE: f32 = 120.0;

/// Height of the Top Navbar the card may never be dragged above
/// (`UIF-MOV-002`'s `navbar_height`), read from `size.control.lg` — the
/// rung the token module already documents as the Top Navbar height.
const NAVBAR_HEIGHT: f32 = size::CONTROL_LG;

/// Height of the draggable title bar (§4.7.1), read from
/// `size.control.md`.
const TITLE_BAR_HEIGHT: f32 = size::CONTROL_MD;

/// §4.7.1 minimum overlay card width (`Min: 420×260px`).
///
/// No token exists for a whole-card minimum: the token set sizes controls,
/// not surfaces, and the card's *default* (`680 × 440`) is a caller's
/// choice rather than an invariant. Only the minimum is enforced here,
/// because only the minimum is what the clamp has to know.
const CARD_MIN_WIDTH: f32 = 420.0;

/// §4.7.1 minimum overlay card height.
const CARD_MIN_HEIGHT: f32 = 260.0;

/// The card's min/max size policy, as enforced by [`clamp_size`] and
/// [`resize`].
///
/// The viewport is deliberately *not* a size constraint for a movable
/// overlay: the card may be larger than the window and hang off its edges,
/// which [`clamp_position`] handles by keeping the title bar visible.
/// Only the caller's own policy bounds the size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizePolicy {
    /// Smallest size the card may be resized to.
    pub min: Size,
    /// Largest size the card may be resized to. `None` is unbounded.
    pub max: Option<Size>,
}

impl SizePolicy {
    /// The §4.7.1 movable-overlay policy: a `420 × 260` floor and no
    /// ceiling, because the spec bounds the card from below only.
    pub const MOVABLE: Self = Self {
        min: Size::new(CARD_MIN_WIDTH, CARD_MIN_HEIGHT),
        max: None,
    };

    /// A policy with both a floor and a ceiling.
    #[must_use]
    pub const fn bounded(min: Size, max: Size) -> Self {
        Self {
            min,
            max: Some(max),
        }
    }

    /// Clamp a size into this policy, per axis.
    ///
    /// A non-finite ceiling is read as unbounded, and a ceiling below the
    /// floor is raised to it, so a misconfigured policy can never collapse
    /// into an empty range.
    #[must_use]
    pub fn clamp(self, size: Size) -> Size {
        Size::new(
            clamp_extent(size.width, self.min.width, self.max.map(|max| max.width)),
            clamp_extent(size.height, self.min.height, self.max.map(|max| max.height)),
        )
    }
}

/// The reachability rule and size policy a movable overlay is clamped
/// against (`UIF-MOV-002`).
///
/// A value rather than a set of module constants so a caller can hold a
/// rule per surface class, and so a test can name the boundary it is
/// probing instead of editing the module.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClampRule {
    /// Minimum title-bar width that must remain inside the viewport
    /// (`MIN_VISIBLE_TITLE`).
    pub min_visible_title: f32,
    /// Top bound for the card's origin: the navbar height the card may
    /// never sit above (`NAVBAR_HEIGHT`).
    pub top_bound: f32,
    /// Height of the title bar, which decides how far down the card may
    /// go before its own grip leaves the screen (`TITLE_BAR_HEIGHT`).
    pub title_bar_height: f32,
    /// The card's min/max size policy, enforced by [`clamp_size`] and
    /// [`resize`].
    pub size_policy: SizePolicy,
}

impl ClampRule {
    /// The spec's movable-overlay rule: `120px` of title bar visible,
    /// `y` within `[navbar, viewport − title bar]`, `420 × 260` minimum
    /// card, no maximum.
    pub const MOVABLE: Self = Self {
        min_visible_title: MIN_VISIBLE_TITLE,
        top_bound: NAVBAR_HEIGHT,
        title_bar_height: TITLE_BAR_HEIGHT,
        size_policy: SizePolicy::MOVABLE,
    };
}

impl Default for ClampRule {
    fn default() -> Self {
        Self::MOVABLE
    }
}

/// The edge or corner a resize drag is holding (`UIF-MOV-002`).
///
/// A resize is always "this edge follows the pointer, the opposite edge
/// stays put". Naming the held edge is what lets the solver keep the
/// bottom-right corner still while the top-left is dragged, and vice
/// versa, without the caller re-deriving which corner moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeHandle {
    /// Top-left corner: the left and top edges move.
    TopLeft,
    /// Top edge only.
    Top,
    /// Top-right corner: the right and top edges move.
    TopRight,
    /// Right edge only.
    Right,
    /// Bottom-right corner: the right and bottom edges move.
    BottomRight,
    /// Bottom edge only.
    Bottom,
    /// Bottom-left corner: the left and bottom edges move.
    BottomLeft,
    /// Left edge only.
    Left,
}

impl ResizeHandle {
    /// All eight handles, clockwise from the top-left corner.
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    /// Does this handle drag the card's leading (left) edge?
    #[must_use]
    pub const fn drags_start_x(self) -> bool {
        matches!(self, Self::TopLeft | Self::Left | Self::BottomLeft)
    }

    /// Does this handle drag the card's trailing (right) edge?
    #[must_use]
    pub const fn drags_end_x(self) -> bool {
        matches!(self, Self::TopRight | Self::Right | Self::BottomRight)
    }

    /// Does this handle drag the card's top edge?
    #[must_use]
    pub const fn drags_start_y(self) -> bool {
        matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
    }

    /// Does this handle drag the card's bottom edge?
    #[must_use]
    pub const fn drags_end_y(self) -> bool {
        matches!(self, Self::BottomLeft | Self::Bottom | Self::BottomRight)
    }
}

/// Clamp a requested card origin so the title bar stays reachable
/// (`UIF-MOV-002`) — the drag case.
///
/// The card's size is an input because the horizontal rule is about the
/// *visible* title-bar width, which depends on how wide the card is.
///
/// The caller owns the accumulation: feed the clamped result back in as
/// the next frame's starting point (or call [`drag_position`], which does
/// it), never the drag-start origin plus a running total. See the
/// [overshoot contract](self).
#[must_use]
pub fn clamp_position(
    position: Point,
    card_size: Size,
    viewport: Rectangle,
    rule: ClampRule,
) -> Point {
    clamp_origin(
        sanitize_point(position),
        sanitize_size(card_size),
        sanitize_rect(viewport),
        rule,
    )
}

/// Advance a drag by one frame's pointer delta without accumulating
/// overshoot (`UIF-MOV-001` + `UIF-MOV-002`).
///
/// `current` is the card's *clamped* position from the previous frame; it
/// is clamped again here so the contract holds even if the caller passed
/// an unclamped value. The frame delta is added to that clamped position
/// and the sum is clamped, which is what makes a blocked delta vanish at
/// the moment it is blocked instead of being banked up to be paid back on
/// the way back.
///
/// The returned point differs from `current + delta` exactly when an axis
/// was blocked, which is how a caller can tell.
#[must_use]
pub fn drag_position(
    current: Point,
    delta: Vector,
    card_size: Size,
    viewport: Rectangle,
    rule: ClampRule,
) -> Point {
    let delta = sanitize_vector(delta);
    let anchored = clamp_position(current, card_size, viewport, rule);
    clamp_position(
        Point::new(anchored.x + delta.x, anchored.y + delta.y),
        card_size,
        viewport,
        rule,
    )
}

/// Make an existing card rectangle reachable without changing its size
/// (`UIF-MOV-002`).
///
/// This is the case a caller usually wants: the card has already been laid
/// out, only its origin may be wrong. The size is carried through
/// untouched — including a size the [`SizePolicy`] would reject. Use
/// [`clamp_size`] when the size itself is in question.
#[must_use]
pub fn clamp_rect(rect: Rectangle, viewport: Rectangle, rule: ClampRule) -> Rectangle {
    let rect = sanitize_rect(rect);
    let view = sanitize_rect(viewport);
    Rectangle::new(
        clamp_origin(rect.position(), rect.size(), view, rule),
        rect.size(),
    )
}

/// Clamp a card's size to the policy and then make the result reachable
/// (`UIF-MOV-002` + §4.7.1) — the resize case.
///
/// Order matters and is the whole point: sizing first can move the title
/// bar. A card pinned against the left of the screen that is shrunk to the
/// `420px` floor has its origin pulled back inside the viewport by the
/// reachability pass that follows, so the top-left corner moves even
/// though only the size was asked about.
#[must_use]
pub fn clamp_size(rect: Rectangle, viewport: Rectangle, rule: ClampRule) -> Rectangle {
    let rect = sanitize_rect(rect);
    let sized = Rectangle::new(rect.position(), rule.size_policy.clamp(rect.size()));
    clamp_rect(sized, viewport, rule)
}

/// Apply a resize drag held at `handle` and return the resulting card
/// (`UIF-MOV-002`).
///
/// The edge opposite the held one follows the pointer; the held edge stays
/// put until the policy or the invariant says otherwise. Three things can
/// stop the dragged edge: the size-policy floor, the size-policy ceiling,
/// and the reachability of the title bar.
///
/// When the held edge and the invariant disagree the invariant wins and
/// the held edge shifts — but with the spec's own [`SizePolicy::MOVABLE`]
/// they never do. Because the card's floor (`420px`) is above the
/// visible-title floor (`120px`), the requirement is `min(120, viewport)`
/// no matter how wide the card is drawn, so re-deriving the extent from
/// the held edge satisfies the invariant exactly. A disagreement needs a
/// policy whose width floor sits *below* the visible-title floor; that
/// case is exercised by `the_title_bar_outranks_the_held_edge_when_the_two_conflict`.
///
/// The mirror case does happen with the spec's own policy and is the right
/// behaviour: shrinking a card that hangs off the left by dragging its
/// *right* edge pulls the whole card back, because the narrower card can
/// no longer reach `120px` of title bar from where it was.
#[must_use]
pub fn resize(
    rect: Rectangle,
    handle: ResizeHandle,
    delta: Vector,
    viewport: Rectangle,
    rule: ClampRule,
) -> Rectangle {
    let rect = sanitize_rect(rect);
    let delta = sanitize_vector(delta);
    let view = sanitize_rect(viewport);
    let policy = rule.size_policy;

    let (start_x, width) = resize_axis(
        rect.x,
        rect.x + rect.width,
        handle.drags_start_x(),
        handle.drags_end_x(),
        delta.x,
        policy.min.width,
        policy.max.map(|max| max.width),
    );
    let (start_y, height) = resize_axis(
        rect.y,
        rect.y + rect.height,
        handle.drags_start_y(),
        handle.drags_end_y(),
        delta.y,
        policy.min.height,
        policy.max.map(|max| max.height),
    );

    // The trailing edge the drag is holding still, in absolute terms.
    let held_right = start_x + width;
    let held_bottom = start_y + height;

    let origin = clamp_origin(
        Point::new(start_x, start_y),
        Size::new(width, height),
        view,
        rule,
    );

    // A drag that moved the leading edge holds the trailing one: if the
    // invariant moved the leading edge back, the extent is what is left
    // over, not what the pointer asked for. Without this, dragging the top
    // edge above the navbar would grow the card downwards instead.
    //
    // What is left over can itself fall short of the policy floor — a card
    // held against the edge of a small viewport cannot be both `420px`
    // wide and reachable from that edge — so the floor is re-applied with
    // the origin where the invariant put it. The held edge gives way
    // rather than the size policy or the invariant.
    let width = if handle.drags_start_x() {
        clamp_extent(
            held_right - origin.x,
            policy.min.width,
            policy.max.map(|max| max.width),
        )
    } else {
        width
    };
    let height = if handle.drags_start_y() {
        clamp_extent(
            held_bottom - origin.y,
            policy.min.height,
            policy.max.map(|max| max.height),
        )
    } else {
        height
    };

    // Final safety pass. Re-deriving the extent changed the width the
    // visible-title rule is measured against, so the origin is checked
    // once more against the card it now belongs to. This is a no-op
    // whenever the re-derived card is already reachable, which is every
    // card the spec's own policy can produce.
    clamp_rect(Rectangle::new(origin, Size::new(width, height)), view, rule)
}

/// Does this card's title bar stay reachable in this viewport
/// (`UIF-MOV-002`)?
///
/// A predicate rather than a log line, so a caller can assert the
/// invariant instead of trusting the clamp that produced the value — and
/// so the sweep in the sidecar can prove the clamp never misses.
///
/// The requirement degrades exactly the way the clamp degrades: a card
/// narrower than the visible-title minimum is held to its own width (all
/// of it visible), and a viewport narrower than the requirement is held to
/// the viewport width. In the normal case — a card and a window both wider
/// than `120px` — this is literally "at least `120px` of title bar shows".
#[must_use]
pub fn is_reachable(rect: Rectangle, viewport: Rectangle, rule: ClampRule) -> bool {
    let rect = sanitize_rect(rect);
    let view = sanitize_rect(viewport);

    let visible_ok =
        visible_title_width(rect, view) >= required_visible(rect.width, view.width, rule);

    let top = view.y + rule.top_bound;
    let bottom = (view.y + view.height - rule.title_bar_height).max(top);
    let title_ok = rect.y >= top && rect.y <= bottom;

    visible_ok && title_ok
}

/// The card's title-bar strip: the top `title_bar_height` of the card.
#[must_use]
pub fn title_bar_rect(rect: Rectangle, rule: ClampRule) -> Rectangle {
    let rect = sanitize_rect(rect);
    Rectangle::new(
        Point::new(rect.x, rect.y),
        Size::new(rect.width, rule.title_bar_height.max(0.0)),
    )
}

/// How much of the card's width is inside the viewport — the horizontal
/// overlap the `120px` rule is measured against.
///
/// `0.0` when the card is entirely outside the viewport horizontally.
#[must_use]
pub fn visible_title_width(rect: Rectangle, viewport: Rectangle) -> f32 {
    let rect = sanitize_rect(rect);
    let view = sanitize_rect(viewport);
    let overlap = (rect.x + rect.width).min(view.x + view.width) - rect.x.max(view.x);
    overlap.max(0.0)
}

/// Clamp an origin against both axes of the invariant.
fn clamp_origin(origin: Point, card: Size, view: Rectangle, rule: ClampRule) -> Point {
    Point::new(
        clamp_horizontal_origin(origin.x, card.width, view, rule),
        clamp_vertical_origin(origin.y, view, rule),
    )
}

/// Horizontal half of the invariant: keep `required` px of title bar
/// inside the viewport.
///
/// The legal range for the card's left edge is
/// `[view.x + required − width, view.x + view.width − required]`: the
/// lower bound is "the card's right edge must reach `required` px past the
/// viewport's left edge", the upper bound is "the card's left edge must be
/// no more than `required` px from the viewport's right edge".
///
/// When the card is narrower than the requirement the lower bound becomes
/// `view.x` and the upper becomes `view.x + view.width − width`: the whole
/// card must show. When the viewport is narrower than the requirement the
/// range inverts, and pinning to the lower bound is not an arbitrary tie
/// break — it puts the card's right edge `required` px past the viewport's
/// left edge, which covers the entire viewport and is the most title bar
/// any position can show.
fn clamp_horizontal_origin(x: f32, width: f32, view: Rectangle, rule: ClampRule) -> f32 {
    let required = required_visible(width, view.width, rule);
    let lower = view.x + required - width;
    let upper = view.x + view.width - required;
    x.clamp(lower, upper.max(lower))
}

/// Vertical half of the invariant: the card's top edge between the navbar
/// and the last position at which the title bar still touches the screen.
///
/// The card's own height is deliberately absent. A `2000px` log card and a
/// `260px` one get the same vertical bounds, because the thing being
/// protected is `32px` tall either way.
fn clamp_vertical_origin(y: f32, view: Rectangle, rule: ClampRule) -> f32 {
    let top = view.y + rule.top_bound;
    let bottom = view.y + view.height - rule.title_bar_height;
    y.clamp(top, bottom.max(top))
}

/// The visible-title width this card-in-this-viewport can be held to.
///
/// `min(requirement, card width, viewport width)`. The two degradations
/// are the two ways the literal requirement is unreachable: a card that
/// does not have `120px` of title bar to give, and a viewport that cannot
/// show `120px` of anything.
fn required_visible(card_width: f32, viewport_width: f32, rule: ClampRule) -> f32 {
    rule.min_visible_title
        .min(card_width)
        .min(viewport_width)
        .max(0.0)
}

/// Resize one axis about the edge the drag holds still.
///
/// `start` and `end` are the card's current edges; `moves_start` and
/// `moves_end` say which one the pointer has. Exactly one is true for a
/// dragging handle; when neither is, the axis is untouched. Returns the
/// axis's new origin and extent with the extent clamped to the policy.
fn resize_axis(
    start: f32,
    end: f32,
    moves_start: bool,
    moves_end: bool,
    delta: f32,
    min: f32,
    max: Option<f32>,
) -> (f32, f32) {
    if moves_start {
        let extent = clamp_extent(end - (start + delta), min, max);
        (end - extent, extent)
    } else if moves_end {
        let extent = clamp_extent(end + delta - start, min, max);
        (start, extent)
    } else {
        (start, end - start)
    }
}

/// Clamp one extent into `[min, max]`, where `max` may be absent.
///
/// A non-finite or absent ceiling means unbounded; a ceiling below the
/// floor is raised to the floor so the range cannot invert. A dragged
/// extent that went negative — the pointer crossed the held edge — lands on
/// the floor, which stops the dragged edge at the minimum rather than
/// letting the card turn inside out.
fn clamp_extent(raw: f32, min: f32, max: Option<f32>) -> f32 {
    let floor = non_negative(min);
    let value = finite(raw).max(floor);
    match max {
        Some(limit) if limit.is_finite() => value.min(limit.max(floor)),
        _ => value,
    }
}

/// A non-finite scalar is meaningless as a coordinate or extent; treat it
/// as `0.0`.
///
/// iced does not guarantee finite geometry — an unmeasured widget or a
/// division by a zero scale can hand over `NaN` or `±∞` — and either one
/// propagates through layout silently. Landing on `0.0` instead keeps the
/// solver a total function whose worst case is a card pinned to the
/// viewport origin.
fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// A negative extent is not a size; treat it as `0.0`.
fn non_negative(value: f32) -> f32 {
    finite(value).max(0.0)
}

/// Sanitise a point. Coordinates may legitimately be negative — a card
/// dragged outside the viewport is a defined case — so only non-finite
/// values are rewritten.
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

/// Sanitise a pointer delta: finite, but either sign is legitimate.
fn sanitize_vector(delta: Vector) -> Vector {
    Vector::new(finite(delta.x), finite(delta.y))
}

#[cfg(test)]
#[path = "clamp_tests.rs"]
mod clamp_tests;
