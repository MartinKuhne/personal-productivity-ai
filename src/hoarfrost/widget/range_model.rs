//! The `RangeModel` shared by `Slider` and `NumberBox` (§3.5,
//! `UIF-ATOM-005`).
//!
//! §3.5 does not give the two controls two models. It gives them **one**
//! — `RangeModel { value, min, max, step, precision, unit_suffix,
//! validation, disabled }` — and that single line is the whole reason this
//! module exists as its own file rather than as a section of each atom. A
//! slider and a number box are the same interval read two ways: one by
//! position, one by digits. Every rule about *where the ends are* and
//! *what counts as a step* therefore belongs somewhere neither control can
//! get it wrong independently, and this is that somewhere.
//!
//! The model is data plus pure arithmetic. It never emits a tier, never
//! touches a pointer and never reads a clock; the tiers live in
//! [`slider_host`](crate::widget::slider_host) and
//! [`number_box_host`](crate::widget::number_box_host), which ask this
//! type what a gesture means and then report it.
//!
//! # Nothing here mutates
//!
//! Every transition takes `&self` and returns a value, following the
//! [`toggle_model`](crate::widget::toggle_model) rule that a model which
//! rewrites itself cannot be snapshotted, replayed or cancelled. The
//! pre-drag value a [`Cancel`](crate::binding::Cancel) hands back is a
//! copy of this struct taken by the host, which is only sound because a
//! copy of a `RangeModel` is a complete, independent value.
//!
//! # Finiteness is checked before the clamp, not after
//!
//! `f64::clamp` passes `NaN` straight through — it is implemented as a
//! pair of comparisons, and `NaN` fails both, so `NaN.clamp(0.0, 10.0)`
//! is `NaN`. A slider that let that through would compute a `NaN`
//! fraction, a `NaN` pixel position and a quad with no bounds, which the
//! renderer propagates rather than rejects. [`RangeModel::clamp`]
//! therefore tests finiteness **first** and only then narrows, the same
//! trap [`ActionPending::determinate`](crate::binding::pending::ActionPending::determinate)
//! documents.
//!
//! # The four edge policies §3.5 does not state
//!
//! §3.5 names the fields and the callbacks and stops. Four cases have to
//! resolve somewhere, and each is recorded rather than guessed silently:
//!
//! | Case | Policy | Why |
//! |---|---|---|
//! | `min > max` | **normalise by swapping** | the host evidently meant that interval; freezing the control would discard the whole range to honour a mistake |
//! | a non-finite bound | the range collapses onto the bound that *is* finite; if neither is, onto `0.0` | there is no interval to present, and a degenerate range is one the control can still draw without lying |
//! | `step == 0` | read as `None` — continuous | a zero distance quantises nothing; `Some(0.0)` and `None` mean the same thing, and a frozen control would be indistinguishable from a disabled one |
//! | `step < 0` | read as its **magnitude** | a step is a distance; the sign is a transcription error, not a request to invert the axis |
//!
//! And one consequence worth stating out loud: with no effective step,
//! `←`/`→`/`PageUp`/`PageDown` do **nothing**. §3.5 defines every
//! keyboard move as a multiple of `step`, and there is no multiple of
//! nothing. `Home` and `End` still work, because they name the ends
//! rather than a distance.
//!
//! # Stepping does not drift
//!
//! A step is taken by re-deriving the grid index from the range's low end
//! — `round((value - low) / step)` — and rebuilding the value as
//! `low + (index + delta) * step`. It is *not* `value + delta * step`
//! accumulated across presses. Accumulating spends a rounding error on
//! every press and a thousand presses later lands off the grid, which for
//! a `step = 0.1` control means the box showing `8.700000000000001`.
//! Re-deriving from `low` means the hundredth press is as exact as the
//! first, and the sidecar walks a thousand of them to prove it.
//!
//! Unit tests live in the sibling `range_model_tests.rs` sidecar.

use std::fmt;

/// The keyboard moves §3.5 assigns to a range control.
///
/// Six keys and no modifier variants. §3.5 prints `←`/`↓` as `-step`,
/// `→`/`↑` as `+step`, `PageDown`/`PageUp` as `±10 × step`, `Home` as
/// `min` and `End` as `max`, and prints no `Shift` increment, so none is
/// invented here. The two arrow pairs collapse into one direction each
/// because the spec gives them the same delta: a control does not need to
/// know which of its two arrow spellings arrived, only which way to move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RangeKey {
    /// `←` or `↓` — one step down.
    Decrease,
    /// `→` or `↑` — one step up.
    Increase,
    /// `PageDown` — ten steps down.
    PageDown,
    /// `PageUp` — ten steps up.
    PageUp,
    /// `Home` — the low end of the range.
    Home,
    /// `End` — the high end of the range.
    End,
}

