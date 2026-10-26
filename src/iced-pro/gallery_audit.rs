//! The contrast audit half of the gallery (`UIF-VER-001`, `UIF-VER-004`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit — and because it was always a different job.
//! The cell constructors *produce* resolved styles and record what they
//! would paint; this half takes those records and decides whether they
//! are **legible**, then renders the report the CI gate reads.
//!
//! # What is here
//!
//! * [`AuditClass`] and [`measure`] — how one [`AuditPair`] is
//!   classified, and the ratio and painted background it was measured
//!   against. Three of the four classes are deliberately not gated, each
//!   for a reason a threshold cannot express.
//! * [`judge`] — the verdict: only [`AuditClass::Judged`] pairs can fail.
//! * [`unmapped_colours`] — the channels no token table can name, which
//!   is the same question from the other side: a colour that reaches a
//!   cell without a token behind it.
//! * [`contrast_failures`] and [`SURFACE_EXCLUSIONS`] — the gated set,
//!   and the channels the renderer protocol says cannot be measured.
//! * [`manifest`] — the text artifact `bin/gallery.rs` writes.
//!
//! Every ratio here is recomputed from raw sRGB through
//! [`crate::token::contrast`]; nothing is a remembered number.
//!
//! The tests for this half are in the gallery's own `gallery_tests.rs`,
//! and in the `unmapped_colours` / `contrast_failures` gates each
//! `gallery_*_tests.rs` sidecar runs over its own cells.
//!
//! [`gallery`]: crate::gallery

use crate::gallery::{AuditPair, Cell, CellKind, ContrastRole, fixed, matrix, rgb8, same_channels};
use crate::token::color::Palette;
use crate::token::contrast;
use iced::Color;

/// How one audit pair is judged.
///
/// Three of the four classes are *not* gated, and each for a reason the
/// threshold itself cannot express. Measuring every produced pair against
/// `3.0:1` regardless of class would fail a stroke that deliberately
/// shares its fill's channels, a `border.subtle` hairline that is by
/// definition not indicator material, and a channel painted over nothing.
/// The classification is the same one the `INC-08` button audit settled
/// on, kept here so the gallery's gate and the widget audits cannot drift
/// into two different definitions of "passes".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditClass {
    /// Judged against the role's threshold.
    Judged,
    /// Nothing is painted behind this channel, so its contrast is the
    /// container's business rather than the style's.
    UnpaintedBackground,
    /// The stroke carries the same channels as the fill it sits on, so it
    /// paints nothing visible. It exists to hold geometry
    /// (`UIF-PRIN-003`), not to signal a state.
    CoincidentWithFill,
    /// `border.subtle` — a **resting** divider. `UIF-TOK-006` rule 2
    /// governs *active* chrome, and this token is not indicator material.
    ///
    /// Matched on the token's own channels rather than on the channel name,
    /// because the gallery meets `border.subtle` under several names —
    /// `border`, `control.border`, `divider` — and the exemption is about
    /// what the token *is*, not what the caller called it. Every
    /// `border_subtle` use in the crate is a resting border or a divider;
    /// no focus ring, selection or destructive indicator is drawn with it,
    /// so the resting-divider class covers the token completely.
    RestingDivider,
}

/// The contrast floor a role owes.
#[must_use]
pub const fn threshold(role: ContrastRole) -> f32 {
    match role {
        ContrastRole::Text => contrast::MIN_TEXT_CONTRAST,
        ContrastRole::Chrome => contrast::MIN_CHROME_CONTRAST,
    }
}

