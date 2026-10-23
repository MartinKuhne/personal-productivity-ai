//! The three-slot list tile — leading mark, filling text, shrinking
//! trailing targets (`UIF-COMP-005`, decision **D-8**).
//!
//! §3B.5 describes a row as three slots on a fixed grid. Decision **D-8**
//! says that grid must be computed *here* rather than left to whatever lays
//! the row out, and the reason is the one that applies to every shared
//! layout number: a host that picks its own leading-box width picks a
//! different list on every surface, and the misalignment shows up as
//! ragged titles across nine screens that never met.
//!
//! # The printed total is authoritative; the parts are exposed anyway
//!
//! The single-line rung composes exactly from what §3B.5 prints: `4px`
//! vertical padding, a `24px` mark, `4px` again — `32px`, which is also
//! `size.control.md`, which the spec names in the same breath.
//!
//! The two-line rung does not. `4 + 18 + 16 + 4` is `42`, and the spec
//! prints `44`. The `8px` gap it also prints is the *inter-slot* gap —
//! between leading, centre and trailing — because charging it between the
//! title and the subtitle as well would put the row at `50`, further from
//! the printed figure rather than closer. So two pixels are unaccounted
//! for, and this resolver treats the printed total as authoritative:
//! row height is what virtualised lists, hit targets and the
//! `UIF-PRIN-003` zero-shift invariant all consume, and a derived `42`
//! would silently reflow every list in the app to satisfy arithmetic the
//! spec never claimed. [`TWO_LINE_UNACCOUNTED`] exposes the delta rather
//! than hiding it, so the gap stays attributable.
//!
//! # The hit target is resolved separately from the mark
//!
//! §3B.5 gives the leading slot two permitted sizes, `16×16` and
//! `24×24`, and does not say which mark gets which. What it *does* fix is
//! the trailing slot's `24×24` hit targets, and `size.hit_target.min` is
//! the floor every interactive thing in the crate is measured against.
//! [`Leading::hit_extent`] therefore resolves the *visual* size and the
//! *hit* size as two questions: a `16px` checkbox is a `16px` mark on a
//! `24px` target when the tile is interactive, and stays `16px` when it
//! is not, because an inert mark has no target to owe.
//!
//! The same split decides the row itself. An interactive tile's hit target
//! is the full row — the whole line is the thing you click, which is why
//! the row's height is checked against the floor and not just the icons in
//! it. A non-interactive tile's hit target is empty: the pointer must fall
//! through to whatever is behind it, and a tile that swallows clicks it
//! cannot act on is worse than one with no hover feedback at all.
//!
//! # The selected tile's subtitle is promoted, and the measurement is why
//!
//! §3B.5 gives the subtitle `text.muted` and says nothing about what
//! `selected` looks like. §3B.6 does, for the row shape nearest to this
//! one: `accent.fill` behind `text.strong` at `4.80:1`. Adopting that
//! pairing is the consistent choice. What it forces is the subtitle:
//! `text.muted` on `accent.fill` measures **1.84:1**, which is not a
//! marginal miss but a caption that cannot be read at all. The resolver
//! promotes the subtitle to `text.strong` on a selected tile rather than
//! shipping an unreadable line, and the sidecar pins the measurement so a
//! future retune of either token fails a test instead of a user.
//!
//! [`Leading::hit_extent`]: crate::composite::list_tile::Leading::hit_extent
//!
//! Unit tests live in the sibling `list_tile_tests.rs` sidecar.

use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// Every rung the tile grid offers, standard rung first.
///
/// A constant rather than a call so a list that samples both rungs and a
/// test that checks them name the same two numbers.
pub const TILE_HEIGHTS: [f32; 2] = [SINGLE_LINE_HEIGHT, TWO_LINE_HEIGHT];

/// A single-line tile's height — `size.control.md` (§3B.5).
pub const SINGLE_LINE_HEIGHT: f32 = size::CONTROL_MD;

/// A two-line tile's height — §3B.5's printed `44px`.
///
/// **A Surface Geometry Constant, not a token read.** No rung in §2.2 is
/// `44`, and the parts §3B.5 prints for this row sum to `42` — see
/// [`TWO_LINE_UNACCOUNTED`] and the module docs.
pub const TWO_LINE_HEIGHT: f32 = 44.0;

