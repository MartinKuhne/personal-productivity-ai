//! Machine-checked enforcement of Perceptual Harmony Through Tokenization
//! (`UIF-PRIN-001`) against this crate's own source text.
//!
//! # Why read the source at all
//!
//! `UIF-PRIN-001` forbids raw numeric literals in individual widget
//! definitions, and `UIF-VER-001` asks for "zero raw RGB or hex literals
//! … outside the token definitions". Before this module the only automated
//! gate was [`gallery::unmapped_colours`](crate::gallery::unmapped_colours),
//! which inspects the colours the gallery *emits*. That gate is real but
//! partial: a hard-coded colour is caught only if the gallery happens to
//! render that cell, and a hard-coded padding, radius, stroke or size is
//! caught by nothing at all. This module reads the crate's text instead,
//! so the rule reaches code no snapshot does.
//!
//! It is a **lint**, not a proof. It cannot see through a variable, and it
//! is deliberately conservative about what it calls a violation — a false
//! alarm here is worse than a miss, because the first person to hit one
//! starts trusting the escape hatch rather than the rule. What it *can*
//! do is make the cost of adding a magic number explicit, in a failing
//! test, naming the file and the line.
//!
//! # Rule A — colour literals live only in `token/color.rs`
//!
//! `Color::from_rgb`, `Color::from_rgba`, `Color::from_rgb8`,
//! `Color::from_rgba8` and hex colour literals (`0xRRGGBB`, `#RRGGBB`)
//! are banned in every production file except `token/color.rs`. There is
//! no argument to be made at a call site: a colour either has a token or
//! it does not exist.
//!
//! The ban is count-checked. [`ScanStats::token_layer_colour_definitions`]
//! is the number of constructors the scan found *inside* `token/color.rs`,
//! and the sidecar pins it. A scanner whose pattern had silently broken
//! would report zero violations by finding nothing at all; the pinned
//! definition count is what makes that impossible.
//!
//! # Rule B — a visual scalar `pub const` must be bound to a token
//!
//! A `pub const … : f32` in `widget/` or `surface/` whose **name** marks
//! it as a visual measurement (see [`VISUAL_NAME_MARKERS`]) must not be
//! bound to a bare numeric literal. Binding it to `space::MD`,
//! `size::CONTROL_MD`, `shape::RADIUS_CARD`, a named geometry constant
//! or a sum of any of those is fine — the rule is about the *literal*,
//! not the arithmetic.
//!
//! Four classes of bare literal are not violations, and each is a
//! distinction the spec itself draws rather than a convenience:
//!
//! * [`ScalarClass::SurfaceBound`] — **the §1.1 carve-out.** "Composite
//!   surface layout bounds (such as default/min/max card dimensions in
//!   §4) are defined as named Surface Geometry Constants per surface."
//!   A card's `CARD_DEFAULT_WIDTH: f32 = 680.0` is legitimate *because
//!   the spec says so*, and the exemption is granted by an enumerated
//!   registry ([`SURFACE_GEOMETRY_CONSTANTS`]) carrying the §-reference
//!   that authorises each entry — not by a name-pattern guess. A
//!   visual-named literal that is not in the registry is a violation even
//!   if its name sounds like a bound.
//! * [`ScalarClass::Zero`] — `0.0` is the *absence* of a measurement,
//!   not a value chosen from a scale. `FOOTER_PADDING_VERTICAL: f32 = 0.0`
//!   says "no padding"; there is no token for nothing.
//! * [`ScalarClass::Ratio`] — a literal strictly inside `(0.0, 1.0)` is
//!   a proportion, not a length. The smallest linear measure anywhere in
//!   this design system is `shape::BORDER_HAIRLINE` at `1.0`, so a bare
//!   sub-unit literal cannot be a pixel measurement.
//! * [`ScalarClass::TokenBound`] — the RHS references a token path and
//!   carries no literal at all. This is the common case and the one the
//!   rule exists to keep common.
//!
//! # What this scan cannot catch
//!
//! Stated plainly, because a gate that overstates itself is worse than no
//! gate:
//!
//! * **Anything not a `pub const` in `widget/` or `surface/`.** Private
//!   constants, locals, struct fields, function arguments and literals
//!   written inline in a paint or layout expression are all invisible to
//!   Rule B. A `12.0` typed straight into a `Padding` literal passes.
//! * **Indirection.** `const X: f32 = Y;` where `Y` is itself a magic
//!   number reads as token-bound at this layer; only the definition site
//!   is checked.
//! * **Semantic correctness.** The scan proves a constant *names a
//!   token*, not that it named the *right* one. `RADIUS: f32 =
//!   shape::RADIUS_WINDOW` on a button is well-formed and wrong.
//! * **Colour reached by other means.** A colour assembled field by
//!   field, or carried in from a host, never trips Rule A. The crate now
//!   uses that route deliberately in
//!   [`scrim`](crate::surface::zorder::scrim), which overrides one
//!   channel of an inherited token; the sidecar pins the composed result
//!   against `color.surface.scrim` so the pattern cannot drift silently.
//! * **Non-`f32` scalars.** Rule B reads `f32` constants; an
//!   integer-typed measurement is not examined.
//! * **The `#RRGGBB` spelling.** Rule A matches it, but in Rust that
//!   form only ever occurs inside a string literal — which is redacted
//!   before the scan runs. The live spelling in this codebase is
//!   `0xRRGGBB`; the `#` branch is kept so the rule reads as stated and
//!   so a non-Rust consumer of `scan_source` is still covered.
//! * **Two constructors on one line.** Only the first `Color::from_` on
//!   a line is examined.
//!
//! # Current exceptions
//!
//! The raw scan of this tree reports three findings. Two were fixed rather
//! than excused, leaving exactly one entry in [`ALLOW_LIST`]. The sidecar
//! asserts that count and asserts every entry still fires, so an exception
//! cannot outlive its reason and no new one can slip in quietly.
//!
//! Unit tests live in the sibling `token_lint_tests.rs` sidecar.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

