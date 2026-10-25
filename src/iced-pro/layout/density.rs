//! Row density homogeneity (`UIF-PRIN-006`, §1.1 pillar 6).
//!
//! §1.1 names two rungs — `Compact = 28px` and `Standard = 32px` — and
//! then forbids mixing them inside one horizontal band. Before this
//! module the two rungs were two unrelated constants
//! ([`size::CONTROL_SM`], [`size::CONTROL_MD`]), which means
//! nothing in the crate could express the sentence the rule is written
//! in: *"this band mixes 28px with 32px"*. [`ControlDensity`] is that
//! sentence's subject, and [`check_row_density`] is its verb.
//!
//! # The rung is derived, never restated
//!
//! [`ControlDensity::height`] returns the existing size tokens. There is
//! no `28.0` and no `32.0` in this file, because a transcribed rung is a
//! second place for the number to be wrong
//! ([`UIF-PRIN-001`](crate::token)). A rung is a *name for a token pair*,
//! not a measurement of its own — which is also why this module lives in
//! `layout/` rather than `token/`: it is a rule about tokens.
//!
//! # Two ways a control can be wrong, and why both are reported
//!
//! A band declares one rung. A control placed in it can then fail in two
//! distinct ways, and collapsing them would lose the difference a fixer
//! needs:
//!
//! * **Wrong rung** — the control is a real control at the *other* rung.
//!   The fix is to move it to the band's rung.
//! * **Off rung** — the control's height is neither 28 nor 32. It is not
//!   a control the density ladder recognises, so no rung it is compared
//!   against will ever be the right one. The fix is upstream, in whatever
//!   chose that height.
//!
//! [`DensityViolation::is_off_rung`] separates them.
//!
//! # Why the spread is reported alongside the violations
//!
//! [`DensityReport::rung_spread`] lists the rungs actually present among
//! the slots, independent of the declaration. A row declared `Standard`
//! whose controls are all `Compact` produces violations *and* a spread of
//! `[Compact]`: the row is internally consistent and still wrong, and a
//! reader who sees only "3 violations" cannot tell those two situations
//! apart.
//!
//! Unit tests live in the sibling `density_tests.rs` sidecar.

use crate::token::size;

/// The two control-density rungs §1.1 pillar 6 admits
/// (`UIF-PRIN-006`).
///
/// Exactly two, because the spec's clause is closed: *"Every control in a
/// single horizontal row MUST share the same `ControlDensity`
/// (`Compact = 28px` or `Standard = 32px`)"*. A third rung is not a
/// density this framework can declare, and a control that wants one is
/// off the ladder rather than on a new step — see
/// [`DensityViolation::is_off_rung`](DensityViolation::is_off_rung).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ControlDensity {
    /// The dense rung — [`size::CONTROL_SM`].
    Compact,
    /// The roomy rung — [`size::CONTROL_MD`].
    Standard,
}

impl ControlDensity {
    /// Both rungs, in ascending height order.
    pub const ALL: [Self; 2] = [Self::Compact, Self::Standard];

    /// The height this rung names, read from the size token.
    ///
    /// A read-through, not a copy: changing [`size::CONTROL_SM`] moves
    /// [`ControlDensity::Compact`] with it, which is the whole reason the
    /// rung exists as a type instead of as two numbers in a match arm.
    #[must_use]
    pub const fn height(self) -> f32 {
        match self {
            Self::Compact => size::CONTROL_SM,
            Self::Standard => size::CONTROL_MD,
        }
    }

    /// The rung a height names, or `None` when it names neither.
    ///
    /// Compared exactly against the token values rather than within an
    /// invented tolerance. A tolerance would be a third number this
    /// module had to justify, and it would silently reclassify a
    /// genuinely off-ladder height as being "close enough" to a rung.
    /// A host measuring a control at layout time and getting `None` back
    /// is hearing the truth: that height is not a density.
    #[must_use]
    pub fn from_height(height: f32) -> Option<Self> {
        Self::ALL.into_iter().find(|rung| rung.height() == height)
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
        }
    }

    /// Whether this is the dense rung.
    #[must_use]
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }
}

/// A horizontal band §1.1 forbids density mixing in
/// (`UIF-PRIN-006`).
///
/// The five bands are the spec's own closed list — `Navbar`, `FormRow`,
/// `OverflowToolbar`, `FilterBar` and dialog footer — carried as a type
/// so a check can name the band it rejected rather than reporting an
/// anonymous row. A band outside this list is not covered by the clause,
/// which is a fact worth being able to say out loud too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DensityBand {
    /// The application navbar.
    Navbar,
    /// A horizontal label-and-control row in a form.
    FormRow,
    /// The [`OverflowToolbar`](crate::composite::overflow_toolbar) strip.
    OverflowToolbar,
    /// A filter bar.
    FilterBar,
    /// A dialog or form footer.
    DialogFooter,
}

