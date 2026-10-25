//! Tabular numerals — the flag, and the boundary it cannot cross
//! (`UIF-PRIN-007`).
//!
//! §1.1: *"All numeric values … MUST render with fixed-width tabular
//! numerals (`FontFamily::Mono` or `font-variant-numeric: tabular-nums`)
//! so live updates never cause horizontal text jitter. Numeric table
//! columns right-align both header and cell values."*
//!
//! # Correcting the premise this increment started from
//!
//! The coverage ledger used to record that `UIF-TOK-004` "models no font
//! *family*, so `FontFamily::Mono` has no rung". **That is not true.**
//! [`Family::Mono`] / [`FAMILY_MONO`](crate::token::typography::FAMILY_MONO)
//! is a declared `font.family.mono` token and
//! [`Family::font`](crate::token::typography::Family::font) turns it
//! into an `iced::Font`. A readout *can* ask for the mono family today.
//!
//! The real boundary is two notches further down, and it is worth stating
//! precisely because the wrong version of it would send a future builder
//! to fix the wrong thing:
//!
//! 1. **No role binds a readout to tabular digits.** Nothing in the
//!    token set says "this number is tabular", so a counter, a
//!    percentage or a numeric column could not assert the requirement
//!    and nothing could check it. That is the gap [`TabularNums`] closes.
//! 2. **`font-variant-numeric: tabular-nums` is unreachable.**
//!    `iced::Font` carries exactly four fields — `family`, `weight`,
//!    `stretch`, `style` — and no OpenType feature selector. The spec's
//!    *second* mechanism for tabular figures therefore cannot be
//!    requested through this framework at all. Only the mono-family
//!    route exists.
//! 3. **Even the mono route is not guaranteed to be fixed-pitch.**
//!    `Family::Mono` resolves to iced's *generic* `Monospace`, which
//!    cosmic-text answers from the host's font fallback. As
//!    [`typography`](crate::token::typography) records, the crate
//!    bundles no monospace face, so whether a fixed-pitch font actually
//!    appears depends on the machine.
//!
//! So: the flag is buildable and checkable here; the guarantee that a
//! live number never jitters is not the crate's to give. That is a
//! framework limitation resting on a spec that names a CSS property as
//! if it were a token, and it is recorded as such rather than papered
//! over with a token that would look complete and resolve to nothing.
//!
//! Unit tests live in the sibling `tabular_tests.rs` sidecar.

use crate::token::typography::Family;

/// Whether a readout must render with fixed-width digits
/// (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabularNums {
    /// The value changes while on screen and must not cause horizontal
    /// text jitter — counters, percentages, timestamps, ranges,
    /// numeric columns.
    Required,
    /// Prose or a label that happens to contain a digit. No fixed-width
    /// requirement.
    NotRequired,
}

impl TabularNums {
    /// Every rung.
    pub const ALL: [Self; 2] = [Self::Required, Self::NotRequired];

    /// The family this flag resolves to, when it resolves to one.
    ///
    /// [`TabularNums::Required`] maps to
    /// [`Family::Mono`] — the `font.family.mono` token. This is the
    /// *only* tabular mechanism this framework can express: the
    /// `font-variant-numeric: tabular-nums` alternative has no
    /// representation in `iced::Font`. See the module docs.
    #[must_use]
    pub const fn family(self) -> Option<Family> {
        match self {
            Self::Required => Some(Family::Mono),
            Self::NotRequired => None,
        }
    }

    /// Whether fixed-width digits are required.
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::NotRequired => "not-required",
        }
    }
}

/// The kind of numeric readout (`UIF-PRIN-007`).
///
/// §1.1's list, minus the widgets this crate does not own. The
/// distinction that matters for checking is that only a table column
/// carries the right-alignment mandate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReadoutKind {
    /// A live line / column or item counter.
    Counter,
    /// A `ProgressBar` percentage readout.
    Percentage,
    /// A timestamp.
    Timestamp,
    /// A `PaginationBar` range such as `1–25 of 300`.
    PaginationRange,
    /// A numeric `DataTable` column.
    NumericColumn,
}

