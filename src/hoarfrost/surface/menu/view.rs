//! The menu row — the §4.9.2 state matrix and the §4.8.1 row grid, rendered
//! (`UIF-CMENU-*`, `UIF-FMENU-*`).
//!
//! # What this increment proves
//!
//! [`super::model`] made a menu row a fact: its kind, its slots, whether it
//! is destructive, whether it closes. It painted nothing. This module turns
//! that fact into pixels on the shared row grid, and solves the row's two
//! hard problems: a matrix with eight printed rows that contains six
//! contrast traps (gap **G-11**), and a grid whose 16px leading box has to
//! keep every label in the card aligned whether or not this particular row
//! carries a checkmark.
//!
//! [`resolve_row_style`] answers the matrix. [`MenuRow`] is a real,
//! focusable, drawing widget that asks it every frame and paints whatever
//! comes back. There is no colour in the widget that is not a token read
//! through the resolver, no state that is not [`state::resolve`] folded
//! through [`derive_row_state`], and no focus mechanism that is not the
//! [`FocusableState`] `INC-10`
//! proved.
//!
//! # The matrix is a function, not a lookup table
//!
//! §4.9.2 prints eight rows. [`state`] makes focus *additive*, so those
//! are not eight exclusive cases. *Keyboard Focused* is
//! [`RowBase::Hovered`] with the outline on, and the bases compose into
//! cells the matrix never drew — a destructive row that also holds the
//! outline, a
//! submenu parent under the cursor *and* focused. [`RowBase`] carries the
//! mutually exclusive bases the matrix implies plus
//! [`DestructivePressed`](RowBase::DestructivePressed), the destructive
//! row held down, and [`RowState`] pairs a base with the additive outline.
//! Every resulting cell resolves through the same function, so a cell the
//! spec never printed cannot be accidentally undefined and cannot disagree
//! with its neighbours.
//!
//! [`RowState::SPEC_ROWS`] is the eight the matrix has always printed;
//! [`RowState::DERIVED_ROWS`] is the three a real menu reaches that the
//! original matrix left out. One of those three is no longer an omission:
//! the 2026-10-22 revision prints *Destructive Row (Pressed)* outright,
//! and this file paints the token it names. The list split itself is left
//! to the coverage pass that owns it — see [`RowBase`]'s docs.
//! [`RowState::matrix_row`] names whichever row a cell answers to, so a
//! test asserts against the spec's own wording rather than a re-labelled
//! guess.
//!
//! # The six traps of G-11, and where each is defused
//!
//! Every one of these was measured by INC-14 against the **retuned**
//! palette. None of them is fixed by copying the matrix's printed token,
//! because the matrix prints the token that fails.
//!
//! 1. **Destructive hover.** The matrix prints `status.danger` behind
//!    `text.strong` at **3.8753** against the 4.5 text floor. The
//!    resolver paints [`status_danger_fill`](Palette::status_danger_fill)
//!    instead — **5.1270** (decision **D-5**).
//! 2. **The focus outline on a destructive row.** `accent.border` on
//!    that fill measures **1.7188**, worse than G-9 ever was for a
//!    button. The resolver never reads the matrix's outline column; it
//!    asks [`resolve_focus_indicator_for_background`] against the
//!    background it *just resolved*, so the outline on a hot destructive
//!    fill comes back [`text_strong`](Palette::text_strong).
//! 3. **A disabled row inheriting an active ancestor's fill.**
//!    `text.disabled` clears 5.2686 on `surface.overlay` and 4.8280 on
//!    `surface.hover`, but only **4.2510** on `surface.active`.
//!    The spec's "Disabled → `transparent`" is load-bearing, so
//!    [`RowBase::Disabled`] paints no fill at all and the row's own
//!    background never picks up what was painted behind it.
//! 4. **`accent.text` carrying a `✓` on a destructive fill** — **2.7460**.
//!    The destructive cells set every slot to `text.strong`, including the
//!    leading glyph, so the checkmark cannot keep the Rest row's accent.
//! 5. **The keyboard outline on a normal row passes thinly** —
//!    `accent.border` on `surface.hover` is **5.4427**, headroom 2.4427.
//!    Nothing here lightens that fill; the sidecar pins the pairing so a
//!    future retune that eats the headroom fails a test.
//! 6. **Fill-vs-fill deltas are ungated.** `surface.hover` against
//!    `surface.overlay` is 1.0913. The row-lift affordance rests on a
//!    delta no WCAG rule protects, so it is asserted as a *delta* in the
//!    sidecar rather than assumed.
//!
//! The separator (`border.subtle` on `surface.overlay` = **1.2017**) is
//! deliberately below the floor: rule 2 of `UIF-TOK-006` governs *active*
//! indicators, and a resting divider is supposed to be quiet.
//!
//! # The leading box is reserved even when it is empty
//!
//! §4.8.1 puts a 16×16 leading slot on every row. A menu where only the
//! checked rows have one would jog every unchecked label 24px to the left
//! the moment a toggle flipped — a `UIF-PRIN-003` violation caused by
//! content, not by state. [`MenuRow::new`] therefore always emits a
//! leading slot for an interactive row, glyph or not. A heading and a
//! separator are the two exceptions the grid itself specifies: a heading
//! has no leading box, and a separator has no slots at all.
//!
//! # No state fill is synthesised from opacity
//!
//! Every fill this resolver emits is either fully opaque or the literal
//! `transparent` of a row that paints nothing. The destructive pressed
//! row used to break that: with no pressed token in existence it painted
//! `status.danger.fill` at 85% alpha, which made the label's contrast a
//! function of whatever the card happened to be rather than of the fill —
//! two defensible numbers for one row, and only one of them the screen.
//! §4.9 now supplies [`status_danger_fill_active`](Palette::status_danger_fill_active),
//! the constant that carried the opacity is deleted rather than parked,
//! and the sidecar asserts the opaque-or-nothing invariant across every
//! cell of every resolver in this crate so a future "just dim it a
//! little" cannot put the hack back.
//!
//! # The focused row is inset, not ringed
//!
//! §3.0.2 gives a row a **`1px` inset outline**, not the `2px` outer
//! ring a button or an input gets, because rows are packed edge to edge
//! and a ring grown outward is cut by its neighbours. [`focus_outline_layer`]
//! draws the outline on the row's own quad, which iced strokes inward,
//! so the whole `1px` sits inside the perimeter and nothing spills.
//!
//! What the row does *not* have is a second colour rule. Its outline is
//! resolved by [`resolve_focus_indicator_for_background`] at
//! [`FocusPlacement::Inset`] — the same function, the same two
//! candidates and the same ordering that produce a button's outer ring,
//! with the placement supplying the stroke weight and the edge. That is
//! why the two focused cells of §4.9.2 fall out of arithmetic instead of
//! being configured:
//!
//! | Focused row | Background | Outline | Why |
//! |---|---|---|---|
//! | Normal | `surface.hover` | `accent.border` | `5.4427:1` clears the `3.00` chrome floor |
//! | Destructive | `status.danger.fill` | `text.strong` | the accent is `1.7188:1` there; `text.strong` is `5.1270:1` |
//!
//! Neither cell names its outline colour, and neither can be made to.
//!
//! **A note on the spec's own number:** §4.9.2 prints the normal focused
//! row's outline at `5.35:1`. The solver says `5.4427:1` — the value
//! trap 5 pins. The crate's solver is authoritative over the spec's
//! rounded column, and the discrepancy is recorded here rather than
//! smoothed over.
//!
//! # What is deliberately not here
//!
//! * **The card.** No perimeter, no 6px card padding, no 224–320px width
//!   policy. This module renders a row and knows nothing about the surface
//!   it is stacked into.
//! * **The cascade.** Whether a submenu parent's child is *open* is
//!   `INC-16`'s state, not the row's. [`MenuRow::submenu_open`] is the
//!   seam that lets the cascade tell this row to wear the
//!   [`RowBase::ActiveSubmenuParent`] skin; nothing
//!   here reads or sets it.
//! * **Cross-row keyboard navigation.** `Up`/`Down`/`Left`/`Right` are
//!   `INC-16` (`UIF-CMENU-003`). This row registers with the focus router
//!   and handles the pointer. `Enter`/`Space` activation is listed under
//!   the same requirement and is deferred with the rest of them.
//! * **Dismissal.** Click-away and single-tree mutual exclusion
//!   (`UIF-CMENU-002`) belong to the tree. The row reports only whether it
//!   *absorbed* a click, which is the fact the tree needs in order not to
//!   close.
//!
//! Unit tests live in the sibling `view_tests.rs` sidecar.

