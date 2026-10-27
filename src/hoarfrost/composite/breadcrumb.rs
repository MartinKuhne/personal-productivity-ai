//! The breadcrumb trail's crumb-elision solver (`UIF-COMP-008`).
//!
//! §3B.8 asks for an ordered trail of clickable crumbs separated by a
//! non-clickable separator, where the last crumb is the current location
//! and a trail that does not fit collapses its **middle** crumbs behind a
//! `[…]` trigger while the first and last crumbs stay visible.
//!
//! # What is decided here, and what the spec left open
//!
//! The spec fixes the invariant ("always keep first and last") and the
//! affordance (`[…]` opens a menu of the hidden crumbs). It does **not**
//! say which middle crumbs go, in what order, when several could be
//! dropped. Three rules were chosen and are the contract:
//!
//! 1. **The collapse set is a pure function of `(items, available_width)`.**
//!    No prior layout, no sticky state, no hysteresis. Two calls with the
//!    same arguments return the same set, byte for byte.
//! 2. **The kept run is contiguous and anchored at the terminal crumb.**
//!    Crumbs are dropped from the *front* of the middle run — the ones
//!    nearest the root go first, the ones nearest the current location are
//!    kept. A trail reads outward from where you are, so the crumbs you
//!    are most likely to want are the ones closest to the terminal.
//! 3. **Monotonicity.** Widening the bar never collapses a *different*
//!    arbitrary set: the hidden set at width `w` is always a superset of
//!    the hidden set at any width `w' > w`. This falls out of rule 2 — the
//!    hidden set is always a prefix of the middle run, so it can only
//!    shrink from the back as room appears — and the sidecar asserts it
//!    across a sweep of widths rather than asserting the author's
//!    confidence that it holds.
//!
//! Rule 2 is a **spec gap**, not a spec reading. [`GAP_COLLAPSE_ORDER`]
//! records it.
//!
//! # The terminal crumb is clickable
//!
//! §3B.8 says "ordered clickable crumbs" and carves out only the
//! separator, so the terminal crumb is a focusable, activatable target
//! like any other. A host that wants activation-on-current-location to be
//! a no-op has it explicitly: [`CrumbRole::Terminal`] is carried on the
//! emission, so the host decides what it means instead of the framework
//! deciding by omission. This is recorded as a spec gap in
//! [`GAP_TERMINAL_CLICKABLE`].
//!
//! # The separator is not a target, structurally
//!
//! A separator is not a thing that is *made* non-clickable by ignoring
//! its hits: it occupies the [`SEGMENT_GAP`] between two crumb rects and
//! is therefore not inside any crumb's hit bounds in the first place.
//! [`separator_rects`] returns exactly those gap rectangles, which is
//! what lets the sidecar prove no separator pixel lies inside a clickable
//! one.
//!
//! # Colour
//!
//! §3B.8 is explicit here, and it names exactly two text tokens: ancestors
//! (root and middle alike) are `text.muted`, the terminal crumb is
//! `text.strong`, and the separator glyph is `text.muted`. Each segment is
//! a `Ghost` button, so the attended states come from §3.1's ghost column
//! — `surface.hover` / `surface.active` beneath a `text.strong` label.
//!
//! Two divergences from the §2.5 token table are recorded rather than
//! smoothed over: [`NOTE_SEPARATOR_TOKEN`] and
//! [`NOTE_PRIMARY_TOKEN_UNUSED`].
//!
//! Unit tests live in the sibling `breadcrumb_tests.rs` sidecar.

use std::fmt;

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::state::BaseState;
use crate::state::VisualState;
use crate::surface::menu::cascade::CascadeOutcome;
use crate::surface::menu::cascade::MenuCascade;
use crate::surface::menu::cascade::MenuKey;
use crate::surface::menu::cascade::MenuTreeKind;
use crate::surface::menu::model::ActivationEffect;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::Row;
use crate::token::color::Palette;
use crate::token::contrast;
use crate::widget::button;
use crate::widget::button::ButtonStyle;
use crate::widget::button::ButtonVariant;

use iced::Color;
use iced::Rectangle;

// ---------------------------------------------------------------------------
// Spec gaps this module closes by decision
// ---------------------------------------------------------------------------

/// **Spec gap** — the spec does not say which middle crumbs collapse.
///
/// §3B.8 fixes the invariant (first and last always visible) and the
/// affordance, but not the selection among middle crumbs. Decided here:
/// the hidden set is a **prefix of the middle run**, i.e. crumbs nearest
/// the root are dropped first and the run kept is contiguous and
/// anchored at the terminal crumb. See the module docs.
pub const GAP_COLLAPSE_ORDER: &str = "UIF-COMP-008 spec gap: collapse order among middle crumbs is \
                                     unspecified; decided as drop-from-root-first, contiguous \
                                     terminal-anchored run";

