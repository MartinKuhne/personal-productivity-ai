//! The responsive overflow toolbar (`UIF-COMP-008`).
//!
//! §3B.8's `CommandBar` pattern: primary controls measure against the
//! toolbar's width, anything that would run past the reserve moves into a
//! trailing `⋯` button, and the toolbar "without wrapping to a second
//! line or shifting toolbar height".
//!
//! Three things had to be decided, because the spec states the outcome and
//! not the mechanism.
//!
//! # Eviction is declared, not positional
//!
//! The spec says a control moves "deterministically" but never says what
//! the determinism is a function *of*. Position is the accident that would
//! fall out of a naive layout, and it is the wrong answer: a host that
//! reorders a toolbar for reasons of its own would silently change which
//! buttons survive a narrow window.
//!
//! So each item declares an [`OverflowRank`], and [`eviction_order`] sorts
//! by that rank alone — **lower ranks are evicted first**. Layout order
//! never enters the primary comparison.
//!
//! # The tie-break
//!
//! Two items sharing a rank are evicted **rightmost first** ([`TIE_BREAK`]).
//! The reason is visual rather than logical: within one declared class of
//! expendability, keeping the leftmost survivors means the inline run stays
//! contiguous from the strip's leading edge, which is what a toolbar reads
//! as. A leftmost-first tie-break would punch holes through the middle of
//! the strip at equal declared importance, which is a worse outcome for no
//! stated gain.
//!
//! # The `0px` shift is structural, not observed
//!
//! [`strip_height`] takes **no parameters at all**. There is no argument
//! through which the inline/overflow split could reach the height, so no
//! eviction decision, no item count and no width can change it. Two
//! compile-time assertions back that up: every element the strip can paint
//! is at most [`STRIP_HEIGHT`] tall, so nothing can push the strip past
//! its own height either.
//!
//! A test that swept eviction counts and asserted a constant height would
//! only prove today's implementation. The signature proves it for every
//! implementation of that signature, which is the stronger claim
//! `UIF-PRIN-003` actually wants.
//!
//! # What overflow does *not* change
//!
//! Placement is the only thing the solver writes. An item's identity, its
//! [`enabled`](OverflowItem::enabled) flag and its
//! [`pending`](OverflowItem::pending) projection are read from the item on
//! both paths, so an overflowed item is the same action with a different
//! address. In particular a **busy** item keeps its `ActionPending`
//! semantics inside the menu: its row stays enabled and keyboard-reachable
//! — a greyed-out row would be an unreachable row — and it is
//! [`RowActionEngine`] that refuses the fresh activation and turns the
//! second press into a stop request, exactly as it does for any other
//! menu row.
//!
//! # The host owns one cascade
//!
//! [`OverflowToolbarHost`] follows the established host pattern: one
//! [`MenuCascade`], one [`RowActionEngine`], no second popup. Opening the
//! overflow menu therefore retires any other Band-2 flyout for free, and
//! the trigger is focused *before* the cascade opens so the trap records
//! it and `Escape` lands the caret back on the `⋯`.
//!
//! Unit tests live in the sibling `overflow_toolbar_tests.rs` sidecar.

use std::fmt;

use crate::binding::ActionPending;
use crate::binding::CancelRequest;
use crate::binding::Commit;
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
use crate::surface::menu::row_action::RowActionEngine;
use crate::surface::menu::row_action::RowEmission;
use crate::surface::menu::row_action::RowSettlement;
use crate::surface::menu::row_action::row_tracks_action;
use crate::token::color::Palette;
use crate::token::contrast;
use crate::widget::button;
use crate::widget::button::ButtonVariant;
use crate::widget::button_controller::ActivationSource;

use iced::Color;
use iced::Rectangle;

// ---------------------------------------------------------------------------
// Spec gaps this module closes by decision
// ---------------------------------------------------------------------------

/// **Spec gap** — how an item declares its overflow priority.
///
/// §3B.8 says a control moves "deterministically" into the overflow menu
/// but never says what the determination reads. Decided here: an explicit
/// [`OverflowRank`] per item, lower evicted first, with layout order
/// excluded from the primary comparison. See the module docs.
pub const GAP_PRIORITY_MECHANISM: &str = "UIF-COMP-008 spec gap: the overflow priority mechanism is \
                                        unspecified; decided as an explicit per-item OverflowRank, \
                                        lower evicted first";