/// The rule a finding breaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    /// Rule A: a colour literal outside `token/color.rs`.
    ColourLiteral,
    /// Rule B: a visual scalar `pub const` bound to a bare numeric literal.
    UnboundVisualScalar,
}

impl Rule {
    /// The column label for this rule.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ColourLiteral => "colour-literal",
            Self::UnboundVisualScalar => "unbound-scalar",
        }
    }
}

/// How a bare numeric literal on a visual scalar constant was classified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScalarClass {
    /// The RHS references a token path; no literal is involved.
    TokenBound,
    /// The literal is `0.0` — the absence of a measurement.
    Zero,
    /// The literal is strictly inside `(0.0, 1.0)` — a proportion.
    Ratio,
    /// The constant is a registered Surface Geometry Constant (§1.1).
    SurfaceBound,
    /// None of the above: a visual primitive that should be a token.
    Violation,
}

impl ScalarClass {
    /// Whether this class satisfies `UIF-PRIN-001` rather than breaching it.
    #[must_use]
    pub const fn is_compliant(self) -> bool {
        !matches!(self, Self::Violation)
    }

    /// The column label for this class.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TokenBound => "token-bound",
            Self::Zero => "zero",
            Self::Ratio => "ratio",
            Self::SurfaceBound => "surface-bound",
            Self::Violation => "violation",
        }
    }
}

/// One thing the scan found.
///
/// A finding is a *raw* observation: the allow-list has not been applied.
/// [`Audit`] is the resolved view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// Which rule fired.
    pub rule: Rule,
    /// Crate-relative path with forward slashes, e.g. `surface/movable.rs`.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// The offending code, trimmed.
    pub code: String,
    /// What specifically was wrong.
    pub detail: String,
}

impl Violation {
    /// Stable identity used to match an allow-list entry.
    #[must_use]
    pub fn key(&self) -> String {
        match self.rule {
            Rule::ColourLiteral => format!("{}:{}", self.file, self.detail),
            Rule::UnboundVisualScalar => {
                format!("{}:{}", self.file, self.constant_name().unwrap_or_default())
            }
        }
    }

    /// The `const` name this finding is about, for Rule B findings.
    #[must_use]
    pub fn constant_name(&self) -> Option<&str> {
        if self.rule != Rule::UnboundVisualScalar {
            return None;
        }
        let after_const = self.code.split_once("const ")?.1;
        Some(after_const.split([' ', ':']).next().unwrap_or_default())
    }
}

/// What one file's scan produced, before and after the allow-list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceScan {
    /// Every raw finding, in file order.
    pub findings: Vec<Violation>,
    /// Colour constructors found inside `token/color.rs`.
    pub token_layer_colour_definitions: usize,
    /// Visual scalar constants that read a token path.
    pub token_bound: usize,
    /// Visual scalar constants bound to `0.0`.
    pub zero: usize,
    /// Visual scalar constants bound to a sub-unit proportion.
    pub ratio: usize,
    /// Visual scalar constants covered by the §1.1 surface-bound registry.
    pub surface_bound: usize,
}

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// The one file allowed to define colours.
pub const COLOUR_DEFINITION_FILE: &str = "token/color.rs";

/// Directories whose `pub const` measurements Rule B inspects.
///
/// `composite/` is in scope alongside the atoms and the surfaces: the
/// composites of §3B carry the same kind of spec-fixed visual bound, and a
/// subsystem that fell outside Rule B would be a hole rather than a
/// narrower audit.
pub const VISUAL_SCOPE_DIRS: [&str; 3] = ["widget/", "surface/", "composite/"];

/// Name fragments that mark a constant as a visual measurement.
///
/// Matched case-insensitively as a substring of the constant's name. This
/// is deliberately broad on purpose: it is cheaper to over-inspect a
/// constant and classify it out (zero, ratio, surface bound) than to miss
/// one that a narrower list would have let through.
pub const VISUAL_NAME_MARKERS: [&str; 10] = [
    "HEIGHT", "WIDTH", "PADDING", "RADIUS", "STROKE", "GAP", "SIZE", "INSET", "MARGIN", "SLOT",
];

/// Colour constructors that may only appear in [`COLOUR_DEFINITION_FILE`].
pub const COLOUR_CONSTRUCTORS: [&str; 4] = [
    "Color::from_rgb",
    "Color::from_rgba",
    "Color::from_rgb8",
    "Color::from_rgba8",
];

/// Composite surface layout bounds the spec's §1.1 carve-out permits.
///
/// Each entry names the constant and the §-reference that authorises the
/// figure. This is the whole basis on which a bare numeric literal on a
/// visually-named constant is excused: no entry, no exemption. Keeping it
/// a registry rather than a name-pattern rule is what stops "sounds like a
/// bound" from standing in for "the spec set this number".
#[derive(Debug)]
pub struct SurfaceGeometryConstant {
    /// Crate-relative file, forward slashes.
    pub file: &'static str,
    /// Constant name.
    pub name: &'static str,
    /// The spec section that fixes the value.
    pub spec: &'static str,
}

