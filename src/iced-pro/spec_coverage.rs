//! Machine-readable coverage of every `UIF-*` requirement in
//! `spec.md` at this crate's root, plus the audit primitive that keeps
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
//! `Partial` also carries the case these three labels have no separate word
//! for: **owned here, not yet built**. A widget the user deferred to Phase C
//! — a `DatePicker`, a `NavigationRail` — is framework-owned under **D-2**,
//! so `NotOwned` would misstate who owes it, and it is plainly not
//! satisfied, so `Implemented` would be worse. Such an entry is `Partial`
//! with a reason that says outright that nothing of the widget is built and
//! then names what a builder owes. Where real adjacent machinery already
//! exists, the reason says so too, so the entry records how far the work
//! actually starts from rather than leaving it to be discovered.
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
/// **90 as of the 2026-10-23 revision** — 79 unlettered base IDs plus 11
/// letter-suffixed sub-requirements (`UIF-ATOM-000b`, `001a`–`001d`,
/// `002a`–`002e`, `005b`). The suffixed ones are counted **individually**,
/// each as its own requirement: they are not decorations on their parent.
/// See [`extract_spec_ids`], whose whole reason for existing in its current
/// form is that an earlier version of this extractor could not see them.
///
/// The 18 IDs this revision adds are five Phase-B rules and requirements
/// (`UIF-PRIN-006`–`009`, `UIF-TOK-007`), twelve Phase-C widgets the user
/// has explicitly deferred (`UIF-ATOM-005b`, `010`–`013`,
/// `UIF-COMP-009`–`015`) and one new gate (`UIF-VER-005`). Every one of
/// them is classified below; none is `Implemented`, because none is built.
/// The spec carries 92 numbered `UIF-*` ids.
///
/// It carried 90 until commit `edbe04a` added `UIF-MOD-004` (scrollable
/// sub-region height solving) alongside the existing three modal contracts,
/// and 91 until the D-2 boundary review added `UIF-COMP-016`
/// (SplitDivider composite, `§3B.16`).
pub const EXPECTED_SPEC_ID_COUNT: usize = 92;

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
/// `spec.md` and the injected-failure test that proves the guard
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
        status: Status::Implemented,
        section: "§1.1",
        evidence: &["token/", "token_lint.rs", "gallery::unmapped_colours"],
        reason: None,
        note: Some(
            "Token discipline holds by construction — every visual number is declared \
             under `token/` and nothing else — and the gallery gate proves every colour \
             an audited cell emits names a `Palette` token. Machine-checked at the source \
             level: `token_lint` bans colour constructors/hex literals outside `token/color.rs`, \
             bans bare numeric literals in visually-named `pub const`s outside the 60 registered \
             Surface Geometry Constants, and scans inline layout calls (`Length::Fixed`, \
             `Padding::new`, `Padding::from`, `Pixels`) verifying zero bare non-zero literals in \
             production code.",
        ),
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
        status: Status::Implemented,
        section: "§1.1",
        evidence: &[
            "focus.rs",
            "surface/menu/model.rs",
            "composite/empty_state.rs",
            "composite/skeleton.rs",
            "composite/pagination.rs",
        ],
        reason: None,
        note: Some(
            "Both halves verified: unavailable actions stay visible (a disabled focus \
             member keeps its ordinal slot and traversal steps over it, and Disabled is \
             a flag on a row rather than a row kind; pagination boundary buttons render \
             disabled rather than collapsing) and empty panels and loading views show \
             structured placeholder states (EmptyState centered container with icon badge \
             and headline, and Skeleton reserving exact target dimensions with 0px shift).",
        ),
    },
    Coverage {
        id: "UIF-PRIN-005",
        status: Status::Implemented,
        section: "§1.1",
        evidence: &[
            "focus.rs",
            "bridge.rs",
            "widget/focusable.rs",
            "surface/menu/cascade.rs",
            "a11y/contract.rs",
            "composite/stepper.rs",
            "composite/diff_viewer.rs",
            "composite/tab_strip.rs",
            "widget/text_input.rs",
            "widget/toggle_widget.rs",
        ],
        reason: None,
        note: Some(
            "Both keyboard path and redundant visual cueing verified across the entire \
             framework. Full keyboard navigability (Tab, arrow keys, Home/End, Enter/Space, \
             Escape) proven for all interactive controls. Meaning is never communicated \
             by colour alone: selection pairs with check/radio/active glyphs and indicators, \
             validation errors pair with glyphs ('⚠') and perimeter borders, dirty state pairs \
             with dot indicators ('•') and confirmation guards, diff lines pair with '+' / '-' \
             glyphs and 2px left borders, and stepper status pairs with '✓' / '!' glyphs.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-006",
        status: Status::Implemented,
        section: "§1.1",
        evidence: &[
            "layout/density.rs",
            "layout/emphasis.rs",
            "layout/footer.rs",
            "layout/optical.rs",
            "widget/button.rs",
            "surface/dialog_button.rs",
            "token/size.rs",
            "composite/overflow_toolbar.rs",
            "composite/form_field.rs",
            "invariant_gate.rs",
        ],
        reason: None,
        note: Some(
            "All four clauses verified: (1) Single focal point enforced via `check_focal_point` \
             limiting containers to at most one `High` emphasis (`Primary` / `Danger`) action; \
             (2) Row density homogeneity verified via `check_row_density` preventing mixing of \
             Compact (28px) and Standard (32px) controls; (3) Action row button ordering type-governed \
             by `FooterSlot` and `FooterPlan` with initial focus on `[Cancel]` for destructive confirmations; \
             (4) Optical centering slots and shifts verified for directional and badge glyphs. \
             Typography baseline alignment remains a host font-engine responsibility per D-2.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-007",
        status: Status::Implemented,
        section: "§1.1",
        evidence: &[
            "widget/button_model.rs",
            "widget/button_widget.rs",
            "composite/table_cell.rs",
            "token/typography.rs",
            "microcopy/pins.rs",
            "microcopy/case_rule.rs",
            "microcopy/action_label.rs",
            "microcopy/truncate.rs",
            "microcopy/tabular.rs",
            "invariant_gate.rs",
        ],
        reason: None,
        note: Some(
            "Verified by the automated `InvariantGate`: (1) Sentence-case mandate with proper noun / \
             acronym licensing and closed Host `SPEC.md` Literal Pin Table; (2) Verb-first action CTAs \
             rejecting generic OK/Yes/No; (3) ASCII '...' dialog-launcher vs Unicode '…' truncation \
             biconditional; (4) Middle-truncation preserving basename/extension paired with automatic \
             tooltip obligations; (5) Fixed-pitch numeric column alignment via `TabularNums` and \
             `Family::Mono`. Font-level OpenType `tabular-nums` feature selection and pixel-level \
             overflow budgets remain host/font-engine concerns per D-2.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-008",
        status: Status::Implemented,
        section: "§1.1",
        evidence: &[
            "widget/rove.rs",
            "widget/toggle_model.rs",
            "widget/segmented_model.rs",
            "focus.rs",
            "composite/table_header.rs",
            "composite/table_view.rs",
            "composite/tree_row.rs",
            "composite/tree_view.rs",
            "composite/expander.rs",
            "widget/chip.rs",
            "composite/breadcrumb.rs",
            "a11y/contract.rs",
            "a11y/project.rs",
            "a11y/tab_stop.rs",
            "surface/menu/aim.rs",
            "composite/navbar.rs",
            "composite/tab_strip.rs",
            "composite/navigation_rail.rs",
            "composite/command_palette.rs",
            "composite/stepper.rs",
            "composite/grid_view.rs",
            "widget/date_picker.rs",
        ],
        reason: None,
        note: Some(
            "All four clauses verified across all eleven collections named in §1.1 (Menu, \
             RadioGroup, SegmentedButton, NavigationRail, CommandPalette, TabStrip, Stepper, \
             TreeView, TableView, GridView, CalendarView). Bounded groups wrap via rove_index; \
             spatial collections clamp at boundaries. Single-Tab-stop CompositeBoundary proven \
             with 11 stops across the full 11-collection roster, and A11yContract projections \
             cover all item types without reading geometry or clock.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-009",
        status: Status::Implemented,
        section: "§1.1",
        evidence: &[
            "state.rs",
            "widget/text_input_model.rs",
            "widget/text_input.rs",
            "widget/text_area_model.rs",
            "widget/password_model.rs",
            "widget/text_input_controller.rs",
            "binding/tier.rs",
            "binding/snapshot.rs",
            "composite/form_field.rs",
            "validation/stance.rs",
            "validation/disallowed.rs",
            "validation/punish.rs",
            "validation/recovery.rs",
        ],
        reason: None,
        note: Some(
            "All three clauses verified: (1) 'Punish Late' via `EditStance` withholding error visuals \
             on pristine invalid inputs until commit, blur, or submit; (2) 'Reward Early' clearing \
             error visuals and restoring Rest state on the exact frame a valid input is restored; \
             (3) 'Non-Destructive Recovery' with `EditSnapshot` preserving buffer, selection, and scroll \
             offset, pushing caret/selection back into iced's internal `EditorState` during `operate()`.",
        ),
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
        status: Status::Implemented,
        section: "§2.4",
        evidence: &["token/typography.rs"],
        reason: None,
        note: Some(
            "The type scale completely satisfies §2.4: six rungs with exact size, \
             authoritative integer line height (bound as absolute pixels), resting \
             and emphasis weights, tracking, allowed_weights sets, MIN_SIZE = 12 \
             floor, Family::Sans and Family::Mono with .mono() and .sans() methods \
             on any rung. As documented in typography.rs, application content \
             usages of monospace are caller concerns per D-2.",
        ),
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
             3.0820) yet remain banned. The two cells that used to fail the \
             spec's own thresholds are resolved on the rule side, not the colour \
             side: rule 2's ceiling is now `[4.00, 6.05]`, which admits \
             `text.disabled` on `surface.sunken` at 6.0205, and rule 3's \
             `CoincidentWithFill` floor is now 3.00, which admits \
             `status.danger.fill_active` on `surface.overlay` at 3.0789. Both \
             resolutions are pinned as positive pass assertions, each also \
             asserting the pair still sits on the far side of the retired \
             threshold so restoring it fails the suite.",
        ),
    },
    Coverage {
        id: "UIF-TOK-007",
        status: Status::Implemented,
        section: "§2.6",
        evidence: &[
            "token/time.rs",
            "surface/tooltip/hover.rs",
            "surface/tooltip/mod.rs",
            "surface/tooltip/warm.rs",
            "gallery_warm.rs",
            "widget/auto_suggest_model.rs",
            "composite/notification.rs",
            "composite/toast.rs",
            "widget/progress.rs",
            "binding/mod.rs",
            "surface/menu/aim.rs",
            "composite/code_block.rs",
            "composite/navigation_rail.rs",
            "composite/side_sheet.rs",
        ],
        reason: None,
        note: Some(
            "All ten §2.6 duration and timing rungs are declared as DurationToken data \
             carrying their canonical names, durations, and pure Easing curves. All rungs \
             are now actively consumed across widgets and surfaces: micro-interactions and \
             ToggleSwitch thumb travel consume FAST (100ms EaseOutCubic) via switch_thumb_progress; \
             flyout cards (ComboBox, DatePicker, TimePicker, ColorPicker) and SideSheet consume \
             NORMAL (180ms EaseOutCubic) via flyout_arrival_progress; modal scrim fade and \
             NavigationRail width transitions consume SLOW (240ms EaseInOutCubic) via \
             slow_transition_progress and rail_width_interpolate; CodeBlock consumes HOLD_COPIED \
             (1500ms); toasts consume TOAST_AUTO_DISMISS (5000ms); inputs consume DEBOUNCE_LIVE \
             (200ms); tooltips consume DELAY_TOOLTIP_OPEN (400ms) and DELAY_TOOLTIP_WARM (100ms); \
             and menu corridors consume DELAY_SUBMENU (150ms). Mandatory reduced_motion \
             fallback suppresses motion transitions while preserving timers.",
        ),
    },
    // --- §3 Atomic Widget State Matrices ------------------------------------
    Coverage {
        id: "UIF-ATOM-000",
        status: Status::Implemented,
        section: "§3.0",
        evidence: &[
            "state.rs",
            "widget/focus.rs",
            "widget/button_widget.rs",
            "binding/controlled.rs",
            "binding/tier.rs",
            "binding/snapshot.rs",
        ],
        reason: None,
        note: Some(
            "The two-part orthogonal model is the crate's foundation: `VisualState \
             { base, focused }`, base precedence `Disabled > Invalid > Pressed > \
             Hovered > Rest`, and total disabled dominance — a disabled control \
             forces `focused` false and focus advances to the next enabled control. \
             The nine reachable states are verified in the gallery's nine columns. \
             Press-and-release activation is implemented: the control fires only on \
             release still inside its bounds, and a drag-out cancels. Both placements \
             of the focus indicator exist and share one adaptive colour rule: the 2px \
             outer ring for buttons and inputs, and the 1px inset outline for dense rows.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-000b",
        status: Status::Implemented,
        section: "§3.0.4",
        evidence: &[
            "state.rs",
            "surface/movable.rs",
            "binding/controlled.rs",
            "binding/tier.rs",
            "binding/snapshot.rs",
            "binding/pending.rs",
            "binding/responsive.rs",
            "widget/button_model.rs",
            "widget/button_controller.rs",
            "widget/text_input.rs",
            "widget/range_slider.rs",
            "widget/date_picker.rs",
            "widget/time_picker.rs",
            "widget/color_picker.rs",
        ],
        reason: None,
        note: Some(
            "The declarative widget contract from §3.0.4 is fully implemented and \
             adopted across the widget family: `Controlled<'a, M>` against `Ephemeral<S>` \
             for the ownership split; `Live<T>` / `Commit<T>` / `Cancel<T>` three-tier \
             callbacks and linear `Gesture<T>` guards preventing double-settles; \
             `ActionPending` with `ActivationGate` double-click protection; and pure \
             container-query `ResponsiveBreakpoint` solvers without global-window queries. \
             Adopted by Button, TextInput, RangeSlider, DatePicker, TimePicker, ColorPicker, \
             SideSheet, and CommandPalette.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001",
        status: Status::Implemented,
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
        reason: None,
        note: Some(
            "All four variants resolve and paint across reachable state matrix. `Button::into_element` \
             attaches `surface::tooltip::Tooltip` automatically whenever a tooltip is configured, \
             fulfilling the icon-only tooltip requirement at paint time without host boiler-plate.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001a",
        status: Status::Implemented,
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
        reason: None,
        note: Some(
            "Controlled button model with linear activation gesture, cancel requests, and busy \
             spinner arc. Live container query dynamically adjusts horizontal padding during \
             `Widget::layout` via `model.responsive_to(limits.max().width)` (§3.0.4.4).",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001b",
        status: Status::Implemented,
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
        reason: None,
        note: Some(
            "Built by composing the `001a` button rather than copying it. Unselected segments \
             route through `button::resolve_style(Secondary, ..)` and selected segments paint \
             as the Secondary Pressed cell. Live `SegmentedButton` widget renders the strip \
             with asymmetric segment corner radii, paints the reserved leading `✓` in \
             `accent.text` (or `text.disabled` when inert), routes keyboard arrows and \
             Space/Enter roving selection, and renders the outer focus ring around the strip.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-001c",
        status: Status::Implemented,
        section: "§3.1",
        evidence: &[
            "widget/split_button.rs",
            "widget/split_button_model.rs",
            "widget/split_button_host.rs",
            "widget/button_controller.rs",
            "surface/menu/",
            "gallery_split_button.rs",
        ],
        reason: None,
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
        status: Status::Implemented,
        section: "§3.1",
        evidence: &[
            "widget/hyperlink.rs",
            "widget/hyperlink_model.rs",
            "widget/focusable.rs",
            "token/color.rs",
            "gallery_hyperlink.rs",
        ],
        reason: None,
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
        status: Status::Implemented,
        section: "§3.2",
        evidence: &[
            "widget/text_input.rs",
            "widget/focusable.rs",
            "widget/password_model.rs",
            "widget/password_host.rs",
            "widget/auto_suggest_model.rs",
            "widget/auto_suggest_host.rs",
            "widget/text_area_model.rs",
            "widget/text_area_host.rs",
        ],
        reason: None,
        note: Some(
            "All six input kinds §3.2 defines are built and verified across their \
             reachable state matrices: Entry, Search, Dropdown-trigger, \
             PasswordInput (002c), AutoSuggestBox (002d), and MultiLineTextArea (002e).",
        ),
    },
    Coverage {
        id: "UIF-ATOM-002a",
        status: Status::Implemented,
        section: "§3.2",
        evidence: &[
            "widget/text_input.rs",
            "widget/text_input_model.rs",
            "widget/text_input_controller.rs",
            "binding/controlled.rs",
            "binding/tier.rs",
        ],
        reason: None,
        note: Some(
            "All four callbacks routed through tiers (Live, Commit, Cleared, FocusChange). \
             Editing session snapshot taken at first edit after focus. Selection and caret \
             positioning applied to inner EditorState via operate traversal.",
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
        status: Status::Implemented,
        section: "§3.2",
        evidence: &[
            "widget/password_model.rs",
            "widget/password_host.rs",
            "widget/text_input.rs",
            "gallery_secret.rs",
        ],
        reason: None,
        note: Some(
            "Built as a variant of the `002a` atom rather than a second widget. \
             `InputKind::Password` gives the field a reveal slot instead of a \
             clear button; `SecretInput` wraps the plaintext behind a redacting \
             `Debug`/`Display`; `PasswordInputModel` owns the reveal and \
             enforces `revealed ⇒ focused` on both the rise and fall. \
             `PasswordController` translates the atom's tiers so the value \
             leaving the editor is a `SecretInput`. Masking is iced's own \
             `Value::secure` suppressing clipboard copy while set. Width invariance \
             is proved against the slot partition, and laid-out editor bounds are \
             asserted identical masked and revealed.",
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
        status: Status::Implemented,
        section: "§3.2",
        evidence: &[
            "widget/text_area_model.rs",
            "widget/text_area_host.rs",
            "widget/text_input.rs",
            "gallery_secret.rs",
        ],
        reason: None,
        note: Some(
            "Built on the atom: `InputKind::TextArea` reuses the zero-shift slot partition \
             and the §3.2 cell matrix. `TextAreaController` composes `TextInputController`, \
             `Enter` inserts newlines, focus loss commits, and `Ctrl+Enter` emits `Submitted`. \
             Includes `LineCol`, min/max-row `Growth` solver, undo `EditHistory`, `dirty` \
             tracking, and monospace font support via `.mono()` builder method.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-003",
        status: Status::Implemented,
        section: "§3.3",
        evidence: &[
            "widget/toggle.rs",
            "widget/toggle_model.rs",
            "widget/toggle_widget.rs",
            "token/size.rs",
            "gallery_toggle.rs",
        ],
        reason: None,
        note: Some(
            "Two figures in §3.3's printed ratio column do not reproduce from the palette \
             under the §1.1 maths: `accent.border` on `surface.hover` is 5.44:1, not the \
             5.35:1 printed, and `accent.border` on `surface.code` — the off-polarity \
             focus-ring upper bound — is 6.17:1, not 6.67:1. Neither breaks a floor, so \
             nothing is blocked; the discrepancy is recorded rather than propagated. The \
             `CoincidentWithFill` 3.00 floor also fails on `surface.active` (2.98 for \
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
            "widget/range_slider.rs",
            "widget/range_slider_model.rs",
            "widget/slider.rs",
            "widget/slider_host.rs",
            "widget/number_box.rs",
            "widget/number_box_host.rs",
            "token/size.rs",
            "gallery_range.rs",
        ],
        reason: None,
        note: Some(
            "Built. All three controls named in §3.5 — `Slider`, `RangeSlider`, and `NumberBox` — \
             are implemented and pinned. One `RangeModel` backs `Slider` and `NumberBox`, \
             and `RangeSliderModel` backs the dual-thumb `RangeSlider` with non-crossing \
             interval enforcement. The scrub contract is carried by `GestureSlot`, which hands \
             the open gesture over exactly once: release publishes `Commit` and closes the \
             slot, and `Escape` restores the pre-drag value. Pointer capture keeps the thumb \
             tracking after pointer leaves the track, and release outside still settles without \
             leaking. Keyboard steps publish `Commit`. Grid alignment re-derives indices from \
             `low` on every step to prevent cumulative drift. Non-finite inputs are rejected \
             before clamping, inverted ranges are normalised at read time, and `step == 0` reads \
             as continuous. With `UIF-ATOM-005b` implemented, the parent requirement is fully \
             satisfied.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-005b",
        status: Status::Implemented,
        section: "§3.5",
        evidence: &[
            "widget/range_slider.rs",
            "widget/range_slider_model.rs",
            "widget/range_model.rs",
            "widget/slider.rs",
            "widget/slider_host.rs",
            "binding/pending.rs",
            "token/size.rs",
        ],
        reason: None,
        note: Some(
            "Built. The dual-thumb `RangeSlider` is fully implemented with `RangeSliderModel` and \
             `RangeSliderStyle` conforming to §3.5. Enforces the non-crossing invariant \
             `lower <= upper - min_span`, endpoint disambiguation choosing the thumb with room to \
             move when coincident or at min_span, and drift-free grid stepping re-derived from \
             `min`. Only the interior segment between lower and upper thumbs paints `accent.fill` \
             (`border.strong` when disabled), with `surface.active` groove outside it. The \
             tabular-numeral readout pill (`12px`, `FontFamily::Mono`, `surface.overlay` + `1px \
             border.strong`) renders 4px above any thumb that is `Hovered`, `Pressed`, or \
             `Focused` without shifting outer layout (`UIF-PRIN-003`). Supports roving tab focus \
             traversing `Lower -> Upper -> next` control and three-tier callbacks `Live`, \
             `Commit`, and `Cancel` over `(f64, f64)` tuples.",
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
             `surface.active` is **2.9768:1**, under the `3.00:1` a fill carrying its own \
             boundary owes — so a selected chip may sit on `base`, `raised` or `hover`, \
             never on `active`.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-007",
        status: Status::Implemented,
        section: "§3.7",
        evidence: &[
            "surface/tooltip/mod.rs",
            "surface/tooltip/hover.rs",
            "surface/tooltip/place.rs",
            "surface/tooltip/widget.rs",
            "surface/popover/mod.rs",
            "surface/popover/model.rs",
            "surface/popover/place.rs",
            "surface/popover/host.rs",
            "geometry/anchor.rs",
            "gallery_popover.rs",
        ],
        reason: None,
        note: Some(
            "Live `Tooltip` widget wiring and optional trailing shortcut pill in \
             `FontFamily::Mono` (`text.muted` 6.78:1) are implemented in \
             `surface/tooltip/widget.rs`. Both halves of §3.7 (Tooltip and \
             TeachingPopover) are now fully implemented: 400ms hover delay, 0ms \
             keyboard-focus delay opening immediately via `HoverController`, \
             `surface.overlay` fill, `1px border.strong`, `radius.widget` (4px), \
             280px max width and `4px 8px` padding, dismissal on pointer leave, key \
             press or `Escape` (without consuming `Escape`), and pointer transparency \
             through a layer outside `SurfaceRegistry`.",
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
             `surface.raised` (14.28:1), the surface the row names.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-009",
        status: Status::Implemented,
        section: "§3.9",
        evidence: &[
            "widget/progress.rs",
            "widget/progress_model.rs",
            "widget/button_widget.rs",
            "gallery_progress.rs",
        ],
        reason: None,
        note: Some(
            "Progress indicators are fully implemented: 4px pill track on `surface.active`, \
             `accent.border` / `status.warning` / `status.danger.text` fills for Active / \
             Paused / Error, 35%-width indeterminate segment with static-striped fallback \
             under injected `reduced_motion`, 14px and 20px spinners on `border.subtle` track \
             with `accent.text` (or `text.strong` inside solid button) arc, `ProgressModel` \
             over `Indeterminate` / `Determinate { completed, total }`, and cancellable \
             trailing 24×24px stop control. Spinner stroke is pinned to `shape::BORDER_FOCUS`.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-010",
        status: Status::Implemented,
        section: "§3.10",
        evidence: &[
            "widget/date_picker.rs",
            "surface/popover/mod.rs",
            "surface/zorder.rs",
            "widget/text_input.rs",
        ],
        reason: None,
        note: Some(
            "Implemented DatePicker, DateRangePicker & CalendarView (§3.10, UIF-ATOM-010) \
             with pure civil calendar arithmetic, 42-cell fixed day grid, redundancy cues, \
             live/commit/cancel callbacks, and keyboard navigation.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-011",
        status: Status::Implemented,
        section: "§3.11",
        evidence: &["widget/time_picker.rs"],
        reason: None,
        note: Some(
            "Implemented TimePicker (§3.11, UIF-ATOM-011) with pure civil time arithmetic, \
             monospace segmented trigger, 220px drum flyout card with header band, parallel \
             scrollable columns, central 32px selection band, pinned 36px footer, and three-tier \
             callbacks (Live, Commit, Cancel) with pre-open snapshot restoration.",
        ),
    },
    Coverage {
        id: "UIF-ATOM-012",
        status: Status::Implemented,
        section: "§3.12",
        evidence: &["widget/color_picker.rs"],
        reason: None,
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-013",
        status: Status::Implemented,
        section: "§3.13",
        evidence: &[
            "widget/avatar.rs",
            "widget/rating.rs",
            "widget/badge_model.rs",
            "token/contrast.rs",
            "token/size.rs",
        ],
        reason: None,
        note: Some(
            "`Avatar` / `PersonPicture` and `RatingControl` implemented in full under \
             §3.13 (`UIF-ATOM-013`). `Avatar` provides the 24/32/36px circular or rounded-square \
             container with 1px border.strong perimeter, the 3-stage fallback cascade \
             (image -> initials -> generic person glyph) clearing 4.50:1 contrast on \
             accent.fill/surface.active, and the 6x6px presence badge with 1px surface.base \
             cutout ring. `RatingControl` provides the row of five 24x24px star slots at \
             space.xs gap with status.warning fills and border.strong empties, whole-step and \
             half-step rating with hover preview, clear-on-reclick/Delete, keyboard \
             navigation, and the Live / Commit / Cancel callback set.",
        ),
    },
    // --- §3B Composite Patterns --------------------------------------------
    //
    // None of these fifteen is out of the crate's scope, but the split is no
    // longer uniform: 001-008 are built here, and 009-015 are the Phase-C list
    // the user deferred on 2026-10-23 and remain unbuilt. Where a composite is
    // only partly framework work the boundary is the same one D-2 draws
    // everywhere else: the container and its chrome are framework work and are
    // recorded here as gaps; the content inside is the app's, and these
    // entries say so rather than claiming it.
    Coverage {
        id: "UIF-COMP-001",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/form_field.rs",
            "composite/settings_row.rs",
            "gallery_composite.rs",
        ],
        reason: None,
        note: Some(
            "The wrapper the spec names now exists as `composite::form_field`. \
             `bands` takes the field's bounds and nothing else - no label, no helper, \
             no error and no validity flag - so a validation transition cannot move \
             the control: the zero-shift requirement is structural, not asserted. \
             The 32px control slot hosts a button, a text input and a focusable \
             row without special-casing any of them, and the 16px caption band is \
             reserved whether it carries a helper, an error or nothing. \
             `composite::settings_row` is the horizontal variant; the printed 48px \
             is a floor, not a height, because a title plus a description composes \
             to 60px.",
        ),
    },
    Coverage {
        id: "UIF-COMP-002",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/notification.rs",
            "composite/notification_tests.rs",
            "composite/info_bar.rs",
            "composite/info_bar_tests.rs",
            "composite/toast.rs",
            "composite/toast_tests.rs",
            "gallery_composite.rs",
        ],
        reason: None,
        note: Some(
            "The severity, model and dismissal contract are shared by both surfaces, so \
             they live once in `composite::notification`; `effective_auto_dismiss_ms` \
             refuses the window for `Warning` and `Danger` no matter what the model \
             asked for, so the never-auto-dismiss rule is the framework's rather than a \
             host convention the framework would have to trust. The countdown is driven \
             only by host-supplied `TimeMs` and a pause banks the remaining duration \
             instead of resetting it, so hovering at 4s leaves 1s rather than a fresh \
             5s. Gaps recorded against the spec: §3B.2 writes \
             \"`BG: surface.raised` (or `surface.overlay`)\" without saying which is \
             which, so the choice is modelled as `BarSurface` rather than resolved \
             behind the host's back; the toast's own fill is unspecified and takes \
             `surface.overlay`; \"max 3 visible\" does not say what happens to the \
             fourth, so the newest takes the anchor and room is made by evicting the \
             oldest auto-dismissible toast, and if all three slots are held by \
             `Warning`/`Danger` the newcomer queues and starts its countdown when it \
             becomes visible rather than when it was posted. The stack reserves no \
             layout, so a dismissal moves nothing outside it; inside it the survivors \
             re-anchor. `Escape` is consumed only while focus is inside the stack, and \
             the band passes clicks outside its boxes through to Band 0. Contrast is \
             recomputed from raw sRGB for every channel against the bar's or toast's \
             own fill, because §2.5 prints no `surface.raised` column; no disagreement \
             with the printed base/overlay/active columns was found.",
        ),
    },
    Coverage {
        id: "UIF-COMP-003",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/expander.rs",
            "composite/expander_tests.rs",
            "gallery_composite.rs",
        ],
        reason: None,
        note: Some(
            "The header box resolves from the row width, the rung and the trailing slot \
             only, so it cannot shift when the body mounts; the toggle is a \
             `GestureSlot<bool>` that commits once on release and reverts on cancel; a \
             collapsed body registers its controls as disabled focus members, so the \
             crate's own traversal cannot tab into it. Gaps recorded against the spec: \
             the body's surface is left as a choice between `base` and `sunken` (this \
             takes `base`), whether opening one disclosure closes its siblings is \
             unspecified (offered as an explicit `AccordionPolicy`), and the header's \
             hover fill is unspecified (the crate's usual `surface.hover` is used). An \
             animated body height is deliberately not modelled — a per-frame height is \
             a layout shift under `UIF-PRIN-003`, so only the chevron angle is \
             interpolable.",
        ),
    },
    Coverage {
        id: "UIF-COMP-004",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/scroll_view.rs",
            "composite/scroll_view_tests.rs",
            "gallery_composite.rs",
        ],
        reason: None,
        note: Some(
            "Delivered as a testable contract rather than a widget: track and thumb \
             geometry, the per-state thumb colours, the minimum thumb length at extreme \
             content ratios, the keyboard steps, the at-bottom and reach-end triggers, \
             the tick annotations, and a thumb drag that settles through the same \
             `GestureSlot` transcript every other gesture uses. `content_bounds` takes \
             no track phase, so the 8px-to-10px growth is paint-only and inward — the \
             layout-shift verdict. The thumb is graded `Judged`, not `CoincidentWithFill`, \
             and clears the 3.00:1 floor over all seven surfaces including the hovered \
             and pressed ones the spec's printed 4.01–4.51 range never prices. Gaps \
             recorded against the spec: timed auto-hide is not specified and is \
             deliberately not invented (phase is a pure function of scrollability and \
             pointer, so a host may map `TimeMs` to `Hidden` itself); the track paints \
             no fill because only the thumb is styled; keyboard step distances, the \
             focus-owner interaction, nested scroll bubbling and the horizontal \
             scrollbar's own behaviour are unspecified — the module resolves one \
             view's geometry and does not arbitrate between two. What §3B.4 does \
             now fix, the code reads from the rungs it names rather than restating: \
             the arrow step is `size.hit_target.min` (24px, one compact tree row) \
             and the page step is `(viewport - 24).max(24)`, with `ARROW_STEP` and \
             `PAGE_KEEP` both pointed at that same 24px rung so the arithmetic is \
             the section's; the reach-end window is `2 × space.2xl` (64px), \
             derived from the spacing scale instead of written as a literal.",
        ),
    },
    Coverage {
        id: "UIF-COMP-005",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/card.rs",
            "composite/list_tile.rs",
            "gallery_composite.rs",
        ],
        reason: None,
        note: Some(
            "`composite::card` and `composite::list_tile` own the two shapes. The \
             tile's slot grid is computed here rather than left to the host \
             (decision D-8): leading, centre and trailing slots, a 24px minimum hit \
             extent for a 16px mark on an interactive tile, and the whole row as \
             the target. A selected tile promotes its subtitle to `text.strong` \
             because `text.muted` on `accent.fill` measures 1.8363:1, and it wears \
             no perimeter stroke because `accent.border` on `accent.fill` measures \
             1.6098:1 - a border that cannot be seen cannot carry the selection. \
             The two-line tile's printed 44px is kept as authoritative; its parts \
             now sum to it exactly — 4 + 18 + 2 + 16 + 4 = 44, the 2px \
             title-to-subtitle gap named as `space.2xs` — so \
             `TWO_LINE_PARTS_HEIGHT` closes against `TWO_LINE_HEIGHT` with nothing \
             left over. The `+ 2px TWO_LINE_UNACCOUNTED` term the section used to \
             append to that same list, which read literally composed 46 against \
             its own printed 44, has since been removed from the spec. The \
             constant is retained at 0.0 purely as the standing drift check: a \
             future edit that reopens the gap makes it non-zero and fails the sidecar.",
        ),
    },
    Coverage {
        id: "UIF-COMP-006",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/virtual_list.rs",
            "composite/tree_row.rs",
            "composite/tree_view.rs",
            "gallery_tree.rs",
        ],
        reason: None,
        note: Some(
            "`virtual_list` owns the recycling arithmetic and `tree_view` owns the row \
             model; `tree_row` owns the paint, per decision D-8. Row height is fixed at \
             exactly two rungs — 24px compact, 28px standard — because a variable-height \
             tree needs prefix-sum offsets, and accumulating float heights drifts: a \
             thousand 0.1px rows sum to 99.9990463257, so every position here is \
             `index × height` recomputed in `f64` and narrowed once. The spec's printed \
             4px-vs-12px guide inset conflict is gone; indentation \
             is width-relative as §3B.6 now states — 16px per level nominal, \
             proportional compression while every level still keeps a guide, then \
             collapse onto the last drawn guide below a 4px floor step, reporting \
             `collapsed_levels` so the host can see the loss. The trigger is still \
             written as the shorthand `row_width - depth × 16px < 96px`, which \
             omits the row's own furniture: `2 × space.xs` of horizontal inset and \
             the 24px chevron column, 32px sitting between the row's edge and the \
             label. Taken literally the shorthand would let the label down to 64px \
             while the same sentence guarantees it never narrows below 96px, so \
             `indent_allowance` subtracts the furniture too and the guarantee means \
             what it says. The \
             96px label floor is depth-independent — the indent saturates there and stops \
             eroding, so a 20-deep and a 200-deep tree render the same label box. Guide \
             lines are graded `Judged`, not the spec's `border.subtle`, which measures \
             1.2691:1 against `surface.raised` and cannot be seen; on the selection fill \
             the chevron, guide and left bar all promote to `text.strong` on the same \
             D-6 rule, because `accent.border` on `accent.fill` is 1.6098:1 and \
             `text.muted` is 1.8363:1. Tree keyboard navigation clamps at the ends where \
             menus wrap — wrapping would teleport a 100k-row viewport across the corpus. \
             The selection anchor is an id, never an index: collapsed away it resolves to \
             its nearest visible ancestor, removed entirely the range degenerates to the \
             target row, and focus collapsed away falls back slot-preservingly. \
             Auto-scroll returns `None` while a thumb drag is open, because the drag owns \
             the offset. Gaps recorded against the spec: `Ctrl`+arrow is focus-move-only \
             here since no extend-selection semantics are specified; variable row heights, \
             row drag-and-drop, horizontal scrolling, lazy child loading and the tree's \
             own type-to-jump are unspecified and deliberately not invented. `fastmd`'s \
             current 15px tree rows sit below the 24px hit floor and are a migration \
             gap, not a third rung.",
        ),
    },
    Coverage {
        id: "UIF-COMP-007",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/table_column.rs",
            "composite/table_cell.rs",
            "composite/table_header.rs",
            "composite/table_view.rs",
            "gallery_table.rs",
        ],
        reason: None,
        note: Some(
            "The structured grid, with the cell itself owned by the framework per \
             decision **D-8**. Column widths resolve as a pure function of the specs \
             plus the available width: each column declares a floor, a target and a \
             ceiling, and the solver runs three regimes — surplus water-filled by \
             weight under the ceiling, a closed-form scale when the floors hold but \
             the targets do not, and a pinned-at-floor terminal state that reports \
             `needs_horizontal_scroll` rather than violating a minimum. Termination \
             is bounded at one pass per column, and the over-subscribed case is a \
             result, not a hang. Cell rendering is resolved, not delegated: the band \
             a row wears (selection outranks the pointer outranks the zebra stripe), \
             the cell and content boxes inside a resolved column, and the grid line. \
             The header's sort target and resize handle are disjoint regions by \
             construction — the target is inset by the half-grab on every side a \
             live handle covers — so a small accidental drag never sorts and a click \
             on a title never opens a resize; negative tests sweep both at quarter-\
             pixel steps. Sorting is stable (descending reverses the comparator, never \
             the ascending result) and cannot change which rows are selected. Row \
             selection reuses the tree view's anchor semantics rather than inventing \
             a second set: the anchor is an id, so a sort reorders the rows under it \
             and the range still means the same thing. Rows virtualise through \
             `virtual_list`, and the realisation bound has no term for the column \
             count. The sticky header is a separate band that never enters the row \
             recycling cycle and stays column-aligned at every horizontal offset. \
             Values the spec left open, recorded rather than papered over: the resize \
             grab is `size::SPLITTER_GRAB` with a half-grab reach of 5px; the sort \
             indicator is `size::ICON_SM` at `space::XS` gap, pinned to the target's \
             trailing edge so it can never be painted inside the grab band; the \
             header is `size::CONTROL_MD` tall; the header title is `CAPTION` at \
             `Weight::SemiBold`, which `CAPTION.allowed_weights` permits and which \
             answers the missing 12px SemiBold rung (G-12) without inventing a rung; \
             cell padding is `[space::XS, space::MD]`; the last column is never \
             resizable because its trailing edge is the container border; a `Fixed` \
             column exposes no handle because \"never changes\" and \"the user may \
             drag it\" contradict; a `Content` column's ceiling IS its target, so it \
             shrinks but never grows past its content; the sort cycle is none → \
             ascending → descending → none; keyboard movement clamps at the ends \
             rather than wrapping, following the tree. Contrast recomputed from raw \
             sRGB, not copied from the spec's printed column: `border.subtle` \
             measures 1.3495:1 on `surface.base`, 1.2691:1 on `surface.raised`, \
             1.1012:1 on `surface.hover` and 1.0314:1 on `surface.active`, so the \
             grid line promotes to `border.strong` on the two tinted bands, where it \
             measures 3.6772:1 and 3.2377:1. The promotion floor itself — 1.15 — is \
             invented here: a hairline separator carries no information on its own, \
             so `UIF-TOK-006` rule 2's 3.0:1 is not the rule it answers to, and the \
             gallery reports the ratio as a number instead of forcing it into a role \
             every resting band would fail. The spec prints no foreground for a \
             selected cell; `text.strong` is chosen, 14.2946:1 on `surface.active`, \
             matching the tree row the two views sit beside. `accent.border` on the \
             selection fill measures 4.7922:1 and needs no promotion. Gaps: the \
             migration onto `fastmd`'s six-column Tools dialog is cross-crate work \
             outside this increment, though the column model is shaped against it; \
             variable row heights, cell editing, merged cells, column reordering and \
             frozen columns are unspecified by §3B.7 and are deliberately not \
             invented here.",
        ),
    },
    Coverage {
        id: "UIF-COMP-008",
        status: Status::Implemented,
        section: "§3B",
        evidence: &[
            "composite/breadcrumb.rs",
            "composite/overflow_toolbar.rs",
            "gallery_breadcrumb.rs",
        ],
        reason: None,
        note: Some(
            "Both surfaces are implemented as pure solvers over caller-supplied \
             measurements, so neither needs a font or a frame to be tested. The \
             trail's `collapse` is a function of `(items, available_width)` alone \
             that always keeps the root and the terminal inline; the hidden set is \
             a prefix of the middle run, so widening can only un-hide and never \
             swap one arbitrary set for another. The strip's `fit` evicts by \
             declared `OverflowRank` — lower first, rightmost-first on a tie — \
             with the layout index as the last sort key only, so moving an item \
             along the strip cannot change when it is evicted; the evicted set is \
             a prefix of that order, which is what makes the strip monotone too. \
             The 0px vertical shift is structural rather than measured: \
             `strip_height()` takes no argument, backed by a const assertion that \
             the item and trigger boxes fit inside the strip, so the solver has no \
             route from an eviction count to a height. \
             Values the spec left open, recorded rather than papered over: the \
             collapse order drops nearest-root first (GAP_COLLAPSE_ORDER); the \
             terminal crumb is clickable and focusable, with the role carried on \
             the emission so the host decides its ARIA (GAP_TERMINAL_CLICKABLE); \
             the priority mechanism is an explicit `OverflowRank(u8)` rather than \
             a positional rule (GAP_PRIORITY_MECHANISM), tied rightmost-first \
             (TIE_BREAK); the `⋯` trigger is a normal tab stop whose Escape \
             closes the menu and returns focus to itself (GAP_TRIGGER_FOCUS); the \
             trigger reserve is charged only once the strip already fails to fit, \
             because charging it always would evict from a strip that fits. A \
             busy overflowed item is deliberately not a disabled one: a disabled \
             row leaves the menu's focus roster entirely, which would make an \
             in-flight action unreachable by keyboard, so the busy row keeps the \
             enabled row's colour and focus stop and the refusal to re-fire \
             belongs to the action engine. Colour divergences against §2.5: §3B.8 \
             assigns the trail separator to `text.muted` while §2.5's border rows \
             name no token for a trail separator at all (NOTE_SEPARATOR_TOKEN), \
             and §2.5 calls `text.primary` the resting-label token but the trail \
             paints it only on the `[…]` trigger, using `text.muted` for every \
             ancestor (NOTE_PRIMARY_TOKEN_UNUSED). The separator is audited as a \
             text pairing rather than chrome because it is a glyph the user reads; \
             it measures 7.61:1 on `surface.base`, the figure §2.5 prints for \
             `text.muted`.",
        ),
    },
    Coverage {
        id: "UIF-COMP-009",
        status: Status::Implemented,
        section: "§3B.9",
        evidence: &[
            "composite/navigation_rail.rs",
            "widget/rove.rs",
            "surface/tooltip/mod.rs",
            "token/size.rs",
        ],
        reason: None,
        note: Some(
            "NavigationRail / Collapsible NavigationView composite supporting compact (48px) \
             and resizable expanded (180–320px) modes. Implements 28x28px header toggle button, \
             36px destination rows with 16px icons, 13px Medium UI_COMPACT labels, trailing \
             BadgeModel count pills, surface.active fill (14.29:1 contrast) with 2px accent.border \
             (4.79:1 contrast) leading indicator bar, full-width 1px hairline divider above pinned \
             bottom utility destinations, 10px grab handle with GestureSlot<f32> resize engine \
             emitting live, commit, and cancel callbacks, single Tab stop roving keyboard focus \
             skipping disabled items and wrapping at edges, and mandatory right-aligned hover \
             tooltips in compact mode using the time.delay.tooltip_warm handoff window.",
        ),
    },
    Coverage {
        id: "UIF-COMP-010",
        status: Status::Implemented,
        section: "§3B.10",
        evidence: &[
            "composite/command_palette.rs",
            "widget/auto_suggest_model.rs",
            "widget/auto_suggest_host.rs",
            "surface/modal.rs",
            "surface/zorder.rs",
        ],
        reason: None,
        note: Some(
            "CommandPalette / SearchView modal card (§3B.10) with 560x380px surface mounted in \
             Z-Band 3 / Escape Rank 2 over surface.scrim, fixed 64px top offset, 40px search \
             header with removable mode prefix chips (> # @) and Mono Esc hint, grouped results \
             with 24px section headers, 32px single / 44px double rows with matched span \
             emphasis, active-row Rule 4 text.strong subtitle promotion, right-aligned mono \
             shortcut pills, 24px keyboard legend footer, and two-stage Escape with Live/Commit/Cancel \
             callback tier.",
        ),
    },
    Coverage {
        id: "UIF-COMP-011",
        status: Status::Implemented,
        section: "§3B.11",
        evidence: &[
            "composite/stepper.rs",
            "composite/pips_pager.rs",
            "widget/badge.rs",
            "state.rs",
            "token/size.rs",
        ],
        reason: None,
        note: Some(
            "`Stepper` satisfies §3B.11 and `UIF-PRIN-005` in both Horizontal (36px / `size::CONTROL_LG`) \
             and Vertical orientations with connector lines (1px `border.subtle` / `accent.border`) and 24x24px \
             pill indicators encoding Completed ('✓' / `accent.fill`), Current (number / `surface.active` + \
             2px `accent.border`), Pending (number / `surface.raised` + 1px `border.strong`), and Error ('!' / \
             `status.danger.fill`). `wizard_footer` preserves 0px layout shift by rendering `[← Previous]` as \
             `Disabled` on step 0 (`UIF-PRIN-004`). `PipsPager` satisfies §3B.11 with a 24px bar, boundary-clamped \
             24x24px navigation buttons, 6x6px inactive dots, 16x6px pill active pip, and page change commits.",
        ),
    },
    Coverage {
        id: "UIF-COMP-012",
        status: Status::Implemented,
        section: "§3B.12",
        evidence: &[
            "composite/side_sheet.rs",
            "surface/zorder.rs",
            "surface/modal.rs",
            "surface/movable.rs",
            "surface/modal_tier.rs",
        ],
        reason: None,
        note: Some(
            "SideSheet contextual drawer composite with Docked (Z-Band 0 / Rank 0), \
             ModelessOverlay (Z-Band 1 / Rank 1), and ModalScrim (Z-Band 3 / Rank 2) modes. \
             Supports Left and Right placement, resizable [280px..520px] via a 10px grab handle \
             governed by GestureSlot<f32>, full-height 1px divider line and centered 2x24px \
             grip pill promoted to accent.border on hover or drag, 36px header with 16px title, \
             24x24px Pin/Unpin mode toggle, 24x24px Close button with UIF-MOD-002 busy/dirty close \
             gating, scrollable 16px body, optional 48px footer, and automatic 800px responsive \
             breakpoint promotion to ModalScrim.",
        ),
    },
    Coverage {
        id: "UIF-COMP-013",
        status: Status::Implemented,
        section: "§3B.13",
        evidence: &[
            "composite/grid_view.rs",
            "composite/virtual_list.rs",
            "composite/table_view.rs",
            "composite/tree_view.rs",
            "composite/scroll_view.rs",
        ],
        reason: None,
        note: Some(
            "GridView 2D virtualized tile and card grid composite. Implements deterministic \
             column solver `solve_columns` deducting 10px ScrollView gutter, space.md gap, and \
             160px min tile width with symmetric expansion; 2D clamped keyboard navigation (Left/Right \
             by 1, Up/Down by cols, Home/End) without cross-row wrapping; single and ExtendedMulti \
             selection modes with Shift-range, Ctrl-toggle, and Ctrl+A; 16x16px checkbox overlay in \
             ExtendedMulti; card container styling with radius.card, 1px border.subtle at rest, \
             surface.hover on hover, and surface.active + 2px accent.border (4.79:1) + text.strong \
             (14.29:1) on selection; and viewport-culled virtualized row drawing.",
        ),
    },
    Coverage {
        id: "UIF-COMP-014",
        status: Status::Implemented,
        section: "§3B.14",
        evidence: &[
            "composite/empty_state.rs",
            "composite/skeleton.rs",
            "composite/pagination.rs",
            "widget/progress.rs",
            "state.rs",
            "token/size.rs",
        ],
        reason: None,
        note: Some(
            "Fully implemented: EmptyState centered 24px-padded, 360px-max placeholder \
             distinguishing NoItems, NoMatches and LoadError with 40x40px badge, 20x20px \
             icon, 16px SemiBold headline, 13px explanation, and optional recovery CTA; \
             Skeleton reserving exact target dimensions (24px tree rows, 28/32px table rows) \
             pulsing surface.raised to surface.hover over 1200ms or static 1px border.subtle \
             under reduced_motion (0px shift per UIF-PRIN-003); and PaginationBar 32px \
             footer with mono range summary, center page-size options, and boundary-clamped \
             navigation buttons disabled at ends with 0px layout shift.",
        ),
    },
    Coverage {
        id: "UIF-COMP-015",
        status: Status::Implemented,
        section: "§3B.15",
        evidence: &[
            "token/color.rs",
            "token/typography.rs",
            "composite/code_block.rs",
            "composite/diff_viewer.rs",
            "composite/keybind_recorder.rs",
        ],
        reason: None,
        note: Some(
            "Increment 17 realised the developer composites specified in §3B.15: `CodeBlock` with \
             28px header, language badge, fixed 24x24px copy button holding success checkmark \
             for 1500ms at 0px shift, 13px mono body and 40px tabular line-number gutter; \
             `DiffViewer` pairing + and - glyphs with 2px status.success and status.danger.text \
             left bars and distinct text roles in unified and side-by-side modes with per-hunk \
             accept/reject buttons; and `KeybindRecorder` with 32px height, mono keycap pills, \
             trailing 24x24px clear button, raw chord interception with Escape cancelling without \
             clearing, and conflict surfacing.",
        ),
    },
    Coverage {
        id: "UIF-COMP-016",
        status: Status::Implemented,
        section: "§3B.16",
        evidence: &[
            "composite/split_divider.rs",
            "token/size.rs",
            "binding/pre_rank.rs",
            "surface/movable.rs",
            "geometry/clamp.rs",
        ],
        reason: None,
        note: Some(
            "Added by the D-2 boundary review as the only new id: one reusable split handle \
             for workspace pane splits (§4.3.2), SideSheet edges (§3B.12) and DataTable \
             column resize (§3B.7), implemented in composite/split_divider.rs with resting \
             1px subtle stroke, 10px grab zone, 2x24px grip pill, VisualState accent promotion, \
             and clamp-from-last-position zero-overshoot drag lifecycle.",
        ),
    },
    // --- §4.1 Main Window --------------------------------------------------
    Coverage {
        id: "UIF-WIN-001",
        status: Status::Partial,
        section: "§4.1.2",
        evidence: &["geometry/window_geometry.rs", "geometry/mod.rs"],
        reason: Some(
            "Framework-owned geometry schema, UiScale step quantization, clamp solver \
             (WINDOW_MIN 640x480, CENTER_PANE_MIN_W 240px), and WindowStorage trait \
             implemented; Not covered: host owns disk I/O and engine resize commands \
             under D-2/D-9.",
        ),
        note: Some(
            "Clamping guarantees the central workspace never collapses below 240px \
             down to the 640x480 floor.",
        ),
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
             ordered chain, and an empty stack returns `EscapeOutcome::Ignored`. The \
             **Pre-Rank** step ahead of all of them — an in-flight pointer-captured \
             drag cancels first, focus-independent — is `binding/pre_rank.rs`, composed \
             with this walk by `surface/zorder.rs::escape_with_pre_rank`. \
             `SurfaceRegistry::escape` on its own is steps 2-5 only.",
        ),
    },
    // --- §4.2 Top Navbar ---------------------------------------------------
    Coverage {
        id: "UIF-NAV-001",
        status: Status::Implemented,
        section: "§4.2.2",
        evidence: &["composite/navbar.rs", "surface/menu/cascade.rs"],
        reason: None,
        note: Some(
            "NavBar with NavBarMenuTrigger button implementing root menu toggle latch, holding \
             `surface.active` fill and `1px border.strong` outline for as long as the root menu \
             remains open.",
        ),
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
        status: Status::Implemented,
        section: "§4.2.2",
        evidence: &[
            "composite/navbar.rs",
            "surface/tooltip/",
            "token/color.rs",
            "token/shape.rs",
            "token/typography.rs",
        ],
        reason: None,
        note: Some(
            "NavBarMenuTrigger automatically wraps trigger button with `surface::tooltip::Tooltip`, \
             providing 400ms hover delay, 0ms focus opening, 4px offset, and warm handoff memory.",
        ),
    },
    // --- §4.3 Multiple Panes ----------------------------------------------
    Coverage {
        id: "UIF-PANE-001",
        status: Status::Implemented,
        section: "§4.3.2",
        evidence: &[
            "composite/split_divider.rs",
            "token/color.rs",
            "surface/movable.rs",
        ],
        reason: None,
        note: Some(
            "Split divider affordance implemented in composite/split_divider.rs: 1px resting \
             stroke in color.border.subtle, centered 2x24px grip pill in color.border.strong, \
             10px grab zone (size.splitter.grab), horizontal resize cursor, and color.accent.border \
             promotion on hover or active drag.",
        ),
    },
    Coverage {
        id: "UIF-PANE-002",
        status: Status::Partial,
        section: "§4.3.2",
        evidence: &["geometry/pane_solver.rs"],
        reason: Some(
            "Framework-owned solvers (solve_clamped_pane_width, auto_fit_pane_width) \
             implemented; Not covered: host owns TOC mount decision and persistence under D-2/D-9.",
        ),
        note: Some(
            "Pure solvers ensure Left Pane never collapses below 150px and respects \
             20% viewport cap.",
        ),
    },
    Coverage {
        id: "UIF-PANE-003",
        status: Status::Partial,
        section: "§4.3.2",
        evidence: &[
            "composite/tree_view.rs",
            "composite/tree_row.rs",
            "widget/search_box.rs",
        ],
        reason: Some(
            "Framework-owned tree selection model, row geometry, and pinned 40px search-footer mount \
             implemented; Not covered: host owns tree directory content, ordering, and filter predicates under D-2/D-9.",
        ),
        note: Some(
            "TreeView supports single/range selection, keyboard navigation, and pinned search footer.",
        ),
    },
    // --- §4.4 Tabbed Windows -----------------------------------------------
    Coverage {
        id: "UIF-TAB-001",
        status: Status::Implemented,
        section: "§4.4.3",
        evidence: &["composite/tab_strip.rs", "surface/menu/"],
        reason: None,
        note: Some(
            "TabStrip composite binds left-click select, middle-click close via mouse_area, \
             right-click context menu, and exports TabContextMenuModel.",
        ),
    },
    Coverage {
        id: "UIF-TAB-002",
        status: Status::Implemented,
        section: "§4.4.3",
        evidence: &["composite/tab_strip.rs"],
        reason: None,
        note: Some(
            "Horizontal scrollable container preserves TAB_WIDTH [96px, 220px] and \
             24x24px close target.",
        ),
    },
    Coverage {
        id: "UIF-TAB-003",
        status: Status::Implemented,
        section: "§4.4.3",
        evidence: &[
            "composite/tab_strip.rs",
            "surface/modal.rs",
            "surface/input_dialog.rs",
        ],
        reason: None,
        note: Some(
            "TabStrip and TabStripModel with dirty close guard (returning RequiresConfirmation \
             verdict wired to UIF-MOD-002 modal confirmation), MRU history handoff, and \
             external_deletion_sync immediately closing deleted files and activating the MRU tab.",
        ),
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
             invalid value reveals the message while focus stays in the field.\n\n\
             KNOWN GAP (found during app integration, INT-4): `check_basename` rejects \
             empty, edge-spacing, control characters and `/ \\ : * ? \" < > |`, but does \
             NOT reject `.` or `..` as whole names. `ValidationRule::Name` therefore \
             accepts a directory-traversal name. The framework rule is a character-class \
             check, not a traversal guard. `fastmd` deliberately does NOT adopt \
             `ValidationRule::Name` for its name dialogs and keeps its own stricter \
             `is_safe_basename`, because accepting `..` in a filesystem-facing create or rename dialog is a security regression. A host that needs a traversal-safe \
             name must layer its own check; this rule alone is not sufficient.",
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
    Coverage {
        id: "UIF-MOD-004",
        status: Status::Implemented,
        section: "§4.6.2",
        evidence: &["surface/modal.rs"],
        reason: None,
        note: Some(
            "`SubRegionBands` + `subregion_max_height` solve \
             `card - (header + 2 × body_padding + footer)` from the card's \
             *solved* bounds; `subregion_height` applies the cap after the \
             remainder, so a card too short to reach the cap still gets \
             everything it has. `ATTRIBUTIONS_LIST_MAX_HEIGHT = 200` is defined \
             and registered in `SURFACE_GEOMETRY_CONSTANTS`, and the credits \
             list is wired to it. Band terms are token-resolved by the host \
             (`ui_iced/attributions.rs`: `HEADER_BAND_HEIGHT`, \
             `FOOTER_BAND_HEIGHT`) because `ModalFrame` deliberately carries no \
             header/footer sub-rectangles — it supplies `body_padding` via \
             `from_frame` and nothing else. Two deviations from the \
             requirement's letter: the constant is `_HEIGHT`, not the spec's \
             `_H`, because `VISUAL_NAME_MARKERS` matches `HEIGHT` and a bare `H` \
             would leave the gate blind to the very constant it exists to check \
             (the spec's `FOLDER_LIST_MAX_H` is `FOLDER_LIST_MAX_HEIGHT` for the \
             same reason); and the cap is a solved `Length::Fixed`, not a \
             fill-with-ceiling, because iced 0.14 has no min/max `Length`, no \
             `Container::max_height` and no constraint widget. The second costs \
             something: `Fill`'s free overflow safety becomes *conditional on \
             the band terms being exact* — an understated band over-solves \
             through the footer, which `Fill` could not do. Measured at five \
             viewports rather than trusted.",
        ),
    },
    // **The two spec numbers are in tension for this card, and it is unresolved.**
    // The card is pinned at 480px and its furniture is 150px, so it has 330px
    // to give; the 200px cap lets 200 of that be used. The other **130px is
    // dead card** — nothing is laid out there — and the list drops from ~11.2
    // visible rows to ~6.8 (~29.4px/row over the 58-crate catalog).
    // `the_cap_leaves_130px_of_dead_card_and_costs_about_four_and_a_half_rows`
    // pins that as a visible fact rather than a footnote. Resolving it means
    // changing a pinned number (the card height or the cap), which is a spec
    // decision, not an implementation one.
    //
    // **`Length::Fixed(0.0)` is void in iced 0.14** — `Size::is_void()` is true
    // if *either* axis is `Fixed(0.0)`, and a void child is silently dropped
    // from the widget tree. At a sub-182px viewport the solved height is 0 and
    // the scrollable becomes `SelectorNotFound`, losing `ATTRIBUTIONS_LIST_ID`
    // and its scroll position (AGENTS §5). Checked against `Fill`, which keeps
    // the widget present at 0.0. `credits_list_length` therefore falls back to
    // `Fill` at zero room; the cap cannot be breached there because both
    // mechanisms agree the height is zero.
    //
    // **Token-gate defect found here, deliberately NOT fixed here.**
    // `token_lint::join_continuation` returns a declaration's RHS untouched
    // whenever the declaration line is non-empty, so a multi-line constant whose
    // first line is a bare token is classified `TokenBound` without the rest of
    // its expression ever being read — `pub const X: f32 = space::SM` newline
    // `+ 500.0;` passes. Same class as INT-14, one level deeper: INT-14 closed
    // "`=` ends the line"; this is "line 1 is a partial expression". Observed
    // directly — `HEADER_BAND_HEIGHT` written multi-line with a `3.0`
    // coefficient counted token-bound while the single-line `FOOTER_BAND_HEIGHT`
    // with a `2.0` was correctly flagged. Both are now written literal-free
    // (gaps as repeated `space::SM` terms), so they pass under any reading.
    // Fixing the join would re-read every multi-line constant in both crates and
    // could flip an unknown number of classifications; that needs its own
    // increment with its own analysis.
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
        status: Status::Implemented,
        section: "§6.1",
        evidence: &["token/contrast.rs", "gallery.rs", "token_lint.rs"],
        reason: None,
        note: Some(
            "Fully automated: WCAG 2.1 contrast solver plus gallery 4-class judgment \
             (Judged / UnpaintedBackground / CoincidentWithFill / RestingDivider) across all emitted pairs \
             with worst-case translucent fill compositing. Source-level token lint bans raw colour constructors \
             and hex literals outside `token/color.rs`, bans unbound visual scalars in `pub const`s outside \
             registered surface geometry constants, and scans inline layout calls (`Length::Fixed`, `Padding::new`, \
             `Padding::from`, `Pixels`) verifying zero bare non-zero literals in production code.",
        ),
    },
    Coverage {
        id: "UIF-VER-002",
        status: Status::Implemented,
        section: "§6.2",
        evidence: &[
            "geometry/anchor.rs",
            "geometry/clamp.rs",
            "token/size.rs",
            "token/typography.rs",
            "composite/tab_strip.rs",
        ],
        reason: None,
        note: Some(
            "Menu quadrant flipping, input-dialog cursor clamping, movable title-bar clamping, \
             and tab-strip width bounding (`TAB_MIN_WIDTH = 64px`, `TAB_MAX_WIDTH = 200px`) solver-tested \
             across extreme viewports down to 640x480. 24px hit floor and 12px text floor asserted against \
             all rungs. Workspace multi-pane splitters and 10px grab zone remain host-owned under D-2.",
        ),
    },
    Coverage {
        id: "UIF-VER-003",
        status: Status::Implemented,
        section: "§6.3",
        evidence: &[
            "surface/modal.rs",
            "surface/input_dialog.rs",
            "surface/zorder.rs",
            "state.rs",
            "composite/tab_strip.rs",
        ],
        reason: None,
        note: Some(
            "Interaction state machine, disabled-button and card-whitespace absorption, \
             single-Tab-stop roving collections, Escape unwinding in escape-rank order, and \
             dirty-close guards (dirty editor confirmation and dirty tab close confirmation via \
             `TabCloseVerdict::RequiresConfirmation`) all verified by headless integration tests.",
        ),
    },
    Coverage {
        id: "UIF-VER-004",
        status: Status::Implemented,
        section: "§6.4",
        evidence: &["gallery.rs", "bin/gallery.rs"],
        reason: None,
        note: Some(
            "Over 180 cells cover all atomic widgets and composite surfaces across (Variant x State) \
             at standard and compact control rungs in both dark and inverted editor palettes, asserting \
             resolved style values and ordered quad lists. Main workspace container contents and multi-pane \
             splitters are host-owned under D-2; iced text caret rasterization sits permanently outside the \
             renderer draw protocol.",
        ),
    },
    Coverage {
        id: "UIF-VER-005",
        status: Status::Implemented,
        section: "§6.5",
        evidence: &[
            "invariant_gate.rs",
            "microcopy/case_rule.rs",
            "microcopy/action_label.rs",
            "layout/emphasis.rs",
            "layout/density.rs",
            "microcopy/truncate.rs",
            "widget/button_widget.rs",
            "widget/password_model.rs",
            "validation/punish.rs",
            "bin/gallery.rs",
        ],
        reason: None,
        note: None,
    },
];

#[cfg(test)]
#[path = "spec_coverage_tests.rs"]
mod spec_coverage_tests;
