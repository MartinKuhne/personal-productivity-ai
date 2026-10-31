//! Data model and invariant enforcement for the dual-thumb `RangeSlider` (§3.5, `UIF-ATOM-005b`, `UIF-ATOM-005`).
//!
//! §3.5 specifies the dual-thumb `RangeSlider` as a continuous or stepped interval
//! control over `[min, max]`, bounded by `lower` and `upper` thumbs with a non-crossing
//! invariant `lower <= upper - min_span`.
//!
//! Stepping is drift-free: indices are re-derived from `min` on every step rather than
//! accumulating floating-point deltas. Finiteness of bounds and values is checked before
//! clamping, ensuring no `NaN` leaks into layout or rendering.
//!
//! Unit tests live in the sibling `range_slider_model_tests.rs` sidecar.

use crate::widget::range_model::RangeKey;

/// Default precision for numeric rendering when none is stated (§3.5, `UIF-ATOM-005b`).
const DEFAULT_PRECISION: u8 = 2;

/// Identity fallback when bounds are non-finite (§3.5, `UIF-ATOM-005b`).
const NO_RANGE_AT: f64 = 0.0;

/// Identifier for one of the two thumbs on a dual-thumb range slider (§3.5, `UIF-ATOM-005b`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RangeThumb {
    /// The lower (left / start) bound thumb (`UIF-ATOM-005b`).
    Lower,
    /// The upper (right / end) bound thumb (`UIF-ATOM-005b`).
    Upper,
}

/// Declarative data model for `RangeSlider` (§3.5, `UIF-ATOM-005b`, `UIF-ATOM-005`).
#[derive(Clone, Debug, PartialEq)]
pub struct RangeSliderModel {
    /// Current lower value (`UIF-ATOM-005b`).
    pub lower: f64,
    /// Current upper value (`UIF-ATOM-005b`).
    pub upper: f64,
    /// Minimum allowed bound (`UIF-ATOM-005b`).
    pub min: f64,
    /// Maximum allowed bound (`UIF-ATOM-005b`).
    pub max: f64,
    /// Minimum span enforced between `lower` and `upper` (`UIF-ATOM-005b`).
    pub min_span: f64,
    /// Discrete step size, or `None` for continuous movement (`UIF-ATOM-005b`).
    pub step: Option<f64>,
    /// Number of decimal digits rendered in readouts (`UIF-ATOM-005b`).
    pub precision: u8,
    /// Optional suffix appended to formatted values (`UIF-ATOM-005b`).
    pub unit_suffix: Option<String>,
    /// Optional validation message (`UIF-ATOM-005b`).
    pub validation: Option<String>,
    /// Whether the control is inert and rejects user interaction (`UIF-ATOM-005b`).
    pub disabled: bool,
}

impl RangeSliderModel {
    /// Create a new `RangeSliderModel` over `[min, max]` with initial `lower` and `upper` values (`UIF-ATOM-005b`).
    #[must_use]
    pub fn new(min: f64, max: f64, lower: f64, upper: f64) -> Self {
        Self {
            lower,
            upper,
            min,
            max,
            min_span: 0.0,
            step: None,
            precision: DEFAULT_PRECISION,
            unit_suffix: None,
            validation: None,
            disabled: false,
        }
    }