/// Every surface token in the elevation ladder, darkest to lightest.
///
/// Also the container set a translucent fill can be mounted over: a
/// translucent background is judged against the *worst* of these, because
/// the resolved style does not know what it is mounted to and the audit
/// must not assume the friendliest answer.
#[must_use]
pub fn all_surfaces() -> [(&'static str, Color); 7] {
    let palette = Palette::dark();
    [
        ("surface.sunken", palette.surface_sunken),
        ("surface.base", palette.surface_base),
        ("surface.raised", palette.surface_raised),
        ("surface.code", palette.surface_code),
        ("surface.overlay", palette.surface_overlay),
        ("surface.hover", palette.surface_hover),
        ("surface.active", palette.surface_active),
    ]
}

/// Measure one audit pair: the ratio, the painted background it was
/// measured against, and how it is classified.
///
/// For an opaque background the painted background is the background. For
/// a translucent one it is the composite over the worst surface in the
/// ladder, so the reported ratio is the one the cell can actually end up
/// painting rather than the best case.
#[must_use]
pub fn measure(pair: &AuditPair) -> (f32, Color, AuditClass) {
    let palette = Palette::dark();
    let foreground = pair.foreground;
    let background = pair.background;

    if background.a == 0.0 {
        return (f32::NAN, background, AuditClass::UnpaintedBackground);
    }
    if same_channels(foreground, background) {
        return (f32::NAN, background, AuditClass::CoincidentWithFill);
    }
    if same_channels(foreground, palette.border_subtle) {
        return (
            contrast::contrast_ratio(foreground, background),
            background,
            AuditClass::RestingDivider,
        );
    }
    if background.a >= 1.0 {
        return (
            contrast::contrast_ratio(foreground, background),
            background,
            AuditClass::Judged,
        );
    }

    let mut worst_ratio = f32::MAX;
    let mut worst_painted = background;
    for (_, surface) in all_surfaces() {
        let painted = contrast::composite_over(background, surface);
        let ratio = contrast::contrast_ratio(foreground, painted);
        if ratio < worst_ratio {
            worst_ratio = ratio;
            worst_painted = painted;
        }
    }
    (worst_ratio, worst_painted, AuditClass::Judged)
}

/// Whether one audit pair clears its gate.
///
/// Only [`AuditClass::Judged`] pairs can fail; the other classes are
/// reported without a verdict because no threshold describes them.
#[must_use]
pub fn judge(pair: &AuditPair) -> bool {
    let (ratio, _, class) = measure(pair);
    match class {
        AuditClass::Judged => ratio >= threshold(pair.role),
        AuditClass::UnpaintedBackground
        | AuditClass::CoincidentWithFill
        | AuditClass::RestingDivider => true,
    }
}

/// Every colour the gallery emitted that no [`Palette`] token accounts for.
///
/// An empty list is the pass condition for `UIF-VER-001`: it says every
/// pixel the gallery can produce traces back to a named token. A non-empty
/// list is a hard failure, not a ledger entry - an unmapped colour is a
/// value no theme can change, which is the exact defect the token rule
/// exists to prevent.
#[must_use]
pub fn unmapped_colours(cells: &[Cell]) -> Vec<String> {
    let mut out = Vec::new();

    for cell in cells {
        for entry in &cell.resolved {
            if let Some(colour) = entry.value.colour() {
                let name = cell.namer.name(colour);
                if name.starts_with("UNMAPPED") {
                    out.push(format!("{} resolved.{} -> {name}", cell.id, entry.channel));
                }
            }
        }

        for quad in &cell.frame.quads {
            if quad.fill.starts_with("UNMAPPED") {
                out.push(format!(
                    "{} frame.{}.fill -> {}",
                    cell.id, quad.layer, quad.fill
                ));
            }
            if quad.border.starts_with("UNMAPPED") {
                out.push(format!(
                    "{} frame.{}.border -> {}",
                    cell.id, quad.layer, quad.border
                ));
            }
        }

        for pair in &cell.audit {
            for (slot, colour) in [
                ("foreground", pair.foreground),
                ("background", pair.background),
            ] {
                let name = cell.namer.name(colour);
                if name.starts_with("UNMAPPED") {
                    out.push(format!(
                        "{} audit.{}.{slot} -> {name}",
                        cell.id, pair.channel
                    ));
                }
            }
        }
    }

    out
}

/// Every audit pair that fails its role's contrast threshold.
///
/// The companion to [`unmapped_colours`]: where an unmapped colour is a
/// token defect, this is a legibility defect. The list is empty today, and
/// keeping it empty is what makes the gallery a gate rather than a report.
#[must_use]
pub fn contrast_failures(cells: &[Cell]) -> Vec<String> {
    let mut out = Vec::new();
    for cell in cells {
        for pair in &cell.audit {
            if judge(pair) {
                continue;
            }
            let (ratio, painted, _) = measure(pair);
            out.push(format!(
                "{} {} {} on {} (role {:?}, ratio {}, worst-painted {})",
                cell.id,
                pair.channel,
                cell.namer.name(pair.foreground),
                cell.namer.name(pair.background),
                pair.role,
                fixed(ratio),
                rgb8(painted),
            ));
        }
    }
    out
}

/// Channels the spec's matrices name that no recorded frame can carry.
///
/// Recorded rather than left absent, so the manifest's silence is never
/// misread as coverage. The iced text caret is the one such channel: the
/// backend's text pass draws it after the widget's own layers, and nothing
/// about it — bounds, fill, token — crosses the
/// [`iced::advanced::renderer::Renderer`] protocol a
/// [`crate::gallery::FrameRecorder`] can see. Snapshotting it would mean the
/// gallery inventing geometry the framework does not own, which is the
/// exact thing `UIF-VER-001` exists to prevent. It stays out of the
/// snapshot surface for the life of the recorder-based gallery.
pub const SURFACE_EXCLUSIONS: [(&str, &str); 1] = [(
    "text.caret",
    "Drawn by the iced text backend rather than by a Layer: no bounds, no \
     fill and no token reaches the renderer protocol, so it cannot be \
     snapshotted without inventing framework geometry the crate does not \
     own. Permanently outside the recorder-based gallery's surface.",
)];

/// Render the whole gallery as the manifest text `bin/gallery` writes.
///
/// A header, then one [`Cell::snapshot`] per cell, then the gate ledgers.
/// The gate output is part of the manifest so an inspectable artifact
/// carries its own pass condition rather than leaving the reader to
/// re-derive it.
#[must_use]
pub fn manifest() -> String {
    let cells = matrix();

    let mut out = String::new();
    out.push_str("iced-pro visual gallery - UIF-VER-004\n");
    out.push_str("snapshot target: resolved style values + recorded quad list\n");
    out.push_str(&format!("cells: {}\n\n", cells.len()));

    for cell in &cells {
        out.push_str(&cell.snapshot());
        out.push('\n');
    }

    out.push_str("== gate: token mapping (UIF-VER-001) ==\n");
    let unmapped = unmapped_colours(&cells);
    if unmapped.is_empty() {
        out.push_str("PASS - every emitted colour names a Palette token\n");
    } else {
        out.push_str("FAIL\n");
        for line in unmapped {
            out.push_str(&format!("  {line}\n"));
        }
    }

    out.push_str("\n== gate: contrast (UIF-TOK-006) ==\n");
    let failures = contrast_failures(&cells);
    if failures.is_empty() {
        out.push_str("PASS - every judged pair clears its role threshold\n");
    } else {
        out.push_str("FAIL\n");
        for line in failures {
            out.push_str(&format!("  {line}\n"));
        }
    }

    out.push_str("\n== gate: invariants (UIF-VER-005) ==\n");
    let invariants = crate::invariant_gate::audit_invariants();
    if invariants.is_clean() {
        out.push_str(&format!(
            "PASS - all 7 automated invariants satisfied ({} checks verified)\n",
            invariants.total_checked
        ));
    } else {
        out.push_str(&format!(
            "FAIL - {} violation(s):\n",
            invariants.violation_count()
        ));
        for v in &invariants.violations {
            out.push_str(&format!(
                "  [{}] {}: {}\n",
                v.category.slug(),
                v.target,
                v.message
            ));
        }
    }

    out.push_str("\n== gate: scope (D-2) ==\n");
    let not_owned: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.kind == CellKind::NotOwned)
        .collect();
    out.push_str(&format!(
        "{} surfaces emitted as NotOwned rather than omitted\n",
        not_owned.len()
    ));
    for cell in not_owned {
        out.push_str(&format!("  {} - {}\n", cell.id, cell.spec));
    }

    out.push_str("\n== recorded exclusions: named channels with no frame ==\n");
    out.push_str(&format!(
        "{} channels recorded as unreachable from a snapshot\n",
        SURFACE_EXCLUSIONS.len()
    ));
    for (channel, why) in SURFACE_EXCLUSIONS {
        out.push_str(&format!("  {channel} - {why}\n"));
    }

    out
}