/// How many steps a `PageUp` / `PageDown` travels — §3.5's `±10 × step`.
pub const PAGE_STEP_MULTIPLIER: i64 = 10;

/// The interval both ends of a range fall back to when neither bound is
/// finite.
///
/// **Not a spec figure and not a token.** §3.5 cannot be read to say what
/// `min = NaN, max = NaN` means, and *some* interval has to come back or
/// every downstream division becomes a `NaN`. Zero is the one value a
/// range with no ends agrees on: it is the identity of the arithmetic
/// that follows, so nothing downstream is perturbed by it.
const NO_RANGE_AT: f64 = 0.0;

/// The decimal places a model with no stated `precision` renders at.
///
/// §3.5 gives `precision: u8` and no default. Two is the smallest count
/// that keeps a continuous `f64` readable — one decimal hides a quarter
/// of a unit, three starts printing the noise in the binary
/// representation — so it is the least-lossy guess, and it is written
/// down here rather than buried in the formatter.
const DEFAULT_PRECISION: u8 = 2;

/// The interval one range control presents, in every state §3.5 gives it.
///
/// The field set is §3.5's line verbatim. Note what is *not* here: no
/// `id`, no `label`, no binding to a second control. §3.5 pairs the
/// `Slider` and the `NumberBox` by giving them the same model, but it
/// never says what happens when both are on screen at once and one is
/// dragged — whether the other follows live, follows at commit, or is a
/// different control entirely. **That rule does not exist in the spec**,
/// and this crate does not invent one: a host that wants a linked pair
/// owns the link, and the gap is recorded in
/// [`spec_coverage`](crate::spec_coverage).
#[derive(Clone, Debug, PartialEq)]
pub struct RangeModel {
    value: f64,
    min: f64,
    max: f64,
    step: Option<f64>,
    precision: u8,
    unit_suffix: Option<String>,
    validation: Option<String>,
    disabled: bool,
}

impl RangeModel {
    /// A model over `[min, max]` carrying `value`.
    ///
    /// The constructor stores its arguments as given rather than
    /// normalising them: [`min`](Self::min) and [`max`](Self::max) keep
    /// answering with what the host declared, and the ordering is applied
    /// at read time by [`ordered_bounds`](Self::ordered_bounds). A model
    /// that silently rewrote the host's `min` and `max` on construction
    /// could not be compared back against the host's own data, and the
    /// inversion — the fact worth seeing — would be erased.
    #[must_use]
    pub fn new(value: f64, min: f64, max: f64) -> Self {
        Self {
            value,
            min,
            max,
            step: None,
            precision: DEFAULT_PRECISION,
            unit_suffix: None,
            validation: None,
            disabled: false,
        }
    }

    /// Set the step — `None` for a continuous control.
    #[must_use]
    pub fn with_step(mut self, step: Option<f64>) -> Self {
        self.step = step;
        self
    }

    /// Set the decimal places [`display`](Self::display) renders at.
    #[must_use]
    pub fn with_precision(mut self, precision: u8) -> Self {
        self.precision = precision;
        self
    }

    /// Set the string appended to the rendered value (`"px"`, `"%"`).
    #[must_use]
    pub fn with_unit_suffix(mut self, unit: impl Into<String>) -> Self {
        self.unit_suffix = Some(unit.into());
        self
    }

    /// Set the host's validation words.
    ///
    /// The model carries these and reads nothing off them. §3B.1 renders
    /// them in the field's reserved caption slot rather than on the
    /// control, which is why neither the slider nor the number box has an
    /// invalid paint keyed to this field.
    #[must_use]
    pub fn with_validation(mut self, message: impl Into<String>) -> Self {
        self.validation = Some(message.into());
        self
    }

    /// Mark the control disabled.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The value as stored. See [`clamped_value`](Self::clamped_value) for
    /// the value the control actually paints.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// The declared lower bound, exactly as the host passed it.
    #[must_use]
    pub const fn min(&self) -> f64 {
        self.min
    }

    /// The declared upper bound, exactly as the host passed it.
    #[must_use]
    pub const fn max(&self) -> f64 {
        self.max
    }

    /// The declared step, before [`effective_step`](Self::effective_step)
    /// decides whether it is a step at all.
    #[must_use]
    pub const fn step(&self) -> Option<f64> {
        self.step
    }

    /// The decimal places the value renders at.
    #[must_use]
    pub const fn precision(&self) -> u8 {
        self.precision
    }