/// **Spec gap** — the spec does not say whether the current-location crumb
/// is clickable.
///
/// Decided here: it is. §3B.8's "ordered clickable crumbs" quantifies
/// over the crumbs and exempts only the separator, and the role is
/// carried on the emission so a host that wants a no-op has it stated
/// rather than inferred from a missing target.
pub const GAP_TERMINAL_CLICKABLE: &str = "UIF-COMP-008 spec gap: terminal-crumb clickability is \
                                        unspecified; decided clickable and focusable, with the \
                                        role carried on the emission";

/// **Recorded divergence** — the separator's token.
///
/// §3B.8 puts the `/` (or `›`) separator in `text.muted`. §2.5's table
/// describes [`border_strong`](Palette::border_strong) as the token for
/// "active/floating perimeters … markdown tables" and
/// [`border_subtle`](Palette::border_subtle) as the one for "passive
/// structural dividers", so neither of the two border rows names the
/// trail's separator and the surface rule is the only thing that does.
/// Followed §3B.8. The measured consequence is that the separator is a
/// **text**-role pairing at `text.muted` on `surface.base`
/// ([`SEPARATOR_CONTRAST_SPEC`] `= 7.61:1`) rather than a chrome-role
/// pairing at `border.strong` (`4.51:1`).
pub const NOTE_SEPARATOR_TOKEN: &str = "UIF-COMP-008 divergence: §3B.8 assigns the separator to \
                                      text.muted; §2.5's border rows name no token for a trail \
                                      separator. Followed §3B.8.";

/// The value §2.5 prints for `color.text.muted` against `color.surface.base`.
pub const SEPARATOR_CONTRAST_SPEC: f32 = 7.61;

/// **Recorded divergence** — `text.primary` is never painted by the trail.
///
/// §2.5 gives [`text_primary`](Palette::text_primary) the usage contract
/// "default body and resting control label text", and a crumb label is
/// exactly that. §3B.8 nonetheless puts every ancestor in `text.muted`
/// and reserves `text.strong` for the terminal crumb, leaving
/// `text.primary` unused anywhere in the bar. Followed §3B.8, because it
/// is the surface-specific rule; recorded because the token's own stated
/// contract says otherwise.
pub const NOTE_PRIMARY_TOKEN_UNUSED: &str = "UIF-COMP-008 divergence: §2.5 gives text.primary the \
                                           resting-label contract, but §3B.8 leaves the trail with \
                                           only text.muted and text.strong. Followed §3B.8.";

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Height of the breadcrumb bar itself.
///
/// The bar is a fixed-height strip. [`collapse`] never returns a layout
/// that would need a second row, so the bar's height cannot depend on how
/// many crumbs were elided — the same `UIF-PRIN-003` zero-shift property
/// the trail shares with the overflow toolbar.
pub const BAR_HEIGHT: f32 = 28.0;

/// Height of one crumb's clickable box.
pub const SEGMENT_HEIGHT: f32 = 24.0;

/// Horizontal padding inside a crumb's box.
///
/// A crumb's *measured* width already includes this padding: the caller
/// measures label plus [`SEGMENT_PADDING_X`] on each side and hands the
/// total to [`BreadcrumbItem::new`], which keeps the solver pure
/// arithmetic with no text metrics inside it.
pub const SEGMENT_PADDING_X: f32 = 6.0;

/// Gap between two adjacent trail elements — and the whole footprint of a
/// separator.
///
/// The separator is painted in this gap rather than being a control with
/// a gap around it, which is why it cannot be a click target.
pub const SEGMENT_GAP: f32 = 4.0;

/// Width of the `[…]` trigger.
pub const ELLIPSIS_TRIGGER_WIDTH: f32 = 28.0;

/// Floor on a crumb's measured width.
///
/// A zero-length or unmeasured label must not collapse the trail's hit
/// targets to nothing, so a crumb is never narrower than its own height.
pub const MIN_SEGMENT_WIDTH: f32 = SEGMENT_HEIGHT;

/// The `[…]` label.
pub const ELLIPSIS_LABEL: &str = "[…]";

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/// A crumb's position-derived role in the trail.
///
/// The role, not the index, is what the colour resolver and the host's
/// navigation callback consume: "go to the root" and "you are here" are
/// stable meanings, whereas "index 0" and "index `n-1`" are facts about
/// the array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrumbRole {
    /// The trail's first crumb — always visible.
    Root,
    /// A crumb that may be elided.
    Middle,
    /// The current location — always visible, and the boldest thing in the
    /// bar.
    Terminal,
}

/// Where a crumb ended up after [`collapse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrumbPlacement {
    /// Painted inline in the bar, and clickable.
    Inline,
    /// Behind the `[…]` trigger — reachable only through the menu.
    Hidden,
}

