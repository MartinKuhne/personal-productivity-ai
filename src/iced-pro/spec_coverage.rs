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
///
/// **72 as of the 2026-10-22 revision** — 62 unlettered base IDs plus 10
/// letter-suffixed sub-requirements (`UIF-ATOM-000b`, `001a`–`001d`,
/// `002a`–`002e`). The suffixed ones are counted **individually**, each
/// as its own requirement: they are not decorations on their parent. See
/// [`extract_spec_ids`], whose whole reason for existing in its current
/// form is that an earlier version of this extractor could not see them.
pub const EXPECTED_SPEC_ID_COUNT: usize = 72;

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
/// An id is `UIF-<FAMILY>-<DIGITS>[<suffix>]`, where `FAMILY` is one or
/// more ASCII uppercase letters, `DIGITS` is one or more ASCII digits, and
/// the optional `suffix` is **exactly one ASCII lowercase letter** — the
/// form the spec uses for a sub-requirement. The whole thing must be
/// delimited on both sides by anything that is not `[A-Za-z0-9-]`.
///
/// # Why the suffix is part of the id and not noise
///
/// A suffixed id is a **different requirement** from its unlettered parent.
/// `UIF-ATOM-002` is the text-input state matrix; `UIF-ATOM-002c` is
/// `PasswordInput`, a widget that either exists or does not. Collapsing
/// the two onto one entry means a parent classified `Implemented` silently
/// covers an unbuilt child — which is precisely how this ledger came to
/// report `Implemented` for `UIF-ATOM-002` while `002c`, `002d` and
/// `002e` described three widgets nothing in this crate contains.
///
/// The previous version of this function stopped its run at the first
/// lowercase letter and then rejected the id as a fragment butted up
/// against a word character. So `UIF-ATOM-002c` produced **nothing**: not
/// a child, not a parent, no finding. The parent was counted only because
/// it also appears elsewhere in the spec unlettered. The gate went green at
/// 47 while the spec carried 72 requirements.
///
/// Lowercasing is therefore treated as *significant*, and only as much as
/// the convention warrants: one trailing lowercase letter extends the id;
/// more than that is a word, not a suffix, and is rejected.
///
/// # What is still rejected
///
/// The shape deliberately rejects the **family wildcards** the spec uses
/// in section headings — `UIF-TOK-*`, `UIF-ZORD-*` — because a wildcard
/// names a family, not a requirement. `UIF-ZORD-*` in particular heads §5
/// while §5 numbers nothing, so counting it would invent a requirement the
/// spec never wrote. An **uppercase** trailing letter (`UIF-TOK-001X`) is
/// likewise not a suffix: the spec's sub-requirements are lowercase, and
/// accepting uppercase would widen the accepted shape beyond what any
/// convention supports.
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
        // fragment of a larger word, not a standalone id. Note this is
        // checked *after* the run has already swallowed any lowercase
        // suffix, so `UIF-ATOM-002c` ends on a delimiter while
        // `UIF-ATOM-002context` ends inside a word and is dropped.
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
///
/// Lowercase letters are run characters so a sub-requirement suffix is
/// captured as part of the id rather than truncating it. Whether the
/// resulting run is actually an id is [`is_numbered_id`]'s decision — a
/// run with lowercase in the wrong place fails there, exactly as it should.
fn is_run_char(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
}

/// A character that makes an adjacent run part of a larger word rather
/// than a standalone id: `preUIF-TOK-002` is a fragment, while
/// `` `UIF-TOK-001` `` and `UIF-TOK-001,` are not.
///
/// This is deliberately **not** the same set as [`is_run_char`]: the left
/// boundary has to reject a lowercase prefix, which is why a lowercase
/// letter counts here even though it also extends a run.
fn is_boundary_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// `UIF-<FAMILY>-<DIGITS>[<one lowercase suffix>]`, with both variable
/// parts non-empty.
///
/// The suffix is optional, and at most one character long. `UIF-ATOM-002`,
/// `UIF-ATOM-002a` and `UIF-ATOM-002e` are ids; `UIF-ATOM-002ab` and
/// `UIF-ATOM-002A` are not.
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
    if family.is_empty() || !family.chars().all(|c| c.is_ascii_uppercase()) {
        return false;
    }
    let digits = number.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }
    let suffix: Vec<char> = number.chars().skip(digits).collect();
    suffix.is_empty() || (suffix.len() == 1 && suffix[0].is_ascii_lowercase())
}

/// The unlettered parent a suffixed requirement id sits under.
///
/// `UIF-ATOM-002c` → `Some("UIF-ATOM-002")`; an unlettered id → `None`.
///
/// This exists so the ledger can assert the **collapse cannot come back**:
/// a suffixed id and its parent are two entries, never one, and a test can
/// walk every suffixed requirement and check that resolving its parent
/// does not answer for the child.
#[must_use]
pub fn parent_id(id: &str) -> Option<&str> {
    let last = id.chars().last()?;
    if !last.is_ascii_lowercase() {
        return None;
    }
    let stem = &id[..id.len() - last.len_utf8()];
    // Only a real requirement underneath counts as a parent: the stem must
    // still be a numbered id on its own.
    if is_numbered_id(stem) {
        Some(stem)
    } else {
        None
    }
}

