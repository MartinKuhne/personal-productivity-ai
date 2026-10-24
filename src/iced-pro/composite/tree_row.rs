//! The tree row renderer — indent, guides, chevron, slots and bands, all
//! computed here (`UIF-COMP-006`, decision **D-8**).
//!
//! # Why the row is framework code
//!
//! Decision **D-8** revised **D-2** and put *tree row rendering* on the
//! framework's side of the line. The reason is the same one that put the
//! `ListTile` grid here: a row is nine screens of the same thing, and a
//! host that picks its own indent step, chevron box and selection band
//! produces a slightly different tree on each of them. The user cannot
//! name what is wrong; they can only feel that the lists do not line up.
//!
//! So every number in this module is resolved here, and the host supplies
//! content — a label, an icon, a badge flag — never geometry.
//!
//! # The grid is `list_tile`'s, not a second one
//!
//! A tree row is a three-slot row with a prefix. The prefix is the
//! indent region plus the reserved disclosure column; past it, the row
//! *is* a [`list_tile`] row: leading mark, filling centre, shrinking
//! trailing targets, the leading slot reserved from the mark's hit
//! extent rather than its visual extent, the gap charged only between
//! slots that exist.
//!
//! That reuse is why [`list_tile::slots_in`] exists. The alternative —
//! writing the partition out again for the tree — would have produced two
//! implementations of the gap-charging rule, and the first person to
//! retune one of them would have shipped a tree whose titles jog against
//! every other list in the application.
//!
//! # Indentation is bounded by the label, not by a depth cap
//!
//! `depth × 16px` is fine at depth 3 and a catastrophe at depth 40 in a
//! 220px pane: the label is squeezed to nothing and the row says
//! nothing. A fixed depth cap is the usual answer and it is the wrong
//! shape, because the depth at which the trouble starts depends on the
//! pane's width, not on the tree.
//!
//! The policy here is therefore width-relative, in three tiers
//! ([`indent_geometry`]):
//!
//! 1. **Nominal.** `space.lg` per level, while it fits.
//! 2. **Compressed.** When the nominal indent would starve the label,
//!    the step shrinks proportionally so *every level keeps its own
//!    guide line*. Depth information survives; only the pitch changes.
//! 3. **Collapsed.** Below [`MIN_INDENT_STEP`] the step cannot shrink
//!    further without the guides merging into a smear, so the deepest
//!    levels share the last drawn guide and
//!    [`IndentGeometry::collapsed_levels`] reports how many.
//!
//! In all three tiers the label keeps at least [`MIN_LABEL_WIDTH`], and
//! that floor does not depend on depth — which is the property the
//! 20-level test checks. A host that would rather show the full
//! indentation than compress it uses
//! [`content_width`] to size a horizontally scrollable viewport; pass
//! that width back in and the policy resolves to tier 1 with nothing
//! clamped.
//!
//! # Contrast, recomputed
//!
//! Every figure below is produced by [`crate::token::contrast`] from raw
//! sRGB in the sidecar, not copied from §3B.6's printed column.
//!
//! | Foreground | Band | Ratio | Note |
//! | --- | --- | --- | --- |
//! | `text.strong` | `accent.fill` | 4.8020:1 | the selection band; spec prints `4.80:1` — agrees |
//! | `text.muted` | `accent.fill` | 1.8363:1 | a chevron here is invisible; promoted |
//! | `border.strong` | `surface.raised` | 4.2379:1 | guide line, resting |
//! | `border.subtle` | `surface.raised` | 1.2691:1 | the naive guide token — unreadable |
//! | `border.strong` | `accent.fill` | 1.0876:1 | the resting guide over a selected band — invisible |
//! | `accent.border` | `accent.fill` | 1.6098:1 | the spec's printed left bar — cannot be seen |
//! | `status.warning` | `accent.fill` | 1.6547:1 | the warning mark over a selected band — promoted |
//!
//! Three of those are disagreements with what §3B.6 prints, and each is
//! resolved by the crate's existing adaptive rule rather than by a new
//! one: a mark that cannot be seen against the thing it is drawn on is
//! promoted to `text.strong`. That is decision **D-6**'s focus-ring rule
//! applied to the three other marks a tree row carries, and it is why
//! the spec's `accent.border` left bar survives as a *bar* while its
//! colour changes.
//!
//! The guide line deserves its own sentence because the spec never
//! prices it at all. `border.subtle` is the token a structural hairline
//! "should" use, and at `1.2691:1` on `surface.raised` it is not a
//! hierarchy indicator — it is a rumour of one. A guide line carries
//! information the user reads to find a node's parent, which makes it
//! indicator material under `UIF-TOK-006` rule 3 rather than a
//! `RestingDivider`, so it is graded `Judged` at `3.00:1` and drawn in
//! `border.strong`.
//!
//! Unit tests live in the sibling `tree_row_tests.rs` sidecar.