/// Every registered Surface Geometry Constant (`UIF-PRIN-001` §1.1).
pub const SURFACE_GEOMETRY_CONSTANTS: &[SurfaceGeometryConstant] = &[
    SurfaceGeometryConstant {
        file: "surface/input_dialog.rs",
        name: "CARD_WIDTH",
        spec: "§4.5.1 — Input Dialog Card width 320px",
    },
    SurfaceGeometryConstant {
        file: "surface/input_dialog.rs",
        name: "FOLDER_LIST_MAX_HEIGHT",
        spec: "§4.5.1 — Destination Folder List max height 200px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "EDITOR_WIDTH_MIN",
        spec: "§4.6.2 — Inline Document Editor min width 520px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "EDITOR_WIDTH_MAX",
        spec: "§4.6.2 — Inline Document Editor max width 960px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "EDITOR_HEIGHT_MIN",
        spec: "§4.6.2 — Inline Document Editor min height 380px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "EDITOR_HEIGHT_MAX",
        spec: "§4.6.2 — Inline Document Editor max height 760px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "TOOLS_WIDTH",
        spec: "§4.6.2 — Tools & Integrations width 720px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "TOOLS_HEIGHT_MIN",
        spec: "§4.6.2 — Tools & Integrations min height 320px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "BATCH_WIDTH",
        spec: "§4.6.2 — Batch Processing width 560px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "CONTENT_HEIGHT_MAX",
        spec: "§4.6.2 — Batch Processing content-fit max height 480px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "ABOUT_WIDTH",
        spec: "§4.6.2 — About width 440px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "ATTRIBUTIONS_WIDTH",
        spec: "§4.6.2 — Attributions width 640px",
    },
    SurfaceGeometryConstant {
        file: "surface/modal.rs",
        name: "ATTRIBUTIONS_HEIGHT",
        spec: "§4.6.2 — Attributions scrollable height 480px",
    },
    SurfaceGeometryConstant {
        file: "surface/movable.rs",
        name: "CARD_DEFAULT_WIDTH",
        spec: "§4.7.1 — Movable Overlay Card default 680x440px",
    },
    SurfaceGeometryConstant {
        file: "surface/movable.rs",
        name: "CARD_DEFAULT_HEIGHT",
        spec: "§4.7.1 — Movable Overlay Card default 680x440px",
    },
    SurfaceGeometryConstant {
        file: "surface/tooltip/mod.rs",
        name: "MAX_WIDTH",
        spec: "§3.7 — Tooltip max width 280px",
    },
    SurfaceGeometryConstant {
        file: "surface/popover/mod.rs",
        name: "WIDTH_MIN",
        spec: "§3.7 — Teaching popover width range, lower bound 280px",
    },
    SurfaceGeometryConstant {
        file: "surface/popover/mod.rs",
        name: "WIDTH_MAX",
        spec: "§3.7 — Teaching popover width range, upper bound 360px",
    },
    SurfaceGeometryConstant {
        file: "composite/table_header.rs",
        name: "RESIZE_REACH",
        spec: "§3.7 — column resize grab 10px, halved so the reach is split evenly across the divider",
    },
    SurfaceGeometryConstant {
        file: "widget/auto_suggest_model.rs",
        name: "FLYOUT_MAX_HEIGHT",
        spec: "§3.2 — AutoSuggest completion flyout max height 240px",
    },
    SurfaceGeometryConstant {
        file: "widget/combo_model.rs",
        name: "FLYOUT_MAX_HEIGHT",
        spec: "§3.4 — ComboBox / Select flyout max height 280px",
    },
    SurfaceGeometryConstant {
        file: "widget/combo_box.rs",
        name: "MIN_WIDTH",
        spec: "§3.4 — ComboBox / Select trigger min width 120px",
    },
    SurfaceGeometryConstant {
        file: "composite/form_field.rs",
        name: "HEADER_HEIGHT",
        spec: "§3B.1 — Stacked form field header row height 20px",
    },
    SurfaceGeometryConstant {
        file: "composite/list_tile.rs",
        name: "TWO_LINE_HEIGHT",
        spec: "§3B.5 — ListTile two-line row height 44px",
    },
    SurfaceGeometryConstant {
        file: "composite/toast.rs",
        name: "WIDTH",
        spec: "§3B.2 — Toast / SnackBar width 360px",
    },
    SurfaceGeometryConstant {
        file: "composite/breadcrumb.rs",
        name: "BAR_HEIGHT",
        spec: "§3B.8 — BreadcrumbBar horizontal path bar height 28px",
    },
    SurfaceGeometryConstant {
        file: "composite/breadcrumb.rs",
        name: "SEGMENT_HEIGHT",
        spec: "§3B.8 — Breadcrumb segment (Ghost button) height 24px",
    },
    SurfaceGeometryConstant {
        file: "composite/breadcrumb.rs",
        name: "SEGMENT_PADDING_X",
        spec: "§3B.8 — Breadcrumb segment padding [0, 6px]",
    },
    SurfaceGeometryConstant {
        file: "composite/breadcrumb.rs",
        name: "SEGMENT_GAP",
        spec: "§3B.8 — BreadcrumbBar gap 4px",
    },
    SurfaceGeometryConstant {
        file: "composite/breadcrumb.rs",
        name: "ELLIPSIS_TRIGGER_WIDTH",
        spec: "§3B.8 — Breadcrumb […] trigger button width 28px (28x24px box)",
    },
    SurfaceGeometryConstant {
        file: "composite/overflow_toolbar.rs",
        name: "STRIP_HEIGHT",
        spec: "§3B.8 — OverflowToolbar strip height 28px, fixed so eviction cannot shift it",
    },
    SurfaceGeometryConstant {
        file: "composite/overflow_toolbar.rs",
        name: "ITEM_HEIGHT",
        spec: "§3B.8 — OverflowToolbar inline item height 24px, matching the breadcrumb segment",
    },
    SurfaceGeometryConstant {
        file: "composite/overflow_toolbar.rs",
        name: "ITEM_GAP",
        spec: "§3B.8 — OverflowToolbar item gap 4px",
    },
    SurfaceGeometryConstant {
        file: "composite/overflow_toolbar.rs",
        name: "TRIGGER_SIZE",
        spec: "§3B.8 — OverflowToolbar […] overflow menu button 28x28px",
    },
    SurfaceGeometryConstant {
        file: "composite/overflow_toolbar.rs",
        name: "TRIGGER_GAP",
        spec: "§3B.8 — OverflowToolbar trigger clearance 8px, the remainder of the 36px reserve",
    },
];