/// One crumb's resolved placement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrumbPlan {
    /// The crumb's index in the caller's item list.
    pub index: usize,
    /// Its position-derived role.
    pub role: CrumbRole,
    /// Inline or hidden.
    pub placement: CrumbPlacement,
    /// The width its box occupies when inline.
    pub width: f32,
}

impl CrumbPlan {
    /// Whether this crumb is painted inline.
    #[must_use]
    pub const fn is_inline(&self) -> bool {
        matches!(self.placement, CrumbPlacement::Inline)
    }

    /// Whether this crumb sits behind the `[…]` trigger.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        matches!(self.placement, CrumbPlacement::Hidden)
    }
}

/// The resolved layout of a trail.
///
/// A value rather than a mutation of the items, so the same item list can
/// be laid out at several widths and the results compared — which is what
/// the monotonicity property is stated over.
#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbLayout {
    plans: Vec<CrumbPlan>,
    inline_width: f32,
    hidden_count: usize,
    trigger_width: f32,
    available_width: f32,
}

impl BreadcrumbLayout {
    /// Every crumb's plan, in trail order.
    #[must_use]
    pub fn plans(&self) -> &[CrumbPlan] {
        &self.plans
    }

    /// The plan for one crumb index.
    #[must_use]
    pub fn plan(&self, index: usize) -> Option<&CrumbPlan> {
        self.plans.get(index)
    }

    /// How many crumbs are painted inline.
    #[must_use]
    pub fn inline_count(&self) -> usize {
        self.plans.iter().filter(|plan| plan.is_inline()).count()
    }

    /// How many crumbs sit behind the `[…]` trigger.
    #[must_use]
    pub const fn hidden_count(&self) -> usize {
        self.hidden_count
    }

    /// The indices behind the trigger, in trail order.
    ///
    /// This is exactly the set the `[…]` menu must contain — the spec's
    /// "menu of exactly the hidden crumbs" is satisfied by construction
    /// because the menu is built from this list and nothing else.
    #[must_use]
    pub fn hidden_indices(&self) -> Vec<usize> {
        self.plans
            .iter()
            .filter(|plan| plan.is_hidden())
            .map(|plan| plan.index)
            .collect()
    }

    /// Whether the trail is elided at all.
    #[must_use]
    pub const fn is_elided(&self) -> bool {
        self.hidden_count > 0
    }

    /// The width the inline crumbs occupy, gaps included.
    #[must_use]
    pub const fn inline_width(&self) -> f32 {
        self.inline_width
    }

    /// The `[…]` trigger's width — `0.0` when nothing is hidden.
    #[must_use]
    pub const fn trigger_width(&self) -> f32 {
        self.trigger_width
    }

    /// The width the whole trail wants: inline crumbs, the gap before the
    /// trigger, and the trigger.
    #[must_use]
    pub const fn total_width(&self) -> f32 {
        if self.trigger_width > 0.0 {
            self.inline_width + SEGMENT_GAP + self.trigger_width
        } else {
            self.inline_width
        }
    }

    /// The width this layout was solved against.
    #[must_use]
    pub const fn available_width(&self) -> f32 {
        self.available_width
    }

    /// Whether the trail fit without eliding anything.
    ///
    /// Distinct from [`is_elided`](Self::is_elided) only in direction:
    /// `fits` is false when the solver had to hide crumbs *or* when it
    /// could not fit even the minimum trail.
    #[must_use]
    pub const fn fits(&self) -> bool {
        self.hidden_count == 0
    }

    /// Whether the minimum trail — root, terminal and the trigger — fit.
    ///
    /// When this is false the bar is narrower than any legal trail and the
    /// overflow is the host's to scroll. The solver does not violate the
    /// always-visible invariant to paper over that; it reports instead.
    #[must_use]
    pub const fn minimum_fits(&self) -> bool {
        self.total_width() <= self.available_width
    }
}

/// The role of the crumb at `index` in a trail of `count` crumbs.
#[must_use]
pub const fn crumb_role(index: usize, count: usize) -> CrumbRole {
    if index == 0 {
        CrumbRole::Root
    } else if index + 1 == count {
        CrumbRole::Terminal
    } else {
        CrumbRole::Middle
    }
}

/// One crumb's declared content.
///
/// `measure` is the crumb's **full box width** — label plus
/// [`SEGMENT_PADDING_X`] on each side — measured by the caller. Keeping
/// text measurement outside the solver is what makes
/// [`collapse`] testable without a font.
#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbItem<Id> {
    /// The host's stable crumb identity.
    pub id: Id,
    /// The crumb's label.
    pub label: String,
    /// The full box width, padding included.
    pub measure: f32,
}