use crate::composite::list_tile::{self, Leading, RowPadding, TileSlots};
use crate::composite::virtual_list::{COMPACT_ROW_HEIGHT, STANDARD_ROW_HEIGHT};
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::contrast::{fill_lock_role, meets_chrome_contrast, meets_disabled_text_band};
use crate::token::{shape, size, space, typography};
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// The nominal indent per level — §3B.6's `depth × 16px`, which is
/// `space.lg`.
pub const INDENT_STEP: f32 = space::LG;

/// The smallest step the compression tier will shrink to.
///
/// Below this the guide lines are close enough to merge into one smear
/// at normal viewing distance, which loses the thing they exist to show.
/// `space.xs` is the smallest spacing in the system that still reads as
/// a gap between two lines.
pub const MIN_INDENT_STEP: f32 = space::XS;

/// The label's guaranteed floor, whatever the depth.
///
/// Four `space.xl` units — written as a sum so the constant is bound to
/// tokens and carries no literal of its own (`UIF-PRIN-001`). At the
/// `type.ui_compact` rung this is roughly a dozen characters, which is
/// about where a file name stops being recognisable.
pub const MIN_LABEL_WIDTH: f32 = space::XL + space::XL + space::XL + space::XL;

/// The reserved disclosure column — §3B.6's `24×24px` chevron box,
/// which is `size.hit_target.min`.
///
/// Reserved on **every** row, leaf included. A leaf that dropped the
/// column would jog its label left against its siblings, which is the
/// same defect the tile's leading-slot rule exists to prevent.
pub const CHEVRON_COLUMN: f32 = size::HIT_TARGET_MIN;

/// The row's horizontal inset — `UIF-PANE-003`'s `4px`.
///
/// **A spec conflict, resolved toward §4.** §3B.5 gives the list tile
/// `[4px, 12px]`, i.e. `12px` horizontally, and §3B.6 says nothing
/// about a tree row's inset; §4's `UIF-PANE-003` gives the tree row
/// `4px`. The tree's own requirement is the specific one and it wins,
/// which also buys back the horizontal space the indentation policy
/// spends.
pub const ROW_INSET_X: f32 = space::XS;

/// The row's vertical inset — `space.xxs`.
///
/// Chosen so the `type.ui_compact` line box fits inside **both**
/// permitted rungs without overflowing: `24 − 2×2 = 20 ≥ 18` and
/// `28 − 2×2 = 24 ≥ 18`. The sidecar asserts that inequality rather
/// than trusting it, because a rung retune that broke it would clip
/// every label in the tree.
pub const ROW_INSET_Y: f32 = space::XXS;

/// The selection band's left bar — §3B.6's `2px`, `border.focus`.
///
/// A token read rather than a literal: `border.focus` *is* the `2px`
/// stroke weight the closed set offers, and §3B.6 names it against this
/// bar. An earlier reading composed `border.focus + border.hairline` for
/// a `3px` bar; the corrected §3B.6 prices the bar at `2px`, so the
/// extra hairline is gone and the width is a single token.
pub const SELECTION_BAR_WIDTH: f32 = shape::BORDER_FOCUS;