/// The tie-break between items declaring the same rank.
///
/// **Rightmost first.** Within one declared class of expendability the
/// leftmost survivors keep the inline run contiguous from the strip's
/// leading edge. Recorded as a decision because the spec gives no rule.
pub const TIE_BREAK: &str = "UIF-COMP-008 decision: equal ranks evict rightmost-first, keeping the \
                            inline run contiguous from the leading edge";

/// **Spec gap** — whether the `⋯` trigger is reachable by keyboard.
///
/// §3B.8 describes the trigger but not its focus behaviour. Decided here:
/// it is a normal focus stop, and `Escape` from the menu returns focus to
/// it — which requires the trigger to be focused *before* the cascade
/// opens, because the trap restores what held focus at push time.
pub const GAP_TRIGGER_FOCUS: &str = "UIF-COMP-008 spec gap: trigger keyboard reachability is \
                                    unspecified; decided focusable, with Escape returning to it";

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Height of the toolbar strip.
///
/// Fixed. See [`strip_height`] for why this is the structural form of the
/// `0px` vertical-shift guarantee.
pub const STRIP_HEIGHT: f32 = 28.0;

/// Height of an inline toolbar item.
pub const ITEM_HEIGHT: f32 = 24.0;

/// Gap between two adjacent inline items.
pub const ITEM_GAP: f32 = 4.0;

/// The `⋯` button's width — and its height (§3B.8: `28×28px`).
pub const TRIGGER_SIZE: f32 = 28.0;

/// Clearance held in front of the `⋯` button.
pub const TRIGGER_GAP: f32 = 8.0;

/// The width held back for the overflow affordance.
///
/// §3B.8 fixes this at `36px`: an item whose right edge would pass
/// `available_width - TRIGGER_RESERVE` is the item that moves. It is the
/// trigger plus its clearance, so the reserve and the thing being reserved
/// cannot drift apart.
pub const TRIGGER_RESERVE: f32 = TRIGGER_SIZE + TRIGGER_GAP;

/// Floor on an item's measured width.
///
/// An unmeasured or zero-width label must not collapse a button's hit
/// target to nothing, so no item is narrower than the item height.
pub const MIN_ITEM_WIDTH: f32 = ITEM_HEIGHT;

/// The `⋯` label.
pub const TRIGGER_LABEL: &str = "⋯";

/// Nothing in the strip may be taller than the strip. A taller element
/// would make the height depend on *which* elements were inline, which is
/// exactly the shift `UIF-PRIN-003` forbids.
const _: () = assert!(ITEM_HEIGHT <= STRIP_HEIGHT);

/// Same for the trigger: it fits inside the strip rather than defining it.
const _: () = assert!(TRIGGER_SIZE <= STRIP_HEIGHT);

/// The strip's height.
///
/// **This function takes no arguments.** That absence is the proof. The
/// inline/overflow split, the item count and the available width are the
/// three things a naive implementation would let reach a height, and none
/// of them is a parameter, so no eviction decision can move the strip by
/// even a pixel. The [`STRIP_HEIGHT`](crate::token) assertions above
/// close the remaining route: every element the strip paints is already
/// shorter than it.
#[must_use]
pub const fn strip_height() -> f32 {
    STRIP_HEIGHT
}

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/// How readily an item gives up its inline slot.
///
/// **Lower values are evicted first.** The direction is stated here
/// because it is the single most likely thing to be read backwards, and a
/// toolbar that evicts its most important button first fails silently
/// rather than loudly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverflowRank(u8);

impl OverflowRank {
    /// A rank from a raw value.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// The raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }

    /// Gives up its slot first — decoration, not function.
    pub const DISPOSABLE: Self = Self(0);

    /// An ordinary secondary control.
    pub const SECONDARY: Self = Self(100);

    /// A control worth keeping on screen.
    pub const PRIMARY: Self = Self(200);

    /// Evicted only when nothing else can be.
    pub const LAST: Self = Self(255);
}