impl<Id> BreadcrumbItem<Id> {
    /// Declare a crumb.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>, measure: f32) -> Self {
        Self {
            id,
            label: label.into(),
            measure,
        }
    }

    /// The crumb's identity.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The crumb's label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

// ---------------------------------------------------------------------------
// The solver
// ---------------------------------------------------------------------------

/// A crumb's width as the solver sees it: never below the hit-target floor.
fn clamped_measure<Id>(item: &BreadcrumbItem<Id>) -> f32 {
    if item.measure < MIN_SEGMENT_WIDTH {
        MIN_SEGMENT_WIDTH
    } else {
        item.measure
    }
}

/// The width of every crumb inline, gaps included.
fn full_inline_total(measures: &[f32]) -> f32 {
    let mut width = 0.0;
    for measure in measures {
        width += measure;
    }
    width + SEGMENT_GAP * measures.len().saturating_sub(1) as f32
}

/// The width of the trail when the last `kept` middle crumbs stay inline.
///
/// The inline set is the root, the contiguous run
/// `measures[len-2-kept .. len-2]`, and the terminal — `kept + 2` crumbs
/// with `kept + 1` gaps between them, plus one gap and the trigger.
/// Because every measure is at least [`MIN_SEGMENT_WIDTH`], this is
/// strictly increasing in `kept`, which is what makes "the largest `kept`
/// that fits" a unique, monotone answer.
fn total_with_kept(measures: &[f32], kept: usize) -> f32 {
    let last = measures.len() - 1;
    let middle_start = last - 1 - kept;
    let mut width = measures[0] + measures[last];
    for measure in &measures[middle_start..middle_start + kept] {
        width += measure;
    }
    width + SEGMENT_GAP * (kept + 2) as f32 + ELLIPSIS_TRIGGER_WIDTH
}

/// Build the layout for a given hidden set.
fn layout_for(measures: &[f32], hidden: &[usize], available_width: f32) -> BreadcrumbLayout {
    let count = measures.len();
    let plans: Vec<CrumbPlan> = (0..count)
        .map(|index| CrumbPlan {
            index,
            role: crumb_role(index, count),
            placement: if hidden.contains(&index) {
                CrumbPlacement::Hidden
            } else {
                CrumbPlacement::Inline
            },
            width: measures[index],
        })
        .collect();

    let mut inline_width = 0.0;
    let mut inline_seen = 0usize;
    for plan in &plans {
        if plan.is_inline() {
            if inline_seen > 0 {
                inline_width += SEGMENT_GAP;
            }
            inline_width += plan.width;
            inline_seen += 1;
        }
    }

    let hidden_count = hidden.len();
    let trigger_width = if hidden_count > 0 {
        ELLIPSIS_TRIGGER_WIDTH
    } else {
        0.0
    };

    BreadcrumbLayout {
        plans,
        inline_width,
        hidden_count,
        trigger_width,
        available_width,
    }
}

/// Solve a trail against an available width.
///
/// The result is a pure function of `(items, available_width)`: no prior
/// layout is consulted, so the same arguments always produce the same
/// hidden set. The invariant — root and terminal always inline — holds at
/// every width, including widths too small to honour it comfortably; at
/// such a width [`minimum_fits`](BreadcrumbLayout::minimum_fits) reports
/// the shortfall rather than the invariant being broken.
///
/// Trails of two crumbs or fewer have no middle run, so they are never
/// elided: there is nothing that could go behind the trigger without
/// violating the always-visible rule.
#[must_use]
pub fn collapse<Id>(items: &[BreadcrumbItem<Id>], available_width: f32) -> BreadcrumbLayout {
    let measures: Vec<f32> = items.iter().map(clamped_measure).collect();
    let count = measures.len();

    if count <= 2 || full_inline_total(&measures) <= available_width {
        return layout_for(&measures, &[], available_width);
    }

    let middle_count = count - 2;
    let mut kept = 0usize;
    for candidate in (0..=middle_count).rev() {
        if total_with_kept(&measures, candidate) <= available_width {
            kept = candidate;
            break;
        }
    }

    // The hidden set is the *front* of the middle run: indices `1..`,
    // taken `middle_count - kept` deep. See GAP_COLLAPSE_ORDER.
    let hidden: Vec<usize> = (1..count - 1).take(middle_count - kept).collect();
    layout_for(&measures, &hidden, available_width)
}

// ---------------------------------------------------------------------------
// Rectangles
// ---------------------------------------------------------------------------

/// A rectangle from its corners.
fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