/// Look up the §1.1 authority for one constant, if it is registered.
#[must_use]
pub fn surface_geometry(file: &str, name: &str) -> Option<&'static SurfaceGeometryConstant> {
    SURFACE_GEOMETRY_CONSTANTS
        .iter()
        .find(|entry| entry.file == file && entry.name == name)
}

/// Which tree a scan governs, and on what terms.
///
/// The scanner itself is scope-free: it is the *scope* that says which file
/// owns colours, which paths carry visual measurements, and which bare
/// literals somebody has registered as a fixed bound. iced-pro's own tree
/// is one such scope ([`crate_scope`]); a host crate that wants the same
/// gate over its own sources supplies a second one instead of writing a
/// second scanner.
///
/// That choice is deliberate. A parallel scanner starts identical and then
/// drifts: a bug fixed in one leaves the other still reporting whatever it
/// always reported, and two numbers that disagree about the same tree are
/// worse than one number that is wrong in a known way. Every rule below is
/// read from this struct rather than hard-coded, so there is exactly one
/// implementation of each.
#[derive(Clone, Copy, Debug)]
pub struct Scope<'a> {
    /// The one file this scope allows to define colours.
    pub colour_definition_file: &'static str,
    /// Path prefixes whose `pub const` measurements Rule B inspects.
    pub visual_scope_prefixes: &'a [&'static str],
    /// Bare literals this scope registers as fixed layout bounds.
    pub surface_geometry: &'a [SurfaceGeometryConstant],
}

/// The scope of the crate this module is compiled into.
///
/// Built from the existing constants rather than restating them, so
/// iced-pro's own audit is exactly what it was before this struct existed.
#[must_use]
pub fn crate_scope<'a>() -> Scope<'a> {
    Scope {
        colour_definition_file: COLOUR_DEFINITION_FILE,
        visual_scope_prefixes: &VISUAL_SCOPE_DIRS,
        surface_geometry: SURFACE_GEOMETRY_CONSTANTS,
    }
}

/// Look up a registered layout bound in an arbitrary registry.
#[must_use]
pub fn lookup_geometry<'s>(
    registry: &'s [SurfaceGeometryConstant],
    file: &str,
    name: &str,
) -> Option<&'s SurfaceGeometryConstant> {
    registry
        .iter()
        .find(|entry| entry.file == file && entry.name == name)
}

/// An individually justified exception to a finding.
///
/// The allow-list is the only way a finding stops being a build failure,
/// and every entry has to carry its own reason. The sidecar asserts that
/// each entry actually fires: an entry that suppresses nothing is a
/// failure, which is what keeps this list from accreting as the tree moves
/// under it.
pub struct AllowEntry {
    /// The exact [`Violation::key`] this entry covers.
    pub key: &'static str,
    /// Why the finding is legitimate.
    pub reason: &'static str,
}

/// Every exception the crate currently takes to the two rules.
///
/// Reviewed against the raw scan of the tree at the time of writing: three
/// findings, two of which were fixed rather than excused
/// (`surface/movable.rs::RESIZE_HANDLE_SIZE` re-pointed at
/// `size::ICON_MD`, which §2.2 assigns to the resize grip glyph at the
/// same `16.0`; `surface/zorder.rs::scrim` rewritten as a struct update
/// so no constructor appears outside `token/color.rs`). One remains.
pub const ALLOW_LIST: &[AllowEntry] = &[AllowEntry {
    key: "surface/input_dialog.rs:BUTTON_MIN_WIDTH",
    reason: "§4.5.1 fixes the footer row's height (`size::CONTROL_MD`) and \
             alignment but leaves button width to the label, which only a \
             renderer knows; `64.0` is the pre-measurement placeholder used \
             until a label measurer is supplied, and no rung in §2 carries \
             the value 64. Deliberately NOT registered as a Surface Geometry \
             Constant: §1.1's carve-out covers composite bounds the spec \
             itself sets, and this is not one of them.",
}];