/// Whether `id` is a letter-suffixed sub-requirement rather than a base id.
#[must_use]
pub fn is_suffixed_id(id: &str) -> bool {
    parent_id(id).is_some()
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
        evidence: &["token/", "token_lint.rs", "gallery::unmapped_colours"],
        reason: Some(
            "Token discipline holds by construction — every visual number is declared \
             under `token/` and nothing else — and the gallery gate proves every colour \
             an audited cell emits names a `Palette` token. The rule is now \
             machine-checked at the source level: `token_lint` bans colour \
             constructors and hex literals outside `token/color.rs`, and bans a bare \
             numeric literal bound to a visually-named `pub const` in a widget or \
             surface file, excepting zero, ratios, and the sixteen Surface Geometry \
             Constants §1.1 explicitly carves out. Missing: the scan reads named \
             constants, so a literal typed inline into a layout expression is still \
             invisible. Of the G-12 compositions the 16px grip now reads \
             `size.icon.md`, which the spec's own token table names for the resize \
             grip glyph; the 10px menu-row padding and 9px separator remain \
             deliberate token compositions rather than named rungs.",
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
        status: Status::Implemented,
        section: "§2.5 rules",
        evidence: &["token/contrast.rs"],
        reason: None,
        note: Some(
            "All four rules are implemented and machine-checked. Rules 1 and 3 are \
             thresholds (4.5 / 3.0) over a WCAG 2.1 solver that composites \
             translucent layers before measuring, and both pass the full matrix \
             against the revised palette; rule 3's four-class chrome classification \
             (`Judged`, `CoincidentWithFill`, `UnpaintedBackground`, \
             `RestingDivider`) makes `border.subtle`'s exemption a class rather than \
             a reading of the word 'active'. Rule 4's foreground lock is enforced by a \
             guard every resolver calls plus a 228-pair crate-wide audit. The lock is \
             role-based, not threshold-based, and that distinction is load-bearing: two \
             forbidden pairings actually clear the 3.00 chrome floor (`accent.text` on \
             `accent.fill_active` at 3.0293 and on `status.danger.fill_active` at \
             3.0820) yet remain banned. Two cells fail the spec's own thresholds and \
             are pinned as known conflicts rather than patched: rule 2's \
             `[4.00, 6.00]` band is breached by `text.disabled` on `surface.sunken` \
             (6.0205), and rule 3's `CoincidentWithFill` floor of 3.08 is missed by \
             `status.danger.fill_active` on `surface.overlay` (3.0789).",
        ),
    },
    // --- §3 Atomic Widget State Matrices ------------------------------------
    Coverage {
        id: "UIF-ATOM-000",
        status: Status::Partial,
        section: "§3.0",
        evidence: &["state.rs", "widget/focus.rs", "widget/button_widget.rs"],
        reason: Some(
            "The state half of §3.0 is implemented and is the crate's foundation: \
             `VisualState { base, focused }`, base precedence `Disabled > Invalid > \
             Pressed > Hovered > Rest`, total disabled dominance — a disabled control \
             forces `focused` false and focus advances to the next enabled control — the \
             nine reachable states as the gallery's nine columns, press-and-release \
             activation, and both focus-indicator placements on one adaptive colour \
             rule. Missing: §3.0.4, `UIF-ATOM-000b`, is unbuilt in its entirety — no \
             Controlled-vs-Ephemeral state split, no `on_change`/`on_commit`/`on_cancel` \
             callback tiers, no `ActionPending` busy state and no container-query \
             `ResponsiveBreakpoint`. The parent cannot honestly stay `Implemented` while a \
             quarter of the section it heads does not exist, so it drops and the child \
             carries its own entry below.",
        ),
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
        id: "UIF-ATOM-000b",
        status: Status::Partial,
        section: "§3.0.4",
        evidence: &[
            "state.rs",
            "surface/movable.rs",
            "binding/controlled.rs",
            "binding/tier.rs",
            "binding/snapshot.rs",
            "binding/pending.rs",
            "binding/responsive.rs",
        ],
        reason: Some(
            "Foundation built **and now adopted by the Button**. `binding/` carries the four \
             pieces §3.0.4 names — `Controlled<'a, M>` against `Ephemeral<S>` for the \
             ownership split, `Live<T>` / `Commit<T>` / `Cancel<T>` as three tiers that \
             deliberately share no trait, with a `Gesture<T>` guard whose consuming \
             `commit`/`cancel` make a double-settle a compile error; `ActionPending` with \
             an `ActivationGate` that closes the double-click window before the app's \
             `busy` projection arrives; and a pure breakpoint solver that takes no \
             window-width parameter at all, making a global-window query unrepresentable. \
             `widget/button_model.rs` and `widget/button_controller.rs` put that \
             vocabulary under a real control: the model is taken as `Controlled`, the \
             interaction half is tagged `Ephemeral<ButtonState>`, and every press settles \
             through a linear gesture into one of the three tiers. Missing: **adoption by \
             every other control** — `text_input.rs`, the menu rows and the dialog hosts \
             still emit a single iced `Message`, so the tier vocabulary is proven on one \
             atom rather than across the set. Missing: the **spinner** — the 14px slot is \
             reserved and reported but nothing paints it, and `size.spinner.sm` / `.md` \
             are still absent from `token/size.rs` pending `UIF-ATOM-009`. Missing: the \
             **overflow selection** — the solver decides `Expanded` versus `Compact`, not \
             *which* toolbar items move into the `⋯` menu, which is `UIF-COMP-008`'s work.",
        ),
        note: Some(
            "This requirement was invisible to the ledger until the 2026-10-22 revision: \
             the extractor could not see a letter-suffixed id, so `000b` was never \
             classified and `UIF-ATOM-000` reported `Implemented` over it. The \
             `Partial` here is deliberate and will stay `Partial` until every control \
             consumes the vocabulary — a foundation one widget uses is coverage of one \
             widget.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001",
        status: Status::Partial,
        section: "§3.1",
        evidence: &[
            "widget/button.rs",
            "widget/button_widget.rs",
            "widget/button_model.rs",
            "widget/button_controller.rs",
            "widget/focus.rs",
            "state.rs",
            "surface/tooltip/",
        ],
        reason: Some(
            "All four variants resolve and paint across the reachable state matrix, with \
             the D-5 danger-fill split and the D-6 adaptive ring, and the icon-only \
             tooltip rule is now *enforced* rather than hoped for: `Button::icon_only` \
             cannot be reached without naming the label, `tooltip_violation` reports the \
             forbidden shape without panicking, and a debug shield fires at the moment the \
             box becomes icon-only. The controlled path (`Button::controlled`) now drives \
             the same paint off a host-owned `ButtonModel` and is pinned to produce \
             pixel-identical layers to the legacy builder. Missing: the tooltip is \
             **available** to a host, not **attached** by the framework at paint time — \
             nothing in this crate drives a `HoverController` from the button widget's \
             own hover event, so connecting the card to the control is still host code \
             (see `UIF-ATOM-007`).",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-001a",
        status: Status::Partial,
        section: "§3.1",
        evidence: &[
            "widget/button_model.rs",
            "widget/button_controller.rs",
            "widget/button_widget.rs",
            "widget/button.rs",
            "binding/controlled.rs",
            "binding/pending.rs",
            "binding/snapshot.rs",
            "binding/responsive.rs",
        ],
        reason: Some(
            "The model and the engine are both real now. `ButtonModel` carries `label`, \
             typed `leading_icon`/`trailing_icon`, `variant`, `density`, `busy`, \
             `disabled`, `cancellable` and `tooltip`, is handed over as \
             `Controlled<'_, ButtonModel>` and never copied, and the widget's own half is \
             tagged `Ephemeral<ButtonState>`. Activation routes through a linear gesture: \
             release inside and `Enter`/`Space` on the focused control emit \
             `Commit<ButtonSettlement>`, release outside and `Escape` emit \
             `Cancel<ButtonSettlement>` carrying the pre-gesture snapshot, and a press or \
             `Escape` while the host reports busy emits `Cancel<CancelRequest>` — \
             `on_cancel_request()` — granted only when the model declared \
             `cancellable`. `ActivationGate` suppresses a second activation before the \
             host's `busy` projection lands, and the collapsed rung resolves through \
             `ResponsiveLayout` so the label drop and the tooltip obligation are the same \
             event and the vertical shift is zero by construction. Missing: the **live \
             container query** — the rung comes from a density the *host declares*, not \
             from a width the widget observed; `ButtonModel::responsive_to(allocated)` is \
             there for a host that has one, but `Widget::layout` does not swap its content \
             element mid-frame, so no button today collapses on its own. Missing: the \
             **spinner glyph** — `SPINNER_SLOT` reserves `size::ICON_SM` and nothing \
             paints it (`UIF-ATOM-009`). Missing: **`size.spinner.sm` / `.md`** are still \
             absent from `token/size.rs`. Deviation: the model carries a `cancellable` \
             flag the spec's field list does not name, because `ActionPending::try_cancel` \
             cannot grant a cancellation the host never offered — without it \
             `on_cancel_request()` would be unreachable for a running action.",
        ),
        note: Some(
            "The previous entry for this id claimed `Enter`/`Space` on the focused control \
             was implemented. It was not — `button_widget.rs` handled no keyboard event \
             at all before this increment, and the claim was ledger drift rather than \
             behaviour (RUST-041). It is now actually implemented and tested.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001b",
        status: Status::Partial,
        section: "§3.1",
        evidence: &["token/shape.rs", "token/color.rs"],
        reason: Some(
            "Not implemented — no `ToggleButton`, `SegmentedButton` or any joined-segment \
             control exists in this crate. Missing: the strip geometry (28px or 32px \
             height, 4px outer radius, `1px border.strong` perimeter), the \
             selected-segment skin (`surface.active` fill, `text.strong`, `1px \
             accent.border`), the reserved 14px leading `✓` in `accent.text` that carries \
             the selection without relying on colour (`UIF-PRIN-005`), \
             `SegmentedModel<Id>` with `SingleSelect`/`MultiSelect` over a \
             `BTreeSet<Id>`, `on_selection_change`, and the `Left`/`Right` roving that \
             moves focus across segments with `Space`/`Enter` toggling. The tokens it \
             would be built from all exist and are named; the widget does not.",
        ),
        note: Some(
            "Framework-owned under D-2: a segmented control is a generic atom, not navbar \
             or tab content, so this is unimplemented framework work rather than a scope \
             exclusion.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001c",
        status: Status::Partial,
        section: "§3.1",
        evidence: &["surface/menu/", "token/shape.rs"],
        reason: Some(
            "Not implemented — no `SplitButton` or `DropDownButton` exists. Missing: the \
             two-zone control geometry (a primary action zone at `[0, 12px]` padding plus \
             a 28px trailing chevron zone behind a `1px border.strong` vertical \
             divider), `SplitButtonModel<ActionId>`, `on_primary_activate()`, \
             `on_menu_toggle(bool)` on chevron click or `Alt+Down`/`Down`, \
             `on_menu_select(ActionId)`, and the Z-Band 2 flyout mount. The menu engine \
             the flyout would be built from is present and proven — card, cascade, roving \
             focus, `Escape Rank` — so the gap is the control and its binding, not the \
             popup machinery behind it.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-001d",
        status: Status::Partial,
        section: "§3.1",
        evidence: &["token/color.rs", "token/shape.rs"],
        reason: Some(
            "Not implemented — there is no `Hyperlink` control anywhere in the crate. \
             Missing: the inline-or-standalone interactive-text control with its 24px hit \
             target and `radius.widget` focus ring; the four painted states (`accent.text` \
             with a 1px underline at rest, `text.strong` on a `surface.hover` pill on \
             hover, `accent.text` on `surface.active` pressed, `text.disabled` with the \
             underline removed when disabled); `HyperlinkModel { label, uri, external, \
             disabled }`; `on_open(String)`; and `on_hover_uri(Option<String>)`, which \
             is what would surface a target in the status bar.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-002",
        status: Status::Partial,
        section: "§3.2",
        evidence: &["widget/text_input.rs", "widget/focusable.rs"],
        reason: Some(
            "Three of the six widgets §3.2 now names are built: the entry, search and \
             dropdown-trigger kinds resolve and paint across the five painted states, with \
             the focused-placeholder and invalid-placeholder cells derived and documented \
             because the matrix leaves them open. Missing: §3.2 also defines \
             `PasswordInput` (`002c`), `AutoSuggestBox` (`002d`) and `MultiLineTextArea` \
             (`002e`), and none of the three exists in this crate — so the section is \
             covered at three kinds out of six, and the parent cannot be `Implemented`. \
             Each child carries its own entry below. The text caret itself is iced's and \
             is not a snapshot cell.",
        ),
        note: Some(
            "Downgraded from `Implemented` on the 2026-10-22 re-baseline. The earlier \
             `Implemented` was not a misreading of the code — it was a ledger that could \
             not see the suffixed requirements its own section had grown.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-002a",
        status: Status::Partial,
        section: "§3.2",
        evidence: &[
            "widget/text_input.rs",
            "widget/text_input_model.rs",
            "widget/text_input_controller.rs",
            "binding/controlled.rs",
            "binding/tier.rs",
        ],
        reason: Some(
            "All four callbacks are routed through real tiers by \
             `TextInput::controlled(kind, Controlled<'_, TextInputModel>, focus_id, emit)`. \
             `on_change` is `Live<String>`, coalesced so an unchanged value produces no \
             tick and however many keystrokes land in one pass at most one does. \
             `on_commit` is `Commit<String>`, settled by `Enter` or by focus loss, and a \
             settled tier discards the pending live tick as superseded rather than \
             trailing it. `on_clear` is its own arm — `Cleared(Commit<String>)` of the \
             empty value — so a clear is not a commit that merely looks the same. \
             `on_focus_change` is the non-value edge in the same sum type, emitted when \
             the boundary moves rather than on every pass. The editing session *is* the \
             gesture, and its snapshot is taken at the **first edit after focus** rather \
             than at focus itself: focus arrives on every `Tab`, and a snapshot taken \
             there goes stale the moment the host syncs while the field is open. `Escape` \
             reverts while a session is open; with none open it clears only where \
             `clear_affordance(kind)` holds — the same predicate that makes the `×` \
             actionable — so a plain entry is never `Escape`-cleared. `read_only` is a \
             real third state: the callback iced gates edits on is withheld, the field \
             still paints as an enabled control, and the focus half of the cell is \
             dropped so it never claims a caret it cannot take. Missing: \
             `selection: Option<TextRange>` is carried and resolved by the model \
             (`selection_in_value`) but never applied to the editor's caret, which needs \
             `operation::TextInput`; and the sibling widgets `002c`–`002e` remain \
             unbuilt, so the parent cannot close either.",
        ),
        note: Some(
            "The widget intercepts `Escape` before the inner editor sees it. Iced's own \
             handler blurs the editor on `Escape`, which would drop the revert on the \
             floor — the focus gate would find no keyboard left to attribute the gesture \
             to — and throw the keyboard away with the edit it had just restored. A bound \
             field therefore keeps the keyboard after a cancel, which is also what \
             `002d`'s two-`Escape` sequence assumes; the legacy unbound path is untouched \
             and still blurs. Because iced publishes the instant a keystroke lands and \
             `Shell::publish` cannot be declined, the inner editor's message type is \
             `Option<Message>` and the wrapper drives it through a shell it owns, \
             republishing only what it chooses.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-002b",
        status: Status::Implemented,
        section: "§3.2",
        evidence: &[
            "widget/text_input.rs",
            "surface/input_dialog.rs",
            "widget/focusable.rs",
        ],
        reason: None,
        note: Some(
            "Structural, not conventional: `InputKind::trailing_slot_width` reserves 24px \
             for the entry and dropdown kinds and 28px for search, `text_area_bounds` and \
             `trailing_slot_bounds` are each derived from the outer control rect alone, \
             and `slot_content` resolves the `invalid > clear > idle` priority as one \
             pure function rather than a special case per glyph. Tests assert the editor \
             bounds do not move when the slot content changes, as the value grows, or \
             across kinds at the same width — the zero-shift invariant is a property of \
             the geometry, not a convention a caller has to honour.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-002c",
        status: Status::Partial,
        section: "§3.2",
        evidence: &["widget/text_input.rs"],
        reason: Some(
            "Not implemented — there is no `PasswordInput`. Missing: `•` (U+2022) masking \
             of the entered value, the clipboard `Copy`/`Cut` suppression that must hold \
             while masked, the reserved 24×24px trailing reveal toggle (`👁` / `👁‍🗨`), \
             `PasswordInputModel { masked_len, revealed_value, validation, disabled }`, \
             `on_change(SecretInput)`, `on_commit()` and `on_reveal_toggle(bool)` with its \
             automatic reset to masked on focus loss. The trailing-slot geometry the toggle \
             would sit in already exists and is proven under `UIF-ATOM-002b`, so the slot \
             is not the gap — the secret-handling widget is.",
        ),
        note: Some(
            "Framework-owned under D-2. A secret entry is a generic control, not app \
             content, so this is unimplemented framework work the host would otherwise \
             have to write itself.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-002d",
        status: Status::Partial,
        section: "§3.2",
        evidence: &["widget/text_input.rs", "surface/menu/", "surface/zorder.rs"],
        reason: Some(
            "Not implemented — there is no `AutoSuggestBox`. Missing: the Z-Band 2 \
             completion flyout (max height 240px, 32px rows, anchored 4px below the \
             input), `AutoSuggestModel<Id>`, `on_query_change(String)`, \
             `on_suggestion_highlight(usize)` with `Down`/`Up` wrapping that does not lose \
             the input caret, `on_suggestion_commit(Id, CommitSource::{Enter, Click, Tab})` \
             and the two-stage `Escape` — flyout closes first, a second `Escape` clears \
             the query per `Escape Rank`. Both halves the widget joins already exist and \
             are separately proven: the input is here, and the menu engine plus the \
             escape-rank registry supply the flyout. What is missing is the binding \
             between them.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-002e",
        status: Status::Partial,
        section: "§3.2",
        evidence: &["widget/text_input.rs"],
        reason: Some(
            "Not implemented — there is no `MultiLineTextArea`. Missing: the scrollable \
             multi-line well (min height 72px, `space.sm` vertical and `space.md` \
             horizontal padding, `font.family.sans` for prompts or `font.family.mono` for \
             code), auto-grow between `min_rows` and `max_rows` (the Bottom Command Bar's \
             44px..120px prompt box is the named case), the `LineCol` cursor and \
             `TextRange` selection model, `dirty` tracking and a \
             `Option<ParseDiagnostic>` validation slot, `on_cursor_change(LineCol)`, \
             `on_submit(String)` on `Ctrl+Enter`/`Cmd+Enter`, and `on_undo()`/`on_redo()`. \
             Every widget in this crate's input layer is single-line; there is no \
             multi-line primitive to extend.",
        ),
        note: Some(
            "The `font.family.mono` requirement inside this one cannot be met either — \
             `UIF-TOK-004` models no font family at all, so a mono text area has no rung \
             to ask for it. That gap is recorded against `UIF-TOK-004`, not papered over \
             here.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-003",
        status: Status::Partial,
        section: "§3.3",
        evidence: &["token/size.rs", "surface/menu/view.rs"],
        reason: Some(
            "Not implemented — no standalone `Checkbox`, `Radio` or `ToggleSwitch` \
             exists. Missing: the 16×16px box at `radius.widget` with its tri-state \
             `Indeterminate` dash, the circular radio indicator with its 8×8px centred \
             `text.strong` dot, the 36×20px switch track with its 14×14px thumb at a 3px \
             inset, the 24px hit target spanning the inline label across an 8px gap, the \
             full two-row polarity state matrix for unchecked and checked, and all three \
             models and callbacks — `on_toggle(CheckState)` with the deterministic \
             indeterminate→checked transition, `RadioGroupModel<Id>` with arrow-key roving \
             and no-op re-selection, and `SwitchModel` with `on_toggle(bool)`. The `✓` and \
             `•` in `surface/menu/view.rs` are menu-row slot paint, not controls, and are \
             not offered as widgets. `size.checkbox.box`, `size.switch.track` and \
             `size.switch.thumb` exist with no consumer.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-004",
        status: Status::Partial,
        section: "§3.4",
        evidence: &[
            "widget/text_input.rs",
            "surface/menu/",
            "geometry/anchor.rs",
        ],
        reason: Some(
            "Not implemented — there is no `ComboBox`/`Select`. Missing: the \
             trigger-plus-flyout pairing (`min_width = trigger.width`, `max_height = \
             280px`, 4px below or flipped above per `UIF-CMENU-001`), the `✓` in the \
             selected row's 16×16px leading slot, `ComboBoxModel<Id>`, \
             `on_open_change(bool)`, `on_filter_change(String)` for the searchable form, \
             and `on_commit(Id)` closing the popup and restoring focus to the trigger. The \
             pieces exist separately — `InputKind::DropdownTrigger` already paints the \
             chevron and the menu engine already supplies the card, the roving focus and \
             the Z-Band 2 registration — but nothing joins them into a picker.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-005",
        status: Status::Partial,
        section: "§3.5",
        evidence: &["token/size.rs"],
        reason: Some(
            "Not implemented — no `Slider` and no `NumberBox`. Missing: the 4px pill track \
             with its unfilled `surface.active` rail and `accent.fill` active rail, the \
             16×16px thumb inside a 24×24px hit target with its hover/press ring change, \
             the optional 2×4px stepped tick marks, the 32px numeric input with its dual \
             stepper, `RangeModel`, and the whole scrub contract that is really the point \
             of the requirement — `on_change(f64)` continuous during drag, `on_commit(f64)` \
             exactly once on release / `Enter` / focus loss, `on_cancel()` reverting on \
             `Escape` mid-drag, and the keyboard step matrix (`←`/`↓` = −step, `→`/`↑` = \
             +step, `PageUp`/`PageDown` = ±10×step, `Home` = min, `End` = max). \
             `size.slider.track_height` and `size.slider.thumb` exist with no consumer.",
        ),
        note: Some(
            "This is the clearest unbuilt instance of the `UIF-ATOM-000b` live/commit/cancel \
             contract: three of the four widgets that would exercise it (§3.2, §3.4, §3.5) \
             are missing, so the contract itself has no proving ground either.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-006",
        status: Status::Partial,
        section: "§3.6",
        evidence: &["token/shape.rs", "token/size.rs"],
        reason: Some(
            "Not implemented — no chip or filter-pill control exists. Missing: the 24px \
             compact / 28px interactive geometry with `radius.pill` for status and count \
             pills or `radius.widget` for filter and input chips, the selected-filter skin \
             (`accent.fill`, `text.strong`, `1px accent.border`) with its leading 14px \
             `✓`, the reserved 24×24px trailing `×` on removable input chips, \
             `ChipModel<Id>` across the `Assist` / `Filter { selected }` / \
             `Input { removable }` variants, and `on_activate(Id)`, `on_toggle(Id, bool)` \
             and `on_remove(Id)` including the `Delete`/`Backspace` removal path when the \
             chip holds focus.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-007",
        status: Status::Partial,
        section: "§3.7",
        evidence: &[
            "surface/tooltip/mod.rs",
            "surface/tooltip/hover.rs",
            "surface/tooltip/place.rs",
            "geometry/anchor.rs",
        ],
        reason: Some(
            "The `Tooltip` half is built and conformed to §3.7 as revised: 400ms hover \
             delay, 0ms keyboard-focus delay that opens immediately through a second \
             arrival channel rather than a retimed hover, `surface.overlay` fill, `1px \
             border.strong`, `radius.widget` (4px), 280px max width and `4px 8px` \
             padding, dismissal on pointer leave, key press or `Escape`, and pointer \
             transparency through a layer that sits above every z-band without joining the \
             registry. Missing: the whole `TeachingPopover` half — the interactive callout \
             at 280–360px with `1px accent.border`, `radius.card` (6px), a title, body \
             and optional footer action, mounted in Z-Band 2 at `Escape Rank 3` — plus \
             `PopoverModel<ActionId>` with `on_action(ActionId)` and `on_dismiss()`. Also \
             missing: the tooltip is not wired to any widget's own hover or focus event \
             inside this crate, so a host must drive a `HoverController` itself; and the \
             optional trailing shortcut pill in `font.family.mono` has no rung to name, \
             because `UIF-TOK-004` models no font family.",
        ),
        note: Some(
            "Three of INC-31's invented values were wrong against the revision and are now \
             conformed: hover delay 500 → 400 ms, radius `RADIUS_CARD` (6px) → \
             `RADIUS_WIDGET` (4px), and max width from 'the host's problem' to 280px \
             owned and clamped here. Keyboard-focus opening at 0 ms did not exist at all \
             and is now a real second arrival channel. The fill/border/text triple INC-31 \
             chose was already right — the spec's own `4.01:1` border and `11.72:1` text \
             figures match what this crate recomputed from raw sRGB, not transcribed.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-008",
        status: Status::Partial,
        section: "§3.8",
        evidence: &["token/size.rs", "token/color.rs"],
        reason: Some(
            "Not implemented — no badge or status-indicator control exists. Missing: the \
             6×6px `radius.pill` dot badge in its four tones (`accent.border` unread, \
             `status.warning` unsaved, `status.success` ready, `status.danger.text` error), \
             the 18px numeric count pill with `[0, 4px]` padding, 12px SemiBold text, \
             `99+` formatting above 99 and both solid and subtle variants, the 24px \
             semantic status chip pairing a mandatory leading glyph with text, and \
             `BadgeModel { kind, tone }`. `size.indicator.dot` exists with no consumer, \
             and the unsaved `●` the tab strip needs (§4.4.1) is app-owned content this \
             crate does not render.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-009",
        status: Status::Partial,
        section: "§3.9",
        evidence: &["token/size.rs", "token/color.rs"],
        reason: Some(
            "Not implemented — no `ProgressBar` and no `Spinner`. Missing: the 4px linear \
             track on `surface.active` with `accent.border` / `status.warning` / \
             `status.danger.text` fills for Active / Paused / Error, the 35%-width \
             indeterminate segment and its static-striped fallback under OS \
             `reduced_motion`, the 14px and 20px circular spinners with `border.subtle` \
             track and `accent.text` (or `text.strong` inside a solid button) active arc, \
             `ProgressModel` over `Indeterminate` / `Determinate { completed, total }`, and \
             the `cancellable` trailing 24×24px stop button. `size.progress.bar_height` \
             and `size.spinner.sm` / `.md` exist with no consumer, which also leaves \
             `UIF-ATOM-000b`'s `ActionPending` indicator with nothing to draw.",
        ),
        note: None,
    },
    // --- §3B Composite Patterns --------------------------------------------
    //
    // None of these eight has a framework type yet, so every one is `Partial`.
    // For the two the app already draws (006, 007) the split is the same one
    // D-2 draws everywhere else: the container and its chrome are framework
    // work and are recorded here as gaps; the content inside is the app's,
    // and these entries say so rather than claiming it.
    Coverage {
        id: "UIF-COMP-001",
        status: Status::Partial,
        section: "§3B",
        evidence: &["widget/text_input.rs", "widget/settings_row.rs"],
        reason: Some(
            "Zero-shift required — no reusable field wrapper exists. `widget::text_input` \
             carries the whole label + helper + error + required stack with its own \
             `FieldGeometry`, and `widget::SettingsRow` is the settings-specific variant \
             of the same shape; `UIF-INP-003` pins the zero-shift behaviour *inside* the \
             input. Missing: the extractable `FormField`/`SettingsRow` the spec names — a \
             wrapper any control (not only the text input) can be placed in, with the \
             reserved label/helper/error band and the required marker exposed as its own \
             API rather than as text-input internals.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-002",
        status: Status::Partial,
        section: "§3B",
        evidence: &["widget/modal.rs"],
        reason: Some(
            "Missing a severity-typed inline banner. `ModalError` is the nearest thing in \
             the codebase: it is an inline banner, but it is scoped to one modal, carries \
             no severity level (info / warning / error / success), no icon slot, no action \
             slot and no dismiss affordance, and it is not exported as a reusable \
             composite. Not covered outside a modal — there is no `InfoBar` and no \
             `Toast`.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-003",
        status: Status::Partial,
        section: "§3B",
        evidence: &["surface/menu/view.rs"],
        reason: Some(
            "Not covered — no disclosure container exists. There is no `Expander` or \
             accordion in the crate: nothing owns a header row with a disclosure chevron \
             that animates a body open and closed, and nothing models expanded/collapsed \
             state as a control. The chevron painted in `surface::menu` belongs to menu \
             rows only. The header-chevron half of the atom family (`UIF-ATOM-003`) is \
             likewise unbuilt, so neither half of this pattern exists.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-004",
        status: Status::Partial,
        section: "§3B",
        evidence: &["widget/log_view.rs"],
        reason: Some(
            "Missing scrollbar styling and a scroll-position contract. iced scrolls, and \
             `FollowState` in `widget::log_view` proves the framework can follow a tail — \
             but that is one widget's private policy, not a `ScrollView` API. Missing: a \
             scrollbar styled from tokens (width, track, thumb, hover / scroll-then-fade), \
             scroll-position persistence and restore, and a scroll-to or anchor entry \
             point. The spec leaves scrollbar styling open, which is a spec gap recorded \
             against the spec rather than a reason to call this built.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-005",
        status: Status::Partial,
        section: "§3B",
        evidence: &["surface/menu/view.rs"],
        reason: Some(
            "Not covered as a reusable pattern. The row shape the spec describes — leading \
             slot, title, secondary text, trailing slot, full-row hit target — exists \
             exactly once, as the menu row in `surface::menu::view`, and it is \
             menu-specific: it is driven by `MenuEntry`, its slots are that module's \
             `LeadingSlot`/`TrailingSlot`, and it is not exported as a `Card` or \
             `ListTile` other code can compose. Missing: a card container with token \
             padding, radius and elevation.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-006",
        status: Status::Partial,
        section: "§3B",
        evidence: &["surface/menu/view.rs"],
        reason: Some(
            "Container unbuilt; content is app-owned per D-2. Missing: the tree's \
             container and chrome — a virtualised viewport, row height and indent \
             step, the disclosure chevron, and the selection model with its keyboard \
             choreography (the spec explicitly leaves tree keyboard navigation \
             unspecified, so that choreography is a framework decision, not app content). \
             None of it exists as a reusable type. What the app owns, and this entry \
             deliberately does not claim, is the filesystem node model, filtering, and the \
             `[PDF]` badge.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-007",
        status: Status::Partial,
        section: "§3B",
        evidence: &["surface/movable.rs"],
        reason: Some(
            "Container unbuilt; content is app-owned per D-2. Missing: the table's \
             container and chrome — sticky header, row height, the column-resize \
             handle (the 10px grab of §3.7), cell focus and traversal, and live-then-\
             commit resize behaviour. None of it exists as a reusable type. What the app \
             owns, and this entry deliberately does not claim, is the column definitions, \
             cell rendering and the sort comparators. Resize handle width and sort \
             indicators are left open by the spec, which is recorded against the spec \
             rather than papered over here.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-008",
        status: Status::Partial,
        section: "§3B",
        evidence: &["surface/menu.rs"],
        reason: Some(
            "Not covered — no breadcrumb and no overflow toolbar exists. There is no \
             `BreadcrumbBar` (segment list, separator, overflow folding of the middle \
             segments, per-segment activation) and no `OverflowToolbar` (primary items \
             plus an overflow menu) anywhere in the crate. The overflow menu primitive \
             such a toolbar would fold into does exist as `surface::menu`, but nothing \
             composes it into either pattern the spec names.",
        ),
        note: None,
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
        evidence: &[
            "surface/tooltip.rs",
            "token/color.rs",
            "token/shape.rs",
            "token/typography.rs",
        ],
        reason: Some(
            "The framework half is now implemented: `surface::tooltip` supplies the \
             hover-delay timer (400ms on pointer arrival, 0ms on keyboard focus per \
             §3.7), the floating card on `surface.overlay` with the `border.strong` \
             perimeter and `type.caption` text, the 4px offset, and the placement and \
             flip rule with width clamping. The host no longer carries that machinery. \
             Missing: the navbar trigger button the card would attach to, and its label \
             wiring — app-owned content that D-2 excludes, and no navbar module exists \
             in this crate. The requirement stays `Partial` only because the navbar \
             itself is not here, not because the tooltip surface is.",
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
        evidence: &["token/contrast.rs", "gallery.rs", "token_lint.rs"],
        reason: Some(
            "Both halves are now automated. The contrast half is the WCAG 2.1 solver \
             plus the gallery's four-class judgement (Judged / UnpaintedBackground / \
             CoincidentWithFill / RestingDivider) over every emitted pair, with \
             worst-case compositing of translucent fills. The literal half is now a \
             source-level scan: no colour constructor or hex literal outside \
             `token/color.rs`, and no bare numeric literal bound to a visually-named \
             `pub const` in a widget or surface file unless it is zero, a ratio, or a \
             registered Surface Geometry Constant. Missing: the scan reads named \
             constants, so a literal typed inline into a layout expression, a private \
             const, or a struct field is still invisible to it.",
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
            "182 cells cover the atoms across (Variant x State) at both the standard \
             and compact control rungs, in both the dark and the inverted editor \
             palette, plus the menu row matrix with separator and glyph placement, \
             the modal with its header, banner and footer sub-rectangles, the input \
             dialog with its per-row bodies, the movable overlay with its trailing \
             actions, and one z-band frame — each asserting resolved style values \
             and the ordered quad list. Missing: Navbar, Panes and Tabs are still \
             `NotOwned` placeholders rather than real cells, because D-2 leaves their \
             geometry app-owned. The iced text caret is permanently outside the \
             recorder's surface rather than deferred: no bounds, fill or token for it \
             ever crosses the renderer protocol, so this crate cannot snapshot it.",
        ),
        note: None,
    },
];

#[cfg(test)]
#[path = "spec_coverage_tests.rs"]
mod spec_coverage_tests;