/// The row's label rung — `type.ui_compact`, `13px` (`UIF-PANE-003`).
pub const LABEL_RUNG: &typography::TypeRung = &typography::UI_COMPACT;

/// The compact row height this renderer lays out at.
pub const ROW_HEIGHT_COMPACT: f32 = COMPACT_ROW_HEIGHT;

/// The standard row height this renderer lays out at.
pub const ROW_HEIGHT_STANDARD: f32 = STANDARD_ROW_HEIGHT;

/// How the indent region resolved for one row at one width.
///
/// The three-tier policy is in the [module docs](self#indentation-is-bounded-by-the-label-not-by-a-depth-cap).
#[derive(Clone, Debug, PartialEq)]
pub struct IndentGeometry {
    /// The step actually drawn — nominal, compressed, or at the floor.
    pub step: f32,
    /// The width the indent region occupies.
    pub width: f32,
    /// The x of each drawn guide line, outermost level first.
    ///
    /// One entry per level that got its own line. In the collapsed tier
    /// this is shorter than the depth.
    pub guides: Vec<f32>,
    /// How many levels had to share the last guide.
    ///
    /// `0` in the nominal and compressed tiers.
    pub collapsed_levels: usize,
}

/// The horizontal room a row's indent can consume before the label floor
/// is breached.
#[must_use]
pub fn indent_allowance(row_width: f32) -> f32 {
    (row_width - ROW_INSET_X - ROW_INSET_X - CHEVRON_COLUMN - MIN_LABEL_WIDTH).max(0.0)
}

/// Resolve the indent region for `depth` inside a row of `row_width`.
///
/// Pure, total, and width-relative: the same depth resolves differently
/// at different widths, which is the point. A row too narrow to hold the
/// chevron and the label floor simply gets zero indent rather than a
/// negative one — every branch clamps.
#[must_use]
pub fn indent_geometry(depth: usize, row_width: f32) -> IndentGeometry {
    if depth == 0 {
        return IndentGeometry {
            step: INDENT_STEP,
            width: 0.0,
            guides: Vec::new(),
            collapsed_levels: 0,
        };
    }

    let allowance = indent_allowance(row_width);
    let nominal = depth as f32 * INDENT_STEP;

    if nominal <= allowance {
        return IndentGeometry {
            step: INDENT_STEP,
            width: nominal,
            guides: guide_positions(depth, INDENT_STEP),
            collapsed_levels: 0,
        };
    }

    if depth as f32 * MIN_INDENT_STEP <= allowance {
        let step = allowance / depth as f32;
        return IndentGeometry {
            step,
            width: allowance,
            guides: guide_positions(depth, step),
            collapsed_levels: 0,
        };
    }

    let drawn = (allowance / MIN_INDENT_STEP).floor().max(0.0) as usize;
    IndentGeometry {
        step: MIN_INDENT_STEP,
        width: allowance,
        guides: guide_positions(drawn, MIN_INDENT_STEP),
        collapsed_levels: depth.saturating_sub(drawn),
    }
}

/// The x of each guide line, measured from the row's own left edge.
///
/// Centred in each step so the line sits under its ancestor's disclosure
/// column rather than on the boundary between two levels.
fn guide_positions(levels: usize, step: f32) -> Vec<f32> {
    (0..levels)
        .map(|level| ROW_INSET_X + level as f32 * step + step / 2.0)
        .collect()
}

/// The row's slot padding at a resolved indent.
#[must_use]
pub fn row_padding(indent_width: f32) -> RowPadding {
    RowPadding {
        left: ROW_INSET_X + indent_width + CHEVRON_COLUMN,
        right: ROW_INSET_X,
        vertical: ROW_INSET_Y,
        gap: list_tile::SLOT_GAP,
    }
}

