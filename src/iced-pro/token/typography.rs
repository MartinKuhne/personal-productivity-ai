//! Typography scale: size, line height, weight and tracking (`UIF-TOK-004`).
//!
//! Unlike the scalar scales beside it, a rung of the type scale carries
//! several properties at once, so a rung is a struct rather than a bare
//! `f32`: [`TypeRung`] bundles the size, line height, resting weight,
//! emphasis weight and tracking for one rung, and the six `pub const`
//! rungs below are the only ones the interface may render with.
//!
//! Most rungs are given two weights — the resting weight un-emphasised
//! text carries, and the weight the same text switches to when
//! emphasised. Where the spec lists a single weight ([`BODY`],
//! [`TITLE_MD`], [`TITLE_LG`]) **both** fields hold that same weight:
//! emphasis on such a rung is defined to leave the weight alone, not left
//! undefined. A caller can therefore read [`TypeRung::emphasis_weight`]
//! unconditionally without asking which shape a rung has.
//!
//! [`MIN_SIZE`] is not a rung. It is the 12px floor the whole interface
//! is measured against, kept here because the scale's smallest rung is
//! what carries that rule.
//!
//! Sizes and line heights are logical pixels; tracking is in em.
//!
//! Unit tests live in the sibling `typography_tests.rs` sidecar.

/// A typographic weight used by the `UIF-TOK-004` scale.
///
/// The scale deliberately draws on four weights and no more, which keeps
/// the hierarchy readable without a caller having to reason about
/// numeric weight classes. [`Weight::rank`] supplies the ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    /// 400 — the resting weight of every rung below the titles.
    Regular,
    /// 500 — the emphasis weight of the two smallest rungs.
    Medium,
    /// 600 — the resting weight of `type.title_sm`.
    SemiBold,
    /// 700 — the emphasis weight of `type.title_sm`, and the resting
    /// weight of both larger titles.
    Bold,
}

impl Weight {
    /// Heaviness rank: [`Weight::Regular`] < [`Weight::Medium`] <
    /// [`Weight::SemiBold`] < [`Weight::Bold`].
    ///
    /// An explicit rank rather than a derived `Ord`, so comparing weights
    /// states a property of the scale instead of relying on the order the
    /// variants happen to be declared in.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Regular => 0,
            Self::Medium => 1,
            Self::SemiBold => 2,
            Self::Bold => 3,
        }
    }
}

/// One rung of the `UIF-TOK-004` type scale.
///
/// A rung is a value, not a knob: the six constants in this module are
/// the scale, and a hand-built [`TypeRung`] outside them is a spec
/// violation in the same way a raw numeric literal in a widget is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeRung {
    /// Font size in logical pixels. No rung may sit below [`MIN_SIZE`].
    pub size: f32,
    /// Line height in logical pixels.
    ///
    /// The spec states this as a pixel value and prints the resulting
    /// multiplier alongside it; read [`TypeRung::line_height_ratio`] for
    /// the multiplier rather than carrying a second copy of it.
    pub line_height: f32,
    /// The rung's resting weight — what text on this rung renders at when
    /// nothing is emphasising it.
    pub weight: Weight,
    /// The weight text on this rung takes when emphasised.
    ///
    /// Where the spec lists two weights, this is the heavier one. Where
    /// it lists only one, this carries the same weight as
    /// [`TypeRung::weight`]: emphasised text on such a rung is specified
    /// to be no heavier, which is a decision rather than a gap.
    pub emphasis_weight: Weight,
    /// Letter-spacing in **em**, so the spec's `+0.01em` is `0.01` and
    /// its `-0.015em` is `-0.015`.
    ///
    /// Positive tracking opens the glyphs up, negative tracking tightens
    /// them. The scale tightens as it grows: the smallest rung is the
    /// only one opened up, and the largest is the tightest.
    pub tracking: f32,
}