// ---------------------------------------------------------------------------
// The resolved audit
// ---------------------------------------------------------------------------

/// The whole scan, with the allow-list applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Audit {
    /// Findings that no allow entry covers. These fail the build.
    pub violations: Vec<Violation>,
    /// Findings the allow-list suppressed, paired with their reason.
    pub suppressed: Vec<(Violation, &'static str)>,
    /// Allow-list keys that matched nothing. These also fail the build.
    pub stale_allow_entries: Vec<&'static str>,
    /// Aggregate counters across everything scanned.
    pub stats: ScanStats,
}

/// Aggregate counters for a whole-crate scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanStats {
    /// Production files read.
    pub files_scanned: usize,
    /// Files Rule A was applied to.
    pub rule_a_files: usize,
    /// Files Rule B was applied to.
    pub rule_b_files: usize,
    /// Colour constructors found inside `token/color.rs`.
    pub token_layer_colour_definitions: usize,
    /// Visual scalar constants that read a token path.
    pub token_bound: usize,
    /// Visual scalar constants bound to `0.0`.
    pub zero: usize,
    /// Visual scalar constants bound to a sub-unit proportion.
    pub ratio: usize,
    /// Visual scalar constants covered by the §1.1 registry.
    pub surface_bound: usize,
}

impl Audit {
    /// Whether the crate is clean: nothing unallowed, nothing stale.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty() && self.stale_allow_entries.is_empty()
    }

    /// A human-readable list of everything wrong, empty when clean.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for v in &self.violations {
            out.push_str(&format!(
                "[{}] {}:{}: {} — {}\n",
                v.rule.label(),
                v.file,
                v.line,
                v.code,
                v.detail
            ));
        }
        for key in &self.stale_allow_entries {
            out.push_str(&format!(
                "[stale-allow] {key} suppresses nothing — the finding is gone, \
                 delete the entry\n"
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Scan one file's text under an explicit [`Scope`]. Pure: no filesystem,
/// no allow-list.
///
/// This is the whole scanner. [`scan_source`] is this function with
/// iced-pro's own scope, and a host crate's gate is this function with its
/// own — the same redaction, the same constructor patterns, the same
/// scalar classifier, the same registry semantics.
///
/// `file` is the root-relative path with forward slashes; it decides which
/// rules apply. Feeding a fixture string through here exercises the same
/// code the tree-wide scan runs, which is what makes the injection test
/// meaningful.
#[must_use]
pub fn scan_source_in(scope: &Scope<'_>, file: &str, text: &str) -> SourceScan {
    let mut scan = SourceScan::default();
    let redacted = redact_comments_and_strings(text);
    let lines: Vec<&str> = redacted.split('\n').collect();

    if is_test_file(file) {
        return scan;
    }

    let colour_host = file == scope.colour_definition_file;

    if !colour_host {
        for (idx, line) in lines.iter().enumerate() {
            if let Some(ctor) = colour_literal_in(line) {
                scan.findings.push(Violation {
                    rule: Rule::ColourLiteral,
                    file: file.to_string(),
                    line: idx + 1,
                    code: line.trim().to_string(),
                    detail: ctor.to_string(),
                });
            }
        }
    }

    if in_scope(file, scope.visual_scope_prefixes) {
        for c in f32_consts(&lines) {
            if !c.is_pub {
                continue;
            }
            if !is_visual_name(c.name) {
                continue;
            }
            match classify_scalar(scope, file, &c) {
                ScalarClass::TokenBound => scan.token_bound += 1,
                ScalarClass::Zero => scan.zero += 1,
                ScalarClass::Ratio => scan.ratio += 1,
                ScalarClass::SurfaceBound => scan.surface_bound += 1,
                ScalarClass::Violation => scan.findings.push(Violation {
                    rule: Rule::UnboundVisualScalar,
                    file: file.to_string(),
                    line: c.line,
                    code: c.decl.trim().to_string(),
                    detail: describe_scalar_violation(&c),
                }),
            }
        }
    }

    if colour_host {
        scan.token_layer_colour_definitions = lines
            .iter()
            .filter(|line| colour_literal_in(line).is_some())
            .count();
    }

    scan
}

/// Scan one file's text under iced-pro's own scope. Pure: no filesystem,
/// no allow-list.
///
/// `file` is the crate-relative path with forward slashes; it decides
/// which rules apply. Feeding a fixture string through here exercises the
/// same code the crate-wide scan runs, which is what makes the injection
/// test meaningful.
#[must_use]
pub fn scan_source(file: &str, text: &str) -> SourceScan {
    scan_source_in(&crate_scope(), file, text)
}

/// Scan every production `.rs` under `root` against an explicit scope and
/// allow-list.
///
/// `root` is the directory the reported paths are relative to, so a host
/// crate passes its own source root and gets host-relative findings. The
/// allow-list is passed in rather than read from a global for the same
/// reason the scope is: the entries belong to whoever owns the tree, and
/// the staleness check below has to run against *that* list.
#[must_use]
pub fn scan_tree(root: &Path, scope: &Scope<'_>, allow: &[AllowEntry]) -> Audit {
    let mut audit = Audit::default();

    for path in source_files(root) {
        let rel = relative(root, &path);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if is_test_file(&rel) {
            continue;
        }

        audit.stats.files_scanned += 1;
        if rel == scope.colour_definition_file {
            audit.stats.rule_a_files += 1;
        }
        if in_scope(&rel, scope.visual_scope_prefixes) {
            audit.stats.rule_b_files += 1;
        }

        let scan = scan_source_in(scope, &rel, &text);
        audit.stats.token_layer_colour_definitions += scan.token_layer_colour_definitions;
        audit.stats.token_bound += scan.token_bound;
        audit.stats.zero += scan.zero;
        audit.stats.ratio += scan.ratio;
        audit.stats.surface_bound += scan.surface_bound;

        for finding in scan.findings {
            match allow.iter().find(|e| e.key == finding.key()) {
                Some(entry) => audit.suppressed.push((finding, entry.reason)),
                None => audit.violations.push(finding),
            }
        }
    }

    audit.stale_allow_entries = allow
        .iter()
        .map(|e| e.key)
        .filter(|key| {
            !audit
                .suppressed
                .iter()
                .any(|(finding, _)| finding.key() == **key)
        })
        .collect();

    audit
}

/// Scan every production `.rs` under `root` and apply the allow-list.
#[must_use]
pub fn scan_and_audit(root: &Path) -> Audit {
    scan_tree(root, &crate_scope(), ALLOW_LIST)
}

/// The crate root, resolved from the manifest directory.
#[must_use]
pub fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Scan the crate this module is compiled into.
#[must_use]
pub fn audit_crate() -> Audit {
    scan_and_audit(&crate_root())
}

/// Render the audit as a plain-text report.
#[must_use]
pub fn render(audit: &Audit) -> String {
    render_scoped("iced-pro", COLOUR_DEFINITION_FILE, audit)
}

/// Render the audit under a caller-supplied label and colour-owner name.
///
/// Same report, different tree: a host crate's gate needs its own header
/// and its own colour-definition filename without copying the report
/// builder and drifting from it.
#[must_use]
pub fn render_scoped(label: &str, colour_file: &str, audit: &Audit) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{label} — token discipline scan (UIF-PRIN-001 / UIF-VER-001)\n"
    ));
    out.push_str(&format!("files scanned: {}\n", audit.stats.files_scanned));
    out.push_str(&format!(
        "colour definitions in {colour_file}: {}\n",
        audit.stats.token_layer_colour_definitions
    ));
    out.push_str(&format!(
        "visual scalars: {} token-bound, {} zero, {} ratio, {} surface-bound\n",
        audit.stats.token_bound, audit.stats.zero, audit.stats.ratio, audit.stats.surface_bound
    ));
    out.push_str(&format!("violations: {}\n", audit.violations.len()));
    for v in &audit.violations {
        out.push_str(&format!(
            "  [{}] {}:{}: {} — {}\n",
            v.rule.label(),
            v.file,
            v.line,
            v.code,
            v.detail
        ));
    }
    out.push_str(&format!("suppressed: {}\n", audit.suppressed.len()));
    for (v, reason) in &audit.suppressed {
        out.push_str(&format!(
            "  {}:{}: {} — {}\n",
            v.file, v.line, v.code, reason
        ));
    }
    out.push_str(&format!(
        "stale allow entries: {}\n",
        audit.stale_allow_entries.len()
    ));
    for key in &audit.stale_allow_entries {
        out.push_str(&format!("  {key}\n"));
    }
    out
}