use crate::focus::FocusId;
use crate::identity::{publish_text, role};
use crate::state::{self, BaseState, InteractionInput, VisualState};
use crate::surface::menu::model::{LeadingSlot, Row, RowKind, TrailingSlot};
use crate::token::color::Palette;
use crate::token::contrast;
use crate::token::{shape, size, space, typography};
use crate::widget::button_widget::Layer;
use crate::widget::focus::{
    FocusIndicator, FocusPlacement, resolve_focus_indicator_for_background,
};
use crate::widget::focusable::FocusableState;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Id, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::widget::text;
use iced::{Background, Border, Color, Element, Event, Length, Point, Rectangle, Size};

// ---------------------------------------------------------------------------
// Row geometry (§4.8.1)
// ---------------------------------------------------------------------------

/// §4.8.1 — a menu row is 32px tall (`size.control.md`).
pub const ROW_HEIGHT: f32 = size::CONTROL_MD;

/// §4.8.1 — the row's `[0, 10px]` horizontal padding.
///
/// There is no 10px rung on the `space` ladder, so the spec's value is
/// *composed* from two rungs rather than written as a literal. The
/// composition is the token read: 8 (`space.sm`) plus 2 (`space.xxs`).
pub const ROW_PADDING_X: f32 = space::SM + space::XXS;

/// §4.8.1 — the 8px gap between the row's three slots (`space.sm`).
pub const ROW_GAP: f32 = space::SM;

/// §4.8.1 — the 4px radius of a row's fill (`shape.radius.widget`).
pub const ROW_RADIUS: f32 = shape::RADIUS_WIDGET;

/// §4.8.1 — the leading slot is a 16×16 box (`size.icon.md`).
pub const LEADING_SLOT: f32 = size::ICON_MD;

/// The leading glyph's line height: the slot's own height, as an
/// **absolute** `iced::LineHeight`.
///
/// [`LEADING_SLOT`] is a pixel figure, and iced reads a bare `f32` line
/// height as a multiple of the font size — `16.0` on a 16px glyph would
/// ask for 256px per line and the glyph would be clipped out of the box
/// entirely. See
/// [`TypeRung::iced_line_height`](typography::TypeRung::iced_line_height),
/// which is the rung-carried equivalent of this constant.
pub const LEADING_LINE_HEIGHT: iced::advanced::text::LineHeight =
    iced::advanced::text::LineHeight::Absolute(iced::Pixels(LEADING_SLOT));

/// §4.8.1 — the trailing slot's type size, 12px (`type.caption`).
pub const TRAILING_TYPE: f32 = typography::CAPTION.size;

/// §4.8.1 — a section heading row is 24px tall (`size.bar.status`).
///
/// The spec assigns this row to the status-bar token, not to the hit-target
/// floor. The two are the same number but different kinds of thing: a
/// heading row is not an interactive control and is not *measured against*
/// a minimum, it is simply built to the height a bar is built to.
pub const HEADING_HEIGHT: f32 = size::BAR_STATUS;

/// §4.8.1 — a heading's `[4px, 10px]` vertical padding (`space.xs`).
pub const HEADING_PADDING_Y: f32 = space::XS;

/// §4.8.1 — a separator row is 9px tall.
///
/// Also composed: 8px of breathing room (`space.sm`) plus the 1px rule
/// itself (`shape.border.hairline`).
pub const SEPARATOR_HEIGHT: f32 = space::SM + shape::BORDER_HAIRLINE;

/// §4.9.1 — the separator's own stroke (`shape.border.hairline`).
pub const SEPARATOR_THICKNESS: f32 = shape::BORDER_HAIRLINE;

/// §3.0.2 / §4.9.2 — the focused row's **inset** outline is `1px`
/// (`shape.border.hairline`), not the `2px` attention stroke the outer
/// ring uses.
///
/// The row's placement decides this, not the row: [`FocusPlacement::Inset`]
/// carries the weight, and the constant exists so a test can name the
/// rung the spec points at without going through the placement.
pub const FOCUS_OUTLINE_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The `✓` of a checked row (§4.8.1, `UIF-FMENU-003`).
pub const GLYPH_CHECK: &str = "\u{2713}";

/// The `•` of a selected radio member (§4.8.1, `UIF-FMENU-003`).
pub const GLYPH_RADIO: &str = "\u{2022}";

/// The `>` of a submenu parent (§4.8.1).
pub const GLYPH_CHEVRON: &str = ">";

/// A fill that paints nothing behind what it covers.
///
/// The *absence* of a background, which §4.9.2 writes literally for
/// `Rest`, `Disabled` and `Section Heading` — and which gap **G-11** trap
/// 3 makes load-bearing rather than cosmetic.
const NO_FILL: Color = Color::TRANSPARENT;

/// A stroke that draws nothing.
const NO_STROKE: f32 = 0.0;

/// The absence of available space, used when a row is narrower than its
/// own padding.
const NO_SPACE: f32 = 0.0;

// ---------------------------------------------------------------------------
// The row state
// ---------------------------------------------------------------------------

/// The mutually exclusive half of a menu row's visual state.
///
/// Seven of these are the §4.9.2 rows that are not the keyboard-focus
/// cell. The eighth, [`RowBase::DestructivePressed`](Self::DestructivePressed),
/// is the destructive row held down. §4.9.2 as revised on 2026-10-22
/// prints that row outright against `status.danger.fill_active`, so this
/// base resolves to the token itself. What stood here before the token
/// existed — the danger fill painted at 85% opacity — is deleted rather
/// than kept as a fallback, because a synthesis with no caller is a
/// synthesis that comes back.
///
/// The crate's own [`RowState::SPEC_ROWS`] / [`RowState::DERIVED_ROWS`]
/// split still files this cell under *derived*, and
/// [`RowState::matrix_row`] still labels it that way. The revision made
/// that classification stale; re-classifying it belongs to the coverage
/// pass, not here, and the paint is correct either way.
///
/// *Keyboard Focused* is absent by design. [`state`] makes focus an
/// indicator drawn *over* a base rather than a competitor with one, so
/// the focused row is [`RowBase::Hovered`](Self::Hovered) plus the
/// outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowBase {
    /// §4.9.2 *Rest (Enabled)* — transparent, `text.primary` label.
    Rest,
    /// §4.9.2 *Hovered* — `surface.hover`, everything `text.strong`.
    Hovered,
    /// §4.9.2 *Pressed* — `surface.active`, everything `text.strong`.
    Pressed,
    /// §4.9.2 *Active Submenu Parent* — `surface.hover` with the
    /// chevron held in [`accent_text`](Palette::accent_text).
    ActiveSubmenuParent,
    /// §4.9.2 *Destructive Row (Hovered)* — the danger **fill**, not
    /// the danger text (trap 1, decision **D-5**).
    DestructiveHovered,
    /// §4.9.2 *Destructive Row (Pressed)* — `status.danger_fill_active`,
    /// the real pressed token, everything `text.strong`.
    ///
    /// The destructive counterpart of [`RowBase::Pressed`](Self::Pressed);
    /// see the [enum docs](Self) for why it is a base at all, and for the
    /// classification still filed as derived.
    DestructivePressed,
    /// §4.9.2 *Disabled / Placeholder* — transparent, `text.disabled`.
    Disabled,
    /// §4.9.2 *Section Heading* — transparent, `text.muted`, no slots.
    SectionHeading,
}

/// A menu row's resolved visual state: a [`RowBase`] plus the additive
/// focus outline.
///
/// Mirrors [`state::VisualState`]. The split is what lets §4.9.2's
/// *Keyboard Focused* row be `surface.hover` **and** an outline rather
/// than one or the other, and what makes the destructive-plus-focused and
/// submenu-parent-plus-focused cells expressible at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RowState {
    /// The mutually-exclusive visual to paint.
    pub base: RowBase,
    /// Whether the keyboard focus outline is drawn on top of
    /// [`base`](Self::base).
    pub focused: bool,
}

impl RowState {
    /// §4.9.2 *Rest (Enabled)*.
    pub const REST: Self = Self {
        base: RowBase::Rest,
        focused: false,
    };