/// Every box a tree row owns.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRowSlots {
    /// The indent region, inside the row's vertical inset.
    pub indent: Rectangle,
    /// One full-height hairline per drawn guide level.
    ///
    /// Full row height rather than the inset interior so consecutive
    /// rows' guides join into a continuous vertical line.
    pub guides: Vec<Rectangle>,
    /// The reserved disclosure column.
    pub chevron: Rectangle,
    /// The three-slot grid past the prefix — the `list_tile` grid.
    pub tile: TileSlots,
    /// The selection band's left bar.
    pub selection_bar: Rectangle,
}

/// Resolve every box in one tree row.
///
/// `row` is the row's full bounds. The selection and hover bands are the
/// whole rectangle — including the indent — because a band that stopped
/// short of the guides would cut the hierarchy in half every time a row
/// was picked.
#[must_use]
pub fn row_slots(
    row: Rectangle,
    depth: usize,
    leading: Leading,
    interactive: bool,
    trailing_width: f32,
) -> TreeRowSlots {
    let geometry = indent_geometry(depth, row.width);
    let padding = row_padding(geometry.width);
    let interior_y = row.y + ROW_INSET_Y;
    let interior_height = (row.height - ROW_INSET_Y - ROW_INSET_Y).max(0.0);

    TreeRowSlots {
        indent: Rectangle {
            x: row.x + ROW_INSET_X,
            y: interior_y,
            width: geometry.width.max(0.0),
            height: interior_height,
        },
        guides: geometry
            .guides
            .iter()
            .map(|guide| Rectangle {
                x: row.x + guide,
                y: row.y,
                width: shape::BORDER_HAIRLINE,
                height: row.height.max(0.0),
            })
            .collect(),
        chevron: Rectangle {
            x: row.x + ROW_INSET_X + geometry.width,
            y: row.y + (row.height - CHEVRON_COLUMN) / 2.0,
            width: CHEVRON_COLUMN,
            height: CHEVRON_COLUMN.min(row.height.max(0.0)),
        },
        tile: list_tile::slots_in(row, &padding, leading, interactive, trailing_width),
        selection_bar: Rectangle {
            x: row.x,
            y: row.y,
            width: SELECTION_BAR_WIDTH.min(row.width.max(0.0)),
            height: row.height.max(0.0),
        },
    }
}

/// The width a row *would* need if nothing were clamped.
///
/// A host that prefers horizontal scrolling to compressed indentation
/// sizes its scrollable content to the maximum of this across the
/// visible rows and passes that back as the row width; the indent policy
/// then resolves to tier 1 for every depth.
#[must_use]
pub fn content_width(depth: usize, label_width: f32) -> f32 {
    ROW_INSET_X + ROW_INSET_X + depth as f32 * INDENT_STEP + CHEVRON_COLUMN + label_width.max(0.0)
}

/// Whether a row of `row_width` can hold the label floor at any depth.
///
/// Deliberately depth-free: the whole claim of the policy is that depth
/// cannot erode the label, so the check that says so must not take depth
/// as an argument.
#[must_use]
pub const fn label_floor_holds(row_width: f32) -> bool {
    row_width >= ROW_INSET_X + ROW_INSET_X + CHEVRON_COLUMN + MIN_LABEL_WIDTH
}

/// Whether the label's line box fits inside a row at a given height.
#[must_use]
pub const fn label_fits_row(row_height: f32) -> bool {
    row_height - ROW_INSET_Y - ROW_INSET_Y >= LABEL_RUNG.line_height
}

/// The surface a row's marks are actually painted against.
///
/// A resting row paints no fill, so its marks sit on whatever the pane
/// is mounted on; every other state paints its own band and the marks
/// move with it.
#[must_use]
pub const fn band_colour(background: Color, surface: Color) -> Color {
    if background.a == 0.0 {
        surface
    } else {
        background
    }
}

