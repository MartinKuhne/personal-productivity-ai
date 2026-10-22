//! Machine-readable coverage of every `UIF-*` requirement in
//! `doc/planning/ui-framework.md`, plus the audit primitive that keeps
//! that coverage from silently going stale.
//!
//! # Why a manifest instead of a document
//!
//! A coverage table written in a Markdown file is a claim. The same table
//! compiled into the crate and compared against the spec at test time is a
//! *guard*. This module is the latter: [`manifest`] is the single source of
//! truth for what this crate does and does not satisfy, [`audit`] compares
//! it against the IDs actually present in the spec, and the sidecar test
//! runs that comparison on every `cargo nextest` run. Add an ID to the
//! spec without classifying it here and the build fails; drop one and the
//! stale manifest entry fails too.
//!
//! # The three statuses
//!
//! The taxonomy is deliberately narrow and each label means one thing:
//!
//! * [`Status::Implemented`] — the crate satisfies the requirement as
//!   written, with nothing outstanding on the framework's side. Such an
//!   entry carries **no** `reason`; anything a reader still has to know
//!   goes in `note`.
//! * [`Status::Partial`] — the crate satisfies part of the requirement, or
//!   satisfies it while deviating from the spec's literal text. `reason`
//!   **must** name exactly what is missing or divergent. A `Partial` whose
//!   reason would not tell an integrator what to go and finish is a failed
//!   entry.
//! * [`Status::NotOwned`] — the requirement sits outside this crate under
//!   decision **D-2** (framework layer only). `reason` **must** name the
//!   owner and say what the framework *does* contribute toward it, so
//!   "not built" is never mistaken for "out of scope".
//!
//! `Partial` is not a failure state and `NotOwned` is not an apology. Both
//! are findings, and each one is worth exactly what it can tell the reader.
//!
//! # What this module is not
//!
//! It asserts nothing about the code it describes. It is a ledger of
//! *claims*, cross-referenced by module path to the code that earns them.
//! The claims are checked against the spec's ID set mechanically; they are
//! backed by the crate's own tests, not by this file. A `Implemented` here
//! whose tests were deleted would still render green — which is why every
//! entry names the modules a reader can go and read.
//!
//! Unit tests live in the sibling `spec_coverage_tests.rs` sidecar.

use std::fmt::Write as _;

/// Total number of numbered `UIF-*` requirement IDs the spec carries.
///
/// Pinned so a spec edit that renames, renumbers or silently adds an ID
/// fails the audit loudly rather than shifting a total nobody checks. The
/// count is of *numbered* IDs only: a family wildcard such as
/// `UIF-ZORD-*` in a section heading is not a requirement and is not
/// counted (see [`extract_spec_ids`]).
pub const EXPECTED_SPEC_ID_COUNT: usize = 47;

/// Where one `UIF-*` requirement stands against this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    /// Satisfied as written; nothing outstanding on the framework's side.
    Implemented,
    /// Satisfied in part, or satisfied with a recorded deviation from the
    /// spec's literal text. `Coverage::reason` names what is missing.
    Partial,
    /// Out of scope under decision **D-2**. `Coverage::reason` names the
    /// owner and what the framework still contributes.
    NotOwned,
}

impl Status {
    /// The column label for this status.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Implemented => "Implemented",
            Self::Partial => "Partial",
            Self::NotOwned => "NotOwned",
        }
    }
}

/// One audited requirement.
///
/// Every field is `&'static` so the whole ledger is const data compiled
/// into the crate rather than something assembled at run time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    /// The requirement ID exactly as the spec prints it, e.g.
    /// `UIF-CMENU-001`.
    pub id: &'static str,
    /// How far this crate satisfies it.
    pub status: Status,
    /// The spec section the requirement lives in, e.g. `§4.9.3`.
    pub section: &'static str,
    /// The modules that satisfy — or partially satisfy — the requirement.
    /// Empty only for a [`Status::NotOwned`] entry with nothing to point
    /// at, which the renderer prints as `(none)`.
    pub evidence: &'static [&'static str],
    /// `Partial` / `NotOwned`: exactly what is missing, or why the
    /// requirement is out of scope. **Must** be `None` for
    /// [`Status::Implemented`], so a gap can never be hidden inside an
    /// `Implemented` row.
    pub reason: Option<&'static str>,
    /// A caveat a reader must know even though nothing is outstanding —
    /// a deliberate deviation, or a responsibility that stays with the
    /// host. Optional on every status.
    pub note: Option<&'static str>,
}

impl Coverage {
    /// Whether this entry still owes the framework work or a spec revision.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        !matches!(self.status, Status::Implemented)
    }
}

/// The whole ledger, in spec order, cloned out of const data.
///
/// Two calls return the same entries in the same order.
#[must_use]
pub fn manifest() -> Vec<Coverage> {
    SPEC_COVERAGE.to_vec()
}

/// The ledger entry for one requirement id, if this crate classifies it.
#[must_use]
pub fn find(id: &str) -> Option<&'static Coverage> {
    SPEC_COVERAGE.iter().find(|entry| entry.id == id)
}

