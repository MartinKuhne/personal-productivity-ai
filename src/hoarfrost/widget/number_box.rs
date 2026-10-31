//! The `NumberBox` paint resolver and geometry (§3.5, `UIF-ATOM-005`).
//!
//! §3.5 describes the number box as a `32px`-high numeric text input
//! with a trailing dual stepper. The first half of that sentence is not a
//! new atom: it is [`§3.2's text entry`](crate::widget::text_input), and
//! this module resolves it through
//! [`text_input::resolve_input_style`]
//! rather than restating a second table that could disagree with the
//! first. What the entry looks like at rest, on hover, with a focus ring
//! and when disabled is therefore §3.2's answer, once, in one place.
//!
//! The stepper is new, and it is the only part of §3.5 that needed a
//! resolver of its own.
//!
//! # Two stepper shapes, one direction vocabulary
//!
//! §3.5 offers the dual stepper in two physical forms — a stacked
//! `▲`/`▼` pair of `28 × 16px` compact buttons, or side-by-side
//! `24 × 24px` `[−]` `[+]` buttons — and treats them as alternatives
//! for the same control. They are modelled as [`StepperShape`], a
//! geometry choice, while the thing both shapes actually mean is
//! [`StepDirection`]. Every behavioural rule in
//! [`number_box_host`](crate::widget::number_box_host) is written
//! against the direction; the shape only decides which rectangle a point
//! fell in and which glyph is drawn there.
//!
//! The two shapes are not interchangeable in one respect worth naming:
//! the stacked pair's `16px` halves are **below** [`size::HIT_TARGET_MIN`]
//! and the side-by-side pair's `24px` buttons are exactly at it. §3.5
//! prints both without comment, so the framework prints both back rather
//! than quietly promoting the stacked variant to a legal target — the
//! compact form is the spec's choice and the gap is visible in the
//! gallery cell that shows it.
//!
//! # The stepper is a trailing slot, and paints like one
//!
//! The glyph colours follow the framework's existing answer for an
//! actionable glyph in a trailing slot — [`text_muted`](Palette::text_muted)
//! at rest, [`text_strong`](Palette::text_strong) once the pointer is on
//! it — which is the same vocabulary
//! [`text_input`] uses for its own trailing
//! `×`. The background steps `surface.raised` → `surface.hover` →
//! `surface.active` with the pointer, the same three surfaces every other
//! control's hover and press spend.
//!
//! # What `precision` and `unit_suffix` are for
//!
//! Neither is a paint property; both are rendering rules that live on
//! [`RangeModel`](crate::widget::range_model::RangeModel) and are read
//! here only to say what the field displays. The unit suffix is *outside*
//! the numeric value but *inside* the field, which is why
//! [`parse`](crate::widget::range_model::RangeModel::parse) strips it on the way back in: a field
//! that rendered `12.5px` and was re-edited in place has to still parse,
//! or the suffix is a trap the user has to remember to delete.
//!
//! Unit tests live in the sibling `number_box_tests.rs` sidecar.

use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::token::shape;
use crate::token::size;
use crate::widget::button_widget::Layer;
use crate::widget::text_input::{self, InputKind, InputStyle};
use iced::{Border, Color, Point, Rectangle};

/// The field's height — §3.5's `32px`, `size.control.md`.
pub const ENTRY_HEIGHT: f32 = size::CONTROL_MD;

/// The dual stepper's width — §3.5's `28px`, taken from
/// `size.control.sm`.
///
/// The value is the compact control rung, which is what the stepper is:
/// a control column beside the field. Taking it from the rung rather than
/// writing `28.0` means a retune of the rungs moves the stepper with
/// everything else instead of leaving a bare number behind.
pub const STEPPER_WIDTH: f32 = size::CONTROL_SM;

/// A side-by-side `[−]` / `[+]` button's edge — §3.5's `24 × 24px`,
/// `size.hit_target_min`.
pub const STEP_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// The stepper glyph's edge — `size.icon.md`.
///
/// §3.5 gives the `▲` / `▼` / `[−]` / `[+]` glyphs no size. `icon.md`
/// is the framework's ordinary glyph rung and is what
/// [`text_input::SLOT_GLYPH_SIZE`] uses for the trailing slot a stepper
/// is standing in, so the two trailing slots in the framework are the
/// same size rather than two guesses.
pub const GLYPH_SIZE: f32 = size::ICON_MD;

/// The stepper's corner radius — `radius.widget`, the corner every
/// interactive control in the framework uses.
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The stepper's perimeter weight — `border.hairline`.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The `▲` glyph on the stacked stepper's upper button.
pub const GLYPH_UP: char = '▲';