    /// §4.9.2 *Hovered*.
    pub const HOVERED: Self = Self {
        base: RowBase::Hovered,
        focused: false,
    };

    /// §4.9.2 *Pressed*.
    pub const PRESSED: Self = Self {
        base: RowBase::Pressed,
        focused: false,
    };

    /// §4.9.2 *Keyboard Focused* — the hover fill with the inset
    /// outline on.
    pub const KEYBOARD_FOCUSED: Self = Self {
        base: RowBase::Hovered,
        focused: true,
    };

    /// §4.9.2 *Active Submenu Parent*.
    pub const ACTIVE_SUBMENU_PARENT: Self = Self {
        base: RowBase::ActiveSubmenuParent,
        focused: false,
    };

    /// §4.9.2 *Destructive Row (Hovered)*.
    pub const DESTRUCTIVE_HOVERED: Self = Self {
        base: RowBase::DestructiveHovered,
        focused: false,
    };

    /// §4.9.2 *Destructive Row (Pressed)* — the destructive row held
    /// down, on the real `status.danger_fill_active` token.
    ///
    /// Filed in [`DERIVED_ROWS`](Self::DERIVED_ROWS) rather than
    /// [`SPEC_ROWS`](Self::SPEC_ROWS), which the 2026-10-22 revision made
    /// a stale label. See [`RowBase`] for why it is left that way.
    pub const DESTRUCTIVE_PRESSED: Self = Self {
        base: RowBase::DestructivePressed,
        focused: false,
    };

    /// A destructive row that is attended *and* holds the focus outline —
    /// G-11 trap 2, and the cell §4.9.2 prints as *Destructive Row
    /// (Focused)*.
    ///
    /// It paints `status.danger_fill` — the **hover** fill, not the
    /// pressed fill, exactly as §4.9.2 writes it — under a `1px` inset
    /// outline. That outline is `text.strong` not because this cell names
    /// it but because `accent.border` measures `1.7188:1` against the
    /// danger fill: the cell hands the fill it just resolved to the same
    /// adaptive rule every other focus indicator in the crate goes
    /// through, and the rule does the rest.
    pub const DESTRUCTIVE_FOCUSED: Self = Self {
        base: RowBase::DestructiveHovered,
        focused: true,
    };

    /// A submenu parent that is attended *and* holds the focus outline.
    pub const SUBMENU_PARENT_FOCUSED: Self = Self {
        base: RowBase::ActiveSubmenuParent,
        focused: true,
    };

    /// A row that is focused while resting, with no pointer on it.
    ///
    /// [`derive_row_state`] never produces this — keyboard attention
    /// makes a row *attended*, so a focused row arrives as
    /// [`KEYBOARD_FOCUSED`](Self::KEYBOARD_FOCUSED). The cell exists so
    /// a hand-built state cannot fall outside the resolver: §4.9.2 gives
    /// focus the hover fill, so this resolves to the same paint.
    pub const REST_FOCUSED: Self = Self {
        base: RowBase::Rest,
        focused: true,
    };

    /// §4.9.2 *Disabled / Placeholder*.
    pub const DISABLED: Self = Self {
        base: RowBase::Disabled,
        focused: false,
    };

    /// §4.9.2 *Section Heading*.
    pub const SECTION_HEADING: Self = Self {
        base: RowBase::SectionHeading,
        focused: false,
    };

    /// The eight rows §4.9.2 prints, in the order the spec lists them.
    pub const SPEC_ROWS: [Self; 8] = [
        Self::REST,
        Self::HOVERED,
        Self::PRESSED,
        Self::KEYBOARD_FOCUSED,
        Self::ACTIVE_SUBMENU_PARENT,
        Self::DESTRUCTIVE_HOVERED,
        Self::DISABLED,
        Self::SECTION_HEADING,
    ];

    /// The three cells a real menu reaches that §4.9.2 does not print.
    ///
    /// Each is still fully defined by [`resolve_row_style`]; listing them
    /// keeps them from being mistaken for spec rows and keeps the audit
    /// honest about which measurements are extrapolations.
    pub const DERIVED_ROWS: [Self; 3] = [
        Self::DESTRUCTIVE_PRESSED,
        Self::DESTRUCTIVE_FOCUSED,
        Self::SUBMENU_PARENT_FOCUSED,
    ];

    /// Every cell the resolver defines.
    pub const ALL: [Self; 12] = [
        Self::REST,
        Self::HOVERED,
        Self::PRESSED,
        Self::KEYBOARD_FOCUSED,
        Self::ACTIVE_SUBMENU_PARENT,
        Self::DESTRUCTIVE_HOVERED,
        Self::DESTRUCTIVE_PRESSED,
        Self::DESTRUCTIVE_FOCUSED,
        Self::SUBMENU_PARENT_FOCUSED,
        Self::REST_FOCUSED,
        Self::DISABLED,
        Self::SECTION_HEADING,
    ];

    /// The §4.9.2 row this cell answers to, in the spec's own words.
    ///
    /// Cells the matrix does not print say so, so an audit line reads as
    /// an extrapolation rather than a citation.
    #[must_use]
    pub const fn matrix_row(&self) -> &'static str {
        match (self.base, self.focused) {
            (RowBase::Rest, false) => "Rest (Enabled)",
            (RowBase::Rest, true) => "Keyboard Focused",
            (RowBase::Hovered, false) => "Hovered",
            (RowBase::Hovered, true) => "Keyboard Focused",
            (RowBase::Pressed, false) => "Pressed",
            (RowBase::Pressed, true) => "Pressed + outline (derived)",
            (RowBase::ActiveSubmenuParent, false) => "Active Submenu Parent",
            (RowBase::ActiveSubmenuParent, true) => "Active Submenu Parent + outline (derived)",
            (RowBase::DestructiveHovered, false) => "Destructive Row (Hovered)",
            (RowBase::DestructiveHovered, true) => "Destructive Row + outline (derived)",
            (RowBase::DestructivePressed, _) => "Destructive Row (Pressed) (derived)",
            (RowBase::Disabled, _) => "Disabled / Placeholder",
            (RowBase::SectionHeading, _) => "Section Heading",
        }
    }
}

// ---------------------------------------------------------------------------
// The resolved style
// ---------------------------------------------------------------------------

/// A row's fully resolved paint for one `(row, state)` pair.
///
/// Every field is a value the renderer consumes as-is. A
/// [`background`](Self::background) of [`Color::TRANSPARENT`] means
/// *nothing is painted behind the label* — the label then sits on whatever
/// the card is, which is exactly what gap **G-11** trap 3 requires of a
/// disabled row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowStyle {
    /// The fill behind the row's slots.
    pub background: Color,
    /// The 16px leading slot's glyph colour.
    pub leading: Color,
    /// The primary label's colour.
    pub label: Color,
    /// The trailing slot's colour.
    pub trailing: Color,
    /// The row's resolved keyboard focus outline — `None` when no outline
    /// is drawn.
    ///
    /// `1px`, [`FocusPlacement::Inset`], and coloured by the same
    /// adaptive rule that picks every outer ring in the crate. It is an
    /// `Option` rather than a transparent-colour sentinel because an
    /// unfocused row has no indicator at all: there is no stroke whose
    /// width and side are worth describing, and a caller cannot be handed
    /// a zero-width inset outline and asked to guess what it meant.
    pub focus_outline: Option<FocusIndicator>,
    /// The pointer cursor §4.9.2 fixes for this row.
    pub cursor: mouse::Interaction,
}

impl RowStyle {
    /// Whether nothing is painted behind the row.
    #[must_use]
    pub const fn is_transparent_background(&self) -> bool {
        self.background.a == 0.0
    }
}

/// The three colour channels a base state fixes, before the outline is added.
struct BasePaint {
    background: Color,
    leading: Color,
    label: Color,
    trailing: Color,
}