// ---------------------------------------------------------------------------
// File selection
// ---------------------------------------------------------------------------

/// Every `.rs` file under `root`, sorted by crate-relative path.
#[must_use]
pub fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(root, &mut out);
    out.sort_by_key(|a| relative(root, a));
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    for d in dirs {
        collect(&d, out);
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Test files are out of scope for both rules.
///
/// A contrast solver cannot be tested without raw reference colours —
/// pure red, mid grey, black on white — and no widget definition lives in
/// a test file. That covers the `*_tests.rs` sidecars, the
/// `test_helpers.rs` harness and the `tests/` integration directory for
/// the same reason. Excluding them keeps Rule A's "zero exemptions" honest
/// about what it governs: shipped code, not fixtures.
#[must_use]
pub fn is_test_file(file: &str) -> bool {
    file.ends_with("_tests.rs") || file.ends_with("test_helpers.rs") || file.starts_with("tests/")
}

/// Whether Rule B inspects this file under iced-pro's own visual scope.
#[must_use]
pub fn in_visual_scope(file: &str) -> bool {
    in_scope(file, &VISUAL_SCOPE_DIRS)
}

/// Whether a path falls under any of a scope's visual prefixes.
#[must_use]
pub fn in_scope(file: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|dir| file.starts_with(dir))
}

/// Whether a constant name marks it as a visual measurement.
#[must_use]
pub fn is_visual_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    VISUAL_NAME_MARKERS
        .iter()
        .any(|marker| upper.contains(marker))
}

// ---------------------------------------------------------------------------
// Rule A — colour literals
// ---------------------------------------------------------------------------

/// The colour constructor or hex literal on a redacted line, if any.
fn colour_literal_in(line: &str) -> Option<&'static str> {
    if let Some(idx) = line.find("Color::from_") {
        for ctor in COLOUR_CONSTRUCTORS {
            let tail = &line[idx + "Color::from_".len()..];
            let ctor_tail = &ctor["Color::from_".len()..];
            if tail.starts_with(ctor_tail) && boundary_after(tail, ctor_tail.len()) {
                return Some(ctor);
            }
        }
    }
    hex_colour_literal(line)
}