impl ReadoutKind {
    /// Every kind.
    pub const ALL: [Self; 5] = [
        Self::Counter,
        Self::Percentage,
        Self::Timestamp,
        Self::PaginationRange,
        Self::NumericColumn,
    ];

    /// Whether this kind carries the right-alignment mandate.
    ///
    /// Only a numeric table column does: §1.1 says numeric columns
    /// right-align *both* header and cell values. A counter or a
    /// timestamp has no header to align.
    #[must_use]
    pub const fn must_right_align(self) -> bool {
        matches!(self, Self::NumericColumn)
    }

    /// Whether §1.1 requires this kind to be tabular.
    ///
    /// Every kind in the spec's list is a live or aligned numeric value,
    /// so the answer is yes for all of them — which is why a readout
    /// declaring [`TabularNums::NotRequired`] is a reportable finding
    /// rather than a legitimate choice.
    #[must_use]
    pub const fn requires_tabular(self) -> bool {
        true
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Percentage => "percentage",
            Self::Timestamp => "timestamp",
            Self::PaginationRange => "pagination-range",
            Self::NumericColumn => "numeric-column",
        }
    }
}

/// A numeric readout and the tabular / alignment contract it declares
/// (`UIF-PRIN-007`).
///
/// The alignment fields bind only when
/// [`kind`](Self::kind) [`must_right_align`](ReadoutKind::must_right_align);
/// for every other kind they are inert, and
/// [`check_numeric_readout`] ignores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumericReadout<'a> {
    /// What to call this readout in a report.
    pub id: &'a str,
    /// What kind of numeric value it is.
    pub kind: ReadoutKind,
    /// Whether it must render tabular.
    pub tabular: TabularNums,
    /// Whether the column header right-aligns.
    pub header_right_aligned: bool,
    /// Whether the cell values right-align.
    pub cell_right_aligned: bool,
}

/// One way a numeric readout breaks the tabular / alignment rule
/// (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabularViolation<'a> {
    /// A readout §1.1 requires to be tabular declared otherwise.
    NotTabular {
        /// The offending readout.
        id: &'a str,
        /// Its kind.
        kind: ReadoutKind,
    },
    /// A numeric column's header is not right-aligned.
    HeaderNotRightAligned {
        /// The offending readout.
        id: &'a str,
    },
    /// A numeric column's cell values are not right-aligned.
    CellNotRightAligned {
        /// The offending readout.
        id: &'a str,
    },
}

impl std::fmt::Display for TabularViolation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotTabular { id, kind } => write!(
                f,
                "'{id}' is a {} readout but does not require tabular numerals; §1.1 requires fixed-width digits so live updates never jitter",
                kind.slug()
            ),
            Self::HeaderNotRightAligned { id } => write!(
                f,
                "'{id}' is a numeric column whose header is not right-aligned; §1.1 requires both header and cell to right-align"
            ),
            Self::CellNotRightAligned { id } => write!(
                f,
                "'{id}' is a numeric column whose cell values are not right-aligned; §1.1 requires both header and cell to right-align"
            ),
        }
    }
}

/// Check a numeric readout's tabular and alignment contract
/// (`UIF-PRIN-007`).
///
/// Returns an empty `Vec` when the readout requires tabular digits and,
/// if it is a numeric column, right-aligns both header and cells.
#[must_use]
pub fn check_numeric_readout<'a>(readout: &NumericReadout<'a>) -> Vec<TabularViolation<'a>> {
    let mut violations = Vec::new();

    if readout.kind.requires_tabular() && !readout.tabular.is_required() {
        violations.push(TabularViolation::NotTabular {
            id: readout.id,
            kind: readout.kind,
        });
    }

    if readout.kind.must_right_align() {
        if !readout.header_right_aligned {
            violations.push(TabularViolation::HeaderNotRightAligned { id: readout.id });
        }
        if !readout.cell_right_aligned {
            violations.push(TabularViolation::CellNotRightAligned { id: readout.id });
        }
    }

    violations
}

#[cfg(test)]
#[path = "tabular_tests.rs"]
mod tabular_tests;