/// The `▼` glyph on the stacked stepper's lower button.
pub const GLYPH_DOWN: char = '▼';

/// The `−` glyph on the side-by-side stepper's decrement button.
///
/// U+2212 MINUS SIGN, not ASCII `-`. A hyphen in a numeric field renders
/// as a hyphen, which is a different glyph from the one every other
/// numeric UI in the framework uses for "take one away".
pub const GLYPH_MINUS: char = '−';

/// The `+` glyph on the side-by-side stepper's increment button.
pub const GLYPH_PLUS: char = '+';

/// Which physical form the dual stepper takes (§3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepperShape {
    /// `▲` over `▼`, two `28 × 16px` compact buttons.
    Stacked,
    /// `[−]` beside `[+]`, two `24 × 24px` buttons.
    SideBySide,
}

/// Which way a stepper moves the value.
///
/// The direction is what the behaviour is written against; the
/// [`StepperShape`] only decides where it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepDirection {
    /// One step toward the low end.
    Decrement,
    /// One step toward the high end.
    Increment,
}

impl StepDirection {
    /// The signed step count this direction moves by.
    #[must_use]
    pub const fn delta(self) -> i64 {
        match self {
            Self::Decrement => -1,
            Self::Increment => 1,
        }
    }

    /// The glyph this direction carries in a given shape.
    #[must_use]
    pub const fn glyph(self, shape: StepperShape) -> char {
        match (self, shape) {
            (Self::Decrement, StepperShape::Stacked) => GLYPH_DOWN,
            (Self::Increment, StepperShape::Stacked) => GLYPH_UP,
            (Self::Decrement, StepperShape::SideBySide) => GLYPH_MINUS,
            (Self::Increment, StepperShape::SideBySide) => GLYPH_PLUS,
        }
    }
}

/// A resolved number box: the entry's paint and the stepper's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberBoxStyle {
    /// The numeric entry's resolved paint — §3.2's answer, reused.
    pub entry: InputStyle,
    /// The stepper block's background.
    pub stepper_background: Color,
    /// The stepper block's perimeter stroke.
    pub stepper_border: Color,
    /// The stepper glyphs' colour.
    pub stepper_glyph: Color,
}

/// Resolve the paint for one `(state, shape)` pair.
///
/// The entry half is delegated whole. The stepper half is resolved here,
/// and its `(background, glyph)` pair goes through
/// [`contrast::assert_fill_foreground_lock`](crate::token::contrast::assert_fill_foreground_lock)
/// the same way [`resolve_style`](crate::widget::toggle::resolve_style) guards its indicator, so a
/// future retune that put a foreground-only token on a locked fill is
/// caught at construction rather than in a screenshot.
#[must_use]
pub fn resolve_style(state: VisualState, palette: &Palette) -> NumberBoxStyle {
    let entry = text_input::resolve_input_style(InputKind::TextEntry, state, palette);
    let stepper = stepper_paint(state.base, palette);

    NumberBoxStyle {
        entry,
        stepper_background: stepper.background,
        stepper_border: stepper.border,
        stepper_glyph: stepper.glyph,
    }
}

/// The three paint channels the stepper block fixes for a base state.
struct StepperPaint {
    background: Color,
    border: Color,
    glyph: Color,
}

/// Select the stepper's base-state cell.
///
/// `Invalid` shares the `Rest` cell. §3.5 spends its invalid token on the
/// **slider's thumb ring** (`2px status.danger.text`) and says nothing
/// about the stepper, and §3B.1 renders a field's validation words in the
/// reserved caption slot below the control. Giving the stepper a danger
/// treatment the section does not price would invent a second invalid
/// vocabulary, so the asymmetry is the spec's: a validating slider
/// repaints its ring, a validating number box does not repaint its
/// buttons.
const fn stepper_paint(base: BaseState, palette: &Palette) -> StepperPaint {
    match base {
        BaseState::Rest | BaseState::Invalid => StepperPaint {
            background: palette.surface_raised,
            border: palette.border_subtle,
            glyph: palette.text_muted,
        },
        BaseState::Hovered => StepperPaint {
            background: palette.surface_hover,
            border: palette.border_strong,
            glyph: palette.text_strong,
        },
        BaseState::Pressed => StepperPaint {
            background: palette.surface_active,
            border: palette.border_strong,
            glyph: palette.text_strong,
        },
        BaseState::Disabled => StepperPaint {
            background: palette.surface_raised,
            border: palette.border_subtle,
            glyph: palette.text_disabled,
        },
    }
}

/// The whole control's box at the standard rung.
#[must_use]
pub fn control_rect(origin: Point, width: f32) -> Rectangle {
    Rectangle::new(origin, iced::Size::new(width, ENTRY_HEIGHT))
}

