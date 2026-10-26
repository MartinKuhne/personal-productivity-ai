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
/// The spec carries 91 numbered `UIF-*` ids.
///
/// It carried 90 until commit `edbe04a` added `UIF-MOD-004` (scrollable
/// sub-region height solving) alongside the existing three modal contracts.
pub const EXPECTED_SPEC_ID_COUNT: usize = 91;

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
    Coverage {
        id: "UIF-PRIN-006",
        status: Status::Partial,
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
        ],
        reason: Some(
            "Three clauses are now modelled and checkable; the fourth is \
             mechanism-only. **Row density** holds as a rule: \
             `ControlDensity::{Compact, Standard}` is derived from \
             `size.control.sm` / `size.control.md` rather than restated, \
             and `check_row_density` reports a band that mixes rungs, \
             naming the control, its height and both rungs — with \
             `off_rung` distinguished from `wrong_rung` because the two \
             need different fixes. `OverflowToolbar` (28px strip → \
             Compact) and `FormField` (32px slot → Standard) are wired to \
             it, as is a dialog footer via `FooterPlan::density_report`. \
             **Single focal point** holds: `check_focal_point` counts \
             over the four scopes §1.1 names, and `Primary` and `Danger` \
             both map to `High`, so a confirm-and-delete pair is two \
             focal points rather than one-each. **Footer ordering** holds \
             as a type: `FooterSlot::{Utility, Cancel, Confirm}` carries \
             the layout order, the far-left pin and the `space.sm` group \
             gap, and `FooterPlan::initial_focus` carries the \
             destructive-confirmation rule as data `validate()` asserts \
             against the `Cancel` action. Missing: nothing in this crate \
             *calls* any of the three — they are pure rules a host runs \
             against a plan before rendering it, so an unruly host stays \
             unruly until it opts in, and no framework composite \
             self-checks on the way out. The fourth clause is only half \
             built: `optical_box` and `optical_shift` separate a glyph's \
             optical box from its em box and are pinned for identity, \
             symmetric application and the slot never being exceeded, \
             but the per-glyph insets are host-supplied because this \
             crate has no font and therefore no metrics, and the \
             *baseline* half of the clause — adjacent inline text and \
             controls sharing a typographic baseline — is not modelled \
             at all.",
        ),
        note: Some(
            "No value invented. `ControlDensity::height()` reads \
             `size.control.sm` / `size.control.md`; \
             `footer::GROUP_GAP` *is* `space.sm`; the optical slots are \
             `size.icon.md` (16px) and `size.hit_target.min` (24px). \
             `OverflowToolbar::ITEM_HEIGHT` (24px) is a hit-target floor \
             rather than a control density, so a Compact strip carrying \
             24px hit targets inside it is not a rung mixture and is not \
             reported as one. `Emphasis::from(DialogButtonRole)` reads \
             intent, not paint: a host that fills a `Secondary`-role \
             control with `accent.fill` is invisible to the role type and \
             must classify through `EmphasisSlot::high`. `Danger` is \
             classed `High` by its peak fill — `danger_paint` rests on \
             `surface_raised` and only reaches `status_danger.fill` on \
             hover — because §1.1 names the variant, not the rest cell. \
             Optical insets are host-supplied by design — the framework \
             owns the shape of optical centring, never the measurements.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-007",
        status: Status::Partial,
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
        ],
        reason: Some(
            "Five of the pillar's clauses are now machine-checkable; the sixth is blocked below \
             the framework. Closed: the sentence-case mandate classifies a label as sentence, \
             all-caps, title case, mid-word capital, first-word-lowercase or uncasable \
             (`microcopy/case_rule.rs`), with acronyms licensed by a named data set rather \
             than a length heuristic, and the closed Host `SPEC.md` Literal Pin Table \
             (`microcopy/pins.rs`) consulted before anything is called a violation — nine \
             literals, exact-match, each carrying its own exemptions, so `'Copy Path'` is \
             exempt and `'Copy path'` is not. Verb-first CTAs are checked by `check_verb_first`, \
             which flags generic `ok`/`okay`/`yes`/`no` and exempts `'Ok'` by pin while leaving \
             `'Cancel'` compliant on its own merits. The ASCII/Unicode ellipsis is a tested \
             biconditional: `ActionLabel::behaviour` carries whether activation opens a further \
             prompt, so all four quadrants are pinned, a spaced `...` is its own finding, \
             `'Create Directory ...'` is exempt per `UI-015`, and `…` (U+2026) on an action \
             label is reported as the wrong *character* because that code point is reserved for \
             truncation, placeholders and `[…]` elision. End versus mandatory middle truncation \
             is implemented in `microcopy/truncate.rs`, producing the spec's own \
             `src/app/…/mod.rs` shape with the basename and extension surviving, and \
             `check_truncation_tooltip` makes the truncation-implies-tooltip pairing a violation \
             when the tooltip is absent or is not the full string. `TabularNums` lets a counter, \
             percentage, timestamp, range or numeric column assert the fixed-width requirement, \
             and `check_numeric_readout` enforces right-aligning both header and cell. \
             Missing: an enforcement point. Every rule here is a pure function a host runs \
             against a plan before rendering it; nothing in this crate calls them, so an unruly \
             host stays unruly until it opts in. Missing: the tabular guarantee, and the reason \
             recorded above for it was wrong. `UIF-TOK-004` *does* model a font family — \
             `Family::Mono` is a declared `font.family.mono` token that resolves to an \
             `iced::Font`. The real boundary is that `iced::Font` carries four fields (`family`, \
             `weight`, `stretch`, `style`) and no OpenType feature selector, so the spec's \
             *second* mechanism, `font-variant-numeric: tabular-nums`, cannot be requested \
             through this framework at all; only the mono-family route exists, and it resolves \
             to iced's generic `Monospace` answered from the host's font fallback, so a \
             fixed-pitch face is not guaranteed. Missing: pixel-aware truncation. With no font \
             loaded where these rules run, the budget is a count of Unicode scalar values, so \
             the framework cannot decide *when* a label overflows — the host supplies the \
             budget, which makes the tooltip pairing a checkable contract rather than the \
             runtime registration the pillar's wording implies. Missing: open-ended proper-noun \
             knowledge. Licensing is a closed list, so a feature renamed to a capitalised word \
             the list has never seen reads as a violation until the host adds it.",
         ),
        note: Some(
            "Invented values, all recorded here. (1) `DEFAULT_LICENSED` — §1.1 names four \
             licensed words (`Markdown`, `PDF`, `MCP`, `JSON`, kept separately as \
             `SPEC_NAMED_LICENSED`); the other seventeen are framework-supplied for chrome \
             this product actually contains and are not a spec transcription. (2) \
             `GENERIC_CTA_LABELS` — the spec names `OK` and `Yes`/`No`; `okay` was added as \
             the same word spelled another way, and `Cancel`/`Done`/`Submit` were deliberately \
             *not* added because widening that set would invent a style rule §1.1 did not \
             write. (3) `SEGMENT_SEPARATORS` = `/` and `\\` — the spec does not enumerate \
             separators. (4) The `PinExemption` partition follows the two spec blocks rather \
             than a blanket pardon: §4.5 exempts `'Ok'`/`'Cancel'` from verb-first only, \
             §4.8/§4.9 exempt the seven chrome rows from sentence-case and ellipsis. (5) Pin \
             citations record `UI-009` and `UI-015` where the spec names them and the spec's \
             own range `UI-009..UI-034, UI-062` elsewhere, rather than a guessed per-literal \
             id. (6) `ReadoutKind::requires_tabular()` returns true for every kind, derived \
             from §1.1's list being entirely live or aligned numerics. (7) Two \
             anti-false-positive rules the spec does not state: a label whose first character \
             is not alphabetic is never first-word-lowercase, and single-letter words are \
             excluded from both the all-caps and title-case tallies, because counting a bare \
             `A` or `I` would report correct copy as a violation. Truncation budgets are \
             Unicode scalar values, not pixels; no contrast figure was taken from a printed \
             number.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-008",
        status: Status::Partial,
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
        ],
        reason: Some(
            "Three of the pillar's four clauses are now machine-checkable; the fourth \
             cannot close because six of the eleven collections it names are unbuilt \
             Phase C. The roving walk is real and genuinely shared: \
             `widget/rove.rs::rove_index` is the single \
             skip-disabled, wrapping walk that `RadioGroupModel::rove` and \
             `SegmentedModel::rove` both run, and `focus.rs` carries the traversal policy, \
             the roving ordinals and the modal trap on the ARIA roving-tabindex convention. \
             The wrap-versus-clamp split goes the right way: bounded groups wrap, while \
             `composite/table_view.rs` and `composite/tree_view.rs` clamp at the ends \
             rather than wrapping, which is what stops a held `Down` teleporting a long \
             virtual viewport back to row zero. Disjoint hit zones are proven wherever this \
             crate paints them — `table_header.rs` defines the sort target as the header \
             cell *minus* the 10px resize band and asserts sort targets and handles \
             pairwise disjoint across the whole row; `tree_row.rs` reserves a 24x24px \
             disclosure column separate from the row body; `expander.rs` derives its title \
             bounds by subtracting the trailing slot; `chip.rs` reserves a disjoint \
             24x24px trailing cross; `breadcrumb.rs` keeps its trigger rectangles disjoint \
             from every segment. Now closed, clause by clause. The `A11yContract` exists \
             as a pure nine-field value type over a closed `A11yRole` enum that carries \
             its real ARIA name (`a11y/contract.rs`), and `a11y/project.rs` projects it \
             from button, checkbox, radio and radio group, segmented control and segment, \
             all six menu row kinds, tree rows, chips, text input, progress and the \
             expander disclosure header — every field `Some` only where the source \
             genuinely carries it, pinned per source by `filled_count`, with a source \
             scan asserting neither file reads geometry or a clock. The single-`Tab`-stop \
             property is a declared fact rather than an emergent accident: \
             `a11y/tab_stop.rs::CompositeBoundary` states its members, live members, \
             active member and edge rule, and answers `tab_stops()` (one while any member \
             is live, zero when none), `entry_member()` (the active member, never member \
             zero, degrading to the first *enabled* member) and `next_within()`; over a \
             roster of three composites holding 100,006 members `total_tab_stops` returns \
             three. The bounded-wrap / spatial-clamp rule is pinned against the shipping \
             code and not merely declared: `rove_index` wraps, `TreeView::move_focus` and \
             `TableView::move_row` clamp, and `check_composite` reports a category/edge \
             contradiction either way round. The 150ms diagonal aiming corridor is a \
             value-driven state machine (`surface/menu/aim.rs`) with no clock read at all \
             — `grace_ms()` reads `DELAY_SUBMENU` from `token::time`, and \
             `MenuCascade::hover_submenu_row` drives it so a sibling crossed inside the \
             window is suppressed, the open branch's own trigger row preserves without \
             flicker, entering the child card preserves, and re-entry restarts the full \
             window. Missing: six of the eleven collections the pillar names (`TabStrip`, \
             `GridView`, `CalendarView`, `NavigationRail`, `Stepper` and \
             `CommandPalette`) are unbuilt Phase C, so their single-stop rosters cannot \
             exist; and none of the three closed clauses has an automatic enforcement \
             point — every check here is opt-in, so a composite that never calls a \
             `project_*` function or declares a `CompositeBoundary` still renders \
             perfectly, and the guarantee holds only where a host or a framework \
             composite invokes it on the way out.",
        ),
        note: Some(
            "Invented values, all recorded here. (1) `A11yRole`'s fourteen-member set is \
             this crate's own closed vocabulary: §1.1 names the `role` field but never its \
             domain. The ARIA strings themselves are real; the assignments are modelling — \
             a single-select `SegmentedButton` projects `radiogroup` and a multi-select \
             one `group`, a `Chip` and an expander header project `button`. (2) \
             `A11yValue::Fraction` is normalised to 0..1 rather than a percentage so the \
             projection makes no display decision. (3) `Checked` is the framework's own \
             tri-state mirroring `CheckState`, not a spec type. (4) `CompositeCategory` \
             (bounded group / spatial collection) and `EdgeBehaviour` are this framework's \
             taxonomy for §1.1's second clause; the spec describes the split but names no \
             types for it. (5) The corridor's grace window is anchored on pointer arrival \
             at the sibling rather than on branch-open time — §4.9 says the switch happens \
             \"after the 150ms diagonal aiming corridor check\" without fixing which \
             instant the 150ms is measured from. (6) Reduced motion does not shorten the \
             corridor: it is an input tolerance, not a transition, and collapsing it to \
             `0ms` would make menus harder to use for exactly the users that setting \
             protects. (7) `AimDecision::Collapse` is reported and never applied, so \
             dismissal keeps a single owner in the cascade. (8) \
             `MenuCascade::hover_submenu_row` takes the corridor as a parameter instead \
             of storing it as a field, so cascade equality keeps meaning \"what is open\" \
             and the pre-existing cascade equality assertions stay meaningful.",
        ),
    },
    Coverage {
        id: "UIF-PRIN-009",
        status: Status::Partial,
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
        reason: Some(
            "Two of the three clauses now hold as framework state plus one pure decision; \
             the third holds in-model only. 'Punish Late' is implemented and enforced: \
             `validation::stance::EditStance` gives a field a real history — `Pristine` until \
             `note_commit`, `note_blur` or `note_submit`, and provably NOT advanced by a \
             programmatic `value` write, a `with_validation` call or a `with_selection` request \
             — carried by `TextInputModel`, `TextAreaModel` and `PasswordInputModel`. \
             `validation::punish::punish(stance, invalid, disallowed)` is the single authority \
             called by the renderer: the runtime enforcement point in `widget/text_input.rs` \
             (`controlled`, `visual`, and `build_editor`) and `widget/text_input_model.rs` \
             (`slot_for`) is now wired directly to `model.get().punishment().painted()`, \
             fulfilling both Punish Late and Reward Early in the actual text input widget. \
             Pristine + invalid withholds error painting and warning glyphs while still reporting \
             `invalid() == true` to keep submits blocked; touched + invalid paints; and a \
             disallowed character (`with_disallowed(DisallowedClass)`) paints immediately \
             regardless of stance. The carve-out is modelled as \
             `DisallowedClass::FilenameBasename` over the eight `FILENAME_ILLEGAL_CHARS` plus the \
             Unicode `Cc` set — the same rule `surface/input_dialog.rs`'s basename validator \
             already applied, whose literal now aliases the canonical constant instead of copying \
             it. 'Reward Early' holds: `painted` is a pure function of this frame's inputs with \
             no latch, no timer and no decay to clear, so the frame the value becomes valid is \
             the frame the paint stops and Rest state is restored. 'Non-Destructive Recovery' is \
             widened but not finished: `validation::recovery::EditSnapshot` carries buffer + \
             selection + scroll through the existing `binding::snapshot` linear guard, and \
             `restore_text_input` / `restore_text_area` write all three back, so a rejected \
             operation no longer loses the selection or the viewport — but the restore is \
             model-only. Missing: caret restore into iced's editor state. \
             `TextInputModel::selection` is documented as a request the editor may \
             or may not honour, and nothing in this crate pushes it into iced's editor state, \
             so the caret is NOT put back on screen; `TextAreaModel`'s scroll offset is the \
             one field a renderer actually reads back off the model.",
        ),
        note: Some(
            "Invented values and judgements, all recorded here. (1) `EditStance`'s two \
             names: §1.1 says `pristine` and names the three events; `Touched` as the \
             complement is framework vocabulary. (2) `PunishReason`'s four arms are an \
             invented taxonomy, added so 'invalid but withheld' and 'valid' stay \
             distinguishable instead of both collapsing to `painted == false`. (3) \
             `Punishment::invalid` is computed as `invalid || disallowed` — the judgement \
             that a refused character is itself an invalidity before the host's validator \
             has run, so the visual and the submit gate cannot come apart. Not stated in \
             the spec. (4) `Cancelled` does NOT advance the stance: the spec lists commit, \
             blur and submit only, and treating `Escape` as a commitment would let a \
             restored value go red the moment the user tried to back out of an edit. (5) \
             `Cleared` DOES advance, on the reading that the trailing `×` is a commit-tier \
             act. (6) `EditSnapshot` carries a fourth field, `was_dirty`, that the pillar \
             does not name: writing a buffer marks a well dirty, so a snapshot that did \
             not remember the pre-edit dirtiness would turn a clean field dirty by \
             reverting it — changing a fact the clause says stays intact. (7) \
             `ScrollOffset` is a newtype that floors negative input at zero. (8) The \
             disallowed set is the crate's pre-existing basename rule reused, not a \
             transcription: §1.1 names only `/` and `\0`, and `UIF-INP-003` says 'valid \
             filesystem basename characters' without enumerating them, so the eight \
             `FILENAME_ILLEGAL_CHARS` plus all of Unicode `Cc` come from `check_basename`. \
             Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, \
             `LPT1`–`LPT9`) are deliberately NOT included: the carve-out is about \
             characters, and a reserved name is legal one character at a time, which puts \
             it on the withheld side of the line rather than the instant-punish side. (9) \
             `EditSnapshot::from_text_input` reports `scroll: None` rather than \
             `Some(ZERO)`, because a single-line model carries no scroll position and \
             claiming one would restore something that was never there. (10) \
             `with_stance` on each model is an escape hatch for a form re-displayed after \
             a failed submit; ordinary construction never needs it. (11) \
             `PIN_FILES_SCANNED` moved 137 → 142 for the five new `validation/` files; \
             `PIN_RULE_B_FILES` did not move, because `validation/` sits outside the \
             visual scope and defines no visual literal.",
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
        status: Status::Partial,
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
        ],
        reason: Some(
             "The token set exists and is complete, and the injection discipline \
              that was already proven still holds. `token/time.rs` declares all \
              ten §2.6 rungs as `DurationToken { name, ms, easing, alias }` — \
              `INSTANT` 0, `FAST` 100, `NORMAL` 180, `SLOW` 240, \
              `DEBOUNCE_LIVE` 200, `DELAY_SUBMENU` 150, `DELAY_TOOLTIP_OPEN` \
              400, `DELAY_TOOLTIP_WARM` 100, `HOLD_COPIED` 1500 and \
              `TOAST_AUTO_DISMISS` 5000 — each carrying its §2.6 curve as a \
              pure `Easing` (`Linear`, `EaseOutCubic`, `EaseInOutCubic`) whose \
              `apply` is defined on the enum rather than left as a name. \
              `resolve_ms(intended, reduced_motion)` is the single shared \
              fallback the whole crate can now route through: it substitutes \
              `time.instant` for a *transition* under injected \
              `reduced_motion` and leaves a *timer* alone, because \
              `easing: None` marks a countdown and zeroing a debounce or a \
              tooltip delay would produce more motion, not less. Nothing under \
              `token/` reads a clock — time enters only as a `TimeMs` value, \
              and the module's sidecar asserts that by scanning its own source \
              for any clock call or import. The four pre-existing per-widget \
              constants now read through their rungs (`HOVER_DELAY` → \
              `DELAY_TOOLTIP_OPEN`, `KEYBOARD_FOCUS_DELAY` → `INSTANT`, \
              `DEBOUNCE_MS` → `DEBOUNCE_LIVE`, `DEFAULT_AUTO_DISMISS_MS` → \
              `TOAST_AUTO_DISMISS`), so each literal lives only under `token/` \
              while call sites keep their domain vocabulary. \
              `time.delay.tooltip_warm` is now consumed as a *behaviour*, not \
              merely as a number. `surface/tooltip/warm.rs` adds `WarmSession`, \
              the cross-control memory `HoverController` deliberately does not \
              have: the host calls `note_visible` on every frame a tooltip \
              actually paints, and `warmth_for(arrival)` answers cold or warm \
              against `handoff_window_ms()`, which reads the rung. A cold \
              arrival pays the full 400 ms; an arrival inside the 100 ms \
              window pays `0 ms` and paints on the arrival frame itself; past \
              the window the session cools by itself, with no timer and no \
              event to cancel, and the next arrival pays 400 again. \
              `HoverController::pointer_enter_warm` carries that verdict into \
              the existing machine without changing what a plain arrival \
              costs, and the gallery's three `tooltip/warm-*` cells drive one \
              real session and two real controllers so the difference is \
              visible rather than asserted. Missing: consumption for the four \
              rungs that still have no caller. `time.fast`, `time.normal`, \
              `time.slow` and `time.hold.copied` wait on the Phase-C widgets \
              that would animate them — `ToggleSwitch` thumb travel, the \
              `ComboBox`/`DatePicker`/`TimePicker`/`ColorPicker` flyouts, the \
              modal scrim and `NavigationRail` width, the `CodeBlock` \
              `[Copy]` tick — and `eased_progress` is called by nothing, \
              because no widget here yet interpolates a value across a \
              duration. The unconsumed count is four rather than five; the \
              status stays `Partial` because those four are most of the \
              transition scale and none of them is reachable from this crate \
              today.",
        ),
        note: Some(
             "All four constants that predate this change were already \
              numerically correct against §2.6 — 400, 0, 200 and 5000 — so \
              routing them through their rungs changed nothing on screen. \
              §2.6's two aliases (`time.menu.aim_grace`, \
              `time.tooltip.hover_delay`) are modelled as data on their rung \
              rather than as extra constants, so one value cannot drift into \
              two definitions. The warmth-decay interval is **not** an \
              invented value: §2.6's own row makes the 100 ms window and the \
              lapse the same thing — `within 100ms` is the whole rule — so \
              `handoff_window_ms()` reads `DELAY_TOOLTIP_WARM` and no second \
              number was needed. The warm wait is likewise the spec's own \
              `0ms wait`, read through `time.instant`, the same rung the \
              keyboard channel already uses. What this increment did decide, \
              and what is recorded here as framework policy rather than as \
              spec, is seven things: (1) warmth is granted by *visible \
              frames*, not by arrival, so a long hover keeps a session warm \
              and a cold, still-pending hover grants none; (2) the session \
              instance **is** the adjacency scope — a host gives one \
              `WarmSession` per group of tooltip-bearing controls, because \
              this layer cannot see which controls touch, and inventing a \
              geometry model here would be a worse guess than an explicit \
              host choice; (3) any visible tooltip warms the session, \
              including one shown by keyboard focus, because warmth records \
              that a label was on screen rather than which channel put it \
              there; (4) dismissal is not a session event — the session \
              simply stops being refreshed and decays from the last frame \
              that was actually visible; (5) the window boundary is \
              inclusive, because the spec says `within`; (6) `note_visible` \
              only moves the record forward, so out-of-order frames cannot \
              rewind warmth; (7) a cold verdict defers to the controller's \
              own configured delay rather than to the token, so the handoff \
              can only ever *remove* a wait and never override one a host \
              raised for accessibility. The row was read as written: 100 ms \
              is the window and the warm wait is 0 ms. A `warm arrival waits \
              100 ms` reading would need a second, unspec'd duration for the \
              window and would leave the row's explicit `0ms wait` \
              unimplemented.",
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
             widget. **Ratified 2026-10-24:** the crate's sum-type emission \
             representation — `enum XxxEmission { Changed(Live<T>), Committed(Commit<T>), \
             Cancelled(Cancel<T>) }`, where the *variant identity* carries the tier — is \
             the authoritative form. The spec's `on_change(Live<T>)` notation describes \
             tier semantics, not a mandated Rust wrapper around a bare payload, so the \
             twenty emission variants an earlier audit flagged as omitting a `Live<>` / \
             `Commit<>` / `Cancel<>` wrapper are a ratified decision, not gaps.",
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
             (`Escape`) were added; and §3.2e now prints the closed form the \
             solver already used — `clamp(line_count, min_rows, max_rows) × \
             line_height + 2 × space.sm`, three lines of `type.body` = 76px — so \
             the 72px-vs-76px mismatch this entry used to record is gone, and the \
             solver and the section read the same arithmetic. What the section still \
             does not reconcile is the figure beside it: the Bottom Command Bar's \
             `44px..120px` is off the body grid by the same arithmetic \
             (44 − 16 = 28 → 1.4 lines; 120 − 16 = 104 → 5.2), and that stays \
             recorded against §4 rather than quietly adjusted here.",
        ),
        note: Some(
            "`FontFamily::Mono` (`UIF-TOK-004`) is now wired to `TextAreaModel` \
             via `.with_font_family()` and the `.mono()` builder method.",
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
    Coverage {
        id: "UIF-ATOM-010",
        status: Status::Partial,
        section: "§3.10",
        evidence: &["surface/popover/mod.rs", "surface/zorder.rs", "widget/text_input.rs"],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this widget to Phase C on 2026-10-23. No `DatePickerModel`, \
             `DateSelection`, `CivilDate`, `YearMonth`, `CalendarViewMode` or `Weekday` \
             type exists in this crate, no segmented trigger, no calendar flyout, and no \
             calendar arithmetic of any kind. Missing: the whole requirement — the 32px \
             segmented `YYYY-MM-DD` trigger with per-segment focus, digit auto-advance and \
             in-place `Up`/`Down` increment that does not open the flyout; the reserved \
             24x24px trailing calendar button on the `002b` zero-shift slot; the 256px \
             fixed 6x7 day grid that holds its height exactly across 28- and 31-day \
             months; the today / selected / in-range / blackout cueing with its diagonal \
             strikethrough on out-of-bounds days; the `Days -> Months -> Years` \
             drill-down; the two-click range selection with hover preview; the full 2D \
             keyboard map; and the `Live` / `Commit` / `Cancel` callback set. The card it \
             would live in is the cheap part and is already proven: `surface::popover` and \
             the menu engine supply a Z-Band 2 / Escape Rank 3 card with anchored \
             placement, dismissal and a focus roster, and `surface/zorder.rs` already \
             registers that band. The date model is the expensive part and is genuinely \
             unbuilt: this crate carries no calendar dependency and no civil-date type, so \
             month lengths, leap years and day-of-week derivation are new work rather than \
             a wrapper over something existing.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-011",
        status: Status::Partial,
        section: "§3.11",
        evidence: &["widget/text_input.rs", "surface/popover/mod.rs", "widget/rove.rs"],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this widget to Phase C on 2026-10-23. No `TimePickerModel`, \
             `CivilTime`, segmented trigger, drum flyout or time arithmetic exists. \
             Missing: the whole requirement — the `HH : MM [: SS] [AM/PM]` monospace \
             segmented trigger with per-segment arrow spinning and its trailing 24x24px \
             clock button; the 200-240px drum card of parallel scrollable 32px-row \
             columns stepped by `minute_increment` and separated by `border.subtle` \
             hairlines; the central 32px selection band; the pinned 36px \
             `[Cancel] / [check OK]` footer that keeps the focus trap non-empty; the \
             12-versus-24-hour and show-seconds modes; and the `Live` / `Commit` / \
             `Cancel` callbacks over `CivilTime`, including the pre-open snapshot the \
             `Escape` tier restores. Reusable pieces are real and would carry a builder a \
             long way: `widget/text_input.rs` supplies the segmented cell and the \
             zero-shift trailing-slot partition, `surface::popover` supplies the banded \
             card with its escape rank, and `widget/rove.rs` supplies the skip-disabled \
             walk the column focus would use. None of it is assembled into a time \
             control. The `time.normal` (180ms) open transition the section assumes is a rung \
             (`token::time::NORMAL`, `EaseOutCubic`), so nothing on the token side blocks \
             a builder; what is missing is a widget that interpolates a value across a \
             duration.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-012",
        status: Status::Partial,
        section: "§3.12",
        evidence: &["token/color.rs", "token/contrast.rs", "widget/slider.rs"],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this widget to Phase C on 2026-10-23. No `ColorPickerModel`, \
             `Rgba8`, `Hsva`, `ColorSwatchButton`, saturation/value canvas, hue or alpha \
             slider, hex input or preset swatch grid exists. Missing: the whole \
             requirement — the 16x16px preview well over the 4x4px neutral checkerboard \
             with its mandatory `1px border.strong` inner frame that keeps a near-black \
             swatch visually bounded; the 180x140px SV canvas with its dual-ring \
             crosshair thumb; the 4px hue and optional alpha sliders; the validated \
             `#RRGGBB[AA]` text input; the 24x24px preset theme grid; and the `Live` / \
             `Commit` / `Cancel` callbacks over `Rgba8`. Two pieces the section leans on \
             are already written and are load-bearing rather than incidental: \
             `token/color.rs` is the closed semantic colour set including \
             `surface.code`, and `token/contrast.rs` is a WCAG 2.1 luminance and \
             contrast-ratio solver that composites translucent layers before measuring — \
             so the picker's live `4.80:1 AA` / `2.40:1 Fail` status chip already has \
             its arithmetic available. What is missing is the colour *type* and every \
             pixel: no `Rgba8`/`Hsva` pair exists and no HSV-to-RGB conversion does, so \
             even the canvas's own coordinate maths is unwritten.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-ATOM-013",
        status: Status::Partial,
        section: "§3.13",
        evidence: &["widget/badge_model.rs", "token/contrast.rs", "token/size.rs"],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this widget to Phase C on 2026-10-23. No `AvatarModel`, \
             `AvatarSize`, `RatingModel`, `RatingStep`, star row, initials fallback or \
             presence cut-out exists. Missing: the whole requirement — the 24 / 32 / \
             36px circular or rounded-square container with its `1px border.strong` \
             perimeter; the deterministic `image -> one-or-two uppercase initials -> \
             generic person glyph` fallback cascade with the 4.50:1 floor its initials \
             must clear on `accent.fill` or `surface.active`; the 6x6px presence badge \
             with its `1px surface.base` cut-out ring; the row of five 24x24px star slots \
             at `space.xs` gap with `status.warning` fills and `border.strong` empties; \
             half-step versus whole-step rating with hover preview; clear-on-reclick and \
             `Delete`; and the `Live` / `Commit` / `Cancel` callback set. Reusable pieces \
             exist: `BadgeTone` and the §3.8 tones the presence dot would borrow are \
             built in `widget/badge_model.rs`, and `token/contrast.rs` can verify the \
             initials-on-fill pairing the moment one is drawn. Neither is assembled into \
             an avatar or a rating control, and the star glyph itself is not among the \
             marks this crate paints.",
        ),
        note: None,
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
        status: Status::Partial,
        section: "§3B.9",
        evidence: &["widget/rove.rs", "surface/tooltip/mod.rs", "token/size.rs"],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this composite to Phase C on 2026-10-23. No `NavigationRailModel`, \
             `NavDestination`, compact/expanded mode, collapse toggle or pinned-utility \
             split exists. Missing: the whole requirement — the 48px compact icon-only rail \
             whose every cell carries a mandatory right-aligned hover tooltip on the \
             `time.delay.tooltip_warm` handoff; the 220px expanded rail resizable within \
             [180px, 320px] with 36px rows, a 16px leading icon, a 13px Medium label and \
             an optional trailing count pill; the `surface.active` fill with its 2px \
             `accent.border` leading indicator bar for the selected destination; the \
             28x28px collapse toggle, the scrollable middle list and the divider above \
             the pinned bottom utilities; and the `Commit`-based select, expand-toggle and \
             resize-cancelling callback set. Primitives a builder would reuse are proven — \
             `widget/rove.rs` for the wrapping single-tab-stop walk over enabled \
             destinations and `surface::tooltip` for the hover card — but the rail, its \
             width transition and its cross-session width persistence are unbuilt. `time.slow` (240ms), \
             the rung that transition is priced at, is a token now (`token::time::SLOW`, \
             `EaseInOutCubic`), so the token layer is not what blocks a builder: the rail \
             itself is.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-010",
        status: Status::Partial,
        section: "§3B.10",
        evidence: &[
            "widget/auto_suggest_model.rs",
            "widget/auto_suggest_host.rs",
            "surface/modal.rs",
            "surface/zorder.rs",
        ],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built as a palette: \
             the user deferred this composite to Phase C on 2026-10-23. No \
             `CommandPaletteModel`, `PaletteMode`, `PaletteGroup`, prefix-mode contract or \
             two-stage `Escape` exists in palette form. Missing: the whole requirement — \
             the 560x380px card top-centred at a fixed 64px offset over `surface.scrim` \
             in Z-Band 3 / Escape Rank 2 so result counts never shift the input; the 40px \
             search field with its leading glyph, its removable `>` / `#` / `@` mode chip \
             that `Backspace` at caret 0 resets to Files, and its trailing `Esc` hint; the \
             virtualized grouped result list with matched-span emphasis, the active-row \
             subtitle promoted to `text.strong` under the Rule 4 foreground lock, and \
             right-aligned mono shortcut pills; the 24px keyboard legend footer; and the \
             `Live` / `Commit` / `Cancel` callback set. The closest relative in this crate \
             is genuinely close and worth naming rather than hiding: `AutoSuggestModel` and \
             `AutoSuggestHost` already implement a trailing-edge 200ms debounce restarted \
             per keystroke, a monotonic generation guard that drops out-of-order answers \
             and counts them, a roving highlight that deliberately leaves the text caret \
             alone, and a two-stage `Escape`. None of that is a palette: there is no \
             scrim-mounted modal card at the palette's band, no mode-prefix state, no \
             grouped-result virtualization and no `Ctrl+K` binding — and the scrim, band \
             and escape-rank plumbing exists independently in `surface/modal.rs` and \
             `surface/zorder.rs` without being joined to any of it.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-011",
        status: Status::Partial,
        section: "§3B.11",
        evidence: &["widget/badge.rs", "state.rs", "token/size.rs"],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this composite to Phase C on 2026-10-23. No `StepperModel`, \
             `StepStatus`, `StepItem`, connector line, `PipsPagerModel` or pip strip \
             exists. Missing: the whole requirement — the 36px horizontal and the vertical \
             step sequences; the 24x24px pill node that encodes `Completed` / `Current` / \
             `Pending` / `Error` by glyph *and* colour together rather than by tint alone; \
             the `border.subtle` versus `accent.border` connector lines; the non-shifting \
             wizard footer that renders `[Previous]` as `Disabled` rather than absent on \
             step 0 so footer button coordinates never move between steps; the linear \
             versus free-navigation gate; the 24px pips strip with its 6x6px inactive dots \
             and 16x6px `radius.pill` active pip between its two 24x24px arrow buttons; \
             and both callback sets. The redundant-cue rule the stepper exists to satisfy \
             (`UIF-PRIN-005`) is honoured elsewhere in the crate and the check and \
             exclamation marks are already painted by `widget/badge.rs`, but no step state \
             machine, no completion ordering and no pip geometry exists anywhere.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-012",
        status: Status::Partial,
        section: "§3B.12",
        evidence: &[
            "surface/zorder.rs",
            "surface/modal.rs",
            "surface/movable.rs",
            "surface/modal_tier.rs",
        ],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this composite to Phase C on 2026-10-23. No `SideSheetModel`, \
             `SheetMode`, docked/overlay/scrim variant, resize grab or pin state exists. \
             Missing: the whole requirement — the 360px drawer resizable across \
             [280px, 520px] over a 10px `size.splitter.grab` edge divider; the 36px \
             header with its 16px SemiBold title, 24x24px `[Pin/Unpin]` and 24x24px \
             `[Close]`; the padded scrollable body and optional 48px action footer; and \
             the three-mode band mapping, `Docked` at Z-Band 0 sharing the horizontal \
             workspace layout without covering content, `ModelessOverlay` at Z-Band 1 / \
             Escape Rank 1 when focused, and `ModalScrim` at Z-Band 3 / Escape Rank 2 \
             with the `UIF-MOD-002` dirty and busy close guard. Of any Phase-C item this \
             one has the most nearly-available plumbing: `surface/zorder.rs` already owns \
             all four bands and their escape ranks, `surface/modal.rs` proves the scrim, \
             the focus trap and the dirty/busy guard, and `surface/movable.rs` proves a \
             drag-and-clamp gesture over an injected geometry seam. None of it is docked, \
             none of it is a sheet, and no rule maps a viewport narrower than 800px onto \
             `ModalScrim`.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-013",
        status: Status::Partial,
        section: "§3B.13",
        evidence: &[
            "composite/virtual_list.rs",
            "composite/table_view.rs",
            "composite/tree_view.rs",
            "composite/scroll_view.rs",
        ],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this composite to Phase C on 2026-10-23. No `GridViewModel`, \
             `GridTile`, column-count solver or two-dimensional selection state exists. \
             Missing: the whole requirement — the deterministic column derivation of \
             `max(1, floor((available + gap) / (min_tile + gap)))` computed over the \
             width left after the reserved 10px `ScrollView` gutter, at `gap = space.md` \
             with a 160px minimum tile, and the matching tile-width division; the \
             virtualized 2D tile window; the `surface.active` plus 2px `accent.border` \
             selection treatment with its top-left 16x16px checkbox overlay in \
             `ExtendedMulti`; and the clamped 2D arrow map (`Left`/`Right` of one, \
             `Up`/`Down` of `cols`, `Home`/`End` to first and last) with `Shift` and \
             `Ctrl` anchored range and toggle matching §3B.6. The one-dimensional \
             relatives are built and good: `composite/virtual_list.rs` virtualizes a row \
             window with overscan and `f64` index arithmetic, `composite/scroll_view.rs` \
             reserves the gutter, and `composite/table_view.rs` and `tree_view.rs` already \
             clamp at the ends rather than wrapping and carry the `SelectionMode` the grid \
             reuses by name. Nothing multiplies those into two dimensions, and the \
             column-count derivation `UIF-VER-002` names has no solver to test.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-014",
        status: Status::Partial,
        section: "§3B.14",
        evidence: &["widget/progress.rs", "state.rs", "token/size.rs"],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this composite to Phase C on 2026-10-23. No `EmptyStateModel`, \
             `EmptyStateKind`, `Skeleton` placeholder or `PaginationModel` exists — a gap \
             this ledger has already had to record from the other direction, because \
             `UIF-PRIN-004` is held `Partial` precisely on the words 'this crate has no \
             empty-state or placeholder component'. Missing: the whole requirement — the \
             centred 24px-padded, 360px-max placeholder that distinguishes `NoItems`, \
             `NoMatches { query }` and `LoadError { code }` with its 40x40px icon badge, \
             16px SemiBold headline, 13px explanation and optional recovery CTA; the \
             `Skeleton` that reserves the exact target widget's dimensions and pulses \
             `surface.raised` to `surface.hover` over 1200ms on an injected phase, going \
             static with a `border.subtle` under injected `reduced_motion`; and the 32px \
             pagination footer with its mono range summary, its centre page-size combobox, \
             its five boundary-clamped navigation buttons rendered `Disabled` rather than \
             absent at the ends, and its `on_page_change` / `on_page_size_change` \
             callbacks. Injected-phase animation input already has a precedent — \
             `widget/progress.rs` takes a `phase` and a `reduced_motion` bool and never \
             reads a clock — but the 1200ms pulse has no `time.*` token under \
             `UIF-TOK-007` and no placeholder paints anywhere in this crate.",
        ),
        note: None,
    },
    Coverage {
        id: "UIF-COMP-015",
        status: Status::Partial,
        section: "§3B.15",
        evidence: &[
            "token/color.rs",
            "token/typography.rs",
            "composite/form_field.rs",
            "widget/text_input.rs",
        ],
        reason: Some(
            "Specified, framework-owned under D-2, and nothing of it is built: the user \
             deferred this composite to Phase C on 2026-10-23. No `CodeBlock`, \
             `DiffViewerModel`, `DiffHunk`, `DiffDisplayMode`, `KeybindRecorderModel` or \
             `KeyChord` type exists. Missing: the whole requirement — the code container \
             with its 28px header, language badge and fixed-width `[Copy]` slot that holds \
             the success tick for `time.hold.copied` (1500ms) at 0px shift, and its 13px \
             mono body with a 40px right-aligned tabular line-number gutter; the diff view \
             that pairs `+` and `-` glyphs with 2px `status.success` and \
             `status.danger.text` left bars and distinct text roles rather than relying \
             on background tint alone, in unified and side-by-side modes with per-hunk \
             accept and reject buttons in the hunk header; and the keybind recorder's \
             32px listening input, mono keycap pills, trailing clear button, raw chord \
             interception with `Escape` cancelling without clearing, and conflict \
             surfacing through `FormField` validation. Two blockers are recorded against \
             other requirements rather than solved here: `UIF-TOK-004` models no font \
             family, so `FontFamily::Mono` — the type all three of these are written in — \
             has no rung. `time.hold.copied` is a rung now (`token::time::HOLD_COPIED`). The \
             `surface.code` background the section asks for already exists in \
             `token/color.rs`, so neither the timing nor the palette is the gap.",
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