/// Resolve the §4.9.2 paint for one `(row, state)` pair.
///
/// A pure function: the same three inputs always produce the same
/// [`RowStyle`]. The base half of `state` selects the four colour
/// channels; the `focused` half adds the outline without touching them.
///
/// The outline is not read from the matrix. It is resolved against the
/// background this function just resolved, through
/// [`resolve_focus_indicator_for_background`] at
/// [`FocusPlacement::Inset`] — the same call, and the same colour rule,
/// a button's outer ring is made from. That is the only way trap 2 stays
/// defused: the matrix's printed `accent.border` measures `1.7188:1` on
/// a destructive fill, and the shared resolver returns `text.strong`
/// there without any caller having to know why. The placement is what
/// §3.0.2 changes for a row, and it changes the stroke and the edge and
/// nothing else.
///
/// `row` supplies the *slot semantics* — whether the leading slot carries
/// a check or a radio, which is what decides the Rest row's accent — and
/// never overrides `state`. A row that is not enabled still resolves to
/// [`RowBase::Disabled`] only because the caller said so through `state`;
/// the resolver does not second-guess it.
///
/// Each of the three painted channels is then put through
/// [`contrast::assert_fill_foreground_lock`] against the background this
/// function just resolved — the `UIF-TOK-006` rule 4 solid-fill
/// foreground lock. The two destructive bases paint the danger fills, and
/// trap 4 is exactly the case the lock is written against: a `✓` that
/// kept the Rest row's `accent.text` while the row went hot behind it.
/// Checking all three slots rather than only the label is what makes that
/// trap a checked invariant instead of a property of the current table.
#[must_use]
pub fn resolve_row_style(row: &Row, state: RowState, palette: &Palette) -> RowStyle {
    let base = base_paint(row, state, palette);

    contrast::assert_fill_foreground_lock(base.background, base.leading, palette);
    contrast::assert_fill_foreground_lock(base.background, base.label, palette);
    contrast::assert_fill_foreground_lock(base.background, base.trailing, palette);

    let focus_outline = if state.focused {
        Some(resolve_focus_indicator_for_background(
            base.background,
            palette,
            FocusPlacement::Inset,
        ))
    } else {
        None
    };

    RowStyle {
        background: base.background,
        leading: base.leading,
        label: base.label,
        trailing: base.trailing,
        focus_outline,
        cursor: cursor_for(row, state.base),
    }
}

/// Select the base-state channels for a row.
///
/// Split per base rather than matched as a `(base, slot)` cross product so
/// each row of §4.9.2 reads top to bottom in one place (`RUST-023`).
fn base_paint(row: &Row, state: RowState, palette: &Palette) -> BasePaint {
    match state.base {
        // §4.9.2 — Rest (Enabled). The leading slot is the only cell in
        // the matrix whose colour depends on what is *in* it: a `✓` or `•`
        // is an accent, an icon is quiet.
        RowBase::Rest => BasePaint {
            background: NO_FILL,
            leading: rest_leading(row, palette),
            label: palette.text_primary,
            trailing: palette.text_muted,
        },
        // §4.9.2 — Hovered.
        RowBase::Hovered => BasePaint {
            background: palette.surface_hover,
            leading: palette.text_strong,
            label: palette.text_strong,
            trailing: palette.text_strong,
        },
        // §4.9.2 — Pressed. The spec prints `RGB(50,50,58)`; the
        // retuned `surface.active` token is the authority (G-7).
        RowBase::Pressed => BasePaint {
            background: palette.surface_active,
            leading: palette.text_strong,
            label: palette.text_strong,
            trailing: palette.text_strong,
        },
        // §4.9.2 — Active Submenu Parent. The chevron keeps
        // `accent.text` here even though everything else went white,
        // because the chevron is the thing saying "there is more".
        RowBase::ActiveSubmenuParent => BasePaint {
            background: palette.surface_hover,
            leading: palette.text_strong,
            label: palette.text_strong,
            trailing: palette.accent_text,
        },
        // §4.9.2 — Destructive Row (Hovered), with trap 1 defused: the
        // FILL, not the text-role red. Every slot goes to `text.strong`,
        // which is also what defuses trap 4 — a `✓` cannot keep the Rest
        // row's `accent.text` here, because this cell has no accent.
        RowBase::DestructiveHovered => BasePaint {
            background: palette.status_danger_fill,
            leading: palette.text_strong,
            label: palette.text_strong,
            trailing: palette.text_strong,
        },
        // §4.9.2 — Destructive Row (Pressed). The real pressed token,
        // `status.danger.fill_active`, exactly as the 2026-10-22 revision
        // prints it. This cell used to paint `status.danger.fill` at 85%
        // opacity because no token existed to paint; that synthesis is
        // gone, and with it the measurement hazard it carried — a
        // translucent fill's contrast depends on what it is composited
        // over, so the same row had two defensible numbers depending on
        // whether the reader composited or not. Opaque settles it:
        // `text.strong` on `status.danger_fill_active` is 5.7543 here, as
        // it is on the button atom.
        RowBase::DestructivePressed => BasePaint {
            background: palette.status_danger_fill_active,
            leading: palette.text_strong,
            label: palette.text_strong,
            trailing: palette.text_strong,
        },
        // §4.9.2 — Disabled / Placeholder. Transparent is load-bearing
        // (trap 3): `text.disabled` fails against `surface.hover` and
        // `surface.active`, so a disabled row must not carry a fill that
        // a hovered ancestor might hand it.
        RowBase::Disabled => BasePaint {
            background: NO_FILL,
            leading: palette.text_disabled,
            label: palette.text_disabled,
            trailing: palette.text_disabled,
        },
        // §4.9.2 — Section Heading. No slots, so no slot colour; the
        // transparent channels make an accidental paint visible in the
        // audit rather than plausible.
        RowBase::SectionHeading => BasePaint {
            background: NO_FILL,
            leading: NO_FILL,
            label: palette.text_muted,
            trailing: NO_FILL,
        },
    }
}

/// The Rest row's leading-slot colour: the accent for a state glyph that is
/// actually drawn, quiet for an icon or an empty box.
///
/// §4.9.2 writes the column as "`accent.text` (`✓` / `•`) or `text.muted`" —
/// the accent belongs to the *glyph*, and an unchecked box draws none. A
/// colour behind nothing is still a colour the audit has to answer for, so
/// the resolver reports the one that matches what is painted.
fn rest_leading(row: &Row, palette: &Palette) -> Color {
    match row.leading_slot() {
        LeadingSlot::Check if row.is_checked() => palette.accent_text,
        LeadingSlot::Radio if row.is_selected() => palette.accent_text,
        LeadingSlot::Check | LeadingSlot::Radio | LeadingSlot::Icon | LeadingSlot::None => {
            palette.text_muted
        }
    }
}

/// The pointer cursor §4.9.2 fixes for a row.
///
/// `Pointer` for anything the user can act on, `None` (the default arrow)
/// for a disabled row and a heading. The matrix's parentheticals —
/// "absorbs click; does not close menu" and "non-interactive" — are
/// [`Row::activatable`]'s facts, not cursor facts. Both are consulted: the
/// base because the matrix keys the column by row state, the row because a
/// cursor must never point at a control that cannot answer.
const fn cursor_for(row: &Row, base: RowBase) -> mouse::Interaction {
    let live = !matches!(base, RowBase::Disabled | RowBase::SectionHeading);

    if row.activatable() && live {
        mouse::Interaction::Pointer
    } else {
        mouse::Interaction::None
    }
}

/// Fold a resolved [`VisualState`] into the [`RowState`] a menu row paints.
///
/// The row's own facts decide three of the matrix's rows that no generic
/// widget state can express: a heading, a disabled row, and a submenu
/// parent whose child is currently open. Everything else is the generic
/// base, with destructiveness folded in wherever the row would otherwise
/// paint a non-danger fill behind a danger label.
///
/// Precedence, highest first:
///
/// 1. **Not [`interactive`](Row::interactive)** → [`RowBase::SectionHeading`]. A heading or a separator is not a control and never takes the hover fill, whatever the pointer does.
/// 2. **Not [`activatable`](Row::activatable)** → [`RowBase::Disabled`]. A greyed-out action keeps the transparent background trap 3 demands.
/// 3. **Pressed** → [`RowBase::Pressed`], or [`RowBase::DestructivePressed`] on a destructive row. Committed outranks everything the pointer or the keyboard said before it.
/// 4. **Attended and destructive** → [`RowBase::DestructiveHovered`]. *Attended* means hovered or keyboard-focused: G-11 trap 2 only exists because a focused destructive row shows the danger fill, so focus has to count as attention here.
/// 5. **Attended submenu parent with its child open** → [`RowBase::ActiveSubmenuParent`].
/// 6. **Attended** → [`RowBase::Hovered`].
/// 7. **Otherwise** → [`RowBase::Rest`].
///
/// Note what steps 4 and 6 do to the keyboard: a focused row is *attended*,
/// so a focused plain row arrives at [`RowBase::Hovered`] with `focused: true` —
/// §4.9.2's *Keyboard Focused* cell — and a focused row is never [`RowBase::Rest`].
#[must_use]
pub fn derive_row_state(row: &Row, visual: VisualState, submenu_open: bool) -> RowState {
    if !row.interactive() {
        return RowState::SECTION_HEADING;
    }

    if !row.activatable() {
        return RowState::DISABLED;
    }

    let attended = visual.base == BaseState::Hovered || visual.focused;

    let base = if visual.base == BaseState::Pressed {
        if row.is_destructive() {
            RowBase::DestructivePressed
        } else {
            RowBase::Pressed
        }
    } else if !attended {
        RowBase::Rest
    } else if row.is_destructive() {
        RowBase::DestructiveHovered
    } else if submenu_open && row.is_submenu_parent() {
        RowBase::ActiveSubmenuParent
    } else {
        RowBase::Hovered
    };

    RowState {
        base,
        focused: visual.focused,
    }
}