impl DensityBand {
    /// Every band the homogeneity clause covers.
    pub const ALL: [Self; 5] = [
        Self::Navbar,
        Self::FormRow,
        Self::OverflowToolbar,
        Self::FilterBar,
        Self::DialogFooter,
    ];

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Navbar => "navbar",
            Self::FormRow => "form-row",
            Self::OverflowToolbar => "overflow-toolbar",
            Self::FilterBar => "filter-bar",
            Self::DialogFooter => "dialog-footer",
        }
    }
}

/// One control placed in a horizontal band, as the density rule sees it
/// (`UIF-PRIN-006`).
///
/// Deliberately minimal: a name to put in the report and the height the
/// control is built at. Nothing about what the control *does* is
/// relevant to whether it fits the band, and carrying more would let the
/// check pretend to know things it does not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSlot<'a> {
    /// What to call this control in a violation report.
    pub name: &'a str,
    /// The height the control is built at, in logical pixels.
    pub height: f32,
}

impl<'a> RowSlot<'a> {
    /// A slot measured in pixels.
    #[must_use]
    pub const fn new(name: &'a str, height: f32) -> Self {
        Self { name, height }
    }

    /// A slot declared at a rung rather than a height.
    ///
    /// The preferred form when the host already knows its density: the
    /// height is read from the token, so the slot cannot drift from the
    /// ladder it claims to sit on.
    #[must_use]
    pub const fn at_density(name: &'a str, density: ControlDensity) -> Self {
        Self {
            name,
            height: density.height(),
        }
    }

    /// The rung this slot sits at, or `None` when it sits on neither.
    #[must_use]
    pub fn rung(&self) -> Option<ControlDensity> {
        ControlDensity::from_height(self.height)
    }
}

/// One control that does not belong in the band it was placed in
/// (`UIF-PRIN-006`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DensityViolation<'a> {
    /// The band the control was placed in.
    pub band: DensityBand,
    /// The declared density of that band.
    pub declared: ControlDensity,
    /// The offending control's name.
    pub control: &'a str,
    /// The height the control is actually built at.
    pub height: f32,
    /// The rung that height names — `None` when it names neither.
    pub rung: Option<ControlDensity>,
}

impl DensityViolation<'_> {
    /// Whether the control is on neither rung rather than merely on the
    /// wrong one.
    ///
    /// The two need different fixes: a wrong rung is a one-line change at
    /// the call site, an off-rung height means something above this check
    /// chose a number the density ladder never offered.
    #[must_use]
    pub const fn is_off_rung(&self) -> bool {
        self.rung.is_none()
    }
}

impl std::fmt::Display for DensityViolation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.rung {
            Some(rung) => write!(
                f,
                "{}: '{}' is {}px ({}) but the band is declared {} ({}px)",
                self.band.slug(),
                self.control,
                self.height,
                rung.slug(),
                self.declared.slug(),
                self.declared.height(),
            ),
            None => write!(
                f,
                "{}: '{}' is {}px, which is on no density rung; the band is declared {} ({}px)",
                self.band.slug(),
                self.control,
                self.height,
                self.declared.slug(),
                self.declared.height(),
            ),
        }
    }
}

/// The verdict of one row-density check (`UIF-PRIN-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct DensityReport<'a> {
    /// The band that was checked.
    pub band: DensityBand,
    /// The density the band declared.
    pub declared: ControlDensity,
    /// Every control that does not sit at the declared rung.
    pub violations: Vec<DensityViolation<'a>>,
    /// The distinct rungs actually present among the slots, ascending.
    ///
    /// Reported independently of [`Self::violations`] so a row that is
    /// internally consistent but declared wrong can be told apart from
    /// a row that genuinely mixes. A slot that is off every rung
    /// contributes nothing here.
    pub rung_spread: Vec<ControlDensity>,
}

impl DensityReport<'_> {
    /// Whether every control sits at the band's declared rung.
    #[must_use]
    pub fn is_homogeneous(&self) -> bool {
        self.violations.is_empty()
    }

    /// Whether more than one rung is physically present in the band.
    ///
    /// This is the literal form of §1.1's "mixing `28px` and `32px`
    /// controls in the same horizontal band is forbidden", and it is
    /// not the same question as [`Self::is_homogeneous`]: a row of two
    /// `Compact` controls declared `Standard` is mixed-free and still
    /// violates its declaration.
    #[must_use]
    pub fn mixes_rungs(&self) -> bool {
        self.rung_spread.len() > 1
    }

    /// How many controls sit on neither rung.
    #[must_use]
    pub fn off_rung_count(&self) -> usize {
        self.violations.iter().filter(|v| v.is_off_rung()).count()
    }
}

