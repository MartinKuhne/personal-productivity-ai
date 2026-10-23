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
             atom rather than across the set. The **spinner** is now painted (`UIF-ATOM-009`): the 14px slot is \
             reserved by `SPINNER_SLOT` and painted by `busy_spinner_layer`, and `size.spinner.sm` / `.md` \
             are now consumed there — `SPINNER_SM` by the slot, `SPINNER_MD` by the standalone
              spinner. Missing: the \
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
             **spinner glyph** is now painted - `busy_spinner_layer` draws the 14px arc in \
             the slot `SPINNER_SLOT` reserves (`UIF-ATOM-009`). \
             `size.spinner.sm` / `.md` now exist in `token/size.rs`, both consumed. Deviation: the model \
             carries a `cancellable` \
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
        evidence: &[
            "widget/segmented.rs",
            "widget/segmented_model.rs",
            "widget/rove.rs",
            "widget/button.rs",
            "token/color.rs",
            "token/shape.rs",
            "gallery_segmented.rs",
        ],
        reason: Some(
            "Built by composing the `001a` button rather than copying it. The resolver \
             routes an unselected segment straight through \
             `button::resolve_style(Secondary, ..)` and paints a selected one as the \
             **Secondary Pressed** cell, so the selected fill is held through hover and \
             press (`surface.active` is already lighter than `surface.hover`) and no new \
             colour is introduced. Strip geometry is 28px/32px tall, 4px outer radius, \
             `1px border.strong` perimeter; every segment charges the reserved \
             `CHECK_SLOT` leading tick whether or not it is selected, so selecting never \
             moves a label (`UIF-PRIN-003`), and a disabled selected segment keeps its \
             tick in disabled ink (`UIF-PRIN-005`). `SegmentedModel<Id>` carries \
             `SingleSelect`/`MultiSelect` over a `BTreeSet<Id>`, emits \
             `on_selection_change`, and `Left`/`Right` roving moves focus across segments \
             with `Space`/`Enter` toggling — the walk is shared with the radio group \
             through `widget/rove.rs`. A ToggleButton is one segment under \
             **MultiSelect**: `SingleSelect::toggle` treats re-picking the held member as \
             a no-op, so a one-segment `SingleSelect` strip could be switched on and \
             never off. Missing: the **live `iced::Widget`** — the resolver, geometry and \
             model are pure and proven, but nothing in this crate yet paints the strip or \
             routes real iced events into it, so the control is host-drivable rather \
             than rendered.",
        ),
        note: Some(
            "Framework-owned under D-2: a segmented control is a generic atom, not navbar \
             or tab content, so this is unimplemented framework work rather than a scope \
             exclusion. Collision found and pinned: a **pressed unselected** segment \
             resolves to the same fill as a **selected** one, so colour alone cannot tell \
             them apart — the reserved tick is the only disambiguator, which is exactly \
             why `UIF-PRIN-005` requires it.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001c",
        status: Status::Partial,
        section: "§3.1",
        evidence: &[
            "widget/split_button.rs",
            "widget/split_button_model.rs",
            "widget/split_button_host.rs",
            "widget/button_controller.rs",
            "surface/menu/",
            "gallery_split_button.rs",
        ],
        reason: Some(
            "Built at maximum composition: the primary half contains a real `ButtonModel` \
             driven by a real `ButtonController`, so `ActionPending` arrives through \
             `model.pending()` rather than a parallel mechanism, and the flyout is the \
             **existing** menu engine — `SplitButtonHost` owns exactly one `MenuCascade` \
             and returns `SplitButtonOutcome { cascade, emissions }`. No second menu was \
             added; escape-rank **G-13** and roving menu focus **D-7** are the engine's \
             own. The two zones are independent hit targets with independent hover and \
             activation, pinned negatively: hovering the label does not light the chevron \
             and pressing the label does not open the menu. The menu opens on chevron \
             **release** — §3.1c says a chevron click, which under §3.0.3.3 is \
             press-and-release. `Escape` after the menu is open calls `close_tree()`, and \
             `FocusState::pop_trap()` restores the focus recorded before the trap was \
             pushed, so focus returns to the control without firing the primary action. \
             `SplitButtonModel<ActionId>` carries the rows and emits \
             `on_primary_activate()` / `on_menu_toggle(bool)` / `on_menu_select(ActionId)`. \
             Missing: the **live `iced::Widget`** that paints both halves and routes real \
             events into the host.",
        ),
        note: Some(
            "Spec gap (§3.1c): the divider is given as one token — `1px border.strong` — \
             for both variants. On a Primary control's `accent.fill` that stroke measures \
             **1.0876:1**, i.e. physically invisible, while on a Secondary control it \
             reads clearly. The spec needs a per-variant stroke or a gap between the \
             halves. The figure is carried in the gallery's `resolved` set rather than \
             judged, so the finding stays visible without reporting a failure against a \
             floor the spec never asked that stroke to meet. Related: §3.4 gives the \
             combobox chevron `text.muted`, unusable here at 1.84:1 on `accent.fill`, so \
             the chevron takes the menu half's own resolved ink (`text.strong`, \
             4.8020:1).",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001d",
        status: Status::Partial,
        section: "§3.1",
        evidence: &[
            "widget/hyperlink.rs",
            "widget/hyperlink_model.rs",
            "widget/focusable.rs",
            "token/color.rs",
            "gallery_hyperlink.rs",
        ],
        reason: Some(
            "Built as the inline-or-standalone interactive-text atom: 24px hit target, \
             `radius.widget` focus ring, and the four painted states §3.1d names — \
             `accent.text` with a 1px underline at rest, `text.strong` on a \
             `surface.hover` pill on hover, `accent.text` on `surface.active` pressed, \
             `text.disabled` with the underline removed when disabled. `HyperlinkState` \
             implements `operation::Focusable` by delegating to a `FocusableState`, so a \
             link joins the framework's focus order rather than carrying a private one. \
             `HyperlinkModel { label, uri, external, disabled }` emits \
             `Commit<HyperlinkTarget { uri, external }>` — a typed value, not a raw \
             message — and `HoverUri(Option<String>)` is a separate tier, so a status-bar \
             target never arrives as a commit. A disabled link emits neither. The focus \
             ring is resolved against the **mount**, not the pill, because the control \
             paints nothing at rest; the label is audited against whatever it actually \
             sits on (the pill where one is painted, the mount otherwise) so the \
             transparent-fill exemption cannot silently drop the figures §3.1d quotes. \
             Missing: the **live `iced::Widget`** that draws the text, underline and pill \
             and routes real events into `HyperlinkState`.",
        ),
        note: Some(
            "Every ratio §3.1d prints was recomputed from raw sRGB through \
             `token/contrast.rs` and matched — including the disabled polarity. The \
             segmented/split fill is deliberately **not** audited against the mount: no \
             surface rung in this palette clears 3.0 against `surface.raised` \
             (`surface.active` measures 1.3089), so selection is signalled by \
             `accent.border` plus the tick rather than by fill contrast.",
        ),
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
        evidence: &[
            "widget/password_model.rs",
            "widget/password_host.rs",
            "widget/text_input.rs",
            "gallery_secret.rs",
        ],
        reason: Some(
            "Built as a variant of the `002a` atom rather than a second widget. \
             `InputKind::Password` gives the field a reveal slot instead of a \
             clear button; `SecretInput` wraps the plaintext behind a redacting \
             `Debug`/`Display`; `PasswordInputModel` owns the reveal and \
             enforces `revealed ⇒ focused` on both the rise (`toggle_revealed` \
             refuses while unfocused, disabled or read-only, and a refused press \
             emits nothing rather than reporting a change that did not happen) \
             and the fall (`note_focus(false)` resets to masked). \
             `PasswordController` translates the atom's tiers so the value \
             leaving the editor is a `SecretInput`, never a bare `String`. \
             Masking is iced's own `Value::secure`, which substitutes one \
             U+2022 per grapheme and suppresses `Ctrl+C`/`Ctrl+X` while set. \
             Recorded as deviations from the spec's literal text: the spec's \
             `👁‍🗨` is a three-scalar ZWJ sequence and `SlotContent` carries a \
             single `char`, so the toggle is bare U+1F441; the spec's \
             `PasswordInputModel` names no `value` field, so this model owns \
             `value: SecretInput` and derives `masked_len` and `revealed_value` \
             from it rather than keeping a second copy of the secret; \
             `on_commit()` is `Commit<SecretInput>`, because a commit that \
             carries no value cannot be persisted by the host that receives it. \
             One gap is iced's rather than a choice: the input method's \
             `Purpose::Secure` is derived from the same flag as masking, so \
             revealing the secret also drops the secure-IME hint for as long as \
             it is shown.",
        ),
        note: Some(
            "Framework-owned under D-2. Width invariance is proved against the \
             partition rather than the glyph run, because the run genuinely \
             changes width when a secret is shown: `text_area_bounds` and \
             `trailing_slot_bounds` take no mask parameter, and the laid-out \
             editor rectangle is asserted identical masked and revealed. The \
             reveal toggle outranks the invalid mark — invalidity is carried by \
             the 2px danger border, so a rejected password never loses the \
             control needed to read it back.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-002d",
        status: Status::Implemented,
        section: "§3.2",
        evidence: &[
            "widget/auto_suggest_model.rs",
            "widget/auto_suggest_host.rs",
            "widget/text_input.rs",
            "surface/menu/",
            "surface/zorder.rs",
            "gallery_suggest.rs",
        ],
        reason: None,
        note: Some(
            "Framework-owned under D-2. Built as the binding the atom is: \
             `AutoSuggestModel<Id>` is the spec's five fields exactly \
             (`query`, `suggestions`, `highlighted_index`, `busy`, \
             `disabled`), and `AutoSuggestHost` joins the already-proven \
             `TextInputController` to the already-proven menu engine without \
             either one knowing the other exists. The flyout is Z-Band 2 at the \
             spec's 240px max height, 32px rows and a 4px gap, every figure \
             read from tokens where tokens own the number. \
             `on_query_change` is the coalesced `Live` tick, \
             `on_suggestion_highlight` is the roving walk, and \
             `on_suggestion_commit` carries `CommitSource::{Enter, Click, \
             Tab}`. The two-stage `Escape` is the whole transition table, \
             including the third row where the key is not consumed and travels \
             on. Recorded deviations: the second `Escape` **clears** the query \
             rather than reverting to the pre-edit snapshot `002a` uses — a \
             search term is disposable text whose useful reset is empty, and \
             the revert tier remains available on the inner controller; the \
             commit carries the settled `label` beside the id, because a \
             commit naming only a candidate cannot settle the field text in the \
             host that receives it; and the debounce window is a \
             framework-chosen named constant (`DEBOUNCE_MS = 200`, \
             trailing-edge, restarted by every keystroke) because §3.2 names \
             no window at all. The flyout's identity is minted once from the \
             **control** (`menu_id_for`), never from the query, so \
             `MenuCascade::sync` keeps resolving the open card across every \
             keystroke instead of orphaning it on the first character. Roving \
             issues no iced focus operation, so the caret stays where the user \
             left it while the highlight moves — the two live in different \
             systems. A query that matches nothing opens nothing: the engine \
             will not host a card with no focusable row, and no empty-state row \
             is invented to work around it. Out-of-order answers are refused \
             by a monotonic generation token echoed through \
             `apply_suggestions` and counted in `stale_dropped` rather than \
             silently discarded. A dismissed query stays dismissed against its \
             own text, so a response still in flight cannot resurrect the list \
             the user just closed. Time enters only as `TimeMs`; nothing in \
             the machine reads a clock.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-002e",
        status: Status::Partial,
        section: "§3.2",
        evidence: &[
            "widget/text_area_model.rs",
            "widget/text_area_host.rs",
            "widget/text_input.rs",
            "gallery_secret.rs",
        ],
        reason: Some(
            "Built on the atom rather than beside it. `InputKind::TextArea` reuses \
             the zero-shift slot partition and the §3.2 cell matrix unchanged, \
             and `TextAreaController` composes `TextInputController` outright, \
             so the snapshot-at-first-edit rule, the coalesced `Live` tick and \
             the `Escape` revert are the same code that drives the single-line \
             field. `Enter` inserts a newline and never settles; the settle \
             moves to focus loss, and `Ctrl+Enter`/`Cmd+Enter` is a distinct \
             `Submitted` arm that deliberately does not settle, so the `Escape` \
             after a send still reverts what was typed. `LineCol`, the `Growth` \
             min/max-row solver, `dirty` tracking and `ParseDiagnostic` are \
             carried by the model, and `EditHistory` fills the undo hole iced \
             leaves — `text_editor`'s `Action` enum covers insert, delete and \
             movement and stops there. Two deviations from the spec's literal \
             text: the item names no `on_commit`/`on_cancel`, yet §3.0.4.2's \
             tier discipline requires a settle and a revert to exist at all, so \
             `Committed` (focus loss with an open session) and `Cancelled` \
             (`Escape`) were added; and the printed `min_height: 72px / 3 lines \
             of type.body` does not compute — `typography::BODY` is 14px on a \
             20px line height, so three lines plus two `space.sm` gaps is 76px, \
             and the Bottom Command Bar's `44px..120px` is off the body grid by \
             the same arithmetic (44−16 = 28 → 1.4 lines; 120−16 = 104 → 5.2). \
             The solver is built from the tokens, not from the printed pixels.",
        ),
        note: Some(
            "The `font.family.mono` requirement inside this one still cannot be \
             met: `UIF-TOK-004` models no font family at all, so a mono text \
             area has no rung to ask for it. That gap stays recorded against \
             `UIF-TOK-004` rather than being papered over here.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-003",
        status: Status::Partial,
        section: "§3.3",
        evidence: &[
            "widget/toggle.rs",
            "widget/toggle_model.rs",
            "token/size.rs",
            "gallery_toggle.rs",
        ],
        reason: Some(
            "Resolver, models and geometry are built; the widget is not. `widget/toggle.rs` \
             resolves the two-row polarity matrix for all three atoms (`resolve_style` keyed \
             on `TogglePolarity`, one matrix shared by every control), classifies the \
             indicator's chrome (§3.5's four classes: `CoincidentWithFill` when the fill \
             meets the 3.08 floor, `RestingDivider` at Rest, `Judged` otherwise), derives \
             the focus ring adaptively rather than storing two constants, and supplies the \
             pure geometry — the 16×16px box at `radius.widget`, the circle with its \
             centred `size.checkbox.box / 2` dot, the 36×20px track with the 14×14px thumb \
             at a derived 3px inset, and the 24px hit target spanning the label across an \
             8px gap. `widget/toggle_model.rs` holds `CheckState` with the deterministic \
             `Indeterminate`→`Unchecked` rule (advance only when every enabled child is \
             already checked; a leaf goes to `Checked`), `RadioGroupModel<Id>` with \
             arrow-key roving that skips disabled options and wraps both ends, and \
             `SwitchModel` that refuses `toggle()` while `busy` or `disabled`. All of it is \
             audited across 30 gallery cells. Missing: the `iced` `Element` wrapper itself — \
             `toggle.rs` paints layers and routes `ToggleKey`, but nothing here implements \
             `Widget`, so no `on_toggle(CheckState)` / `on_toggle(bool)` callback reaches \
             an application, and the `✓` is carried as a colour in `resolved.mark` rather \
             than drawn as a glyph.",
        ),
        note: Some(
            "Two figures in §3.3's printed ratio column do not reproduce from the palette \
             under the §1.1 maths: `accent.border` on `surface.hover` is 5.44:1, not the \
             5.35:1 printed, and `accent.border` on `surface.code` — the off-polarity \
             focus-ring upper bound — is 6.17:1, not 6.67:1. Neither breaks a floor, so \
             nothing is blocked; the discrepancy is recorded rather than propagated. The \
             `CoincidentWithFill` 3.08 floor also fails on `surface.active` (2.98 for \
             `accent.fill`, 2.53 for `accent.fill_active`), a limitation the Primary \
             button already carries; that class is exempt from the build-time gate, so the \
             gallery cell is the only place it is visible.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-004",
        status: Status::Implemented,
        section: "§3.4",
        evidence: &[
            "widget/combo_model.rs",
            "widget/combo_box.rs",
            "widget/combo_host.rs",
            "surface/menu/",
            "geometry/anchor.rs",
            "gallery_combo.rs",
        ],
        reason: None,
        note: Some(
            "Built on the existing menu engine rather than a second flyout: \
             `ComboHost` owns exactly one `MenuCascade` and one `GestureSlot`, opens \
             through `MenuTreeKind::Fixed`, and inherits `UIF-CMENU-002`'s mutual \
             exclusion, the roving focus, the escape-rank behaviour and the Z-Band 2 \
             registration from the reuse. The trigger resolves through \
             `InputKind::DropdownTrigger`, so its fill, border and value are the \
             §3.2 dropdown row; §3.4 overrides one channel, the chevron, to \
             `text.strong` on hover/open. `ComboBoxModel<Id>` carries every field \
             §3.4 prints plus four it does not: `editable` (the unnamed \"when \
             searchable\" mode, default off, making the closed select the base \
             case), `read_only` (imported from the text input's meaning — browsable, \
             never commits), `filter_policy` and `menu_id` (derived from the control, \
             never the filter, so `MenuCascade::sync` keeps resolving the open card \
             across keystrokes). The `✓` is the engine's own `Check` kind, painted \
             `accent.text` as §3.4 requires. `Up` from a closed control opens onto \
             the last row and `Home`/`End` jump the roster, both as wrapping \
             arithmetic against `Menu::navigable_indices` — no new engine primitive. \
             **Four §3.4 silences, recorded rather than silently resolved.** \
             (1) The section gives `filter_query` and `on_filter_change(String)` and \
             never says how a query matches; the defaults are case-insensitive \
             substring with the selected row kept, each overridable through \
             `FilterPolicy` so the choice is visible rather than a hidden \
             `str::contains`. (2) What shows when nothing matches is forced by the \
             engine, not chosen: a card with no focusable row is refused \
             (`IgnoredReason::NoFocusableRows`), so the flyout cannot be shown and \
             the keystroke is still reported live. (3) The snapshot `Escape` \
             restores covers the filter as well as the selection, because reverting \
             only the selection would hand the user back a value the surviving \
             filter is hiding. (4) Closing by the trigger is not a cancellation and \
             reverts nothing; only `Escape` reverts. **Two things §3.4 does not \
             reach.** Type-ahead on the closed select is not implemented and cannot \
             be — the control owns no text buffer, so printable keys are outside \
             its vocabulary; the editable variant owns them through a different \
             door, which is also what stops the two variants contending for one \
             keystroke. `ActionPending` is unsupported: §3.4 carries no `busy` or \
             `progress` and §3.0.4.3 does not name the combobox among the \
             pending-capable controls, so a long-running option load is \
             unspecified.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-005",
        status: Status::Implemented,
        section: "§3.5",
        evidence: &[
            "widget/range_model.rs",
            "widget/slider.rs",
            "widget/slider_host.rs",
            "widget/number_box.rs",
            "widget/number_box_host.rs",
            "token/size.rs",
            "gallery_range.rs",
        ],
        reason: None,
        note: Some(
            "Built. One `RangeModel` backs both controls, so the value, the bounds and the \
             step live in exactly one place; the slider and the number box are two paints of \
             the same model rather than two sources of truth. The scrub contract is carried \
             by `GestureSlot`, which hands the open gesture over exactly once: the release \
             publishes `Commit` and closes the slot, so a second settle finds nothing to \
             commit and cannot publish a second time. `Escape` restores the value the model \
             held before the drag began. Pointer capture keeps the thumb tracking after the \
             pointer leaves the track, and a release outside the track still settles — the \
             gesture cannot leak open. Keyboard steps publish `Commit`, not `Live`, because \
             §3.5 lists them under `on_commit`. Grid alignment re-derives the index from \
             `low` on every step instead of accumulating, so a thousand steps cannot drift. \
             Non-finite input is rejected before clamping, an inverted range is normalised at \
             read time, and `step == 0` reads as continuous. \
             SPEC GAPS, recorded rather than invented: §3.5 never says what happens when a \
             Slider and a NumberBox are bound to the same model and both are live, so no \
             cross-control arbitration exists; §3.5 gives the slider no `invalid` skin, so \
             the slider has no invalid column; the 2×4px tick has no token and is derived \
             from the track; and the `28×32px` stepper block cannot physically hold two \
             `24×24px` buttons side by side, so the side-by-side shape is priced at 48px \
             wide. The filled rail against the unfilled rail measures 2.98:1, under the 3.0 \
             chrome floor; both are interior to one control rather than a perimeter, so the \
             pair is reported but not judged.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-006",
        status: Status::Implemented,
        section: "§3.6",
        evidence: &[
            "widget/chip.rs",
            "widget/chip_model.rs",
            "token/shape.rs",
            "token/size.rs",
            "gallery_chip.rs",
        ],
        reason: None,
        note: Some(
            "Built. The two heights §3.6 prices are a rung on the model \
             (`ChipHeight::Compact` = `size.bar.status` 24px, `ChipHeight::Bar` = \
             `size.control.sm` 28px) rather than variant-derived, because the section \
             ties the choice to where the chip sits. `space.sm` padding, `space.xs` gap, \
             `radius.pill` and `radius.widget` as §3.6 assigns them, and the reserved \
             24×24px trailing `×` hit target on removable input chips \
             (`size.hit_target_min`). `ChipModel<Id>` carries the `Assist` / \
             `Filter { selected }` / `Input { removable }` variants, and `respond` maps \
             one gesture to exactly one event: `Assist` and `Input` emit \
             `Activated(id)` (`on_activate`), `Filter` emits `Toggled(id, !selected)` \
             (`on_toggle`), and the trailing `×` or a `Delete`/`Backspace` while the \
             chip holds focus emits `Removed(id)`. The resolver is keyed on \
             `ChipPolarity` (Unselected / Selected), which is §3.6's own table key; the \
             variant reaches paint only through the radius and the `✓` / `×` slots. \
             **§3.6 gap, recorded rather than silently resolved.** The section's radius \
             sentence assigns `radius.pill` to \"status/count pills\" and `radius.widget` \
             to \"filter/input chips\", and names `Assist` in neither group. The pill \
             branch is taken for `Assist` — the chip that is neither a filter nor an \
             input — because the competing reading cannot hold: §3.8's *status chip* is \
             also called a chip, and §3.6's own sentence puts it in the pill branch. A \
             selected chip's disabled skin is a deliberate concession: the fill drops \
             from `accent.fill` to `surface.raised` because `text_disabled` on \
             `accent.fill` measures **1.4280:1**, but the `✓` is held at `text.strong` \
             (**18.7108:1**), because the glyph is the state and a chip may not report a \
             state it does not have. One mount is disallowed: `accent.fill` on \
             `surface.active` is **2.9768:1**, under the `3.08:1` a fill carrying its own \
             boundary owes — so a selected chip may sit on `base`, `raised` or `hover`, \
             never on `active`.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-007",
        status: Status::Partial,
        section: "§3.7",
        evidence: &[
            "surface/tooltip/mod.rs",
            "surface/tooltip/hover.rs",
            "surface/tooltip/place.rs",
            "surface/popover/mod.rs",
            "surface/popover/model.rs",
            "surface/popover/place.rs",
            "surface/popover/host.rs",
            "geometry/anchor.rs",
            "gallery_popover.rs",
        ],
        reason: Some(
            "Both halves built. The `Tooltip` half is conformed to §3.7 as revised: 400ms \
             hover delay, 0ms keyboard-focus delay that opens immediately through a second \
             arrival channel rather than a retimed hover, `surface.overlay` fill, `1px \
             border.strong`, `radius.widget` (4px), 280px max width and `4px 8px` \
             padding, dismissal on pointer leave, key press or `Escape`, and pointer \
             transparency through a layer that sits above every z-band without joining the \
             registry. The `TeachingPopover` half is built as `surface::popover`: the \
             interactive callout clamped to the 280–360px band with `1px accent.border`, \
             `radius.card` (6px) and `space.md` padding, a 13px SemiBold title over a \
             13px Regular body on `type.ui_compact`, and `PopoverModel<ActionId>` whose \
             footer produces `on_action(ActionId)` or `on_dismiss()`. Placement reuses \
             `geometry::anchor::solve` — `PreferredSide` maps onto the existing alignment \
             axes, so the two side placements share the submenu's flip and clamp rather \
             than a second solver. The card enters `SurfaceRegistry` as \
             `SurfaceKind::TeachingPopover` over `ZBand::Menu`, which is exactly §3.7's \
             \"Z-Band 2 / Escape Rank 3\": `ZBand::Menu` *is* band 2 and its \
             `escape_rank()` *is* 3, so no new band was added and no rank retuned. \
             `open` pushes its own focus trap the way `MenuCascade::mount` does, so a \
             callout opened inside a modal nests rather than replacing, and closing \
             restores what the modal held. Missing, unchanged from before this increment: the tooltip is not wired to any \
             widget's own hover or focus event inside this crate, so a host must drive a \
             `HoverController` itself; and the optional trailing shortcut pill in \
             `font.family.mono` has no rung to name, because `UIF-TOK-004` models no font \
             family.",
        ),
        note: Some(
            "Three of INC-31's invented values were wrong against the revision and are now \
             conformed: hover delay 500 → 400 ms, radius `RADIUS_CARD` (6px) → \
             `RADIUS_WIDGET` (4px), and max width from 'the host's problem' to 280px \
             owned and clamped here. Keyboard-focus opening at 0 ms did not exist at all \
             and is now a real second arrival channel. The fill/border/text triple INC-31 \
             chose was already right — the spec's own `4.01:1` border and `11.72:1` text \
             figures match what this crate recomputed from raw sRGB, not transcribed. \
             **§3.7's printed `accent.border` / `surface.overlay` figure of `5.94:1` was \
             recomputed this increment at `5.9394:1` — the section rounds, and the \
             measurement agrees; so does the title at `17.7168:1` and the body at \
             `11.7180:1`. No disagreement with the printed column, unlike §3.8.** Two \
             framework-supplied decisions are recorded here because §3.7 does not state \
             them: the dismiss button is **always present** (`DISMISS_LABEL`), since the \
             section's model carries only `primary_action: Option<…>` and a popover with \
             no focusable control would trap the keyboard around an empty scope; and \
             pointer-leave deliberately dismisses **nothing**, which is what lets the \
             cursor travel from the trigger across the gap onto a footer button without \
             the card vanishing under it. `Escape` on a callout dismisses *and consumes* \
             at rank 3, so a modal underneath stays open — the opposite of the tooltip's \
             \"dismiss but do not consume\", and the reason the callout joins the registry \
             where the tooltip stays out.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-008",
        status: Status::Implemented,
        section: "§3.8",
        evidence: &[
            "widget/badge.rs",
            "widget/badge_model.rs",
            "gallery_badge.rs",
            "token/size.rs",
        ],
        reason: None,
        note: Some(
            "Three of §3.8's printed contrast figures do not describe the pairs they sit on, \
             and were resolved by measurement rather than transcription: `13.52` is \
             `status.success` on `surface.overlay` (13.5237), not on `surface.raised` \
             (14.2825); `6.10` is `status.warning` on `surface.overlay` (6.1048), not on \
             `surface.raised` (6.4474); and `9.96` is reproduced by no surface at all \
             (`surface.code` gives 9.8543). The Solid count pill is governed by the §2.3 \
             rule-4 lock, which fixes its foreground at `text.strong` regardless — 4.80:1 \
             on `accent.fill`, 5.13:1 on `status.danger.fill` — so the mis-transcription \
             could not have hidden a failing pair. The Neutral chip's 9.96 is judged against \
             `surface.raised` (14.28:1), the surface the row names."
        ),
    },
    Coverage {
        id: "UIF-ATOM-009",
        status: Status::Partial,
        section: "§3.9",
        evidence: &[
            "widget/progress.rs",
            "widget/progress_model.rs",
            "widget/button_widget.rs",
            "gallery_progress.rs",
        ],
        reason: Some(
            "Two deviations from the section as written, both deliberate. (1) The section \
             says the indeterminate bar falls back to a static striped fill \"under OS \
             `reduced_motion`\"; this crate never reads an OS setting — `reduced_motion` \
             arrives as an injected `bool` on the resolve call, because a wall-clock or \
             platform read inside a resolver would make the frame non-reproducible and the \
             gallery unauditable. The host owns reading the OS preference and passing it \
             in. (2) The 2px spinner stroke has no rung in §2.3's closed width set \
             (`hairline` 0.5 / `border` 1.0 / `border_focus` 2.0 is the closest), so \
             `SPINNER_STROKE` is pinned to `shape::BORDER_FOCUS` and the token gap is \
             recorded here rather than invented silently. Everything the section specifies \
             is otherwise implemented: the 4px pill track on `surface.active`, the \
             `accent.border` / `status.warning` / `status.danger.text` fills for Active / \
             Paused / Error, the 35%-width indeterminate segment with its static-striped \
             fallback, the 14px and 20px spinners on a `border.subtle` track with an \
             `accent.text` (or `text.strong` inside a solid button) arc, `ProgressModel` \
             over `Indeterminate` / `Determinate { completed, total }`, and the \
             `cancellable` trailing 24×24px stop control.",
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