// ---------------------------------------------------------------------------
// Paint layers
// ---------------------------------------------------------------------------

/// The row's fill, if it paints one.
///
/// `None` for the three matrix rows that specify `transparent`, which is
/// how a disabled row is kept from carrying a hovered ancestor's fill
/// (trap 3): there is no layer at all, so there is nothing to inherit.
#[must_use]
pub fn background_layer(bounds: Rectangle, style: &RowStyle) -> Option<Layer> {
    if style.is_transparent_background() {
        return None;
    }

    Some(Layer {
        bounds,
        fill: style.background,
        border: Border::default()
            .color(NO_FILL)
            .width(NO_STROKE)
            .rounded(ROW_RADIUS),
    })
}

/// The row's inset focus outline, if one is due (§3.0.2 rule 2, §4.9.2).
///
/// The inverse of the concentric annulus
/// [`button_widget`](crate::widget::button_widget) draws. iced strokes a
/// quad inward from its bounds, so a ring needs a quad *grown* by its
/// width to land outside the control, and an inset outline needs the
/// row's *own* quad to land its `1px` stroke just inside the row's
/// perimeter. Nothing spills into the rows packed above and below it,
/// which is the entire reason §3.0.2 wants a row's indicator inset
/// rather than ringed.
///
/// None of that geometry is decided here. [`FocusIndicator::bounds`] and
/// [`FocusIndicator::radius`] own it, so a row cannot end up drawing its
/// outline the way a button draws its ring.
#[must_use]
pub fn focus_outline_layer(bounds: Rectangle, style: &RowStyle) -> Option<Layer> {
    let outline = style.focus_outline?;

    Some(Layer {
        bounds: outline.bounds(bounds),
        fill: NO_FILL,
        border: Border::default()
            .color(outline.color)
            .width(outline.width)
            .rounded(outline.radius(ROW_RADIUS)),
    })
}

/// The 1px `border.subtle` rule of a separator row (§4.9.1), centred in
/// the row's 9px band.
///
/// The rule is deliberately quiet — 1.2017 against `surface.overlay`,
/// far below the chrome floor. That is not a violation: `UIF-TOK-006`
/// rule 2 governs *active* indicators, and a resting divider that
/// shouted would break the grouping it exists to create.
#[must_use]
pub fn separator_layer(bounds: Rectangle, palette: &Palette) -> Layer {
    let y = bounds.y + (bounds.height - SEPARATOR_THICKNESS) / 2.0;

    Layer {
        bounds: Rectangle::new(
            Point::new(bounds.x, y),
            Size::new(bounds.width, SEPARATOR_THICKNESS),
        ),
        fill: palette.border_subtle,
        border: Border::default()
            .color(NO_FILL)
            .width(NO_STROKE)
            .rounded(0.0),
    }
}

/// Every layer a row paints, in draw order.
///
/// Fill, then inset outline, then nothing else — the slots are drawn as
/// child widgets on top. A separator paints its rule instead of a fill,
/// and a row that is both a separator and focused is not expressible (a
/// separator is never focusable), so the two branches never meet.
#[must_use]
pub fn paint_layers(
    row: &Row,
    bounds: Rectangle,
    style: &RowStyle,
    palette: &Palette,
) -> Vec<Layer> {
    let mut layers = Vec::with_capacity(2);

    if row.is_separator() {
        layers.push(separator_layer(bounds, palette));
    } else if let Some(layer) = background_layer(bounds, style) {
        layers.push(layer);
    }

    if let Some(layer) = focus_outline_layer(bounds, style) {
        layers.push(layer);
    }

    layers
}

/// The row height §4.8.1 fixes for a kind.
#[must_use]
pub fn row_height(row: &Row) -> f32 {
    match row.kind() {
        RowKind::Heading => HEADING_HEIGHT,
        RowKind::Separator => SEPARATOR_HEIGHT,
        _ => ROW_HEIGHT,
    }
}

/// The vertical padding inside a row, above and below its slots.
///
/// Zero for a normal row — the 32px band centres its content — and
/// `space.xs` for a heading, whose `[4px, 10px]` padding is the only
/// one §4.8.1 states as a pair.
#[must_use]
pub fn row_padding_y(row: &Row) -> f32 {
    match row.kind() {
        RowKind::Heading => HEADING_PADDING_Y,
        _ => NO_SPACE,
    }
}

// ---------------------------------------------------------------------------
// What the row reports
// ---------------------------------------------------------------------------

/// What a [`MenuRow`] tells its host about a click or a pointer crossing.
///
/// The click half is exactly `UIF-CMENU-002`'s: an *activated* row wants
/// the host to run [`Menu::activate`](super::model::Menu::activate) and
/// then honour the row's close policy; an *absorbed* click must leave the
/// menu open and change nothing. Collapsing the two into "the row was
/// clicked" would force the host to re-read the model to find out whether
/// the menu should close, which is the fact the row already has.
///
/// The pointer half — [`Hovered`](Self::Hovered) and
/// [`Unhovered`](Self::Unhovered) — exists for the submenu aiming
/// corridor (`UIF-PRIN-008`, `UIF-FMENU-002`), which cannot be driven
/// from a click at all: the whole point of the 150 ms grace window is
/// that a pointer crossing a sibling row is *not* a decision.
///
/// # Edges, not levels
///
/// [`iced::advanced::widget::Widget::update`] runs for
/// every live widget on every event, so a level-triggered report would
/// publish for as long as the pointer sat still — re-solving the host's
/// state each frame for a fact that has not changed, and pushing any
/// timer keyed off it forward forever. Both variants are therefore
/// **edge-triggered**: published once on the crossing, never again until
/// the pointer leaves and comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowEvent {
    /// An activatable row was pressed and released inside its bounds.
    Activated,
    /// A disabled row, heading or separator took the click so the
    /// click-away layer cannot dismiss the menu (`UIF-CMENU-002`).
    Absorbed,
    /// The pointer crossed into the row's bounds (`false` → `true`).
    ///
    /// Reported for **every** row kind — action, submenu parent, heading,
    /// separator, disabled. The widget reports raw observations and the
    /// host judges them: withholding hover from a non-submenu sibling
    /// would leave the corridor blind to exactly the crossings it exists
    /// to tolerate, and a row that disagreed with the cursor on purpose
    /// is worse than a row that says nothing.
    ///
    /// A row that is *removed* from the tree while hovered never reports
    /// the matching [`Unhovered`](Self::Unhovered) — there is no widget
    /// left to report it. A host that keeps state keyed on hover must
    /// clear it when the surface closes, not wait for this event.
    Hovered,
    /// The pointer crossed out of the row's bounds (`true` → `false`).
    ///
    /// The counterpart of [`Hovered`](Self::Hovered), and equally
    /// kind-agnostic. It says the pointer left **this row** — nothing
    /// about whether it left the menu, and a host must not read it as a
    /// dismissal cue.
    Unhovered,
}

// ---------------------------------------------------------------------------
// The row's slots
// ---------------------------------------------------------------------------