impl std::fmt::Display for DensityReport<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_homogeneous() {
            return write!(
                f,
                "{} band is homogeneous at {}",
                self.band.slug(),
                self.declared.slug()
            );
        }
        write!(
            f,
            "{} band declared {} has {} violation(s) across rung spread [{}]",
            self.band.slug(),
            self.declared.slug(),
            self.violations.len(),
            self.rung_spread
                .iter()
                .map(|rung| rung.slug())
                .collect::<Vec<_>>()
                .join(", "),
        )?;
        for violation in &self.violations {
            write!(f, "; {violation}")?;
        }
        Ok(())
    }
}

/// Check a horizontal band's controls against the density it declares
/// (`UIF-PRIN-006`).
///
/// A pure function of the declaration and the slots: no widget, no
/// layout pass, no live width. The host declares the band's rung once and
/// hands over what it is about to place in it.
///
/// * `band` — which of §1.1's five forbidden-mixing bands this is, so
///   the report can name it.
/// * `declared` — the rung the band is built at.
/// * `slots` — the controls about to be placed in the band.
///
/// An empty `slots` is homogeneous: an empty band mixes nothing.
#[must_use]
pub fn check_row_density<'a>(
    band: DensityBand,
    declared: ControlDensity,
    slots: &[RowSlot<'a>],
) -> DensityReport<'a> {
    let mut violations = Vec::new();
    let mut rung_spread: Vec<ControlDensity> = Vec::new();

    for slot in slots {
        let rung = slot.rung();
        if let Some(rung) = rung
            && !rung_spread.contains(&rung)
        {
            rung_spread.push(rung);
        }
        if rung != Some(declared) {
            violations.push(DensityViolation {
                band,
                declared,
                control: slot.name,
                height: slot.height,
                rung,
            });
        }
    }

    rung_spread.sort_by_key(|rung| *rung as u8);
    DensityReport {
        band,
        declared,
        violations,
        rung_spread,
    }
}

/// The rung an [`OverflowToolbar`](crate::composite::overflow_toolbar)
/// strip declares, read from its own `STRIP_HEIGHT` rather than restated.
///
/// `None` means the strip's height is no longer on the density ladder —
/// a change a caller must hear about rather than have assumed away.
#[must_use]
pub fn overflow_toolbar_declared_density() -> Option<ControlDensity> {
    ControlDensity::from_height(crate::composite::overflow_toolbar::STRIP_HEIGHT)
}

/// The rung a [`FormField`](crate::composite::form_field) control slot
/// declares, read from its own `CONTROL_SLOT_HEIGHT`.
#[must_use]
pub fn form_field_declared_density() -> Option<ControlDensity> {
    ControlDensity::from_height(crate::composite::form_field::CONTROL_SLOT_HEIGHT)
}

/// Check the controls a host places in an
/// [`OverflowToolbar`](crate::composite::overflow_toolbar) strip against
/// the rung the strip itself declares.
///
/// Returns `None` when the strip has left the two-rung ladder, which is a
/// finding about the strip rather than about the controls, and must not be
/// reported as a control-level violation.
#[must_use]
pub fn check_overflow_toolbar<'a>(controls: &[RowSlot<'a>]) -> Option<DensityReport<'a>> {
    overflow_toolbar_declared_density()
        .map(|declared| check_row_density(DensityBand::OverflowToolbar, declared, controls))
}

/// Check the controls a host places in a
/// [`FormField`](crate::composite::form_field) row against the rung the
/// field's control slot declares.
///
/// Note what this does *not* settle: `FormField` centres a shorter
/// control inside its fixed `32px` band rather than shrinking the band,
/// so a `Compact` control placed in a `Standard` field still renders
/// without moving anything. That is a rendering accommodation, not a
/// licence to mix — §1.1 forbids the mixture itself, which is exactly
/// what this check reports.
#[must_use]
pub fn check_form_field_row<'a>(controls: &[RowSlot<'a>]) -> Option<DensityReport<'a>> {
    form_field_declared_density()
        .map(|declared| check_row_density(DensityBand::FormRow, declared, controls))
}

#[cfg(test)]
#[path = "density_tests.rs"]
mod density_tests;