    /// The rendered unit suffix, if any.
    #[must_use]
    pub fn unit_suffix(&self) -> Option<&str> {
        self.unit_suffix.as_deref()
    }

    /// The host's validation words, if any.
    #[must_use]
    pub fn validation(&self) -> Option<&str> {
        self.validation.as_deref()
    }

    /// Whether the control is disabled.
    #[must_use]
    pub const fn disabled_flag(&self) -> bool {
        self.disabled
    }

    // -- the interval ---------------------------------------------------------

    /// The range as the control presents it: finite, and with the lower
    /// bound first.
    ///
    /// This is the only place the inverted-range and non-finite-bound
    /// policies are written, so both controls read the same interval and
    /// cannot disagree about where the ends are.
    ///
    /// ```text
    ///   min 0   max 100   -> (0,   100)
    ///   min 100 max 0     -> (0,   100)   swapped, not frozen
    ///   min NaN max 10    -> (10,  10)    degenerate at the finite end
    ///   min NaN max NaN   -> (0,   0)     no interval at all
    /// ```
    #[must_use]
    pub fn ordered_bounds(&self) -> (f64, f64) {
        match (self.min.is_finite(), self.max.is_finite()) {
            (true, true) => (self.min.min(self.max), self.min.max(self.max)),
            (true, false) => (self.min, self.min),
            (false, true) => (self.max, self.max),
            (false, false) => (NO_RANGE_AT, NO_RANGE_AT),
        }
    }

    /// The low end of the presented range.
    #[must_use]
    pub fn low(&self) -> f64 {
        self.ordered_bounds().0
    }

    /// The high end of the presented range.
    #[must_use]
    pub fn high(&self) -> f64 {
        self.ordered_bounds().1
    }

    /// Whether the host declared the bounds the wrong way round.
    ///
    /// Reported rather than hidden: a host that can see this can fix the
    /// data it came from, even though the control copes.
    #[must_use]
    pub fn is_inverted(&self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.min > self.max
    }

    /// Whether the range has no width and so cannot be travelled.
    #[must_use]
    pub fn is_degenerate(&self) -> bool {
        let (low, high) = self.ordered_bounds();
        high <= low
    }

    // -- clamping and snapping ------------------------------------------------

    /// Narrow `value` into the presented range.
    ///
    /// Finiteness is tested **before** the clamp. `f64::clamp` is a pair
    /// of comparisons and `NaN` fails both, so clamping a `NaN` returns
    /// the `NaN` unchanged and every pixel derived from it is `NaN` too.
    /// A non-finite value is not a value, so it is sent to the low end —
    /// the left end of the track, where a control with nothing to show
    /// rests.
    #[must_use]
    pub fn clamp(&self, value: f64) -> f64 {
        if !value.is_finite() {
            return self.low();
        }
        let (low, high) = self.ordered_bounds();
        value.clamp(low, high)
    }

    /// The value the control paints: the stored value, clamped.
    #[must_use]
    pub fn clamped_value(&self) -> f64 {
        self.clamp(self.value)
    }

    /// The step the model actually quantises to, or `None` for a
    /// continuous control.
    ///
    /// `Some(0.0)` and `Some(f64::NAN)` both come back `None`: neither
    /// can quantise anything, and `None` is already the model's word for
    /// "continuous", so inventing a third answer would be a third state
    /// nothing downstream can distinguish. A negative step comes back as
    /// its magnitude — see the [module docs](self) for why the sign is
    /// read as an error rather than as an inverted axis.
    #[must_use]
    pub fn effective_step(&self) -> Option<f64> {
        self.step
            .filter(|step| step.is_finite() && *step != 0.0)
            .map(f64::abs)
    }

    /// Snap `value` onto the step grid, then clamp it.
    ///
    /// Snapping runs on the clamped value so a value outside the range
    /// cannot snap to a grid point that is itself outside — with
    /// `min = 0`, `max = 10`, `step = 3` the grid is `0, 3, 6, 9` and
    /// `10` snaps down to `9`, never up to `12`.
    #[must_use]
    pub fn snap(&self, value: f64) -> f64 {
        let clamped = self.clamp(value);
        match self.effective_step() {
            Some(step) => {
                let (low, _) = self.ordered_bounds();
                self.clamp(low + grid_index(clamped, low, step) * step)
            }
            None => clamped,
        }
    }