/// The status recorded for one requirement id.
#[must_use]
pub fn status_of(id: &str) -> Option<Status> {
    find(id).map(|entry| entry.status)
}

/// Every fully satisfied requirement.
#[must_use]
pub fn implemented() -> Vec<Coverage> {
    by_status(Status::Implemented)
}

/// Every requirement satisfied in part, or with a recorded deviation.
#[must_use]
pub fn partial() -> Vec<Coverage> {
    by_status(Status::Partial)
}

/// Every requirement excluded under decision **D-2**.
#[must_use]
pub fn not_owned() -> Vec<Coverage> {
    by_status(Status::NotOwned)
}

/// Everything that is not fully satisfied — the `Partial` and `NotOwned`
/// entries together, in ledger order.
#[must_use]
pub fn unimplemented() -> Vec<Coverage> {
    manifest().into_iter().filter(Coverage::is_open).collect()
}

fn by_status(status: Status) -> Vec<Coverage> {
    manifest()
        .into_iter()
        .filter(|entry| entry.status == status)
        .collect()
}

/// The outcome of comparing a spec's extracted ids against the ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditReport {
    /// Ids the spec requires that this ledger does not classify.
    pub missing_in_manifest: Vec<String>,
    /// Ledger entries the spec no longer contains.
    pub stale_in_manifest: Vec<String>,
    /// How many numbered ids the spec text carried.
    pub spec_total: usize,
    /// The total the ledger expects the spec to carry.
    pub expected_total: usize,
}

impl AuditReport {
    /// Whether the ledger and the spec agree exactly.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.missing_in_manifest.is_empty()
            && self.stale_in_manifest.is_empty()
            && self.spec_total == self.expected_total
    }

    /// A human-readable list of everything wrong, empty when clean.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for id in &self.missing_in_manifest {
            let _ = writeln!(
                out,
                "{id} is required by the spec but classified by nothing — give it a \
                 Status (Implemented / Partial / NotOwned) in spec_coverage.rs"
            );
        }
        for id in &self.stale_in_manifest {
            let _ = writeln!(
                out,
                "{id} is in the ledger but no longer exists in the spec — stale entry"
            );
        }
        if self.spec_total != self.expected_total {
            let _ = writeln!(
                out,
                "spec carries {} numbered UIF-* ids, ledger expects {}",
                self.spec_total, self.expected_total
            );
        }
        out
    }
}

/// Compare extracted spec ids against the ledger, in both directions.
///
/// Pure: it takes the ids [`extract_spec_ids`] produced and never touches
/// the filesystem, so the same function backs the real audit against
/// `ui-framework.md` and the injected-failure test that proves the guard
/// actually fires.
#[must_use]
pub fn audit(spec_ids: &[String]) -> AuditReport {
    let missing_in_manifest = spec_ids
        .iter()
        .filter(|id| find(id).is_none())
        .cloned()
        .collect();
    let stale_in_manifest = SPEC_COVERAGE
        .iter()
        .map(|entry| entry.id.to_string())
        .filter(|id| !spec_ids.iter().any(|spec| spec == id))
        .collect();

    AuditReport {
        missing_in_manifest,
        stale_in_manifest,
        spec_total: spec_ids.len(),
        expected_total: EXPECTED_SPEC_ID_COUNT,
    }
}

/// Pull every numbered `UIF-*` id out of spec text, in first-seen order.
///
/// An id is `UIF-<FAMILY>-<DIGITS>`, where `FAMILY` is one or more ASCII
/// uppercase letters and `DIGITS` is one or more ASCII digits, delimited
/// on both sides by anything that is not `[A-Z0-9-]`.
///
/// That shape deliberately rejects the **family wildcards** the spec uses
/// in section headings — `UIF-TOK-*`, `UIF-ZORD-*` — because a wildcard
/// names a family, not a requirement. `UIF-ZORD-*` in particular heads
/// §5 while §5 numbers nothing, so counting it would invent a requirement
/// the spec never wrote.
#[must_use]
pub fn extract_spec_ids(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let prefix = ['U', 'I', 'F', '-'];
    let mut ids: Vec<String> = Vec::new();
    let mut i = 0;

    while i + prefix.len() <= chars.len() {
        let starts_here =
            chars[i..i + prefix.len()] == prefix && (i == 0 || !is_boundary_char(chars[i - 1]));
        if !starts_here {
            i += 1;
            continue;
        }

        let mut end = i + prefix.len();
        while end < chars.len() && is_run_char(chars[end]) {
            end += 1;
        }
        // A run butted up against a letter or digit on either side is a
        // fragment of a larger word, not a standalone id.
        if end < chars.len() && is_boundary_char(chars[end]) {
            i = end;
            continue;
        }

        let run: String = chars[i..end].iter().collect();
        if is_numbered_id(&run) && !ids.contains(&run) {
            ids.push(run);
        }
        i = end;
    }

    ids
}

/// A character that continues an id run.
fn is_run_char(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'
}