/// A 6- or 8-digit hex colour literal, outside identifiers.
fn hex_colour_literal(line: &str) -> Option<&'static str> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        let start_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        if start_ok && bytes[i] == b'0' && bytes[i + 1] == b'x' {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
                j += 1;
            }
            let digits = j - (i + 2);
            let end_ok =
                j == bytes.len() || !(bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_');
            if (digits == 6 || digits == 8) && end_ok {
                return Some("hex colour literal");
            }
        }
        i += 1;
    }
    if let Some(rest) = line.split('#').nth(1) {
        let digits: usize = rest.bytes().take_while(|b| b.is_ascii_hexdigit()).count();
        if digits == 6 || digits == 8 {
            return Some("hex colour literal");
        }
    }
    None
}

fn boundary_after(hay: &str, at: usize) -> bool {
    match hay.as_bytes().get(at) {
        None => true,
        Some(b) => !(b.is_ascii_alphanumeric() || *b == b'_'),
    }
}

// ---------------------------------------------------------------------------
// Rule B — visual scalar constants
// ---------------------------------------------------------------------------

/// One parsed `const … : f32 = …;` declaration.
struct F32Const<'a> {
    line: usize,
    decl: &'a str,
    is_pub: bool,
    name: &'a str,
    expr: String,
}

/// Parse every `const … : f32 = …;` out of redacted lines.
///
/// Line-oriented rather than a token stream on purpose: every declaration
/// in this crate is single-line, and a parser that quietly gives up on a
/// construct is more dangerous than one that simply does not look. The
/// sidecar asserts the parser finds the constants it is expected to find.
fn f32_consts<'a>(lines: &[&'a str]) -> Vec<F32Const<'a>> {
    let mut out = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let Some(decl) = parse_f32_const(raw.trim()) else {
            continue;
        };
        out.push(F32Const {
            line: idx + 1,
            decl: raw,
            is_pub: decl.is_pub,
            name: decl.name,
            expr: decl.expr.to_string(),
        });
    }
    out
}

/// The borrowed pieces of one `const … : f32 = …` declaration.
struct Decl<'a> {
    is_pub: bool,
    name: &'a str,
    expr: &'a str,
}

/// Parse a trimmed line into a `const … : f32 = …` declaration.
///
/// Returns `None` for anything that is not a `f32` constant declaration,
/// including `const fn` (the name slot would read `fn`, which is rejected)
/// and non-`f32` types.
fn parse_f32_const<'a>(trimmed: &'a str) -> Option<Decl<'a>> {
    let (is_pub, after_vis) = match trimmed.strip_prefix("pub ") {
        Some(rest) => (true, rest),
        None => (false, trimmed),
    };
    let body = after_vis.strip_prefix("const ")?;
    if body.starts_with("fn ") {
        return None;
    }
    let name = body.split([' ', ':']).next().unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    let tail = body.get(name.len()..)?.trim_start_matches([' ', ':']);
    let (ty, expr) = tail.split_once('=')?;
    if ty.trim() != "f32" {
        return None;
    }
    Some(Decl { is_pub, name, expr })
}

/// Classify one visual scalar constant against a scope's registry.
///
/// The four non-violating classes are the ones the spec itself draws (see
/// the module header); the registry consulted is the scope's, so a host
/// crate registers *its own* fixed bounds rather than borrowing iced-pro's
/// §-references.
fn classify_scalar(scope: &Scope<'_>, file: &str, c: &F32Const<'_>) -> ScalarClass {
    let literals = numeric_literals(&c.expr);
    if literals.is_empty() {
        return ScalarClass::TokenBound;
    }
    if literals.iter().all(|v| *v == 0.0) {
        return ScalarClass::Zero;
    }
    if literals.iter().all(|v| *v > 0.0 && *v < 1.0) {
        return ScalarClass::Ratio;
    }
    if lookup_geometry(scope.surface_geometry, file, c.name).is_some() {
        return ScalarClass::SurfaceBound;
    }
    ScalarClass::Violation
}

fn describe_scalar_violation(c: &F32Const<'_>) -> String {
    let literals = numeric_literals(&c.expr);
    let shown: Vec<String> = literals.iter().map(|v| format_number(*v)).collect();
    format!(
        "`{}` is a visual measurement bound to the literal {} — read a `space::`, \
         `size::`, `shape::` or `typography::` token, or register it as a \
         Surface Geometry Constant with its §-reference if §1.1's composite \
         bound carve-out really applies here",
        c.name,
        if shown.is_empty() {
            "a bare number".to_string()
        } else {
            format!("`{}`", shown.join(", "))
        },
    )
}