/// The clickable rectangles of the inline crumbs, in trail order.
///
/// Consecutive rectangles are exactly [`SEGMENT_GAP`] apart, which is the
/// separator's footprint — see [`separator_rects`].
#[must_use]
pub fn segment_rects(layout: &BreadcrumbLayout, x: f32, y: f32) -> Vec<(usize, Rectangle)> {
    let mut cursor = x;
    let mut out = Vec::with_capacity(layout.inline_count());
    for plan in layout.plans().iter().filter(|plan| plan.is_inline()) {
        out.push((plan.index, rect(cursor, y, plan.width, SEGMENT_HEIGHT)));
        cursor += plan.width + SEGMENT_GAP;
    }
    out
}

/// The separator rectangles — the gaps between consecutive inline crumbs.
///
/// These are disjoint from every rectangle [`segment_rects`] returns by
/// exactly [`SEGMENT_GAP`], and no focus member names them. That is the
/// structural form of "the separator is never a clickable target": it is
/// not a target that ignores clicks, it is the space between targets.
#[must_use]
pub fn separator_rects(layout: &BreadcrumbLayout, x: f32, y: f32) -> Vec<Rectangle> {
    let rects = segment_rects(layout, x, y);
    rects
        .windows(2)
        .map(|pair| {
            let (_, left) = pair[0];
            let (_, right) = pair[1];
            rect(
                left.x + left.width,
                left.y,
                right.x - (left.x + left.width),
                left.height,
            )
        })
        .collect()
}