/// A character that makes an adjacent run part of a larger word rather
/// than a standalone id: `preUIF-TOK-002` and `UIF-TOK-001x` are
/// fragments, while `` `UIF-TOK-001` `` and `UIF-TOK-001,` are not.
fn is_boundary_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// `UIF-<FAMILY>-<DIGITS>` with both variable parts non-empty.
fn is_numbered_id(run: &str) -> bool {
    let mut parts = run.split('-');
    let Some(head) = parts.next() else {
        return false;
    };
    if head != "UIF" {
        return false;
    }
    let Some(family) = parts.next() else {
        return false;
    };
    let Some(number) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    !family.is_empty()
        && family.chars().all(|c| c.is_ascii_uppercase())
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Column widths for [`render_table`], fixed rather than content-derived so
/// the rendered text is byte-stable as entries change.
const ID_COL: usize = 14;
/// Status column width.
const STATUS_COL: usize = 12;
/// Section column width.
const SECTION_COL: usize = 14;
/// Evidence column width.
const EVIDENCE_COL: usize = 34;
/// Detail (reason / note) column width.
const DETAIL_COL: usize = 62;

/// Render entries as a plain-text table.
///
/// Deterministic: the same entries always produce the same bytes. The
/// ledger and a human read the same rendering, so nothing has to be kept
/// in step by hand.
#[must_use]
pub fn render_table(entries: &[Coverage]) -> String {
    let rule = "-".repeat(ID_COL + STATUS_COL + SECTION_COL + EVIDENCE_COL + DETAIL_COL + 4);
    let mut out = String::new();
    let _ = writeln!(out, "iced-pro — UIF-* spec coverage ledger");
    let _ = writeln!(out, "entries: {}", entries.len());
    let _ = writeln!(out, "{rule}");
    let _ = writeln!(
        out,
        "{:<ID_COL$}{:<STATUS_COL$}{:<SECTION_COL$}{:<EVIDENCE_COL$}DETAIL",
        "ID", "STATUS", "SECTION", "EVIDENCE"
    );
    let _ = writeln!(out, "{rule}");

    for entry in entries {
        let evidence = if entry.evidence.is_empty() {
            "(none)".to_string()
        } else {
            entry.evidence.join(", ")
        };

        let mut detail: Vec<String> = Vec::new();
        if let Some(reason) = entry.reason {
            detail.extend(wrap(&format!("reason: {reason}"), DETAIL_COL));
        }
        if let Some(note) = entry.note {
            detail.extend(wrap(&format!("note: {note}"), DETAIL_COL));
        }
        if detail.is_empty() {
            detail.push("-".to_string());
        }

        let evidence_lines = wrap(&evidence, EVIDENCE_COL);
        let rows = evidence_lines.len().max(detail.len());

        for row in 0..rows {
            let head = if row == 0 { entry.id } else { "" };
            let status = if row == 0 { entry.status.label() } else { "" };
            let section = if row == 0 { entry.section } else { "" };
            let ev = evidence_lines.get(row).map(String::as_str).unwrap_or("");
            let de = detail.get(row).map(String::as_str).unwrap_or("");
            let _ = writeln!(
                out,
                "{:<ID_COL$}{:<STATUS_COL$}{:<SECTION_COL$}{:<EVIDENCE_COL$}{de}",
                head, status, section, ev
            );
        }
    }

    out.push_str(&rule);
    out.push('\n');
    out
}

/// Render the whole ledger.
#[must_use]
pub fn render() -> String {
    render_table(&manifest())
}

/// Greedy word wrap to `width` characters.
///
/// Breaks on whitespace only; a word longer than the width is emitted on a
/// line of its own rather than hyphenated, so no token is ever split.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ---------------------------------------------------------------------------
// The ledger
// ---------------------------------------------------------------------------

const SPEC_COVERAGE: &[Coverage] = &[
    // --- §1.1 Core Design Pillars -------------------------------------------
    Coverage {
        id: "UIF-PRIN-001",
        status: Status::Partial,
        section: "§1.1",
        evidence: &["token/", "gallery::unmapped_colours"],
        reason: Some(
            "Token discipline holds by construction — every visual number is declared \
             under `token/` and nothing else — and the gallery gate proves every colour \
             an audited cell emits names a `Palette` token. Missing: no source-level \
             check forbids a raw spacing, size, radius or stroke literal in a widget or \
             surface file; the gate inspects emitted colours, not the crate's text. \
             G-12 also records values (10px menu-row padding, 9px separator, \
             16px grip) that exist only as compositions, not named rungs. The footer is \
              no longer one of them — the revision supplied `size.bar.status` and it \
              now reads that token.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-PRIN-002",
        status: Status::Implemented,
        section: "§1.1",
        evidence: &["token/size.rs", "widget/button.rs", "widget/text_input.rs"],
        reason: None,
        note: Some(
            "Control rungs 28 / 32 / 36 with the 24x24 floor asserted against every \
             rung and against the icon-only minimum.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-003",
        status: Status::Implemented,
        section: "§1.1",
        evidence: &[
            "widget/text_input.rs",
            "surface/input_dialog.rs",
            "surface/menu/view.rs",
            "widget/button.rs",
        ],
        reason: None,
        note: Some(
            "Structural, not conventional: `InputLayout::solve` takes no \
             'is an error showing' parameter, the input's trailing slot is reserved per \
             kind, and the menu row's 16px leading box is reserved whether or not the \
             row carries a glyph.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-004",
        status: Status::Partial,
        section: "§1.1",
        evidence: &["focus.rs", "surface/menu/model.rs"],
        reason: Some(
            "The 'unavailable actions stay visible' half is modelled: a disabled focus \
             member keeps its ordinal slot and traversal steps over it, and Disabled is \
             a flag on a row rather than a row kind, so label, command identity and \
             close policy all survive greying out. Missing: the 'empty panels and \
             filtered views show structured placeholder states rather than collapsing \
             to zero dimensions' half is app-owned pane content under D-2 — this crate \
             has no empty-state or placeholder component.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-PRIN-005",
        status: Status::Partial,
        section: "§1.1",
        evidence: &[
            "focus.rs",
            "bridge.rs",
            "widget/focusable.rs",
            "surface/menu/cascade.rs",
        ],
        reason: Some(
            "The keyboard half is proven for every framework-owned atom, the menu \
             engine and the dialog hosts: D-7 custom focusable widgets, a one-way \
             model-to-router bridge, roving focus and the modal trap. Missing: \
             redundant cueing is honoured only where this crate paints (check and radio \
             glyphs, a `destructive` flag rather than a colour, the search-to-clear \
             glyph swap). Nothing checks that a meaning is never carried by colour \
             alone, and the app-owned surfaces the rule also covers — the tab dirty dot \
             and the navbar status pill — are outside the crate.",
        ),
        note: None,
    },
    // --- §2 Foundation Design Tokens ----------------------------------------
    Coverage {
        id: "UIF-TOK-001",
        status: Status::Implemented,
        section: "§2.1",
        evidence: &["token/space.rs"],
        reason: None,
        note: Some("Seven rungs on the base-4 grid with `XXS` as the single 2px half-step."),
    },
    Coverage {
        id: "UIF-TOK-002",
        status: Status::Implemented,
        section: "§2.2",
        evidence: &["token/size.rs"],
        reason: None,
        note: Some(
            "`HIT_TARGET_MIN` is modelled as the rule the control and icon rungs are \
             checked against, not as a size to pick.",
        ),
    },
    Coverage {
        id: "UIF-TOK-003",
        status: Status::Implemented,
        section: "§2.3",
        evidence: &["token/shape.rs"],
        reason: None,
        note: None,
    },
    Coverage {
        id: "UIF-TOK-004",
        status: Status::Partial,
        section: "§2.4",
        evidence: &["token/typography.rs"],
        reason: Some(
            "The six rungs carry size, line height, resting and emphasis weight and \
             tracking, and the 12px floor is a token. Missing: the scale models no font \
             *family*, so the monospace the spec requires for code, paths, timestamps \
             and log views (§2.4, §4.7.1) has no rung at all; and there is no 12px \
             SemiBold or 13px Bold rung, so §4.9.1's section heading and §4.7.1's \
             overlay title are composed as `CAPTION.size + Weight::Semibold` and \
             `UI_COMPACT.size + Weight::Bold` rather than being rungs (ledger G-12).",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-TOK-005",
        status: Status::Implemented,
        section: "§2.5",
        evidence: &["token/color.rs"],
        reason: None,
        note: Some(
            "The 2026-10-22 revision adopted the polarity split this crate argued for \
             at D-5 and D-6, so the earlier value deviation is closed: every token in \
             §2.5 now exists with the exact sRGB the spec prints, across `Palette` and \
             the separate `EditorPalette`. The set is the six-token polarity model — \
             `accent.{fill,fill_active,border}` and \
             `status.danger.{text,fill,fill_active}`. One printed figure in the spec \
             is wrong and the crate deliberately carries the true value instead: \
             `status.danger.fill_active` luminance is `0.1325`, not the `0.1309` the \
             table prints. Every other figure in this crate was independently \
             recomputed rather than transcribed.",
        ),
    },
    Coverage {
        id: "UIF-TOK-006",
        status: Status::Partial,
        section: "§2.5 rules",
        evidence: &["token/contrast.rs"],
        reason: Some(
            "Rules 1 and 3 are implemented as thresholds (4.5 / 3.0) over a WCAG 2.1 \
             luminance and ratio solver that composites translucent layers before \
             measuring, and both pass the full matrix against the revised palette. \
             Rule 3's four-class chrome classification (`Judged`, `CoincidentWithFill`, \
             `UnpaintedBackground`, `RestingDivider`) is modelled, which settles the \
             old reading problem around `border.subtle` — it is a `RestingDivider` and \
             exempt by class, not by an informal reading of the word 'active'. \
             Missing: rule 4's Selection Foreground Lock is honoured by construction in \
             every resolver but is not *enforced* — nothing forbids a non-`text.strong` \
             foreground, or a foreground-only `status.*`/`accent.text` colour, on a \
             solid `*.fill`. Two further cells fail the spec's own thresholds and are \
             pinned as known conflicts rather than patched: rule 2's `[4.00, 6.00]` \
             band is breached by `text.disabled` on `surface.sunken` (6.0205), and \
             rule 3's `CoincidentWithFill` floor of 3.08 is missed by \
             `status.danger.fill_active` on `surface.overlay` (3.0789).",
        ),
        note: None,
    },
    // --- §3 Atomic Widget State Matrices ------------------------------------
    Coverage {
        id: "UIF-ATOM-000",
        status: Status::Implemented,
        section: "§3.0",
        evidence: &["state.rs", "widget/focus.rs", "widget/button_widget.rs"],
        reason: None,
        note: Some(
            "The two-part orthogonal model is the crate's foundation: `VisualState \
             { base, focused }`, base precedence `Disabled > Invalid > Pressed > \
             Hovered > Rest`, and total disabled dominance — a disabled control \
             forces `focused` false and focus advances to the next enabled control. \
             The nine reachable states are the gallery's nine columns. Press-and-release \
             activation is implemented: the button fires only on release still inside \
             its bounds and a drag-out cancels. Both placements of the focus indicator \
             exist and share one adaptive colour rule: the 2px outer ring for buttons \
             and inputs, and the 1px inset outline for dense rows, whose colour is \
             resolved against the row's actual fill rather than named per row type.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001",
        status: Status::Partial,
        section: "§3.1",
        evidence: &[
            "widget/button.rs",
            "widget/button_widget.rs",
            "widget/focus.rs",
            "state.rs",
        ],
        reason: Some(
            "All four variants resolve and paint across the reachable state matrix, \
             with the D-5 danger-fill split and the D-6 adaptive ring. Missing: §3.1's \
             geometry note requires an icon-only button to display a hover tooltip. \
             The 28x28 minimum is enforced; the tooltip is not implemented anywhere in \
             this crate (see UIF-NAV-003).",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-002",
        status: Status::Implemented,
        section: "§3.2",
        evidence: &["widget/text_input.rs", "widget/focusable.rs"],
        reason: None,
        note: Some(
            "Three kinds (entry, search, dropdown trigger) across the five painted \
             states; the focused placeholder/icon and invalid-placeholder cells are \
             derived and documented because §3.2 leaves them open. The text caret \
             itself is iced's and is not a snapshot cell.",
        ),
    },
    // --- §4.1 Main Window --------------------------------------------------
    Coverage {
        id: "UIF-WIN-001",
        status: Status::NotOwned,
        section: "§4.1.2",
        evidence: &["geometry/mod.rs", "surface/movable.rs"],
        reason: Some(
            "App-owned under D-2. Window dimensions, screen position, split-pane \
             ratios, the 0.75x-2.00x UI-scale multiplier and their cross-session \
             persistence belong to the host. The framework contributes the viewport as \
             a `Rectangle` every solver is written against, so the geometry stays \
             correct down to the 640x480 floor, and an injected `GeometryStorage` seam \
             — but that seam is scoped to movable-overlay geometry only. There is no \
             window-geometry persistence here and no pane-minimum policy.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-WIN-002",
        status: Status::Implemented,
        section: "§4.1.2",
        evidence: &["geometry/mod.rs", "geometry/anchor.rs", "surface/zorder.rs"],
        reason: None,
        note: Some(
            "The viewport is taken as a `Rectangle`, not a bare `Size`, so a content \
             area that does not start at the origin still solves; every rule is written \
             against the rectangle's four inset edges.",
        ),
    },
    Coverage {
        id: "UIF-WIN-003",
        status: Status::Implemented,
        section: "§4.1.2",
        evidence: &[
            "surface/zorder.rs",
            "surface/menu/cascade.rs",
            "surface/modal.rs",
            "surface/input_dialog.rs",
        ],
        reason: None,
        note: Some(
            "Steps 1-3 (deepest submenu, root menu, topmost dialog) are native and run \
             on `escape_rank`, which is deliberately **not** band rank (G-13): the \
             topmost-painted surface is not the first asked to handle Escape. Step 4 \
             (search filter / in-flight prompt) is app content — the registry's unwind \
             closure is caller-supplied, so an app-registered surface joins the same \
             ordered chain, and an empty stack returns `EscapeOutcome::Ignored`.",
        ),
    },
    // --- §4.2 Top Navbar ---------------------------------------------------
    Coverage {
        id: "UIF-NAV-001",
        status: Status::Partial,
        section: "§4.2.2",
        evidence: &["surface/menu/cascade.rs"],
        reason: Some(
            "The latch's behaviour is framework-owned and implemented: \
             `MenuCascade::toggle` closes an already-open root menu rather than \
             reopening it. Missing: the navbar trigger button itself, and the held \
             `surface.active` plus `border.strong` paint for as long as the menu stays \
             open, are app-owned composition under D-2. No navbar module exists in this \
             crate.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-NAV-002",
        status: Status::NotOwned,
        section: "§4.2.2",
        evidence: &[],
        reason: Some(
            "App-owned under D-2. The ellipsis convention governs navbar button labels \
             and icon tooltips, which are app content. This crate renders no navbar \
             label and validates none, and contributes no text-formatting helper for \
             the convention.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-NAV-003",
        status: Status::Partial,
        section: "§4.2.2",
        evidence: &["token/color.rs", "token/shape.rs", "token/typography.rs"],
        reason: Some(
            "Every token a tooltip card is made of exists — `surface.overlay` fill, \
             `border.strong` perimeter, `type.caption` text, the 4px offset. Missing: \
             the tooltip surface itself. This crate has no hover-delay timer, no \
             floating card and no placement or flip rule. D-2 excludes the navbar's \
             contents, not a generic hover-delayed floating card, so this is \
             unimplemented framework work the host currently has to carry itself.",
        ),
        note: None,
    },
    // --- §4.3 Multiple Panes ----------------------------------------------
    Coverage {
        id: "UIF-PANE-001",
        status: Status::NotOwned,
        section: "§4.3.2",
        evidence: &["token/color.rs", "surface/movable.rs"],
        reason: Some(
            "App-owned under D-2: no splitter or divider module exists. The framework \
             contributes the tokens the divider paints with (`border.subtle` at rest, \
             `accent.primary` on hover and drag) and the drag state-machine pattern \
             proven in the movable overlay, but no divider widget, no 10px grab-zone \
             geometry and no resize-cursor contract.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-PANE-002",
        status: Status::NotOwned,
        section: "§4.3.2",
        evidence: &[],
        reason: Some(
            "App-owned under D-2: auto-fit to the longest visible label, the \
             [150px, 20% viewport] clamp and the conditional mounting of the Table of \
             Contents pane are workspace policy. `geometry::clamp` solves *overlay \
             title-bar reachability*, not pane proportions, and this crate owns no pane \
             width policy at all.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-PANE-003",
        status: Status::NotOwned,
        section: "§4.3.2",
        evidence: &[
            "widget/focusable.rs",
            "widget/text_input.rs",
            "token/size.rs",
        ],
        reason: Some(
            "App-owned under D-2: the directory tree, its folder-first ordering and \
             tag/content filtering, row ergonomics, single and range selection and the \
             pinned search footer are app content. The framework supplies the pieces a \
             row is built from — `Focusable` / `FocusableState` for row focusability, \
             the `Search` input kind with its idle-to-clear glyph swap, and the 24px \
             row floor — but no tree, no selection model and no filter.",
        ),
        note: None,
    },
    // --- §4.4 Tabbed Windows -----------------------------------------------
    Coverage {
        id: "UIF-TAB-001",
        status: Status::NotOwned,
        section: "§4.4.3",
        evidence: &["surface/menu/", "focus.rs"],
        reason: Some(
            "App-owned under D-2: no tab-strip module exists, and a tab is deliberately \
             not a variant of the §3.1 button. The framework supplies the menu engine a \
             Tab Context Menu would be built from and the focus plumbing a tab would \
             register with, but owns no tab body, no middle-click close and no \
             right-click binding.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-TAB-002",
        status: Status::NotOwned,
        section: "§4.4.3",
        evidence: &[],
        reason: Some(
            "App-owned under D-2: the strip, its horizontal scroll container, the 96px \
             minimum tab width and the preserved 24x24 close target are tab-strip \
             internals this crate was never given, so there is no framework geometry to \
             satisfy or test.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-TAB-003",
        status: Status::Partial,
        section: "§4.4.3",
        evidence: &["surface/modal.rs", "surface/input_dialog.rs"],
        reason: Some(
            "The confirmation mechanism a dirty close needs is framework-owned and \
             implemented — the modal's dirty guard (UIF-MOD-002) refuses a silent \
             dismiss, and `DeleteTargets` with `InputDialogHost` already model the \
             plural confirmation a 'Close All' raises. Missing: no tab strip exists to \
             raise it, and external-deletion sync — a tab whose file vanishes closes \
             and hands focus to the most-recently-active remaining tab — is app-owned.",
        ),
        note: None,
    },
    // --- §4.5 Input Dialogs ------------------------------------------------
    Coverage {
        id: "UIF-INP-001",
        status: Status::Implemented,
        section: "§4.5.2",
        evidence: &["geometry/anchor.rs", "surface/input_dialog.rs"],
        reason: None,
        note: Some(
            "`Anchor::CursorProximal` carries the +2/+2 offset and the 8px margin from \
             `space::XXS` / `space::SM`, and its `keyboard_invoked` flag centres the \
             card in the viewport when no pointer invoked the dialog.",
        ),
    },
    Coverage {
        id: "UIF-INP-002",
        status: Status::Implemented,
        section: "§4.5.2",
        evidence: &["surface/input_dialog.rs"],
        reason: None,
        note: Some(
            "The card rectangle is tested first in `on_pointer`, so no arm behind that \
             test can reach the scrim — a click on a disabled `Ok` cannot leak through \
             a boundary it was never outside of.",
        ),
    },
    Coverage {
        id: "UIF-INP-003",
        status: Status::Implemented,
        section: "§4.5.2",
        evidence: &["surface/input_dialog.rs", "widget/text_input.rs"],
        reason: None,
        note: Some(
            "Validation is injected data (`ValidationRule::{Name, Rename}`) over the \
             host's illegal-basename set; `InputLayout::solve` has no 'error showing' \
             parameter, so the reserved slot cannot shift the footer, and `Enter` on an \
             invalid value reveals the message while focus stays in the field.",
        ),
    },
    Coverage {
        id: "UIF-INP-004",
        status: Status::Implemented,
        section: "§4.5.2",
        evidence: &["surface/input_dialog.rs"],
        reason: None,
        note: Some(
            "`Selection::rename_stem` selects the filename stem on open (a dotfile \
             keeps its whole name); destructive parity is `ButtonIntent::Danger` plus \
             `DeleteTargets::{Single, Multiple}` on the same host, not a second dialog.",
        ),
    },
    // --- §4.6 Modal Dialogs ------------------------------------------------
    Coverage {
        id: "UIF-MOD-001",
        status: Status::Implemented,
        section: "§4.6.3",
        evidence: &["focus.rs", "bridge.rs", "surface/modal.rs"],
        reason: None,
        note: Some(
            "`push_trap` confines traversal to the card's own focus group and traps \
             nest, with only the topmost governing; every path — Tab, click-to-focus, \
             programmatic focus — passes the same gate.",
        ),
    },
    Coverage {
        id: "UIF-MOD-002",
        status: Status::Implemented,
        section: "§4.6.3",
        evidence: &["surface/modal.rs"],
        reason: None,
        note: Some(
            "`attempt_dismiss` is the only route to a user-initiated close: Escape, \
             the scrim and the header Close all go through it, and a dirty card answers \
             `DeclineReason::CannotUnwind` instead of discarding.",
        ),
    },
    Coverage {
        id: "UIF-MOD-003",
        status: Status::Implemented,
        section: "§4.6.3",
        evidence: &["surface/modal.rs"],
        reason: None,
        note: Some(
            "`ModalError` carries a 1-based line and column and a `Recovery::Retry` \
             affordance (`retry_pending` / `take_retry`); only `set_error` and \
             `clear_error` can write the banner, so 'clicking elsewhere must not clear \
             the error' is a non-event.",
        ),
    },
    // --- §4.7 Movable Overlay Dialogs --------------------------------------
    Coverage {
        id: "UIF-MOV-001",
        status: Status::Implemented,
        section: "§4.7.2",
        evidence: &["surface/movable.rs"],
        reason: None,
        note: Some(
            "Title-bar press enters `Dragging` and translates by the pointer delta; \
             `reopen` restores the geometry across open/close toggles. Cross-restart \
             persistence is injected through `GeometryStorage` (`Noop` / `Memory` \
             in-crate) — under D-2 and RUST-024 this crate never touches a storage \
             location, so the host supplies the sink.",
        ),
    },
    Coverage {
        id: "UIF-MOV-002",
        status: Status::Implemented,
        section: "§4.7.2",
        evidence: &["geometry/clamp.rs", "surface/movable.rs"],
        reason: None,
        note: Some(
            "The clamp protects the title bar, not the card: at least 120px of title \
             bar stays inside the viewport horizontally, and the top edge stays within \
             [navbar, viewport.height - title_bar] vertically regardless of card \
             height.",
        ),
    },
    Coverage {
        id: "UIF-MOV-003",
        status: Status::Implemented,
        section: "§4.7.2",
        evidence: &["surface/movable.rs", "surface/zorder.rs"],
        reason: None,
        note: Some(
            "No scrim; a miss outside the card keeps the pointer walk going down to \
             Band 0, and `pointer_down` promotes the clicked card within Band 1 without \
             moving focus.",
        ),
    },
    Coverage {
        id: "UIF-MOV-004",
        status: Status::Implemented,
        section: "§4.7.2",
        evidence: &["surface/movable.rs"],
        reason: None,
        note: Some(
            "`FollowState::resolve` is the whole pause/resume matrix as a pure function \
             of (setting, at_bottom), with `at_bottom` derived against a one-line \
             tolerance.",
        ),
    },
    // --- §4.8 Fixed Menus --------------------------------------------------
    Coverage {
        id: "UIF-FMENU-001",
        status: Status::Implemented,
        section: "§4.8.2",
        evidence: &["geometry/anchor.rs"],
        reason: None,
        note: Some(
            "`Anchor::Control` with `ControlAlignment`: below the trigger across the \
             4px gap by default, right-edge aligned when the trigger sits on the right \
             of the bar.",
        ),
    },
    Coverage {
        id: "UIF-FMENU-002",
        status: Status::Implemented,
        section: "§4.8.2",
        evidence: &["surface/menu/cascade.rs", "geometry/anchor.rs"],
        reason: None,
        note: Some(
            "The parent card stays open and interactive; activating a sibling row \
             retires the branch its parent already had open; the child flips to the \
             left near the right viewport edge, aligned to the parent row's top.",
        ),
    },
    Coverage {
        id: "UIF-FMENU-003",
        status: Status::Implemented,
        section: "§4.8.2",
        evidence: &["surface/menu/model.rs"],
        reason: None,
        note: Some(
            "`ClosePolicy::{Close, StayOpen}` with `default_for(kind)`: Action and Radio \
             close, Check stays open so several toggles fit one visit, and the caller \
             overrides per row where close-on-toggle is wanted.",
        ),
    },
    // --- §4.9 Context Menus ------------------------------------------------
    Coverage {
        id: "UIF-CMENU-001",
        status: Status::Implemented,
        section: "§4.9.3",
        evidence: &["geometry/anchor.rs"],
        reason: None,
        note: Some(
            "`Anchor::Pointer` opens +2/+2 and flips either axis when the preferred \
             side cannot hold the card; the roomier-side fallback picks the larger \
             margin, clamps to 8px and reports `scrollable` so the view can wrap the \
             card.",
        ),
    },
    Coverage {
        id: "UIF-CMENU-002",
        status: Status::Implemented,
        section: "§4.9.3",
        evidence: &["surface/menu/cascade.rs", "surface/zorder.rs"],
        reason: None,
        note: Some(
            "At most one tree is live across context and fixed menus; opening one closes \
             the other. A click outside dismisses the whole hierarchy, while disabled \
             rows, headings and separators are absorbed inside the card without \
             dismissing.",
        ),
    },
    Coverage {
        id: "UIF-CMENU-003",
        status: Status::Implemented,
        section: "§4.9.3",
        evidence: &["surface/menu/cascade.rs", "focus.rs"],
        reason: None,
        note: Some(
            "`MenuKey` handling: Up/Down cycle enabled rows skipping headings, \
             separators and disabled items; Right opens the focused submenu and focuses \
             its first enabled child; Left/Escape close the deepest card back to the \
             parent row; Enter/Space activate.",
        ),
    },
    // --- §6 Design Quality Gates -------------------------------------------
    Coverage {
        id: "UIF-VER-001",
        status: Status::Partial,
        section: "§6.1",
        evidence: &["token/contrast.rs", "gallery.rs"],
        reason: Some(
            "The contrast half is automated: the WCAG 2.1 solver plus the gallery's \
             four-class judgement (Judged / UnpaintedBackground / CoincidentWithFill / \
             RestingDivider) over every emitted pair, with worst-case compositing of \
             translucent fills. Missing: the 'zero raw RGB or hex literals, inline \
             string literals or magic spacing numbers outside the token definitions' \
             half has no source-level enforcement — the token-mapping gate inspects the \
             colours gallery cells emit, not the crate's source text.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-VER-002",
        status: Status::Partial,
        section: "§6.2",
        evidence: &[
            "geometry/anchor.rs",
            "geometry/clamp.rs",
            "token/size.rs",
            "token/typography.rs",
        ],
        reason: Some(
            "Menu quadrant flipping, input-dialog cursor clamping and movable \
             title-bar clamping are solver-tested across extreme viewports down to \
             640x480, and the 24px hit floor and 12px text floor are asserted against \
             the rungs. Not covered: tab-strip overflow scrolling and the 10px \
             splitter grab zone — the tab strip and the pane splitters are app-owned \
             under D-2, so no framework geometry exists to test.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-VER-003",
        status: Status::Partial,
        section: "§6.3",
        evidence: &[
            "surface/modal.rs",
            "surface/input_dialog.rs",
            "surface/zorder.rs",
            "state.rs",
        ],
        reason: Some(
            "Headless tests cover disabled-button and card-whitespace absorption, \
             Escape unwinding in escape-rank order, and the dirty-editor confirmation. \
             Not covered: closing a *dirty tab* triggers confirmation — no tab module \
             exists under D-2, so only the modal and editor half of this gate is \
             exercised.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-VER-004",
        status: Status::Partial,
        section: "§6.4",
        evidence: &["gallery.rs", "bin/gallery.rs"],
        reason: Some(
            "91 cells cover the atoms across (Variant x State), the menu row matrix, \
             the modal, the input dialog, the movable overlay and one z-band frame, \
             asserting resolved style values plus the ordered quad list. Not cells: \
             only `Palette::dark()` is snapshotted (no inverted editor palette); only \
             the standard control height (no compact rung); modal internals carry no \
             header, banner or footer sub-rectangles; menu separator and glyph \
             placement, input-dialog per-row bodies, movable trailing actions and the \
             iced caret are all uncovered. Navbar, Panes and Tabs appear only as \
             `NotOwned` cells.",
        ),
        note: None,
    },
];

#[cfg(test)]
#[path = "spec_coverage_tests.rs"]
mod spec_coverage_tests;