/// A mark's colour, promoted when the resting token cannot be seen on the
/// band it is drawn over.
///
/// The crate's rule, restated from decision **D-6**: an indicator that
/// cannot be seen is not an indicator, so the mark adapts rather than
/// the token being honoured into invisibility. `text.strong` is the
/// fallback because it clears the selection fill in every palette this
/// crate has shipped.
#[must_use]
pub fn promoted_over(candidate: Color, band: Color, palette: &Palette) -> Color {
    if meets_chrome_contrast(candidate, band) {
        candidate
    } else {
        palette.text_strong
    }
}

/// A mark's colour under §3B.6's **role-based** selection lock.
///
/// §3B.6 locks the label, disclosure chevron, depth guides and left
/// selection bar to `text.strong` because of *what the band is* — a
/// solid accent fill — and says so is "strictly role-based, not
/// threshold-based". The distinction is load-bearing: a threshold rule
/// releases the lock the moment a palette retune lifts the resting token
/// over `3:1`, and the spec does not permit the guide lines to come back
/// on a selected row because of an arithmetic accident.
///
/// Off a locked fill the resting token is returned untouched, so an
/// unselected row keeps the `border.strong` guides, `text.muted`
/// chevron and `accent.border` bar §3B.6 prices for it.
#[must_use]
pub fn locked_over(candidate: Color, band: Color, palette: &Palette) -> Color {
    if fill_lock_role(band, palette).is_locked_fill() {
        palette.text_strong
    } else {
        candidate
    }
}

/// The guide line's colour against a band.
///
/// Locked by role on a selected row (§3B.6); `border.strong` elsewhere.
#[must_use]
pub fn guide_colour(band: Color, palette: &Palette) -> Color {
    locked_over(palette.border_strong, band, palette)
}

/// The disclosure chevron's colour against a band.
///
/// Locked by role on a selected row (§3B.6); `text.muted` elsewhere.
#[must_use]
pub fn chevron_colour(band: Color, palette: &Palette) -> Color {
    locked_over(palette.text_muted, band, palette)
}

/// The selection band's left bar colour.
///
/// §3B.6 prints `accent.border` for this bar. Against `accent.fill` —
/// the band it is drawn on — that measures `1.6098:1`, so the bar is
/// locked to `text.strong` by role rather than promoted by measurement.
#[must_use]
pub fn selection_bar_colour(band: Color, palette: &Palette) -> Color {
    locked_over(palette.accent_border, band, palette)
}

/// The `special_warning` mark's colour against a band.
///
/// **Deliberately still threshold-promoted, not role-locked.** §3B.6's
/// lock list names the label, chevron, guides and selection bar — and
/// not the warning dot. Locking it by role would replace `status.warning`
/// with `text.strong` on every selected row, which is the safe colour
/// but destroys the warning's meaning exactly when the row is selected.
/// The threshold rule keeps the warning's own hue wherever it clears
/// `3:1` and falls back to `text.strong` where it does not — including
/// on `accent.fill`, where `status.warning` measures `1.6547:1` and so
/// is promoted. That is the crate's decision **D-6** and it is the
/// stricter of the two readings here; the spec simply does not cover
/// this mark.
#[must_use]
pub fn warning_colour(band: Color, palette: &Palette) -> Color {
    promoted_over(palette.status_warning, band, palette)
}

/// A tree row's resolved paint.
///
/// Every channel is a colour this module can be measured against
/// [`band`](band_colour); nothing here is left for the host to decide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeRowStyle {
    /// The row's band fill — transparent at rest.
    pub background: Color,
    /// The surface the row's marks sit on.
    pub band: Color,
    /// The label.
    pub label_colour: Color,
    /// The leading icon.
    pub leading_colour: Color,
    /// The trailing metadata.
    pub trailing_colour: Color,
    /// The disclosure chevron.
    pub chevron: Color,
    /// The indent guide lines.
    pub guide: Color,
    /// The selection band's left bar — transparent when unselected.
    pub selection_bar: Color,
    /// The `special_warning` mark — transparent when the row carries no
    /// warning.
    pub warning: Color,
}

