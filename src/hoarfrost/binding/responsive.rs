//! Container-query responsive adaptation (§3.0.4.4) — a pure width solver.
//!
//! The spec's requirement is that widgets adapt to **allocated parent container
//! width** rather than global window dimensions. That distinction is the whole
//! content of the section, and it is easy to satisfy accidentally-inversely: a
//! helper that reads the window width is easier to write and produces a control
//! that goes icon-only when the *window* is narrow, even when its own toolbar has
//! 400 px to itself — or stays full-width while being crushed to nothing.
//!
//! # A global-window query is not merely avoided; it is unrepresentable
//!
//! [`ResponsiveLayout::solve`] takes exactly two inputs: the width allocated to
//! this container, and a [`Measurement`] of what the content needs. There is no
//! window-width parameter, no `Window` argument, and no ambient context object that
//! could carry one. A caller who wanted the wrong behaviour would have to add a
//! parameter to this module first, which makes the mistake visible in review
//! rather than invisible at the call site.
//!
//! # Two rungs, because the spec names one transition
//!
//! `Expanded → Compact` is the only transition §3.0.4.4 describes, so
//! [`Breakpoint`] has exactly two rungs. A third (`Medium`, `Narrow`, …) would be
//! an invented requirement with no token budget and no acceptance criteria behind
//! it. Adding one later is a spec change, not a code change.
//!
//! # What is deliberately *not* solved here
//!
//! The section bundles three consequences. This module answers the two that are
//! pure functions of width:
//!
//! - secondary labels collapse to icon-only ([`LabelMode::IconOnly`]), which
//!   then owe a hover tooltip under `UIF-ATOM-007`;
//! - horizontal padding compresses `space.md` → `space.sm`.
//!
//! It does **not** decide *which* toolbar items move into the overflow `⋯` menu.
//! That is a selection over an ordered item list with priority rules, and it
//! belongs to the `UIF-COMP-008` composite that owns the toolbar, not to a width
//! solver that has never seen the items.
//!
//! Unit tests live in the sibling `responsive_tests.rs` sidecar.

use crate::token::space;

/// Which side of the one transition §3.0.4.4 names the container is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breakpoint {
    /// Enough width for the full presentation.
    Expanded,
    /// Not enough; the compressed presentation applies.
    Compact,
}

impl Breakpoint {
    /// Whether the compressed presentation applies.
    #[must_use]
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    /// Whether the full presentation applies.
    #[must_use]
    pub const fn is_expanded(self) -> bool {
        matches!(self, Self::Expanded)
    }
}

/// What a container's content needs, at each rung.
///
/// Both numbers are produced by the caller from its own content — a measured
/// label, an icon slot, the padding rung that applies. The solver never measures
/// text and never guesses, so nothing here can drift from what the real layout
/// would need.
///
/// Inputs are sanitised on the way in: negative widths become `0.0`, non-finite
/// widths become "never fits", and `compact_width` is clamped to at most
/// `expanded_width`. The clamp matters because an inverted measurement — a
/// compact layout that needs *more* room than the expanded one, from a bug
/// upstream — would otherwise make the solver oscillate rather than settle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    expanded_width: f32,
    compact_width: f32,
}

impl Measurement {
    /// Build a measurement pair, sanitised and ordered.
    #[must_use]
    pub fn new(expanded_width: f32, compact_width: f32) -> Self {
        let expanded = sanitise(expanded_width);
        let compact = sanitise(compact_width).min(expanded);
        Self {
            expanded_width: expanded,
            compact_width: compact,
        }
    }

    /// Width needed to stay `Expanded`.
    #[must_use]
    pub const fn expanded_width(&self) -> f32 {
        self.expanded_width
    }

    /// Width needed to be `Compact` at all.
    #[must_use]
    pub const fn compact_width(&self) -> f32 {
        self.compact_width
    }

    /// Whether `allocated` room is enough for the full presentation.
    #[must_use]
    pub const fn fits_expanded(&self, allocated: f32) -> bool {
        fits(allocated, self.expanded_width)
    }

    /// Whether `allocated` room is enough for the compressed presentation.
    #[must_use]
    pub const fn fits_compact(&self, allocated: f32) -> bool {
        fits(allocated, self.compact_width)
    }
}

/// The resolved rung for one container, and what that rung implies.
///
/// A value, not a side effect: the caller asks it questions during layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponsiveLayout {
    breakpoint: Breakpoint,
}

impl ResponsiveLayout {
    /// Resolve the rung for `allocated` against `measurement`.
    ///
    /// The rule is deliberately one comparison: `Expanded` only when the full
    /// presentation genuinely fits, `Compact` otherwise. Everything ambiguous —
    /// a NaN allocation, a negative width, a measurement that never fits —
    /// resolves to the *smaller* presentation, because a control that collapses
    /// too early is degraded and one that fails to collapse is broken.
    #[must_use]
    pub fn solve(allocated: f32, measurement: &Measurement) -> Self {
        let breakpoint = if measurement.fits_expanded(allocated) {
            Breakpoint::Expanded
        } else {
            Breakpoint::Compact
        };
        Self { breakpoint }
    }

    /// The resolved rung.
    #[must_use]
    pub const fn breakpoint(self) -> Breakpoint {
        self.breakpoint
    }

    /// Whether the compressed rung applies.
    #[must_use]
    pub const fn is_compact(self) -> bool {
        self.breakpoint.is_compact()
    }

    /// §3.0.4.4 — horizontal padding: `space.md` expanded, `space.sm` compact.
    #[must_use]
    pub const fn horizontal_padding(self) -> f32 {
        match self.breakpoint {
            Breakpoint::Expanded => space::MD,
            Breakpoint::Compact => space::SM,
        }
    }

    /// §3.0.4.4 — how secondary labels are presented.
    #[must_use]
    pub const fn label_mode(self) -> LabelMode {
        match self.breakpoint {
            Breakpoint::Expanded => LabelMode::Full,
            Breakpoint::Compact => LabelMode::IconOnly,
        }
    }

    /// §3.0.4.4 — the vertical layout shift caused by crossing the rung.
    ///
    /// Constant `0.0` by design, and constant *structurally*: there is no field
    /// it could read and no branch that could return anything else. The spec fixes
    /// the answer outright ("0px vertical layout shift"), so a method that could
    /// compute a non-zero value would be a place for that guarantee to leak.
    /// Every rung returns the same number, and the test asserts exactly that.
    #[must_use]
    pub const fn vertical_shift(self) -> f32 {
        0.0
    }
}

/// How a secondary label is being shown at the current rung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelMode {
    /// The label is visible.
    Full,
    /// Icon only — and therefore owing a hover tooltip under `UIF-ATOM-007`.
    IconOnly,
}

impl LabelMode {
    /// Whether the label text itself is being shown.
    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Whether a tooltip is mandatory for this presentation (`UIF-ATOM-007`).
    #[must_use]
    pub const fn requires_tooltip(self) -> bool {
        matches!(self, Self::IconOnly)
    }
}

/// A width is usable only if it is finite and non-negative; anything else means
/// "this never fits", which fails closed to the smaller presentation.
const fn sanitise(width: f32) -> f32 {
    if width.is_finite() {
        if width < 0.0 { 0.0 } else { width }
    } else {
        f32::INFINITY
    }
}

/// Whether `allocated` covers `needed`, treating a non-finite allocation as
/// covering nothing.
const fn fits(allocated: f32, needed: f32) -> bool {
    allocated.is_finite() && allocated >= needed
}

#[cfg(test)]
#[path = "responsive_tests.rs"]
mod responsive_tests;