/// Every numeric literal in an expression, ignoring identifier digits.
///
/// `space::SM + space::XXS` yields nothing; `16.0` yields `16.0`;
/// `CARD_MIN_HEIGHT - CHROME_HEIGHT` yields nothing. Digits inside an
/// identifier (`CONTROL_MD`, `RADIUS_PILL`, `UI_COMPACT`) are not
/// literals and are never collected.
fn numeric_literals(expr: &str) -> Vec<f64> {
    let b = expr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if !(b[i].is_ascii_digit() && start_ok) {
            i += 1;
            continue;
        }
        let start = i;
        // Hex literals: consume the whole run, keep the raw text.
        if b[i] == b'0' && i + 1 < b.len() && (b[i + 1] == b'x' || b[i + 1] == b'X') {
            let mut j = i + 2;
            while j < b.len() && (b[j].is_ascii_hexdigit() || b[j] == b'_') {
                j += 1;
            }
            let raw: String = expr[start..j].chars().filter(|ch| *ch != '_').collect();
            if let Ok(v) = u64::from_str_radix(raw.trim_start_matches("0x"), 16) {
                out.push(v as f64);
            }
            i = j;
            continue;
        }
        let mut j = i;
        while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'_') {
            j += 1;
        }
        if j < b.len() && b[j] == b'.' {
            j += 1;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'_') {
                j += 1;
            }
        }
        if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
            let mut k = j + 1;
            if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
                k += 1;
            }
            if k < b.len() && b[k].is_ascii_digit() {
                while k < b.len() && (b[k].is_ascii_digit() || b[k] == b'_') {
                    k += 1;
                }
                j = k;
            }
        }
        let raw: String = expr[start..j].chars().filter(|ch| *ch != '_').collect();
        if let Ok(v) = raw.parse::<f64>() {
            out.push(v);
        }
        i = j;
    }
    out
}

fn format_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{:.1}", v)
    } else {
        format!("{v}")
    }
}

// ---------------------------------------------------------------------------
// Comment / string redaction
// ---------------------------------------------------------------------------

/// Blank out comments and string literals, preserving line and column
/// offsets.
///
/// Doc comments quote the very tokens the scan hunts for — `token/color.rs`
/// discusses `Color::from_rgb8`, and a test fixture names `"#808080"` in
/// prose — so scanning raw text would report documentation as code. Every
/// redacted character becomes a space, which keeps line numbers and
/// columns pointing at the same place they did in the input.
#[must_use]
pub fn redact_comments_and_strings(text: &str) -> String {
    let b: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(b.len());
    let mut i = 0;
    let mut block: usize = 0;

    while i < b.len() {
        let c = b[i];

        if block > 0 {
            if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                block += 1;
                out.push(' ');
                out.push(' ');
                i += 2;
                continue;
            }
            if c == '*' && i + 1 < b.len() && b[i + 1] == '/' {
                block -= 1;
                out.push(' ');
                out.push(' ');
                i += 2;
                continue;
            }
            out.push(if c == '\n' { '\n' } else { ' ' });
            i += 1;
            continue;
        }

        // Line comment.
        if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }

        // Block comment start.
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            block = 1;
            out.push(' ');
            out.push(' ');
            i += 2;
            continue;
        }

        // Char literal — redact it, but never let a lifetime quote swallow
        // the rest of the line.
        if c == '\''
            && let Some(len) = char_literal_len(&b, i)
        {
            out.extend(std::iter::repeat_n(' ', len));
            i += len;
            continue;
        }

        // String literal, with or without `r` / `#` delimiters.
        if c == '"' || (c == 'r' && matches_raw_string(&b, i)) {
            let len = string_literal_len(&b, i);
            match len {
                Some(len) => {
                    for k in 0..len {
                        out.push(if b[i + k] == '\n' { '\n' } else { ' ' });
                    }
                    i += len;
                    continue;
                }
                None => {
                    // Unterminated at EOF: redact to the end rather than
                    // mis-reading the tail as code.
                    for ch in &b[i..] {
                        out.push(if *ch == '\n' { '\n' } else { ' ' });
                    }
                    break;
                }
            }
        }

        out.push(c);
        i += 1;
    }

    out.into_iter().collect()
}

fn matches_raw_string(b: &[char], i: usize) -> bool {
    let mut j = i + 1;
    let mut hashes = 0;
    while j < b.len() && b[j] == '#' {
        hashes += 1;
        j += 1;
    }
    j < b.len() && b[j] == '"' && (hashes > 0 || j == i + 1)
}

fn string_literal_len(b: &[char], i: usize) -> Option<usize> {
    if b[i] == '"' {
        let mut j = i + 1;
        while j < b.len() {
            if b[j] == '\\' {
                j += 2;
                continue;
            }
            if b[j] == '"' {
                return Some(j - i + 1);
            }
            j += 1;
        }
        return None;
    }
    // raw string: r"…" or r#"…"#
    let mut j = i + 1;
    let mut hashes = 0;
    while j < b.len() && b[j] == '#' {
        hashes += 1;
        j += 1;
    }
    if j >= b.len() || b[j] != '"' {
        return None;
    }
    j += 1;
    while j < b.len() {
        if b[j] == '"' {
            let mut k = j + 1;
            let mut seen = 0;
            while k < b.len() && b[k] == '#' && seen < hashes {
                k += 1;
                seen += 1;
            }
            if seen == hashes {
                return Some(k - i);
            }
        }
        j += 1;
    }
    None
}

fn char_literal_len(b: &[char], i: usize) -> Option<usize> {
    if b[i] != '\'' {
        return None;
    }
    let mut j = i + 1;
    if j >= b.len() {
        return None;
    }
    if b[j] == '\\' {
        j += 1;
        if j >= b.len() {
            return None;
        }
        if b[j] == 'u' {
            let mut k = j + 1;
            if k < b.len() && b[k] == '{' {
                while k < b.len() && b[k] != '}' {
                    k += 1;
                }
                if k < b.len() && b[k] == '}' {
                    return Some(k + 1 - i);
                }
                return None;
            }
            return None;
        }
        j += 1;
    } else {
        j += 1;
    }
    if j < b.len() && b[j] == '\'' {
        Some(j + 1 - i)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "token_lint_tests.rs"]
mod token_lint_tests;