/// The `[…]` trigger's rectangle, if the trail is elided.
#[must_use]
pub fn trigger_rect(layout: &BreadcrumbLayout, x: f32, y: f32) -> Option<Rectangle> {
    if layout.is_elided() {
        Some(rect(
            x + layout.inline_width() + SEGMENT_GAP,
            y,
            ELLIPSIS_TRIGGER_WIDTH,
            SEGMENT_HEIGHT,
        ))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Focus
// ---------------------------------------------------------------------------

/// Prefix for an inline crumb's focus id.
pub const CRUMB_FOCUS_PREFIX: &str = "uif.breadcrumb.crumb";

/// The `[…]` trigger's focus id.
pub const ELLIPSIS_FOCUS_ID: &str = "uif.breadcrumb.ellipsis";

/// The [`FocusId`] for the crumb at `index`.
#[must_use]
pub fn crumb_focus_id(index: usize) -> FocusId {
    FocusId::new(format!("{CRUMB_FOCUS_PREFIX}:{index}"))
}

/// The [`FocusId`] for the `[…]` trigger.
#[must_use]
pub fn ellipsis_focus_id() -> FocusId {
    FocusId::new(ELLIPSIS_FOCUS_ID)
}

/// The [`FocusMember`] for the crumb at `index`.
#[must_use]
pub fn crumb_member(index: usize) -> FocusMember {
    FocusMember::new(crumb_focus_id(index))
}

/// The [`FocusMember`] for the `[…]` trigger.
#[must_use]
pub fn ellipsis_member() -> FocusMember {
    FocusMember::new(ellipsis_focus_id())
}

/// Every member a `Tab` may stop on in this trail.
///
/// Hidden crumbs are deliberately absent: a crumb that is not painted
/// must not be a focus stop, and the menu the trigger opens is what makes
/// them reachable.
#[must_use]
pub fn navigable_members(layout: &BreadcrumbLayout) -> Vec<FocusMember> {
    let mut members: Vec<FocusMember> = layout
        .plans()
        .iter()
        .filter(|plan| plan.is_inline())
        .map(|plan| crumb_member(plan.index))
        .collect();
    if layout.is_elided() {
        members.push(ellipsis_member());
    }
    members
}

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

/// §3.1's ghost column — the column every crumb and every trigger is
/// painted from.
///
/// §3B.8 calls each segment a `Ghost` button and calls `[…]` a trigger
/// button, so the bar has exactly one underlying variant and this is the
/// only place its paint is read. Re-deciding the hover fill here would
/// put a second opinion between the trail and §3.1.
fn ghost(state: VisualState, palette: &Palette) -> ButtonStyle {
    button::resolve_style(ButtonVariant::Ghost, state, palette)
}

/// The fill behind a crumb in a visual state.
///
/// Transparent at rest — the trail sits on the bar's own surface — so
/// only the attended states produce a pill.
#[must_use]
pub fn crumb_fill(palette: &Palette, state: VisualState) -> Color {
    ghost(state, palette).background
}

/// The surface a crumb or trigger label is actually painted on.
///
/// A resting ghost button paints no fill, which is not a backdrop — the
/// label sits on the bar. Resolving the *backdrop* rather than the fill is
/// what makes [`crumb_contrast`] and [`trigger_contrast`] measure the
/// pairing the eye sees instead of comparing a label against transparency.
#[must_use]
pub fn bar_backdrop(palette: &Palette, state: VisualState) -> Color {
    match state.base {
        BaseState::Pressed => palette.surface_active,
        BaseState::Hovered => palette.surface_hover,
        BaseState::Rest | BaseState::Disabled | BaseState::Invalid => palette.surface_base,
    }
}

/// The colour of a crumb's label.
///
/// §3B.8 names two tokens for the whole bar and §3.1 supplies the ghost
/// column for the attended states:
///
/// | Role | Rest | Attended |
/// |---|---|---|
/// | Root (ancestor) | `text_muted` | `text_strong` |
/// | Middle (ancestor) | `text_muted` | `text_strong` |
/// | Terminal | `text_strong` | `text_strong` |
///
/// Root and middle share one resting colour because the spec's split is
/// ancestor-vs-current, not root-vs-middle: the trail says "you are here"
/// against "everything else", and a third weight would say something the
/// spec does not.
///
/// A disabled crumb takes `text_disabled` whatever its role: the state has
/// to read before the position does, or a greyed root looks like a
/// differently-styled root rather than an unavailable one.
#[must_use]
pub fn crumb_colour(palette: &Palette, role: CrumbRole, state: VisualState) -> Color {
    match state.base {
        // §3B.8 overrides the ghost's resting foreground: ancestors are
        // muted, the current location is strong.
        BaseState::Rest | BaseState::Invalid => match role {
            CrumbRole::Root | CrumbRole::Middle => palette.text_muted,
            CrumbRole::Terminal => palette.text_strong,
        },
        BaseState::Hovered | BaseState::Pressed | BaseState::Disabled => {
            ghost(state, palette).foreground
        }
    }
}

/// The separator's colour — `text.muted`, per §3B.8.
///
/// The separator is a glyph, not a stroke, which is why it is measured as
/// a **text** pairing rather than as chrome. See
/// [`NOTE_SEPARATOR_TOKEN`] for why this is not a `border.*` token.
#[must_use]
pub fn separator_colour(palette: &Palette) -> Color {
    palette.text_muted
}

/// The `[…]` trigger's glyph colour — §3.1's ghost foreground, unchanged.
///
/// That makes it `text.primary` at rest, which is brighter than the
/// `text.muted` crumbs on either side of it. The composition is the
/// spec's, not a preference: §3B.8 calls `[…]` a *button* and only ever
/// assigns `text.muted` to ancestor labels and the separator glyph. It is
/// also where `text.primary` turns up in a bar that otherwise paints none —
/// see [`NOTE_PRIMARY_TOKEN_UNUSED`].
#[must_use]
pub fn trigger_glyph_colour(palette: &Palette, state: VisualState) -> Color {
    ghost(state, palette).foreground
}

/// The `[…]` trigger's fill — §3.1's ghost background.
#[must_use]
pub fn trigger_fill(palette: &Palette, state: VisualState) -> Color {
    ghost(state, palette).background
}

/// The contrast of a crumb's label against its actual backdrop.
#[must_use]
pub fn crumb_contrast(palette: &Palette, role: CrumbRole, state: VisualState) -> f32 {
    contrast::contrast_ratio(
        crumb_colour(palette, role, state),
        bar_backdrop(palette, state),
    )
}

/// The separator's contrast against the bar.
#[must_use]
pub fn separator_contrast(palette: &Palette) -> f32 {
    contrast::contrast_ratio(separator_colour(palette), palette.surface_base)
}

/// The `[…]` glyph's contrast against its actual backdrop.
#[must_use]
pub fn trigger_contrast(palette: &Palette, state: VisualState) -> f32 {
    contrast::contrast_ratio(
        trigger_glyph_colour(palette, state),
        bar_backdrop(palette, state),
    )
}

/// Whether a crumb's resting label clears the text band.
#[must_use]
pub fn crumb_meets_text(palette: &Palette, role: CrumbRole) -> bool {
    contrast::meets_text_contrast(
        crumb_colour(
            palette,
            role,
            VisualState {
                base: BaseState::Rest,
                focused: false,
            },
        ),
        palette.surface_base,
    )
}

/// Whether the separator clears the text band.
///
/// The separator is a glyph painted in a text token, so rule 1 is the
/// band it answers to — not the `3.0:1` chrome threshold a stroke would
/// get. Measuring it against the looser band would let a dimmer separator
/// pass while still being the same kind of thing §2.5 gates at `4.5:1`.
#[must_use]
pub fn separator_meets_text(palette: &Palette) -> bool {
    contrast::meets_text_contrast(separator_colour(palette), palette.surface_base)
}

/// Whether a disabled crumb sits inside the muted legibility band
/// `UIF-TOK-006` rule 2 sets for `text.disabled`.
#[must_use]
pub fn disabled_crumb_in_band(palette: &Palette, role: CrumbRole) -> bool {
    contrast::meets_disabled_text_band(
        crumb_colour(
            palette,
            role,
            VisualState {
                base: BaseState::Disabled,
                focused: false,
            },
        ),
        palette.surface_base,
    )
}

// ---------------------------------------------------------------------------
// The overflow menu
// ---------------------------------------------------------------------------

/// The `[…]` menu's [`MenuId`](crate::surface::menu::model::MenuId).
pub const MENU_ID: &str = "uif.breadcrumb.menu";

/// The row key for a crumb.
///
/// Keyed by the host's own crumb identity rather than by position, so a
/// row the engine reports back still names the same place in the trail
/// after the trail has been re-solved at a different width.
#[must_use]
pub fn crumb_row_key<Id: fmt::Display>(item: &BreadcrumbItem<Id>) -> String {
    item.id.to_string()
}

/// Build the `[…]` menu for a solved layout.
///
/// The rows are exactly [`BreadcrumbLayout::hidden_indices`], which is
/// what makes "the menu contains exactly the elided crumbs" true by
/// construction rather than by the host remembering to keep two lists in
/// step. The rows are built from the live items on every call, so a crumb
/// whose label changed since the menu last opened shows the new label.
#[must_use]
pub fn overflow_menu<Id: fmt::Display>(
    items: &[BreadcrumbItem<Id>],
    layout: &BreadcrumbLayout,
) -> Menu {
    let rows: Vec<Row> = layout
        .hidden_indices()
        .into_iter()
        .map(|index| {
            let item = &items[index];
            Row::action(crumb_row_key(item), item.label.clone())
        })
        .collect();
    Menu::new(MENU_ID, rows)
}

// ---------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------

/// One piece of output the breadcrumb host produced.
#[derive(Clone)]
pub enum BreadcrumbEmission<Id> {
    /// The `[…]` menu opened (`true`) or closed (`false`).
    MenuToggled(bool),
    /// A crumb was activated, inline or from the menu — `on_segment_activate`.
    CrumbActivated(Id),
}

impl<Id: PartialEq> PartialEq for BreadcrumbEmission<Id> {
    /// Hand-written because the variants carry an `Id` rather than a
    /// linear wrapper, and the two variants must never compare equal.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MenuToggled(a), Self::MenuToggled(b)) => a == b,
            (Self::CrumbActivated(a), Self::CrumbActivated(b)) => a == b,
            _ => false,
        }
    }
}