/// Which slot of the §4.8.1 grid a child fills.
///
/// The draw pass needs this because §4.9.2 fixes a *different* colour per
/// slot — `accent.text` leading against `text.muted` trailing on the same
/// Rest row — and iced hands one `text_color` down a subtree. The slot
/// identity is what lets the row override each child with its own
/// resolved colour instead of letting the parent's style leak through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotKind {
    /// The 16×16 leading box.
    Leading,
    /// The fill-width primary label.
    Label,
    /// The shrink-to-fit trailing slot.
    Trailing,
    /// A host-supplied body, drawn with whatever style it was given.
    Custom,
}

/// The row's children paired with the slot each one fills.
///
/// Two parallel vectors rather than a vector of pairs so
/// [`Tree::diff_children`] can be handed the elements slice directly —
/// it wants `&[impl Borrow<dyn Widget>]`, and a `Vec<Slot>` cannot
/// borrow as one. The two are built together by [`new`](MenuRow::new) /
/// [`with_body`](MenuRow::with_body) and never mutated apart; nothing
/// here exposes a way to change one without the other.
struct RowSlots<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    kinds: Vec<SlotKind>,
    elements: Vec<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> RowSlots<'a, Message, Theme, Renderer> {
    /// Build the parallel vectors from `(slot, element)` pairs.
    fn new(entries: Vec<(SlotKind, Element<'a, Message, Theme, Renderer>)>) -> Self {
        let (kinds, elements): (Vec<SlotKind>, Vec<Element<'a, Message, Theme, Renderer>>) =
            entries.into_iter().unzip();

        Self { kinds, elements }
    }

    fn kinds(&self) -> &[SlotKind] {
        &self.kinds
    }

    fn elements(&self) -> &[Element<'a, Message, Theme, Renderer>] {
        &self.elements
    }

    fn elements_mut(&mut self) -> &mut [Element<'a, Message, Theme, Renderer>] {
        &mut self.elements
    }

    /// The index of `kind`, if a child fills it.
    fn index_of(&self, kind: SlotKind) -> Option<usize> {
        self.kinds.iter().position(|slot| *slot == kind)
    }

    /// The slot a child index fills.
    fn kind_at(&self, index: usize) -> Option<SlotKind> {
        self.kinds.get(index).copied()
    }
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

/// What the row has observed about itself, before any judgement.
///
/// Every field is a raw signal from a different source — the pointer, the
/// focus router — and none of them is reconciled here.
/// ``MenuRow::row_state`` hands them to [`state::resolve`] and
/// [`derive_row_state`], which own the precedence. That is the same
/// contract [`button_widget::ButtonState`](crate::widget::button_widget::ButtonState)
/// is built on, and it is why the 32-row table in `state_tests.rs` covers
/// what this widget paints.
///
/// `captured` is the row's own addition. A disabled row, a heading and a
/// separator must absorb a click they are not allowed to act on, so the
/// row needs to remember it took the press even though it never became
/// ``pressed``. Without that half, releasing over a
/// heading would fall through to the click-away layer and dismiss the
/// menu — the exact behaviour `UIF-CMENU-002` forbids.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowTreeState {
    focusable: FocusableState,
    hovered: bool,
    pressed: bool,
    captured: bool,
}

impl RowTreeState {
    /// A resting, unfocused, unpressed row state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focusable: FocusableState::new(),
            hovered: false,
            pressed: false,
            captured: false,
        }
    }

    /// The focus state the router drives.
    #[must_use]
    pub const fn focusable(&self) -> &FocusableState {
        &self.focusable
    }

    /// Whether the pointer is inside the row's bounds.
    #[must_use]
    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Whether a button is currently held down on this row.
    #[must_use]
    pub const fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Whether this row took the current press and owes a release decision.
    #[must_use]
    pub const fn is_captured(&self) -> bool {
        self.captured
    }
}

// ---------------------------------------------------------------------------
// Text construction
// ---------------------------------------------------------------------------

/// Text on the row's primary-label rung — 13px Regular (`type.ui_compact`).
fn label_text<'a, Message, Theme, Renderer>(
    content: String,
) -> Element<'a, Message, Theme, Renderer>
where
    Theme: iced::advanced::widget::text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    text(content)
        .size(typography::UI_COMPACT.size)
        .line_height(typography::UI_COMPACT.iced_line_height())
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

/// Text on the row's trailing rung — 12px (`type.caption`).
fn trailing_text<'a, Message, Theme, Renderer>(
    content: String,
) -> Element<'a, Message, Theme, Renderer>
where
    Theme: iced::advanced::widget::text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    text(content)
        .size(TRAILING_TYPE)
        .line_height(typography::CAPTION.iced_line_height())
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

/// The section-heading rung: `type.caption` on the sans family at
/// SemiBold — §4.8.1's "12px SemiBold".
///
/// **Legal by token.** `type.caption`'s `Allowed Weights` column is
/// Regular / Medium / SemiBold, so [`typography::Weight::SemiBold`] is a
/// member of the set [`CAPTION`](typography::CAPTION) permits. Size,
/// line height and font all come from the scale, and
/// [`TypeRung::font`](typography::TypeRung::font) asserts the rung
/// permits the weight it is asked to carry. What G-12 recorded as a
/// hand-composed pairing — size read from the token, weight bolted on —
/// is now a rung-carried composition, and the `iced::Font` literal this
/// function used to assemble is gone with it.
fn heading_text<'a, Message, Theme, Renderer>(
    content: String,
) -> Element<'a, Message, Theme, Renderer>
where
    Theme: iced::advanced::widget::text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    text(content)
        .size(TRAILING_TYPE)
        .line_height(typography::CAPTION.iced_line_height())
        .font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..iced::Font::with_name("Fira Sans")
        })
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

/// A glyph filling the 16px leading box.
fn leading_glyph<'a, Message, Theme, Renderer>(
    glyph: String,
) -> Element<'a, Message, Theme, Renderer>
where
    Theme: iced::advanced::widget::text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    text(glyph)
        .size(LEADING_SLOT)
        .line_height(LEADING_LINE_HEIGHT)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

/// An empty leading box: reserved space with nothing drawn.
///
/// Present so an unchecked row's label sits at the same x as a checked
/// one's. See the [module docs](self) for why this is a
/// `UIF-PRIN-003` requirement rather than a detail.
fn empty_leading<'a, Message, Theme, Renderer>() -> Element<'a, Message, Theme, Renderer>
where
    Theme: iced::advanced::widget::text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    text(String::new())
        .size(LEADING_SLOT)
        .line_height(LEADING_LINE_HEIGHT)
        .into()
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

/// A themed, keyboard-focusable menu row on the §4.8.1 shared grid.
///
/// The row owns its three slots and nothing else: no card, no cascade, no
/// dismissal. [`Row`] supplies what the slots hold, [`state::resolve`]
/// plus [`derive_row_state`] decide which matrix cell is being painted,
/// and [`resolve_row_style`] turns that cell into the four colours and
/// the inset outline the draw pass lays down.
///
/// The row is keyboard-focusable under a [`FocusId`] and is *absent from
/// the focus router* when it is not navigable, so "greyed out", "not a
/// control" and "cannot be tabbed to" are one fact rather than three
/// that can drift. That mirrors [`focusable::Focusable`](crate::widget::focusable::Focusable)
/// and `Button` exactly: the router has no notion of a disabled widget,
/// so gating happens by not registering.
///
/// # Every colour is re-resolved every frame
///
/// [`draw`](Widget::draw) does not cache a style. It folds the tree state
/// through the resolver and paints the answer, which is the only way the
/// destructive-outline rule (trap 2) can hold: the outline is resolved
/// against the fill of *this* frame, so a row that becomes destructive
/// between frames cannot keep an indicator that was visible against the
/// fill it no longer has.
pub struct MenuRow<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    row: Row,
    slots: RowSlots<'a, Message, Theme, Renderer>,
    on_event: Box<dyn Fn(RowEvent) -> Message + 'a>,
    focus_id: FocusId,
    widget_id: Id,
    palette: Palette,
    submenu_open: bool,
}