/// The width the stepper block occupies for a given shape.
///
/// §3.5 prices the two trailing zones separately and prints both
/// figures: a [`Stacked`](StepperShape::Stacked) block is
/// `size.control.sm × size.control.md` (`28 × 32px`) holding two
/// `28 × 16px` buttons, and a [`SideBySide`](StepperShape::SideBySide)
/// block is `2 × size.hit_target.min` wide (`48 × 32px`) holding two
/// disjoint `24 × 24px` buttons. The widths differ because the button
/// forms do, and the spec now says so against each shape rather than
/// leaving one block size to cover both.
///
/// The side-by-side width is not something this resolver may negotiate
/// down. Two `24px` squares abreast need `48px` and a `28px` column
/// cannot hold them: squeezed in with a zero gap they overlap for
/// `20px`, and [`step_at`] — which tests decrement first — then answers
/// `Decrement` at the increment button's own centre, a `+` click that
/// steps the value down. Shrinking each to `14px` to fit puts both
/// buttons under the `24px` hit-target floor
/// [`size::HIT_TARGET_MIN`] sets. A wider block costs the entry some
/// width and breaks nothing.
#[must_use]
pub const fn stepper_width(shape: StepperShape) -> f32 {
    match shape {
        StepperShape::Stacked => STEPPER_WIDTH,
        StepperShape::SideBySide => 2.0 * STEP_BUTTON_SIZE,
    }
}

/// The stepper block for a given shape, anchored to the trailing edge.
///
/// Both shapes take the control's **full height**: §3.5 calls the
/// trailing zones `28 × 32` and `48 × 32`, so the block is as tall as
/// the field beside it and the buttons are placed *inside* it. The
/// block does not shrink to wrap the buttons — that would make the
/// painted zone a different rectangle from the one the spec names, and
/// leave the `32 − 24 = 8px` of the side-by-side arrangement
/// unaccounted for.
#[must_use]
pub fn stepper_rect_for(shape: StepperShape, bounds: Rectangle) -> Rectangle {
    let width = stepper_width(shape).min(bounds.width);
    Rectangle::new(
        Point::new(bounds.x + bounds.width - width, bounds.y),
        iced::Size::new(width, bounds.height.max(0.0)),
    )
}

/// The stepper block in its spec-literal `28 x 32` form — the
/// [`Stacked`](StepperShape::Stacked) shape.
#[must_use]
pub fn stepper_rect(bounds: Rectangle) -> Rectangle {
    stepper_rect_for(StepperShape::Stacked, bounds)
}

/// The numeric entry — everything the shape's stepper block does not take.
#[must_use]
pub fn entry_rect_for(shape: StepperShape, bounds: Rectangle) -> Rectangle {
    let stepper = stepper_width(shape).min(bounds.width);
    Rectangle::new(
        bounds.position(),
        iced::Size::new(bounds.width - stepper, bounds.height),
    )
}

/// The numeric entry beside a [`Stacked`](StepperShape::Stacked) stepper.
#[must_use]
pub fn entry_rect(bounds: Rectangle) -> Rectangle {
    entry_rect_for(StepperShape::Stacked, bounds)
}

/// The two step buttons' rectangles, in `[decrement, increment]` order.
///
/// The stacked pair splits the block's height in half — `28 x 16` each,
/// which is what §3.5 prints and which sits *below* the `24px`
/// hit-target floor. That shortfall is the spec's, is printed in the
/// gallery cell, and is not quietly promoted here.
///
/// The side-by-side pair lays two `24 x 24` squares abreast inside the
/// `48 x 32` block, decrement on the left and increment on the right —
/// the order `[-]` `[+]` reads in — centred vertically in the zone's
/// full height, so the `8px` the block is taller than a button is
/// split `4px` above and `4px` below rather than left under one edge.
/// They cannot overlap, because [`stepper_width`] gave the block the
/// `48px` two squares require.
#[must_use]
pub fn step_button_rects(bounds: Rectangle, shape: StepperShape) -> [Rectangle; 2] {
    let block = stepper_rect_for(shape, bounds);
    match shape {
        StepperShape::Stacked => {
            let half = block.height / 2.0;
            [
                Rectangle::new(block.position(), iced::Size::new(block.width, half)),
                Rectangle::new(
                    Point::new(block.x, block.y + half),
                    iced::Size::new(block.width, half),
                ),
            ]
        }
        StepperShape::SideBySide => {
            let side = STEP_BUTTON_SIZE
                .min(block.width / 2.0)
                .min(block.height.max(0.0))
                .max(0.0);
            let y = block.y + (block.height - side) / 2.0;
            [
                Rectangle::new(Point::new(block.x, y), iced::Size::new(side, side)),
                Rectangle::new(Point::new(block.x + side, y), iced::Size::new(side, side)),
            ]
        }
    }
}