impl<Id: fmt::Debug> fmt::Debug for BreadcrumbEmission<Id> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MenuToggled(open) => f.debug_tuple("MenuToggled").field(open).finish(),
            Self::CrumbActivated(id) => f.debug_tuple("CrumbActivated").field(id).finish(),
        }
    }
}

/// Everything one host input did.
#[derive(Clone, Debug)]
pub struct BreadcrumbOutcome<Id> {
    /// The cascade engine's report for the menu side of the input.
    pub cascade: CascadeOutcome,
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<BreadcrumbEmission<Id>>,
}

impl<Id> BreadcrumbOutcome<Id> {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            cascade: CascadeOutcome::noop(),
            emissions: Vec::new(),
        }
    }

    /// An outcome carrying `emissions` and no cascade movement.
    #[must_use]
    pub fn emissions(emissions: Vec<BreadcrumbEmission<Id>>) -> Self {
        Self {
            cascade: CascadeOutcome::noop(),
            emissions,
        }
    }
}

/// The per-control memory a breadcrumb trail needs.
///
/// One [`MenuCascade`] and nothing else. Openness lives in the cascade,
/// never in a `popup_open` flag on the trail's own model — §3B.8 sketches
/// `elided_Popup_open: bool` on `BreadcrumbModel`, and that field is
/// deliberately *not* implemented, because the crate's menu engine owns
/// openness and a second copy of it is a copy able to disagree with the
/// one that actually drives what is drawn.
#[derive(Debug)]
pub struct BreadcrumbHost {
    /// The one menu engine this trail is allowed to have.
    cascade: MenuCascade,
    /// The `[…]` trigger's focus identity.
    trigger: FocusId,
    /// The crumb each open menu row stands for, captured at build time.
    row_items: Vec<usize>,
}

impl BreadcrumbHost {
    /// A host whose `[…]` trigger is focusable as `trigger`.
    #[must_use]
    pub fn new(trigger: FocusId) -> Self {
        Self {
            cascade: MenuCascade::new(),
            trigger,
            row_items: Vec::new(),
        }
    }

    /// Whether the `[…]` menu is showing.
    #[must_use]
    pub fn is_menu_open(&self) -> bool {
        self.cascade.is_open()
    }

    /// The cascade, read-only.
    #[must_use]
    pub const fn cascade(&self) -> &MenuCascade {
        &self.cascade
    }

    /// The trigger's focus identity.
    #[must_use]
    pub const fn trigger(&self) -> &FocusId {
        &self.trigger
    }