    /// Set the minimum span required between `lower` and `upper` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_min_span(mut self, min_span: f64) -> Self {
        self.min_span = min_span;
        self
    }

    /// Set discrete step interval, or `None` for continuous movement (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_step(mut self, step: Option<f64>) -> Self {
        self.step = step;
        self
    }

    /// Set decimal places for formatted readout display (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_precision(mut self, precision: u8) -> Self {
        self.precision = precision;
        self
    }

    /// Set unit suffix string (`"px"`, `"%"` etc.) (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_unit_suffix(mut self, unit: impl Into<String>) -> Self {
        self.unit_suffix = Some(unit.into());
        self
    }

    /// Set validation message for the control (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_validation(mut self, message: impl Into<String>) -> Self {
        self.validation = Some(message.into());
        self
    }

    /// Set whether the control is disabled (`UIF-ATOM-005b`).
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Declared minimum bound as passed (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn min(&self) -> f64 {
        self.min
    }

    /// Declared maximum bound as passed (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn max(&self) -> f64 {
        self.max
    }

    /// Declared lower value as passed (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn lower(&self) -> f64 {
        self.lower
    }

    /// Declared upper value as passed (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn upper(&self) -> f64 {
        self.upper
    }

    /// Declared minimum span between thumbs (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn min_span(&self) -> f64 {
        self.min_span
    }

    /// Declared step interval (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn step(&self) -> Option<f64> {
        self.step
    }

    /// Readout decimal precision (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn precision(&self) -> u8 {
        self.precision
    }

    /// Unit suffix if present (`UIF-ATOM-005b`).
    #[must_use]
    pub fn unit_suffix(&self) -> Option<&str> {
        self.unit_suffix.as_deref()
    }

    /// Validation message if present (`UIF-ATOM-005b`).
    #[must_use]
    pub fn validation(&self) -> Option<&str> {
        self.validation.as_deref()
    }

    /// Whether the control is marked disabled (`UIF-ATOM-005b`).
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        self.disabled
    }

    // -- Normalization & bounds -----------------------------------------------

    /// Ordered, finite bounds `(low, high)` where `low <= high` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn ordered_bounds(&self) -> (f64, f64) {
        match (self.min.is_finite(), self.max.is_finite()) {
            (true, true) => (self.min.min(self.max), self.min.max(self.max)),
            (true, false) => (self.min, self.min),
            (false, true) => (self.max, self.max),
            (false, false) => (NO_RANGE_AT, NO_RANGE_AT),
        }
    }

    /// Low end of the normalized bounds (`UIF-ATOM-005b`).
    #[must_use]
    pub fn low(&self) -> f64 {
        self.ordered_bounds().0
    }

    /// High end of the normalized bounds (`UIF-ATOM-005b`).
    #[must_use]
    pub fn high(&self) -> f64 {
        self.ordered_bounds().1
    }

    /// Effective minimum span, clamped to the available total interval (`UIF-ATOM-005b`).
    #[must_use]
    pub fn effective_min_span(&self) -> f64 {
        let (low, high) = self.ordered_bounds();
        let max_span = (high - low).max(0.0);
        if self.min_span.is_finite() && self.min_span > 0.0 {
            self.min_span.min(max_span)
        } else {
            0.0
        }
    }

    /// Effective step size, returning `None` if zero, non-finite or negative magnitude (`UIF-ATOM-005b`).
    #[must_use]
    pub fn effective_step(&self) -> Option<f64> {
        self.step
            .filter(|step| step.is_finite() && *step != 0.0)
            .map(f64::abs)
    }

    // -- Clamping & non-crossing invariant ------------------------------------

    /// Clamp a proposed lower value to `[min, upper - min_span]` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn clamp_lower(&self, val: f64) -> f64 {
        let (low, high) = self.ordered_bounds();
        let span = self.effective_min_span();
        let eff_upper = if self.upper.is_finite() {
            self.upper.clamp(low + span, high)
        } else {
            high
        };
        let max_lower = (eff_upper - span).max(low);
        if !val.is_finite() {
            return low;
        }
        val.clamp(low, max_lower)
    }

    /// Clamp a proposed upper value to `[lower + min_span, max]` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn clamp_upper(&self, val: f64) -> f64 {
        let (low, high) = self.ordered_bounds();
        let span = self.effective_min_span();
        let eff_lower = if self.lower.is_finite() {
            self.lower.clamp(low, high - span)
        } else {
            low
        };
        let min_upper = (eff_lower + span).min(high);
        if !val.is_finite() {
            return high;
        }
        val.clamp(min_upper, high)
    }

    /// The clamped lower value obeying bounds and current upper value (`UIF-ATOM-005b`).
    #[must_use]
    pub fn clamped_lower(&self) -> f64 {
        self.clamp_lower(self.lower)
    }

    /// The clamped upper value obeying bounds and current lower value (`UIF-ATOM-005b`).
    #[must_use]
    pub fn clamped_upper(&self) -> f64 {
        self.clamp_upper(self.upper)
    }

    /// Clamped `(lower, upper)` pair guaranteeing `lower <= upper - min_span` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn clamped_range(&self) -> (f64, f64) {
        let (low, high) = self.ordered_bounds();
        let span = self.effective_min_span();
        let raw_l = if self.lower.is_finite() {
            self.lower
        } else {
            low
        };
        let raw_u = if self.upper.is_finite() {
            self.upper
        } else {
            high
        };

        // Normalize if raw lower exceeds raw upper
        let (l, u) = if raw_l <= raw_u {
            (raw_l, raw_u)
        } else {
            (raw_u, raw_l)
        };

        let clamped_l = l.clamp(low, high - span);
        let clamped_u = u.clamp(clamped_l + span, high);
        (clamped_l, clamped_u)
    }

    // -- Snapping & fractions -------------------------------------------------

    /// Snap `value` onto the step grid if step is defined, clamped to bounds (`UIF-ATOM-005b`).
    #[must_use]
    pub fn snap(&self, value: f64) -> f64 {
        let (low, high) = self.ordered_bounds();
        if !value.is_finite() {
            return low;
        }
        let clamped = value.clamp(low, high);
        match self.effective_step() {
            Some(step) => {
                let index = ((clamped - low) / step).round();
                (low + index * step).clamp(low, high)
            }
            None => clamped,
        }
    }

    /// Lower thumb position along the track in `0.0..=1.0` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn lower_fraction(&self) -> f32 {
        let (low, high) = self.ordered_bounds();
        if high <= low {
            return 0.0;
        }
        let ratio = (self.clamped_lower() - low) / (high - low);
        (ratio as f32).clamp(0.0, 1.0)
    }

    /// Upper thumb position along the track in `0.0..=1.0` (`UIF-ATOM-005b`).
    #[must_use]
    pub fn upper_fraction(&self) -> f32 {
        let (low, high) = self.ordered_bounds();
        if high <= low {
            return 1.0;
        }
        let ratio = (self.clamped_upper() - low) / (high - low);
        (ratio as f32).clamp(0.0, 1.0)
    }

    /// Map a track fraction in `0.0..=1.0` to a value snapped to grid (`UIF-ATOM-005b`).
    #[must_use]
    pub fn value_at_fraction(&self, fraction: f32) -> f64 {
        let (low, high) = self.ordered_bounds();
        if high <= low {
            return low;
        }
        if !fraction.is_finite() {
            return self.snap(low);
        }
        let t = f64::from(fraction.clamp(0.0, 1.0));
        self.snap(low + t * (high - low))
    }

    // -- Endpoint disambiguation ----------------------------------------------

    /// Disambiguate which thumb a pointer click targets (`UIF-ATOM-005b`).
    ///
    /// When `lower` and `upper` coincide or are separated by `min_span`:
    /// - If `click_fraction < current_fraction` (click is to the left), selects [`RangeThumb::Lower`].
    /// - If `click_fraction > current_fraction` (click is to the right), selects [`RangeThumb::Upper`].
    /// - If exactly on it, picks whichever thumb has room to move in that direction.
    ///
    /// When thumbs are separated, selects whichever thumb is nearest to the click.
    #[must_use]
    pub fn disambiguate_thumb(&self, click_fraction: f64) -> RangeThumb {
        let click_f = if click_fraction.is_finite() {
            click_fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };

        let (low, high) = self.ordered_bounds();
        let (lower_val, upper_val) = self.clamped_range();
        let lower_f = if high > low {
            (lower_val - low) / (high - low)
        } else {
            0.0
        };
        let upper_f = if high > low {
            (upper_val - low) / (high - low)
        } else {
            1.0
        };
        let min_span_f = if high > low {
            self.effective_min_span() / (high - low)
        } else {
            0.0
        };

        // Check if thumbs coincide or are at min_span
        let are_coincident_or_at_span = (upper_f - lower_f) <= (min_span_f + 1e-6);

        if are_coincident_or_at_span {
            let current_f = (lower_f + upper_f) / 2.0;
            if click_f < current_f {
                RangeThumb::Lower
            } else if click_f > current_f {
                RangeThumb::Upper
            } else {
                // Exactly on it: pick the one with room to move in that direction
                if upper_val < high {
                    RangeThumb::Upper
                } else {
                    RangeThumb::Lower
                }
            }
        } else {
            let d_lower = (click_f - lower_f).abs();
            let d_upper = (click_f - upper_f).abs();
            if d_lower <= d_upper {
                RangeThumb::Lower
            } else {
                RangeThumb::Upper
            }
        }
    }

    // -- Drift-free keyboard stepping -----------------------------------------

    /// Step a chosen thumb using keyboard controls without cumulative drift (`UIF-ATOM-005b`, `UIF-ATOM-005`).
    ///
    /// Re-derives the grid index from `min` using `round((val - min) / step)` + delta,
    /// clamped to the non-crossing range:
    /// - `Decrease` / `Increase` = ±1 step
    /// - `PageDown` / `PageUp` = ±10 steps
    /// - `Home` = `min` (for Lower) / `lower + min_span` (for Upper)
    /// - `End` = `upper - min_span` (for Lower) / `max` (for Upper)
    #[must_use]
    pub fn step_thumb(&self, thumb: RangeThumb, key: RangeKey) -> (f64, f64) {
        let (low, high) = self.ordered_bounds();
        let min_span = self.effective_min_span();
        let (clamped_l, clamped_u) = self.clamped_range();

        match thumb {
            RangeThumb::Lower => {
                let max_l = (clamped_u - min_span).max(low);
                let new_l = match key {
                    RangeKey::Home => low,
                    RangeKey::End => max_l,
                    RangeKey::Decrease
                    | RangeKey::Increase
                    | RangeKey::PageDown
                    | RangeKey::PageUp => {
                        if let Some(step) = self.effective_step() {
                            let delta = match key {
                                RangeKey::Decrease => -1.0,
                                RangeKey::Increase => 1.0,
                                RangeKey::PageDown => -10.0,
                                RangeKey::PageUp => 10.0,
                                _ => 0.0,
                            };
                            let idx = ((clamped_l - low) / step).round() + delta;
                            (low + idx * step).clamp(low, max_l)
                        } else {
                            clamped_l
                        }
                    }
                };
                (new_l, clamped_u)
            }
            RangeThumb::Upper => {
                let min_u = (clamped_l + min_span).min(high);
                let new_u = match key {
                    RangeKey::Home => min_u,
                    RangeKey::End => high,
                    RangeKey::Decrease
                    | RangeKey::Increase
                    | RangeKey::PageDown
                    | RangeKey::PageUp => {
                        if let Some(step) = self.effective_step() {
                            let delta = match key {
                                RangeKey::Decrease => -1.0,
                                RangeKey::Increase => 1.0,
                                RangeKey::PageDown => -10.0,
                                RangeKey::PageUp => 10.0,
                                _ => 0.0,
                            };
                            let idx = ((clamped_u - low) / step).round() + delta;
                            (low + idx * step).clamp(min_u, high)
                        } else {
                            clamped_u
                        }
                    }
                };
                (clamped_l, new_u)
            }
        }
    }

    // -- Readout formatting ---------------------------------------------------

    /// Format a numeric value using model precision and unit suffix (`UIF-ATOM-005b`).
    #[must_use]
    pub fn format_value(&self, val: f64) -> String {
        format!(
            "{:.*}{}",
            usize::from(self.precision),
            val,
            self.unit_suffix().unwrap_or_default()
        )
    }

    /// Formatted string for current clamped lower value (`UIF-ATOM-005b`).
    #[must_use]
    pub fn display_lower(&self) -> String {
        self.format_value(self.clamped_lower())
    }

    /// Formatted string for current clamped upper value (`UIF-ATOM-005b`).
    #[must_use]
    pub fn display_upper(&self) -> String {
        self.format_value(self.clamped_upper())
    }

    /// Formatted `(lower, upper)` strings (`UIF-ATOM-005b`).
    #[must_use]
    pub fn display(&self) -> (String, String) {
        (self.display_lower(), self.display_upper())
    }
}

#[cfg(test)]
#[path = "range_slider_model_tests.rs"]
mod range_slider_model_tests;