    /// Move `value` by `delta` steps — the whole of the keyboard contract,
    /// and the stepper's, in one function.
    ///
    /// `None` when the model has no effective step: §3.5 defines every
    /// keyed move as a multiple of `step`, and a continuous model has no
    /// multiple to take. Returning `Some(value)` unchanged would let a
    /// caller emit a `Commit` that changed nothing and could not be told
    /// from one that did, so the absence is the answer.
    ///
    /// The result is rebuilt from the range's low end rather than added to
    /// `value`, which is what keeps a long run of presses on the grid.
    #[must_use]
    pub fn step_by(&self, value: f64, delta: i64) -> Option<f64> {
        let step = self.effective_step()?;
        let (low, _) = self.ordered_bounds();
        let index = grid_index(self.clamp(value), low, step) + delta as f64;
        Some(self.clamp(low + index * step))
    }

    /// Apply one keyboard key, returning the value it asks for.
    ///
    /// `Home` and `End` name the ends and so work on a continuous model
    /// as well as a stepped one; they return the declared bounds exactly
    /// rather than the nearest grid point, because §3.5 says `Home = min`
    /// and not `Home = the first step`.
    #[must_use]
    pub fn apply_key(&self, key: RangeKey) -> Option<f64> {
        match key {
            RangeKey::Home => Some(self.low()),
            RangeKey::End => Some(self.high()),
            RangeKey::Decrease => self.step_by(self.clamped_value(), -1),
            RangeKey::Increase => self.step_by(self.clamped_value(), 1),
            RangeKey::PageDown => self.step_by(self.clamped_value(), -PAGE_STEP_MULTIPLIER),
            RangeKey::PageUp => self.step_by(self.clamped_value(), PAGE_STEP_MULTIPLIER),
        }
    }

    // -- the position mapping -------------------------------------------------

    /// Where the value sits along the range, in `0.0..=1.0`.
    ///
    /// A degenerate range reads `0.0` rather than dividing by zero. The
    /// control is showing one point, and `0.0` is where the thumb sits
    /// when there is nowhere to go.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        let (low, high) = self.ordered_bounds();
        if high <= low {
            return 0.0;
        }
        let ratio = (self.clamped_value() - low) / (high - low);
        (ratio as f32).clamp(0.0, 1.0)
    }

    /// The value at a position along the range, snapped to the grid.
    ///
    /// A non-finite fraction reads the low end, for the same reason
    /// [`clamp`](Self::clamp) does: a pointer position that is not a
    /// number is not a request to move anywhere.
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

    // -- rendering ------------------------------------------------------------

    /// The value as the `NumberBox` shows it: fixed decimals, unit suffix.
    ///
    /// Always finite and never empty. The formatter is handed
    /// [`clamped_value`](Self::clamped_value), so a stored `NaN` renders
    /// as the low end rather than as the word `NaN` in a numeric field.
    ///
    /// No separator is inserted before the suffix: §3.5 types the field as
    /// `unit_suffix: Option<String>`, so the host that owns the string owns
    /// whether it reads `"px"` or `" px"`, and a formatter that added its
    /// own space could not be undone from outside.
    #[must_use]
    pub fn display(&self) -> String {
        format!(
            "{:.*}{}",
            usize::from(self.precision),
            self.clamped_value(),
            self.unit_suffix().unwrap_or_default()
        )
    }

    /// Read user-entered text back into a value.
    ///
    /// Surrounding whitespace and the model's own unit suffix are
    /// stripped before parsing, so a field that displayed `12.5px` and was
    /// re-edited in place still parses. `"nan"` and `"inf"` are rejected
    /// rather than accepted by [`str::parse`] and propagated: they are
    /// words the user can type, and a numeric field that takes them is a
    /// numeric field that lies.
    ///
    /// A parsed value is snapped and clamped on the way out, so typing
    /// `7` into a `step = 5` field yields `5`, not `7` — the field shows
    /// what the control can hold.
    #[must_use]
    pub fn parse(&self, text: &str) -> Option<f64> {
        let mut trimmed = text.trim();
        if let Some(unit) = self.unit_suffix().filter(|unit| !unit.is_empty()) {
            trimmed = trimmed.strip_suffix(unit).unwrap_or(trimmed);
        }
        let trimmed = trimmed.trim();
        if trimmed.is_empty() {
            return None;
        }
        let parsed: f64 = trimmed.parse().ok()?;
        if !parsed.is_finite() {
            return None;
        }
        Some(self.snap(parsed))
    }
}

impl fmt::Display for RangeModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

/// The grid index `value` sits nearest to, measured from `low`.
///
/// The rounding happens here and nowhere else so the "nearest grid point"
/// rule is one line with one definition. Rounding rather than truncating
/// matters: truncation would pull `2.9` down to `0` on a `step = 3` grid
/// when the nearest point is `3`.
fn grid_index(value: f64, low: f64, step: f64) -> f64 {
    ((value - low) / step).round()
}

#[cfg(test)]
#[path = "range_model_tests.rs"]
mod range_model_tests;