    /// The crumb a menu row stands for.
    #[must_use]
    pub fn item_for_row(&self, row: usize) -> Option<usize> {
        self.row_items.get(row).copied()
    }

    /// The cascade, mutably — test-only.
    ///
    /// Exists so a test can put a *foreign* Band-2 tree into the same
    /// cascade the host drives and prove that opening the overflow
    /// retires it. Production code has no reason to reach in.
    #[cfg(test)]
    pub(crate) fn cascade_mut(&mut self) -> &mut MenuCascade {
        &mut self.cascade
    }

    /// Activate an inline crumb — `on_segment_activate`.
    ///
    /// The role travels with the id so the host can decide what
    /// activating the current location means instead of inferring it from
    /// a position. See [`GAP_TERMINAL_CLICKABLE`].
    #[must_use]
    pub fn activate_crumb<Id: Clone>(
        items: &[BreadcrumbItem<Id>],
        index: usize,
    ) -> BreadcrumbOutcome<Id> {
        match items.get(index) {
            Some(item) => BreadcrumbOutcome::emissions(vec![BreadcrumbEmission::CrumbActivated(
                item.id.clone(),
            )]),
            None => BreadcrumbOutcome::noop(),
        }
    }

    /// Open the `[…]` menu.
    ///
    /// The trigger is focused *first* so the trap the cascade pushes
    /// records it and `Escape` returns the caret to the trigger. A trail
    /// with nothing hidden opens nothing: an empty menu is a dead control.
    #[must_use]
    pub fn open_overflow<Id: fmt::Display>(
        &mut self,
        items: &[BreadcrumbItem<Id>],
        layout: &BreadcrumbLayout,
        focus: &mut FocusState,
    ) -> BreadcrumbOutcome<Id> {
        if !layout.is_elided() {
            return BreadcrumbOutcome::noop();
        }
        let menu = overflow_menu(items, layout);
        if menu.first_navigable().is_none() {
            return BreadcrumbOutcome::noop();
        }
        if focus.current() != Some(&self.trigger) {
            focus.focus(&self.trigger);
        }
        self.row_items = layout.hidden_indices();
        let cascade = self.cascade.open(&menu, MenuTreeKind::Fixed, focus);
        if cascade.ignored_reason().is_some() {
            self.row_items.clear();
            return BreadcrumbOutcome {
                cascade,
                emissions: Vec::new(),
            };
        }
        BreadcrumbOutcome {
            cascade,
            emissions: vec![BreadcrumbEmission::MenuToggled(true)],
        }
    }

    /// Close the `[…]` menu.
    #[must_use]
    pub fn close_overflow<Id>(&mut self, focus: &mut FocusState) -> BreadcrumbOutcome<Id> {
        if !self.cascade.is_open() {
            return BreadcrumbOutcome::noop();
        }
        let cascade = self.cascade.close_tree(focus);
        self.row_items.clear();
        BreadcrumbOutcome {
            cascade,
            emissions: vec![BreadcrumbEmission::MenuToggled(false)],
        }
    }

    /// Drive one menu key through the engine (`UIF-CMENU-003`).
    #[must_use]
    pub fn menu_key<Id: Clone + fmt::Display>(
        &mut self,
        key: MenuKey,
        items: &[BreadcrumbItem<Id>],
        focus: &mut FocusState,
    ) -> BreadcrumbOutcome<Id> {
        let was_open = self.cascade.is_open();
        let cascade = self.cascade.on_key(key, focus);
        let mut emissions = Vec::new();

        if let Some(activation) = cascade.activation()
            && !activation.is_absorbed()
            && !matches!(activation.effect, ActivationEffect::OpenSubmenu { .. })
            && let Some(index) = self.item_for_row(activation.row)
            && let Some(item) = items.get(index)
        {
            emissions.push(BreadcrumbEmission::CrumbActivated(item.id.clone()));
        }

        if was_open && !self.cascade.is_open() {
            self.row_items.clear();
            emissions.push(BreadcrumbEmission::MenuToggled(false));
        }

        BreadcrumbOutcome { cascade, emissions }
    }

    /// `Escape`: dismiss the menu; the trap returns focus to the trigger.
    #[must_use]
    pub fn escape<Id>(&mut self, focus: &mut FocusState) -> BreadcrumbOutcome<Id> {
        self.close_menu(focus)
    }

    /// Close the menu (see [`Self::close_overflow`]).
    #[must_use]
    pub fn close_menu<Id>(&mut self, focus: &mut FocusState) -> BreadcrumbOutcome<Id> {
        self.close_overflow(focus)
    }
}

#[cfg(test)]
#[path = "breadcrumb_tests.rs"]
mod breadcrumb_tests;