impl<'a, Message, Theme, Renderer> MenuRow<'a, Message, Theme, Renderer> {
    /// Build a row whose body is `body` instead of the model's slots.
    ///
    /// The body is drawn with the text style the parent passes down, not
    /// the resolved slot colours — it is opaque to the grid. This is the
    /// seam a host uses for a row whose content the three-slot model
    /// cannot express, and the seam the tests use to drive the widget with
    /// content that needs no text renderer.
    ///
    /// The row's own paint — fill, inset outline, separator rule — is
    /// unaffected:
    /// it comes from the resolver either way.
    #[must_use]
    pub fn with_body(
        row: Row,
        body: impl Into<Element<'a, Message, Theme, Renderer>>,
        focus_id: impl Into<FocusId>,
        on_event: impl Fn(RowEvent) -> Message + 'a,
    ) -> Self {
        let slots = RowSlots::new(vec![(SlotKind::Custom, body.into())]);

        Self::assemble(row, slots, focus_id, on_event)
    }

    /// The shared tail of both constructors.
    fn assemble(
        row: Row,
        slots: RowSlots<'a, Message, Theme, Renderer>,
        focus_id: impl Into<FocusId>,
        on_event: impl Fn(RowEvent) -> Message + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = crate::bridge::widget_id(&focus_id);

        Self {
            row,
            slots,
            on_event: Box::new(on_event),
            focus_id,
            widget_id,
            palette: Palette::dark(),
            submenu_open: false,
        }
    }

    // -- builders ---------------------------------------------------------

    /// Paint from a palette other than [`Palette::dark`].
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Tell the row whether its submenu child is currently open.
    ///
    /// This is the seam `INC-16`'s cascade pushes through. The row cannot
    /// know: [`RowKind::Submenu`] carries the child's *structure*, and
    /// openness is cascade state the model deliberately does not hold.
    /// Until the cascade says otherwise the row paints as any other.
    #[must_use]
    pub fn submenu_open(mut self, open: bool) -> Self {
        self.submenu_open = open;
        self
    }

    // -- accessors --------------------------------------------------------

    /// The model row this widget renders.
    #[must_use]
    pub fn row(&self) -> &Row {
        &self.row
    }

    /// The framework-level identity this row answers to.
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`Id`] this row registers with the focus router.
    #[must_use]
    pub fn widget_id(&self) -> &Id {
        &self.widget_id
    }

    /// Whether this row's submenu child is currently open.
    #[must_use]
    pub fn is_submenu_open(&self) -> bool {
        self.submenu_open
    }

    /// The slot kinds this row laid out, in grid order.
    #[must_use]
    pub fn slot_kinds(&self) -> &[SlotKind] {
        self.slots.kinds()
    }

    // -- state ------------------------------------------------------------

    /// The matrix cell this row paints, from the tree's own observations.
    fn row_state(&self, state: &RowTreeState) -> RowState {
        self.row_state_hovered(state, state.hovered)
    }

    /// The matrix cell for the tree's observations with `hovered`
    /// overridden.
    ///
    /// The override exists because [`Widget::mouse_interaction`] is asked
    /// what the cursor *would* be right now, from a tree whose
    /// [`hovered`](RowTreeState::hovered) flag was written by the last
    /// event pass and may not match the cursor it is being handed.
    fn row_state_hovered(&self, state: &RowTreeState, hovered: bool) -> RowState {
        let visual = state::resolve(InteractionInput {
            enabled: self.row.activatable(),
            hovered,
            pressed: state.pressed,
            focused: state.focusable.is_focused(),
            invalid: false,
        });

        derive_row_state(&self.row, visual, self.submenu_open)
    }

    /// The paint for the tree's current cell.
    #[must_use]
    pub fn current_style(&self, state: &RowTreeState) -> RowStyle {
        resolve_row_style(&self.row, self.row_state(state), &self.palette)
    }
}
// ---------------------------------------------------------------------------
// Text construction (the only half that needs a text renderer)
// ---------------------------------------------------------------------------

impl<'a, Message, Theme, Renderer> MenuRow<'a, Message, Theme, Renderer>
where
    Theme: iced::advanced::widget::text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    /// Build a row for `row` that publishes `on_event` when it is
    /// clicked.
    ///
    /// `focus_id` is the identity the focus router knows this row by —
    /// the same [FocusId] the host registered in
    /// [FocusState](crate::focus::FocusState), via
    /// [Menu::row_focus_id](super::model::Menu::row_focus_id), so a
    /// model transition and a click address the same control.
    ///
    /// The slots are built from the model's own
    /// [leading_slot](Row::leading_slot) /
    /// [`trailing_slot`](Row::trailing_slot) answers. The leading box is
    /// emitted for every interactive row even when the slot is
    /// [None](LeadingSlot::None), because the grid's alignment is a
    /// property of the card, not of the row.
    ///
    /// This is the only constructor that needs a text renderer.
    /// [with_body](Self::with_body) and the whole
    /// [the iced `Widget` trait](iced::advanced::widget::Widget) implementation run on
    /// the base renderer protocol alone.
    #[must_use]
    pub fn new(
        row: Row,
        focus_id: impl Into<FocusId>,
        on_event: impl Fn(RowEvent) -> Message + 'a,
    ) -> Self {
        let slots = RowSlots::new(Self::build_slots(&row));

        Self::assemble(row, slots, focus_id, on_event)
    }

    /// The `(slot, element)` pairs the §4.8.1 grid calls for.
    fn build_slots(row: &Row) -> Vec<(SlotKind, Element<'a, Message, Theme, Renderer>)> {
        match row.kind() {
            // §4.8.1 gives a heading no leading box and no trailing slot:
            // the label is the whole row.
            RowKind::Heading => {
                vec![(SlotKind::Label, heading_text(row.label().to_owned()))]
            }
            // A separator is a rule, not a row of content.
            RowKind::Separator => Vec::new(),
            _ => {
                let mut entries = Vec::with_capacity(3);

                entries.push((SlotKind::Leading, Self::leading_for(row)));
                entries.push((SlotKind::Label, label_text(row.label().to_owned())));

                if let Some(trailing) = Self::trailing_for(row) {
                    entries.push((SlotKind::Trailing, trailing));
                }

                entries
            }
        }
    }

    /// The leading box's content for `row`.
    ///
    /// The box is always present for an interactive row; only its glyph
    /// changes. An unchecked `Check`, an unselected `Radio` and a `None`
    /// slot all get the empty box, which is what keeps the label column
    /// aligned across the card.
    fn leading_for(row: &Row) -> Element<'a, Message, Theme, Renderer> {
        let glyph = match row.leading_slot() {
            LeadingSlot::Check if row.is_checked() => Some(GLYPH_CHECK.to_owned()),
            LeadingSlot::Radio if row.is_selected() => Some(GLYPH_RADIO.to_owned()),
            LeadingSlot::Icon => row.icon().map(str::to_owned),
            LeadingSlot::Check | LeadingSlot::Radio | LeadingSlot::None => None,
        };

        match glyph {
            Some(glyph) => leading_glyph(glyph),
            None => empty_leading(),
        }
    }

    /// The trailing slot's content for `row`, if it has one.
    ///
    /// Unlike the leading box this is genuinely optional: §4.8.1 sizes it
    /// `Shrink`, so an absent trailing slot costs nothing and shifts
    /// nothing.
    fn trailing_for(row: &Row) -> Option<Element<'a, Message, Theme, Renderer>> {
        match row.trailing_slot() {
            TrailingSlot::Chevron => Some(trailing_text(GLYPH_CHEVRON.to_owned())),
            TrailingSlot::Shortcut => row.shortcut().map(|hint| trailing_text(hint.to_owned())),
            TrailingSlot::None => None,
        }
    }
}

/// Centre a child of `child_height` inside a band of `band_height`.
fn centre_y(band_height: f32, child_height: f32) -> f32 {
    ((band_height - child_height) / 2.0).max(NO_SPACE)
}