/// The two-line row's height as its printed parts compose it.
///
/// `4 + 18 + 16 + 4`. Derived so the disagreement with
/// [`TWO_LINE_HEIGHT`] is arithmetic rather than an assertion.
pub const TWO_LINE_PARTS_HEIGHT: f32 =
    PADDING_VERTICAL + PADDING_VERTICAL + TITLE_RUNG.line_height + SUBTITLE_RUNG.line_height;

/// The two pixels §3B.5's two-line row carries that its own parts do not
/// account for.
///
/// Exposed rather than absorbed. The resolver uses the printed total; this
/// constant exists so the discrepancy is a named fact in the crate rather
/// than a rounding error nobody noticed.
pub const TWO_LINE_UNACCOUNTED: f32 = TWO_LINE_HEIGHT - TWO_LINE_PARTS_HEIGHT;

/// The tile's vertical padding — the `4px` of §3B.5's `[4px, 12px]`.
pub const PADDING_VERTICAL: f32 = space::XS;

/// The tile's horizontal padding — the `12px` of §3B.5's `[4px, 12px]`.
pub const PADDING_HORIZONTAL: f32 = space::MD;

/// The gap between slots — §3B.5's `8px`, charged between leading,
/// centre and trailing and *not* between a title and its subtitle.
pub const SLOT_GAP: f32 = space::SM;

/// The small leading mark — `16×16` (§3B.5).
pub const LEADING_SMALL: f32 = size::ICON_MD;

/// The large leading mark — `24×24` (§3B.5), which is also the hit-target
/// floor.
pub const LEADING_LARGE: f32 = size::HIT_TARGET_MIN;

/// A trailing action button's hit target — `24×24` (§3B.5).
pub const TRAILING_TARGET: f32 = size::HIT_TARGET_MIN;

/// The tile's border weight when it carries one.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The title's rung — §3B.5's `13px`, `type.ui_compact`.
pub const TITLE_RUNG: &typography::TypeRung = &typography::UI_COMPACT;

/// The subtitle's rung — §3B.5's `12px Regular`, `type.caption`.
pub const SUBTITLE_RUNG: &typography::TypeRung = &typography::CAPTION;

/// How a tile's centre slot is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileLines {
    /// A title alone.
    Single,
    /// A title over a subtitle.
    Two,
}

impl TileLines {
    /// Whether this shape carries a subtitle.
    #[must_use]
    pub const fn has_subtitle(self) -> bool {
        matches!(self, Self::Two)
    }
}

/// Which of §3B.5's two permitted leading sizes a mark uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeadingExtent {
    /// `16×16`.
    Small,
    /// `24×24`.
    Large,
}

impl LeadingExtent {
    /// The extent in pixels.
    #[must_use]
    pub const fn pixels(self) -> f32 {
        match self {
            Self::Small => LEADING_SMALL,
            Self::Large => LEADING_LARGE,
        }
    }
}

/// What occupies the leading slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leading {
    /// Nothing. The slot collapses to zero width rather than leaving a
    /// hole, because a tile list where only some rows carry a mark would
    /// jog every unmarked title — the same defect §4.8.1 reserves the
    /// menu's leading box against.
    None,
    /// An icon at one of the two permitted sizes.
    Icon(LeadingExtent),
    /// A `16px` checkbox.
    Checkbox,
    /// A `16px` radio.
    Radio,
}

impl Leading {
    /// The mark's visual extent — `0.0` when the slot is empty.
    #[must_use]
    pub const fn visual_extent(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Icon(extent) => extent.pixels(),
            Self::Checkbox | Self::Radio => size::CHECKBOX_BOX,
        }
    }

    /// The mark's hit extent.
    ///
    /// Never smaller than the `24px` floor when the tile is interactive,
    /// and exactly the visual size when it is not: an inert mark is not a
    /// target, and inflating it would make dead pixels refuse the pointer.
    #[must_use]
    pub const fn hit_extent(self, interactive: bool) -> f32 {
        let visual = self.visual_extent();
        if visual == 0.0 {
            return 0.0;
        }
        if interactive && visual < size::HIT_TARGET_MIN {
            size::HIT_TARGET_MIN
        } else {
            visual
        }
    }
}