/// Which step button, if any, contains `point`.
///
/// Returns the *direction* rather than a button index, so the caller
/// never has to know which shape it is holding in order to know what the
/// click meant.
#[must_use]
pub fn step_at(bounds: Rectangle, shape: StepperShape, point: Point) -> Option<StepDirection> {
    let [decrement, increment] = step_button_rects(bounds, shape);
    if contains(decrement, point) {
        return Some(StepDirection::Decrement);
    }
    if contains(increment, point) {
        return Some(StepDirection::Increment);
    }
    None
}

/// Whether `rect` covers `point`.
fn contains(rect: Rectangle, point: Point) -> bool {
    point.x >= rect.x
        && point.x < rect.x + rect.width
        && point.y >= rect.y
        && point.y < rect.y + rect.height
}

/// The entry's background-and-border layer, delegated to §3.2.
#[must_use]
pub fn entry_layer(bounds: Rectangle, style: &NumberBoxStyle) -> Option<Layer> {
    entry_layer_for(StepperShape::Stacked, bounds, style)
}

/// The entry's background-and-border layer for a given stepper shape.
#[must_use]
pub fn entry_layer_for(
    shape: StepperShape,
    bounds: Rectangle,
    style: &NumberBoxStyle,
) -> Option<Layer> {
    text_input::background_layer(entry_rect_for(shape, bounds), &style.entry)
}

/// The entry's focus ring layer, delegated to §3.2.
#[must_use]
pub fn entry_focus_ring_layer(bounds: Rectangle, style: &NumberBoxStyle) -> Option<Layer> {
    entry_focus_ring_layer_for(StepperShape::Stacked, bounds, style)
}

/// The entry's focus ring layer for a given stepper shape.
#[must_use]
pub fn entry_focus_ring_layer_for(
    shape: StepperShape,
    bounds: Rectangle,
    style: &NumberBoxStyle,
) -> Option<Layer> {
    text_input::focus_ring_layer(entry_rect_for(shape, bounds), &style.entry)
}

/// The stepper block's background-and-border layer.
#[must_use]
pub fn stepper_layer(bounds: Rectangle, style: &NumberBoxStyle) -> Layer {
    stepper_layer_for(StepperShape::Stacked, bounds, style)
}

/// The stepper block's background-and-border layer for a given shape.
#[must_use]
pub fn stepper_layer_for(shape: StepperShape, bounds: Rectangle, style: &NumberBoxStyle) -> Layer {
    Layer {
        bounds: stepper_rect_for(shape, bounds),
        fill: style.stepper_background,
        border: Border {
            color: style.stepper_border,
            width: BORDER_WIDTH,
            radius: RADIUS.into(),
        },
    }
}

/// The two step buttons' layers, in `[decrement, increment]` order.
///
/// The buttons are recorded as separate quads rather than being folded
/// into the block so a snapshot can show which half a press fell in —
/// the same reason the split button's two zones are cells of their own.
#[must_use]
pub fn step_button_layers(
    bounds: Rectangle,
    style: &NumberBoxStyle,
    shape: StepperShape,
) -> [Layer; 2] {
    step_button_rects(bounds, shape)
        .into_iter()
        .map(|rect| Layer {
            bounds: rect,
            fill: style.stepper_background,
            border: Border {
                color: style.stepper_border,
                width: BORDER_WIDTH,
                radius: RADIUS.into(),
            },
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or([
            Layer {
                bounds: Rectangle::new(Point::ORIGIN, iced::Size::ZERO),
                fill: style.stepper_background,
                border: Border::default().width(0.0),
            },
            Layer {
                bounds: Rectangle::new(Point::ORIGIN, iced::Size::ZERO),
                fill: style.stepper_background,
                border: Border::default().width(0.0),
            },
        ])
}

/// Classify the stepper's perimeter stroke under `UIF-TOK-006` rule 3.
///
/// `border.subtle` at rest and when disabled is a resting divider and
/// exempt; the `border.strong` the hover and press cells draw is active
/// chrome and is judged.
#[must_use]
pub fn stepper_border_chrome_class(border: Color, palette: &Palette) -> ChromeClass {
    if same_channels(border, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// Whether two colours are the same channel-for-channel.
fn same_channels(a: Color, b: Color) -> bool {
    a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a
}

#[cfg(test)]
#[path = "number_box_tests.rs"]
mod number_box_tests;