/// The layout node slot `index` was given, falling back to the row's own.
///
/// [`MenuRow::layout`] builds one child node per slot and [`children`]
/// builds one [`Tree`] per slot, so for every row this widget assembles
/// the two counts agree and `nth(index)` always lands. The fallback
/// covers a row that somehow disagrees — a host that rebuilt part of the
/// tree, or a future slot kind the grid does not place yet.
///
/// Falling back rather than skipping is deliberate. Skipping would take
/// the child's focus registration, its published boxes and its events
/// away with it: a control that silently stops existing to the router is
/// a worse failure than one reporting a box that is too large, and it
/// is exactly the state the row was in before per-slot layouts were
/// threaded through at all. A mismatch is therefore degraded, never
/// dropped.
///
/// [`children`]: iced::advanced::widget::Widget::children
fn slot_layout<'a>(layout: Layout<'a>, index: usize) -> Layout<'a> {
    layout.children().nth(index).unwrap_or(layout)
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MenuRow<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<RowTreeState>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(RowTreeState::new())
    }

    fn children(&self) -> Vec<Tree> {
        self.slots.elements().iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(self.slots.elements());
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(row_height(&self.row)))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let height = row_height(&self.row);
        let width = limits.max().width;
        let inner = (width - ROW_PADDING_X * 2.0).max(NO_SPACE);
        let content_height = (height - row_padding_y(&self.row) * 2.0).max(NO_SPACE);

        // A host-supplied body is not a grid: it fills the padded inner
        // box and keeps whatever size it asked for.
        if self.slots.kinds() == [SlotKind::Custom] {
            let node = self.slots.elements_mut()[0].as_widget_mut().layout(
                &mut tree.children[0],
                renderer,
                &layout::Limits::NONE
                    .loose()
                    .max_width(inner)
                    .max_height(content_height),
            );

            let y = centre_y(height, node.size().height);

            return layout::Node::with_children(
                Size::new(width, height),
                vec![node.move_to((ROW_PADDING_X, y))],
            );
        }

        // Measure in the order the grid needs, not the order the slots
        // are declared in: the leading box is fixed and the trailing slot
        // shrinks to its content, and only once both are known can the
        // label be handed exactly what is left. Measuring left to right
        // would give the label the whole width and push the trailing slot
        // off the right edge.
        let leading = self.slots.index_of(SlotKind::Leading).map(|index| {
            self.slots.elements_mut()[index].as_widget_mut().layout(
                &mut tree.children[index],
                renderer,
                &layout::Limits::NONE
                    .width(LEADING_SLOT)
                    .height(LEADING_SLOT),
            )
        });

        let trailing = self.slots.index_of(SlotKind::Trailing).map(|index| {
            self.slots.elements_mut()[index].as_widget_mut().layout(
                &mut tree.children[index],
                renderer,
                &layout::Limits::NONE.loose().max_height(content_height),
            )
        });

        let used = if leading.is_some() {
            LEADING_SLOT + ROW_GAP
        } else {
            NO_SPACE
        } + trailing
            .as_ref()
            .map(|node| node.size().width + ROW_GAP)
            .unwrap_or(NO_SPACE);

        let label_width = (inner - used).max(NO_SPACE);
        let y = centre_y(height, content_height);

        let mut nodes = Vec::with_capacity(3);
        let mut x = ROW_PADDING_X;

        if let Some(node) = leading {
            nodes.push(node.move_to((x, y)));
            x += LEADING_SLOT + ROW_GAP;
        }

        if let Some(index) = self.slots.index_of(SlotKind::Label) {
            let node = self.slots.elements_mut()[index].as_widget_mut().layout(
                &mut tree.children[index],
                renderer,
                &layout::Limits::NONE
                    .width(label_width)
                    .height(content_height),
            );

            nodes.push(node.move_to((x, y)));
        }

        if let Some(node) = trailing {
            // Right-aligned against the row's own padding, not stacked
            // after the label: the trailing column is what the eye
            // scans shortcuts down, and it has to line up card-wide.
            let trailing_x = (width - ROW_PADDING_X - node.size().width).max(x);

            nodes.push(node.move_to((trailing_x, y)));
        }

        layout::Node::with_children(Size::new(width, height), nodes)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<RowTreeState>();

        if self.row.navigable() {
            operation.focusable(Some(&self.widget_id), layout.bounds(), &mut state.focusable);
        } else {
            // Unregistered, so the router cannot take this focus back —
            // it cannot see the widget at all. Drop it here rather than
            // leave a heading or a dead row holding a focus nothing can
            // traverse to. Reached through the trait because
            // `FocusableState` has no inherent `unfocus`: the router's
            // protocol is the only way in and out of focus.
            <FocusableState as iced::advanced::widget::operation::Focusable>::unfocus(
                &mut state.focusable,
            );
        }

        let label_slot = self.slots.index_of(SlotKind::Label);

        // The caption is published here, under the row's derived id, and
        // is deliberately left out of the traversal below. Its own
        // `Text` widget can only report itself anonymously, so publishing
        // it here is the only way to name it — and doing so reports the
        // box the §4.8.1 grid actually gave the label, the same node
        // `draw` paints it into (`UIF-CMENU-*`).
        if let Some(index) = label_slot
            && let Some(child) = layout.children().nth(index)
        {
            publish_text(
                operation,
                &self.widget_id,
                role::LABEL,
                child.bounds(),
                self.row.label(),
            );
        }

        operation.traverse(&mut |operation| {
            for (index, element) in self.slots.elements_mut().iter_mut().enumerate() {
                if Some(index) == label_slot {
                    continue;
                }
                element.as_widget_mut().operate(
                    &mut tree.children[index],
                    slot_layout(layout, index),
                    renderer,
                    operation,
                );
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for (index, element) in self.slots.elements_mut().iter_mut().enumerate() {
            element.as_widget_mut().update(
                &mut tree.children[index],
                event,
                slot_layout(layout, index),
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }

        if shell.is_event_captured() {
            return;
        }

        let is_over = cursor.is_over(layout.bounds());
        let state = tree.state.downcast_mut::<RowTreeState>();

        // Where the pointer is, is recorded whether or not this row may
        // act on it. The widget reports raw observations and `resolve`
        // judges them; a row that stopped recording hover would be
        // silently disagreeing with the cursor.
        //
        // The prior value is read **before** the write because the hover
        // report is an edge, not a level: `Hovered` belongs to the
        // `false` → `true` crossing and `Unhovered` to `true` → `false`,
        // and each needs to know what the row believed before this event
        // contradicted it. Reporting the level instead would publish on
        // every frame the pointer sat still.
        let was_hovered = state.hovered;
        state.hovered = is_over;

        if is_over != was_hovered {
            let crossing = if is_over {
                RowEvent::Hovered
            } else {
                RowEvent::Unhovered
            };

            shell.publish((self.on_event)(crossing));
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if is_over => {
                // Only an activatable row takes the *pressed skin*; any
                // row inside the card takes the *press*, because a
                // heading that let the press through would let the
                // click-away layer dismiss the menu (UIF-CMENU-002).
                state.pressed = self.row.activatable();
                state.captured = true;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.captured => {
                // The press is released either way, so the pressed skin
                // cannot stick. Only a release still inside the row
                // reports anything at all.
                state.pressed = false;
                state.captured = false;

                if is_over {
                    let reported = if self.row.activatable() {
                        RowEvent::Activated
                    } else {
                        RowEvent::Absorbed
                    };

                    shell.publish((self.on_event)(reported));
                }

                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }

        // The cursor is a column of §4.9.2 like any other, so it comes
        // from the resolver rather than from a second rule here. The
        // `hovered` override is what makes that true: this is asked with
        // the live cursor, before the tree has been told about it.
        let state = tree.state.downcast_ref::<RowTreeState>();
        let cell = self.row_state_hovered(state, true);

        resolve_row_style(&self.row, cell, &self.palette).cursor
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<RowTreeState>();
        let paint = self.current_style(state);
        let bounds = layout.bounds();

        for layer in paint_layers(&self.row, bounds, &paint, &self.palette) {
            renderer.fill_quad(layer.quad(), Background::Color(layer.fill));
        }

        // Each slot inherits the colour this cell resolved for *it*, not
        // whatever text style the parent was passing down. §4.9.2 fixes a
        // different colour per slot — `accent.text` leading against
        // `text.muted` trailing on the same Rest row — and one inherited
        // `text_color` down the subtree cannot express that.
        for (index, child) in layout.children().enumerate() {
            let slot_color = match self.slots.kind_at(index) {
                Some(SlotKind::Leading) => paint.leading,
                Some(SlotKind::Label) => paint.label,
                Some(SlotKind::Trailing) => paint.trailing,
                Some(SlotKind::Custom) | None => style.text_color,
            };

            self.slots.elements()[index].as_widget().draw(
                &tree.children[index],
                renderer,
                theme,
                &renderer::Style {
                    text_color: slot_color,
                },
                child,
                cursor,
                viewport,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<MenuRow<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer,
{
    fn from(row: MenuRow<'a, Message, Theme, Renderer>) -> Element<'a, Message, Theme, Renderer> {
        Element::new(row)
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod view_tests;