impl fmt::Display for OverflowRank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Where an item landed after [`fit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemPlacement {
    /// Painted inline in the strip.
    Inline,
    /// Behind the `⋯` button.
    Overflow,
}

/// One item's resolved placement.
///
/// Carries the rank so a caller can see *why* an item landed where it did
/// without going back to the declaration, but carries no enabled or
/// pending state: placement is the only thing the solver is allowed to
/// decide, and mirroring the mutable state here would create a second copy
/// able to disagree with the first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemPlan {
    /// The item's index in the caller's list.
    pub index: usize,
    /// The rank it declared.
    pub rank: OverflowRank,
    /// Inline or overflowed.
    pub placement: ItemPlacement,
    /// The width it occupies when inline.
    pub width: f32,
}

impl ItemPlan {
    /// Whether this item is painted inline.
    #[must_use]
    pub const fn is_inline(&self) -> bool {
        matches!(self.placement, ItemPlacement::Inline)
    }

    /// Whether this item sits behind the `⋯` button.
    #[must_use]
    pub const fn is_overflow(&self) -> bool {
        matches!(self.placement, ItemPlacement::Overflow)
    }
}

/// One item's declaration.
///
/// `measure` is the item's **full inline width**, supplied by the caller,
/// which keeps text measurement out of the solver.
#[derive(Clone, Debug, PartialEq)]
pub struct OverflowItem<Id> {
    /// The host's stable item identity.
    pub id: Id,
    /// The item's label.
    pub label: String,
    /// The full inline width.
    pub measure: f32,
    /// What the item declared about eviction.
    pub rank: OverflowRank,
    /// Whether the action is available.
    pub enabled: bool,
    /// The action's in-flight projection.
    pub pending: ActionPending,
}

impl<Id> OverflowItem<Id> {
    /// Declare an item at a rank.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>, measure: f32, rank: OverflowRank) -> Self {
        Self {
            id,
            label: label.into(),
            measure,
            rank,
            enabled: true,
            pending: ActionPending::idle(),
        }
    }

    /// Set the enabled flag.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the pending projection.
    #[must_use]
    pub const fn with_pending(mut self, pending: ActionPending) -> Self {
        self.pending = pending;
        self
    }

    /// The item's identity.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The item's label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The item's width as the solver sees it: never below the floor.
    #[must_use]
    pub fn width(&self) -> f32 {
        if self.measure < MIN_ITEM_WIDTH {
            MIN_ITEM_WIDTH
        } else {
            self.measure
        }
    }
}

/// The resolved layout of a strip.
#[derive(Clone, Debug, PartialEq)]
pub struct OverflowLayout {
    plans: Vec<ItemPlan>,
    inline_width: f32,
    trigger_width: f32,
    available_width: f32,
}

impl OverflowLayout {
    /// Every item's plan, in declaration order.
    #[must_use]
    pub fn plans(&self) -> &[ItemPlan] {
        &self.plans
    }

    /// The plan for one item index.
    #[must_use]
    pub fn plan(&self, index: usize) -> Option<&ItemPlan> {
        self.plans.get(index)
    }

    /// How many items are painted inline.
    #[must_use]
    pub fn inline_count(&self) -> usize {
        self.plans.iter().filter(|plan| plan.is_inline()).count()
    }

    /// How many items sit behind the `⋯` button.
    #[must_use]
    pub fn overflow_count(&self) -> usize {
        self.plans.iter().filter(|plan| plan.is_overflow()).count()
    }

    /// The indices behind the `⋯` button, in declaration order.
    ///
    /// The overflow menu is built from exactly this list, which is what
    /// makes "the menu contains exactly the overflowed items" true by
    /// construction rather than by care.
    #[must_use]
    pub fn overflow_indices(&self) -> Vec<usize> {
        self.plans
            .iter()
            .filter(|plan| plan.is_overflow())
            .map(|plan| plan.index)
            .collect()
    }

    /// Whether anything overflowed.
    #[must_use]
    pub fn is_overflowing(&self) -> bool {
        self.overflow_count() > 0
    }

    /// The width the inline items occupy, gaps included.
    #[must_use]
    pub const fn inline_width(&self) -> f32 {
        self.inline_width
    }

    /// The `⋯` button's width — `0.0` when nothing overflowed.
    #[must_use]
    pub const fn trigger_width(&self) -> f32 {
        self.trigger_width
    }

    /// The width the whole strip wants.
    #[must_use]
    pub const fn total_width(&self) -> f32 {
        if self.trigger_width > 0.0 {
            self.inline_width + TRIGGER_GAP + self.trigger_width
        } else {
            self.inline_width
        }
    }

    /// The width this layout was solved against.
    #[must_use]
    pub const fn available_width(&self) -> f32 {
        self.available_width
    }

    /// Whether nothing overflowed.
    #[must_use]
    pub const fn fits(&self) -> bool {
        self.trigger_width == 0.0
    }
}

// ---------------------------------------------------------------------------
// The solver
// ---------------------------------------------------------------------------

/// The order items are evicted in.
///
/// Sorted by declared rank ascending — lower rank goes first — and by
/// layout index *descending* within a rank, which is the
/// [`TIE_BREAK`]. Layout index is the last key, never the first: the whole
/// point of the declaration is that moving an item along the strip does
/// not change when it gets evicted.
#[must_use]
pub fn eviction_order<Id>(items: &[OverflowItem<Id>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|a, b| items[*a].rank.cmp(&items[*b].rank).then(b.cmp(a)));
    order
}

/// The inline width when exactly the indices in `evicted` are removed.
fn inline_width_excluding(measures: &[f32], evicted: &[usize]) -> f32 {
    let mut width = 0.0;
    let mut seen = 0usize;
    for (index, measure) in measures.iter().enumerate() {
        if evicted.contains(&index) {
            continue;
        }
        if seen > 0 {
            width += ITEM_GAP;
        }
        width += measure;
        seen += 1;
    }
    width
}

/// Solve a strip against an available width.
///
/// Items are evicted in [`eviction_order`] until what remains fits inside
/// `available_width - TRIGGER_RESERVE`, the threshold §3B.8 fixes. The
/// evicted set is always a **prefix of the eviction order**, which makes
/// the result monotone: widening the strip can only shorten that prefix,
/// never swap one arbitrary set for another.
///
/// The reserve is charged as soon as anything has to go, even if the
/// trigger is the only thing left standing — a strip with no inline items
/// still has to be able to reach them.
#[must_use]
pub fn fit<Id>(items: &[OverflowItem<Id>], available_width: f32) -> OverflowLayout {
    let measures: Vec<f32> = items.iter().map(OverflowItem::width).collect();
    let order = eviction_order(items);
    let mut evicted: Vec<usize> = Vec::new();

    // The reserve is only charged once the trigger is actually needed.
    // Charging it always would evict an item from a strip that already
    // fits, which is the opposite of what §3B.8 asks for.
    if inline_width_excluding(&measures, &[]) > available_width {
        while inline_width_excluding(&measures, &evicted) > available_width - TRIGGER_RESERVE {
            let Some(next) = order.get(evicted.len()).copied() else {
                break;
            };
            evicted.push(next);
        }
    }

    let plans: Vec<ItemPlan> = measures
        .iter()
        .enumerate()
        .map(|(index, width)| ItemPlan {
            index,
            rank: items[index].rank,
            placement: if evicted.contains(&index) {
                ItemPlacement::Overflow
            } else {
                ItemPlacement::Inline
            },
            width: *width,
        })
        .collect();

    let trigger_width = if evicted.is_empty() {
        0.0
    } else {
        TRIGGER_SIZE
    };
    let inline_width = inline_width_excluding(&measures, &evicted);

    OverflowLayout {
        plans,
        inline_width,
        trigger_width,
        available_width,
    }
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

/// The inline items' rectangles, in declaration order.
///
/// Every rectangle is [`ITEM_HEIGHT`] tall and centred on the strip, so
/// none of them can make the strip taller no matter how many survive.
#[must_use]
pub fn item_rects(layout: &OverflowLayout, x: f32, strip_y: f32) -> Vec<(usize, Rectangle)> {
    let inset = (STRIP_HEIGHT - ITEM_HEIGHT) / 2.0;
    let mut cursor = x;
    let mut out = Vec::with_capacity(layout.inline_count());
    for plan in layout.plans().iter().filter(|plan| plan.is_inline()) {
        out.push((
            plan.index,
            rect(cursor, strip_y + inset, plan.width, ITEM_HEIGHT),
        ));
        cursor += plan.width + ITEM_GAP;
    }
    out
}

/// The `⋯` button's rectangle, if anything overflowed.
#[must_use]
pub fn trigger_rect(layout: &OverflowLayout, x: f32, strip_y: f32) -> Option<Rectangle> {
    if layout.trigger_width == 0.0 {
        return None;
    }
    let inset = (STRIP_HEIGHT - TRIGGER_SIZE) / 2.0;
    Some(rect(
        x + layout.inline_width() + TRIGGER_GAP,
        strip_y + inset,
        TRIGGER_SIZE,
        TRIGGER_SIZE,
    ))
}

// ---------------------------------------------------------------------------
// Focus
// ---------------------------------------------------------------------------

/// Prefix for an inline item's focus id.
pub const ITEM_FOCUS_PREFIX: &str = "uif.overflow.toolbar.item";

/// The `⋯` button's focus id.
pub const TRIGGER_FOCUS_ID: &str = "uif.overflow.toolbar.trigger";

/// The [`FocusId`] for the item at `index`.
#[must_use]
pub fn item_focus_id(index: usize) -> FocusId {
    FocusId::new(format!("{ITEM_FOCUS_PREFIX}:{index}"))
}

/// The [`FocusId`] for the `⋯` button.
#[must_use]
pub fn trigger_focus_id() -> FocusId {
    FocusId::new(TRIGGER_FOCUS_ID)
}

/// The [`FocusMember`] for the item at `index`.
#[must_use]
pub fn item_member(index: usize) -> FocusMember {
    FocusMember::new(item_focus_id(index))
}

/// The [`FocusMember`] for the `⋯` button.
#[must_use]
pub fn trigger_member() -> FocusMember {
    FocusMember::new(trigger_focus_id())
}

/// Every member a `Tab` may stop on in this strip.
///
/// Overflowed items are absent from the strip but not unreachable: the
/// menu the trigger opens carries them as navigable rows.
#[must_use]
pub fn navigable_members(layout: &OverflowLayout) -> Vec<FocusMember> {
    let mut members: Vec<FocusMember> = layout
        .plans()
        .iter()
        .filter(|plan| plan.is_inline())
        .map(|plan| item_member(plan.index))
        .collect();
    if layout.is_overflowing() {
        members.push(trigger_member());
    }
    members
}

// ---------------------------------------------------------------------------
// The overflow menu
// ---------------------------------------------------------------------------

/// The overflow menu's [`MenuId`](crate::surface::menu::model::MenuId).
pub const MENU_ID: &str = "uif.overflow.toolbar.menu";

/// The row key for an item.
///
/// Keyed by the host's own identity rather than by position, so a row the
/// engine reports back still names the same action after the strip has been
/// re-solved at a different width.
#[must_use]
pub fn row_key<Id: fmt::Display>(item: &OverflowItem<Id>) -> String {
    item.id.to_string()
}

/// Build the overflow menu for a solved layout.
///
/// The rows are built from the *live* items, so opening the menu a second
/// time after an item's enabled flag or label changed shows the change:
/// there is no cached menu to go stale.
///
/// A busy item's row is **not** disabled. Disabling it would remove it
/// from the menu's focus roster and make an in-flight action unreachable
/// by keyboard; the refusal belongs to [`RowActionEngine`], which turns a
/// second press into a stop request instead.
#[must_use]
pub fn overflow_menu<Id: fmt::Display>(
    items: &[OverflowItem<Id>],
    layout: &OverflowLayout,
) -> Menu {
    let rows: Vec<Row> = layout
        .overflow_indices()
        .into_iter()
        .map(|index| {
            let item = &items[index];
            Row::action(row_key(item), item.label.clone()).with_enabled(item.enabled)
        })
        .collect();
    Menu::new(MENU_ID, rows)
}

/// Whether an item's action needs its activations gated.
///
/// The toolbar's own view of
/// [`crate::surface::menu::row_action::row_tracks_action`]:
/// work is underway, or the host said it can settle what gets dispatched.
/// Exposed so a caller can tell whether a busy posture is actually being
/// enforced without reaching into the binding layer.
#[must_use]
pub fn item_gates_action<Id>(item: &OverflowItem<Id>) -> bool {
    row_tracks_action(&item.pending)
}

/// The colour of the `⋯` glyph — §3.1's ghost foreground.
///
/// The trigger is a quiet control like every other toolbar button, so its
/// paint comes from the ghost column rather than from a second opinion
/// here.
#[must_use]
pub fn trigger_glyph_colour(palette: &Palette, state: VisualState) -> Color {
    button::resolve_style(ButtonVariant::Ghost, state, palette).foreground
}

/// The `⋯` button's fill — §3.1's ghost background.
#[must_use]
pub fn trigger_fill(palette: &Palette, state: VisualState) -> Color {
    button::resolve_style(ButtonVariant::Ghost, state, palette).background
}

/// The surface the `⋯` glyph is actually painted on.
#[must_use]
pub fn trigger_backdrop(palette: &Palette, state: VisualState) -> Color {
    match state.base {
        BaseState::Pressed => palette.surface_active,
        BaseState::Hovered => palette.surface_hover,
        BaseState::Rest | BaseState::Disabled | BaseState::Invalid => palette.surface_base,
    }
}

/// The `⋯` glyph's contrast against its actual backdrop.
#[must_use]
pub fn trigger_contrast(palette: &Palette, state: VisualState) -> f32 {
    contrast::contrast_ratio(
        trigger_glyph_colour(palette, state),
        trigger_backdrop(palette, state),
    )
}

/// The contrast of an overflowed item's label in the menu.
///
/// `menu_fill` is the row's own resolved background so the measurement
/// follows the row rather than assuming the strip's surface — a disabled
/// item on a hovered row is a different pairing from an enabled item at
/// rest, and the two are gated differently.
#[must_use]
pub fn overflow_row_contrast(foreground: Color, menu_fill: Color) -> f32 {
    contrast::contrast_ratio(foreground, menu_fill)
}

/// Whether a disabled item's label in the overflow menu sits inside the
/// muted legibility band `UIF-TOK-006` rule 2 sets for `text.disabled`.
#[must_use]
pub fn disabled_row_in_band(palette: &Palette, menu_fill: Color) -> bool {
    contrast::meets_disabled_text_band(palette.text_disabled, menu_fill)
}

// ---------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------

/// One piece of output the toolbar host produced.
///
/// A sum type for the reason every other host in the crate has one: a
/// caller wiring these is forced to write an arm for each tier rather
/// than remember to wire the one that matters.
#[derive(Clone)]
pub enum ToolbarEmission<Id> {
    /// The overflow menu opened (`true`) or closed (`false`).
    MenuToggled(bool),
    /// An item dispatched a command, inline or through the menu.
    Activated(Commit<RowSettlement>),
    /// A second press landed on an already-busy item and asked it to stop.
    StopRequested(CancelRequest),
    /// The item the [`Activated`](Self::Activated) tier names.
    ///
    /// Carried alongside rather than inside the commit, because
    /// [`RowSettlement`] keys by string and the host wants its own `Id`
    /// back.
    ActivatedId(Id),
}

impl<Id> ToolbarEmission<Id> {
    /// The settlement carried by the activation tier.
    #[must_use]
    pub fn settlement(&self) -> Option<&RowSettlement> {
        match self {
            Self::Activated(commit) => Some(commit.value()),
            Self::MenuToggled(_) | Self::StopRequested(_) | Self::ActivatedId(_) => None,
        }
    }
}

impl<Id: PartialEq> PartialEq for ToolbarEmission<Id> {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] is a linear wrapper that
    /// deliberately does not implement `PartialEq`.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MenuToggled(a), Self::MenuToggled(b)) => a == b,
            (Self::Activated(a), Self::Activated(b)) => a.value() == b.value(),
            (Self::StopRequested(a), Self::StopRequested(b)) => a == b,
            (Self::ActivatedId(a), Self::ActivatedId(b)) => a == b,
            _ => false,
        }
    }
}

impl<Id: fmt::Debug> fmt::Debug for ToolbarEmission<Id> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MenuToggled(open) => f.debug_tuple("MenuToggled").field(open).finish(),
            Self::Activated(commit) => f.debug_tuple("Activated").field(commit.value()).finish(),
            Self::StopRequested(request) => f.debug_tuple("StopRequested").field(request).finish(),
            Self::ActivatedId(id) => f.debug_tuple("ActivatedId").field(id).finish(),
        }
    }
}

/// Everything one host input did.
#[derive(Clone, Debug)]
pub struct ToolbarOutcome<Id> {
    /// The cascade engine's report for the menu side of the input.
    pub cascade: CascadeOutcome,
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<ToolbarEmission<Id>>,
}

impl<Id> ToolbarOutcome<Id> {
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
    pub fn emissions(emissions: Vec<ToolbarEmission<Id>>) -> Self {
        Self {
            cascade: CascadeOutcome::noop(),
            emissions,
        }
    }
}

/// The per-control memory an overflow toolbar needs.
///
/// One cascade, one row-action engine, and the mapping from menu row back
/// to the item it was built for. Nothing else — in particular there is no
/// `popup_open` flag, because openness is cascade state and a second copy
/// of it here is a second copy able to disagree.
#[derive(Debug)]
pub struct OverflowToolbarHost {
    /// The one menu engine this control is allowed to have.
    cascade: MenuCascade,
    /// The one activation engine overflowed rows run on.
    actions: RowActionEngine,
    /// The trigger's focus identity — what the trap is told to come back to.
    trigger: FocusId,
    /// The item each open menu row stands for, captured at build time.
    row_items: Vec<usize>,
    /// Whether the trigger is hovered.
    hovered: bool,
    /// Whether a press is held on the trigger.
    pressed: bool,
}

impl OverflowToolbarHost {
    /// A host whose trigger is focusable as `trigger`.
    #[must_use]
    pub fn new(trigger: FocusId) -> Self {
        Self {
            cascade: MenuCascade::new(),
            actions: RowActionEngine::new(),
            trigger,
            row_items: Vec::new(),
            hovered: false,
            pressed: false,
        }
    }

    /// Whether the overflow menu is showing.
    #[must_use]
    pub fn is_menu_open(&self) -> bool {
        self.cascade.is_open()
    }

    /// The cascade, read-only.
    #[must_use]
    pub const fn cascade(&self) -> &MenuCascade {
        &self.cascade
    }

    /// The row-action engine, read-only.
    #[must_use]
    pub const fn actions(&self) -> &RowActionEngine {
        &self.actions
    }

    /// The trigger's focus identity.
    #[must_use]
    pub const fn trigger(&self) -> &FocusId {
        &self.trigger
    }

    /// The trigger's visual state.
    #[must_use]
    pub fn trigger_state(&self, focused: bool) -> VisualState {
        let base = if self.pressed {
            BaseState::Pressed
        } else if self.hovered {
            BaseState::Hovered
        } else {
            BaseState::Rest
        };
        VisualState { base, focused }
    }

    /// The item a menu row stands for.
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

    /// Open the overflow menu.
    ///
    /// The trigger is focused *first* so the trap the cascade pushes
    /// records it, which is what makes `Escape` return the caret to the
    /// `⋯` rather than dropping it. A menu with no navigable row is
    /// refused rather than opened empty, matching the engine's own rule.
    #[must_use]
    pub fn open_menu<Id: fmt::Display>(
        &mut self,
        items: &[OverflowItem<Id>],
        layout: &OverflowLayout,
        focus: &mut FocusState,
    ) -> ToolbarOutcome<Id> {
        if !layout.is_overflowing() {
            return ToolbarOutcome::noop();
        }
        let menu = overflow_menu(items, layout);
        if menu.first_navigable().is_none() {
            return ToolbarOutcome::noop();
        }
        if focus.current() != Some(&self.trigger) {
            focus.focus(&self.trigger);
        }
        self.row_items = layout.overflow_indices();
        let cascade = self.cascade.open(&menu, MenuTreeKind::Fixed, focus);
        if cascade.ignored_reason().is_some() {
            self.row_items.clear();
            return ToolbarOutcome {
                cascade,
                emissions: Vec::new(),
            };
        }
        ToolbarOutcome {
            cascade,
            emissions: vec![ToolbarEmission::MenuToggled(true)],
        }
    }

    /// Close the overflow menu.
    #[must_use]
    pub fn close_menu<Id>(&mut self, focus: &mut FocusState) -> ToolbarOutcome<Id> {
        if !self.cascade.is_open() {
            return ToolbarOutcome::noop();
        }
        let cascade = self.cascade.close_tree(focus);
        self.row_items.clear();
        ToolbarOutcome {
            cascade,
            emissions: vec![ToolbarEmission::MenuToggled(false)],
        }
    }

    /// Toggle the overflow menu.
    #[must_use]
    pub fn toggle_menu<Id: fmt::Display>(
        &mut self,
        items: &[OverflowItem<Id>],
        layout: &OverflowLayout,
        focus: &mut FocusState,
    ) -> ToolbarOutcome<Id> {
        if self.cascade.is_open() {
            self.close_menu(focus)
        } else {
            self.open_menu(items, layout, focus)
        }
    }

    /// Drive one menu key through the engine (`UIF-CMENU-003`).
    ///
    /// The activated row is translated back to the host's own `Id` here,
    /// and the activation is put through [`RowActionEngine`] rather than
    /// dispatched directly — which is the whole reason a busy overflowed
    /// item still behaves like a busy item.
    #[must_use]
    pub fn menu_key<Id: fmt::Display + Clone>(
        &mut self,
        key: MenuKey,
        items: &[OverflowItem<Id>],
        focus: &mut FocusState,
    ) -> ToolbarOutcome<Id> {
        let was_open = self.cascade.is_open();
        let cascade = self.cascade.on_key(key, focus);
        let mut emissions = Vec::new();

        if let Some(activation) = cascade.activation()
            && !activation.is_absorbed()
            && !matches!(activation.effect, ActivationEffect::OpenSubmenu { .. })
            && let Some(item_index) = self.item_for_row(activation.row)
        {
            let item = &items[item_index];
            let row = Row::action(row_key(item), item.label.clone()).with_enabled(item.enabled);
            self.actions.observe(item.pending);
            if let Some(emission) =
                self.actions
                    .commit(&row, &activation.effect, ActivationSource::Keyboard)
            {
                match emission {
                    RowEmission::Committed(commit) => {
                        emissions.push(ToolbarEmission::Activated(commit));
                        emissions.push(ToolbarEmission::ActivatedId(item.id.clone()));
                    }
                    RowEmission::CancelRequested(request) => {
                        emissions.push(ToolbarEmission::StopRequested(request));
                    }
                    RowEmission::Cancelled(_) => {}
                }
            }
        }

        if was_open && !self.cascade.is_open() {
            self.row_items.clear();
            emissions.push(ToolbarEmission::MenuToggled(false));
        }

        ToolbarOutcome { cascade, emissions }
    }

    /// `Escape`: dismiss the menu, and the trap returns focus to the
    /// trigger.
    #[must_use]
    pub fn escape<Id: fmt::Display>(&mut self, focus: &mut FocusState) -> ToolbarOutcome<Id> {
        self.close_menu(focus)
    }

    /// Record the pointer moving onto the trigger.
    pub fn hover_trigger(&mut self) {
        self.hovered = true;
    }

    /// Record the pointer leaving the trigger.
    pub fn pointer_leave(&mut self) {
        self.hovered = false;
    }

    /// Record a press starting on the trigger.
    pub fn press_trigger(&mut self) {
        self.pressed = true;
    }

    /// Settle a press released over the trigger.
    #[must_use]
    pub fn release_trigger<Id: fmt::Display>(
        &mut self,
        items: &[OverflowItem<Id>],
        layout: &OverflowLayout,
        focus: &mut FocusState,
    ) -> ToolbarOutcome<Id> {
        self.pressed = false;
        self.toggle_menu(items, layout, focus)
    }

    /// Drop pointer state without emitting, leaving the menu alone.
    pub fn reset(&mut self) {
        self.hovered = false;
        self.pressed = false;
        self.actions.reset();
    }
}

#[cfg(test)]
#[path = "overflow_toolbar_tests.rs"]
mod overflow_toolbar_tests;