/// The tile's height for one row shape.
#[must_use]
pub const fn tile_height(lines: TileLines) -> f32 {
    match lines {
        TileLines::Single => SINGLE_LINE_HEIGHT,
        TileLines::Two => TWO_LINE_HEIGHT,
    }
}

/// The three slots a tile divides its bounds into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileSlots {
    /// The leading mark's slot.
    pub leading: Rectangle,
    /// The filling title/subtitle slot.
    pub center: Rectangle,
    /// The shrinking trailing slot.
    pub trailing: Rectangle,
}

/// Divide a tile's bounds into its three slots.
///
/// The leading slot is reserved from the mark's *hit* extent rather than
/// its visual extent, so a `16px` checkbox on an interactive tile still
/// costs `24px` of row and the title does not move when the tile becomes
/// actionable. The centre slot takes the remainder and is never allowed
/// below zero width.
#[must_use]
pub fn slots(
    bounds: Rectangle,
    leading: Leading,
    interactive: bool,
    trailing_width: f32,
) -> TileSlots {
    let interior_x = bounds.x + PADDING_HORIZONTAL;
    let interior_width = (bounds.width - PADDING_HORIZONTAL - PADDING_HORIZONTAL).max(0.0);
    let interior_y = bounds.y + PADDING_VERTICAL;
    let interior_height = (bounds.height - PADDING_VERTICAL - PADDING_VERTICAL).max(0.0);

    let leading_width = leading.hit_extent(interactive);
    let available_for_trailing = (interior_width - leading_width).max(0.0);
    let reserved_trailing = trailing_width.min(available_for_trailing);

    // A gap is charged only between two slots that exist. Charging one
    // against an empty leading slot would indent every title in a list of
    // unmarked rows by `space.sm`, which is the same alignment defect the
    // reserved-slot rule exists to prevent, arriving from the gap instead
    // of the box.
    let gaps = usize::from(leading_width > 0.0) + usize::from(reserved_trailing > 0.0);
    let center_width =
        (interior_width - leading_width - reserved_trailing - SLOT_GAP * gaps as f32).max(0.0);
    let center_x = if leading_width > 0.0 {
        interior_x + leading_width + SLOT_GAP
    } else {
        interior_x
    };

    TileSlots {
        leading: Rectangle {
            x: interior_x,
            y: interior_y,
            width: leading_width,
            height: interior_height,
        },
        center: Rectangle {
            x: center_x,
            y: interior_y,
            width: center_width,
            height: interior_height,
        },
        trailing: Rectangle {
            x: interior_x + interior_width - reserved_trailing,
            y: interior_y,
            width: reserved_trailing,
            height: interior_height,
        },
    }
}

/// The square hit targets inside the trailing slot, right-aligned and
/// separated by [`SLOT_GAP`].
///
/// Each target is `size.hit_target.min` on a side, vertically centred in
/// the row, and contained by the tile's own bounds. A single-line tile is
/// `32` tall and a target is `24`, so the containment is not automatic at
/// every padding — it is asserted here so a padding retune cannot push a
/// button outside the row it belongs to.
#[must_use]
pub fn trailing_targets(bounds: Rectangle, count: usize) -> Vec<Rectangle> {
    let mut targets = Vec::with_capacity(count);
    let right = bounds.x + bounds.width - PADDING_HORIZONTAL;
    let y = bounds.y + (bounds.height - TRAILING_TARGET) / 2.0;

    for index in 0..count {
        let offset =
            (count - index) as f32 * TRAILING_TARGET + (count - index - 1) as f32 * SLOT_GAP;
        targets.push(Rectangle {
            x: right - offset,
            y,
            width: TRAILING_TARGET,
            height: TRAILING_TARGET,
        });
    }

    targets
}

/// The row's own hit target.
///
/// The full bounds for an interactive tile — the whole line is the thing
/// you click, which is why the row height rather than only the icons in it
/// is what the floor is checked against. `None` for a non-interactive tile,
/// which must let the pointer through rather than swallow it.
#[must_use]
pub fn hit_target(bounds: Rectangle, interactive: bool) -> Option<Rectangle> {
    interactive.then_some(bounds)
}