/// Resolve a tree row's paint.
///
/// The precedence is the crate's own and it is stated rather than
/// implied, because the ordering *is* the behaviour:
///
/// 1. **Disabled** wins outright — a dead row shows no band at all, so
///    a hover cannot make an inert node look live.
/// 2. **Selected** outranks the pointer. Selection is a fact about the
///    model; the pointer is a fact about the moment.
/// 3. **Pressed** outranks hover, and both paint their surface rung.
/// 4. **Rest** paints nothing.
///
/// `surface` is the pane's own fill, used wherever the row paints
/// transparent. It is a parameter rather than a constant because the same
/// renderer sits on `surface.base`, `surface.raised` and `surface.overlay`
/// across the app, and the guide line's contrast depends on which.
#[must_use]
pub fn resolve_style(
    selected: bool,
    disabled: bool,
    warning: bool,
    base: BaseState,
    surface: Color,
    palette: &Palette,
) -> TreeRowStyle {
    if disabled {
        let band = surface;
        return TreeRowStyle {
            background: Color::TRANSPARENT,
            band,
            label_colour: palette.text_disabled,
            leading_colour: palette.text_disabled,
            trailing_colour: palette.text_disabled,
            chevron: palette.text_disabled,
            guide: guide_colour(band, palette),
            selection_bar: Color::TRANSPARENT,
            warning: Color::TRANSPARENT,
        };
    }

    let background = if selected {
        palette.accent_fill
    } else {
        match base {
            BaseState::Pressed => palette.surface_active,
            BaseState::Hovered => palette.surface_hover,
            BaseState::Rest | BaseState::Disabled | BaseState::Invalid => Color::TRANSPARENT,
        }
    };

    let band = band_colour(background, surface);

    if selected {
        return TreeRowStyle {
            background,
            band,
            label_colour: palette.text_strong,
            leading_colour: palette.text_strong,
            trailing_colour: palette.text_strong,
            chevron: chevron_colour(band, palette),
            guide: guide_colour(band, palette),
            selection_bar: selection_bar_colour(band, palette),
            warning: if warning {
                warning_colour(band, palette)
            } else {
                Color::TRANSPARENT
            },
        };
    }

    TreeRowStyle {
        background,
        band,
        label_colour: palette.text_primary,
        leading_colour: palette.text_primary,
        trailing_colour: palette.text_muted,
        chevron: chevron_colour(band, palette),
        guide: guide_colour(band, palette),
        selection_bar: Color::TRANSPARENT,
        warning: if warning {
            warning_colour(band, palette)
        } else {
            Color::TRANSPARENT
        },
    }
}

/// The row's band layer, `None` when the row paints nothing.
#[must_use]
pub fn row_layer(bounds: Rectangle, style: &TreeRowStyle) -> Option<Layer> {
    if style.background.a == 0.0 {
        return None;
    }
    Some(Layer {
        bounds,
        fill: style.background,
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0_f32.into(),
        },
    })
}

/// The selection band's left bar layer, `None` when unselected.
#[must_use]
pub fn selection_bar_layer(bar: Rectangle, style: &TreeRowStyle) -> Option<Layer> {
    if style.selection_bar.a == 0.0 {
        return None;
    }
    Some(Layer {
        bounds: bar,
        fill: style.selection_bar,
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0_f32.into(),
        },
    })
}

/// The guide-line layers, one per drawn level.
#[must_use]
pub fn guide_layers(slots: &TreeRowSlots, style: &TreeRowStyle) -> Vec<Layer> {
    slots
        .guides
        .iter()
        .map(|guide| Layer {
            bounds: *guide,
            fill: style.guide,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0_f32.into(),
            },
        })
        .collect()
}

/// Whether a disabled row's label still lands inside the
/// `[4.0, 6.0]` legibility band on a given surface (`UIF-TOK-006`
/// rule 2).
#[must_use]
pub fn disabled_label_in_band(surface: Color, palette: &Palette) -> bool {
    meets_disabled_text_band(palette.text_disabled, surface)
}

#[cfg(test)]
#[path = "tree_row_tests.rs"]
mod tree_row_tests;