impl TypeRung {
    /// The line-height multiplier the spec prints beside the pixel pair,
    /// derived as `line_height / size` so the two figures can never
    /// quietly disagree.
    ///
    /// The helper is total. A rung with a non-positive
    /// [`size`](Self::size) has no meaningful multiplier, and the raw
    /// division would escape as `inf` or `NaN` — values a layout engine
    /// propagates silently into collapsed or unbounded text — so the
    /// guard returns `0.0` instead. That value is unreachable for any
    /// rung on the scale, where every ratio is at least `1.33×`, so a
    /// caller that cares can tell "no ratio" from a real one. Guarding
    /// rather than panicking keeps this usable on a render path, where a
    /// malformed hand-built rung must not take the frame down with it.
    #[must_use]
    pub const fn line_height_ratio(&self) -> f32 {
        if self.size > 0.0 {
            self.line_height / self.size
        } else {
            0.0
        }
    }
}

/// `type.caption` — 12px on a 16px line (`1.33×`).
///
/// Primary usage: shortcut hints, validation error captions, status
/// chips, menu section headings and timestamps.
///
/// The smallest rung in the scale, and therefore the one that sits
/// exactly on the [`MIN_SIZE`] floor. Its `+0.01em` is the only positive
/// tracking on the scale: at this size the glyphs need opening up rather
/// than tightening.
pub const CAPTION: TypeRung = TypeRung {
    size: 12.0,
    line_height: 16.0,
    weight: Weight::Regular,
    emphasis_weight: Weight::Medium,
    tracking: 0.01,
};

/// `type.ui_compact` — 13px on an 18px line (`1.38×`).
///
/// Primary usage: dense UI lists — directory tree rows, table of
/// contents, tab labels, menu items and table cells.
pub const UI_COMPACT: TypeRung = TypeRung {
    size: 13.0,
    line_height: 18.0,
    weight: Weight::Regular,
    emphasis_weight: Weight::Medium,
    tracking: 0.0,
};

/// `type.body` — 14px on a 20px line (`1.43×`).
///
/// Primary usage: document prose, dialog prompt text, text input values
/// and the inline editor buffer.
///
/// The scale's single-weight rung: [`weight`](TypeRung::weight) and
/// [`emphasis_weight`](TypeRung::emphasis_weight) are both
/// [`Weight::Regular`], so emphasising body text changes nothing about
/// its weight.
pub const BODY: TypeRung = TypeRung {
    size: 14.0,
    line_height: 20.0,
    weight: Weight::Regular,
    emphasis_weight: Weight::Regular,
    tracking: 0.0,
};

/// `type.title_sm` — 16px on a 24px line (`1.50×`).
///
/// Primary usage: dialog titles, the navbar brand title, movable overlay
/// headers and panel section titles.
///
/// The only rung that rests on [`Weight::SemiBold`] and emphasises to
/// [`Weight::Bold`], and the first rung to track negative.
pub const TITLE_SM: TypeRung = TypeRung {
    size: 16.0,
    line_height: 24.0,
    weight: Weight::SemiBold,
    emphasis_weight: Weight::Bold,
    tracking: -0.01,
};

/// `type.title_md` — 20px on a 28px line (`1.40×`).
///
/// Primary usage: document H2 headings and the About dialog header.
pub const TITLE_MD: TypeRung = TypeRung {
    size: 20.0,
    line_height: 28.0,
    weight: Weight::Bold,
    emphasis_weight: Weight::Bold,
    tracking: -0.015,
};

/// `type.title_lg` — 24px on a 32px line (`1.33×`).
///
/// Primary usage: document H1 headings.
///
/// The largest rung and the tightest-tracked one: at `-0.02em` the
/// letters are pulled together far enough that a heading reads as one
/// shape rather than a row of letters.
pub const TITLE_LG: TypeRung = TypeRung {
    size: 24.0,
    line_height: 32.0,
    weight: Weight::Bold,
    emphasis_weight: Weight::Bold,
    tracking: -0.02,
};

/// The absolute floor for rendered text: nothing anywhere in the
/// interface may render smaller than `12px` (`UIF-TOK-004`).
///
/// This is a rule token rather than a rung — the same role
/// [`size::HIT_TARGET_MIN`](super::size::HIT_TARGET_MIN) plays for
/// pointer targets. It is checked against a chosen size, never picked
/// as one. [`CAPTION`] sits exactly on it, which is what makes the
/// smallest rung the one that carries the rule.
pub const MIN_SIZE: f32 = 12.0;

#[cfg(test)]
#[path = "typography_tests.rs"]
mod typography_tests;