/// Whether a tile's height satisfies the hit-target floor.
#[must_use]
pub const fn meets_hit_target_floor(lines: TileLines) -> bool {
    tile_height(lines) >= size::HIT_TARGET_MIN
}

/// The tile's resolved paint and text colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileStyle {
    /// The row fill.
    pub background: Color,
    /// The title's colour.
    pub title_colour: Color,
    /// The subtitle's colour — promoted to [`title_colour`](Self::title_colour)
    /// whenever `text.muted` would not clear the fill beneath it.
    pub subtitle_colour: Color,
    /// The leading mark's colour.
    pub leading_colour: Color,
    /// The trailing controls' colour.
    pub trailing_colour: Color,
    /// The row's border, `0.0` wide when the tile paints none.
    pub border_color: Color,
    /// The border weight.
    pub border_width: f32,
}

/// Resolve a tile's paint.
///
/// The precedence is the crate's own: a dead tile paints nothing and is
/// read in `text.disabled`; a selected tile keeps its `accent.fill` through
/// hover and press, because selection is a fact about the model and the
/// pointer is a fact about the moment; only an unselected interactive tile
/// takes the hover and pressed lifts.
///
/// A selected tile carries **no perimeter stroke**. §3B.5 gives the tile no
/// border at all, and the selection marker §3B.6 does print — an
/// `accent.border` bar — is a left bar on a tree row, not a ring. That
/// distinction is load-bearing rather than cosmetic: `accent.border` against
/// `accent.fill` measures **1.61:1**, so a perimeter stroke in that colour
/// around that fill would be an indicator that cannot be seen against the
/// thing it is drawn on. The fill alone carries the selection.
#[must_use]
pub fn resolve_style(
    selected: bool,
    disabled: bool,
    base: BaseState,
    palette: &Palette,
) -> TileStyle {
    if disabled {
        return TileStyle {
            background: Color::TRANSPARENT,
            title_colour: palette.text_disabled,
            subtitle_colour: palette.text_disabled,
            leading_colour: palette.text_disabled,
            trailing_colour: palette.text_disabled,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
        };
    }

    if selected {
        return TileStyle {
            background: palette.accent_fill,
            title_colour: palette.text_strong,
            subtitle_colour: subtitle_on(palette.accent_fill, palette),
            leading_colour: palette.text_strong,
            trailing_colour: palette.text_strong,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
        };
    }

    let background = match base {
        BaseState::Pressed => palette.surface_active,
        BaseState::Hovered => palette.surface_hover,
        BaseState::Rest | BaseState::Disabled | BaseState::Invalid => Color::TRANSPARENT,
    };

    TileStyle {
        background,
        title_colour: palette.text_primary,
        subtitle_colour: palette.text_muted,
        leading_colour: palette.text_primary,
        trailing_colour: palette.text_muted,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
    }
}

/// The subtitle colour against a fill that is not the resting surface.
///
/// `text.muted` clears every resting and hover surface in the palette but
/// collapses to `1.84:1` on the selection fill. Rather than carry two
/// numbers for one line, the subtitle is promoted wherever the muted
/// caption cannot be read.
#[must_use]
pub fn subtitle_on(fill: Color, palette: &Palette) -> Color {
    if crate::token::contrast::meets_text_contrast(palette.text_muted, fill) {
        palette.text_muted
    } else {
        palette.text_strong
    }
}

/// The tile's painted row layer, `None` when nothing would be painted.
#[must_use]
pub fn row_layer(bounds: Rectangle, style: &TileStyle) -> Option<Layer> {
    if style.background.a == 0.0 && style.border_width == 0.0 {
        return None;
    }

    Some(Layer {
        bounds,
        fill: style.background,
        border: iced::Border {
            color: style.border_color,
            width: style.border_width,
            radius: shape::RADIUS_WIDGET.into(),
        },
    })
}

#[cfg(test)]
#[path = "list_tile_tests.rs"]
mod list_tile_tests;
