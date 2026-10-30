# Tab Strip Dead Width — Reduction Proposal

**Status: PROPOSED — needs owner rulings on D-1 and D-2 before implementation.**
No code has been changed on the strength of this document.

Measures the horizontal space a tab in `src/app/ui_iced/panels/center.rs` renders
around its title and close button, and proposes the smallest set of changes that
recovers it *without* leaving hoarfrost's `spec.md` behind. Every number below is
either measured from a laid-out `iced_test::Simulator` or quoted from the spec
with a citation.

---

## 1. What is actually being rendered

Measured from a live strip (`tab_strip(&vm)` at 1200 × 36), reading bounds out of
the layout tree via `Candidate::Container` / `Text` / `Focusable`. The tab is
shrink-wrapped, so chrome is constant and only the title varies:

| title | tab box | title text | close box | chrome outside text + close |
|---|---|---|---|---|
| `a.md` | 104.306 × 28 | 29.269 × 18 | 33.037 × 24 | **42.000** |
| `notes.md` | 130.001 × 28 | 54.964 × 18 | 33.037 × 24 | **42.000** |
| `the-quick-brown-fox-jumps-over.md` | 290.870 × 28 | 215.833 × 18 | 33.037 × 24 | **42.000** |

The 42.000px decomposes exactly:

| part | px | constant | token |
|---|---|---|---|
| left padding | 12 | `TAB_PADDING_X` | `space.md` |
| unsaved-marker slot | 12 | `UNSAVED_MARKER_WIDTH` | `space.md` |
| marker → title | 2 | `TAB_MARKER_SPACING` | `space.2xs` |
| title → close | 4 | `TAB_CLOSE_SPACING` | `space.xs` |
| right padding | 12 | `TAB_PADDING_X` | `space.md` |

Cross-checked against the render: left gap (`slot.x → text.x`) = 26.000 =
12 + 12 + 2; right gap = 12.000. The 1px `TAB_BORDER_WIDTH` costs nothing — iced
0.14 never reads a container's `border` in its layout path.

**The close button is the second half of the problem.** It renders 33.037px wide.
`Button::outer_size` computes `content.width + padding.x()`, floored at
`ICON_ONLY_MIN_WIDTH` (28); at 33.037 the floor is not binding, so the glyph is
33.037 − 2 × `button::HORIZONTAL_PADDING` (12) = **9.037px** wide inside a
33.037px box. [INFERENCE — the glyph's own bounds are not directly readable; the
selector returns the button's box. The figure is derived from `outer_size`'s
formula at `widget/button_widget.rs:809-811`.]

Total non-title width per tab: **75.037px**. On `a.md` the title is 28% of the
tab; on `notes.md`, 42%.

---

## 2. Why there is an unsaved area at all — the audit finding

**Nothing in the shipped app can ever light it.** The claim was checked, not
assumed:

- `TabEntry::unsaved` is written in exactly one production site,
  `tab_entries()` at `panels/center.rs:624`, which hardcodes `unsaved: false`.
- `tab_entries()` is called from exactly one place, `from_app_with_note_skills`
  at `center.rs:1261`. There is no other construction path.
- A grep for `unsaved: true`, `unsaved = true` and `.unsaved` across all of
  `src/` returns three hits: the render site (`center.rs:1420`) and two tests
  asserting the value is `false`. **No writer exists.**
- The module already admits this (`center.rs:95-100`): "The app has **no**
  per-tab dirty tracking anywhere … Flagged as spec drift per [RUST-041]."

So 14.000px of every tab — the 12px slot plus the 2px spacing reserved for it —
is paid for a signal that is unreachable in every frame of the shipped build.

**But the app is not read-only, and that matters for the fix.** `ui_iced/editor.rs`
is a full inline markdown editor (UI-037…UI-044: front-matter-preserving save,
undo, pre-save GFM validation) with its own `is_dirty()` and a discard-confirmation
guard (`UIF-MOD-002`, wired at `app.rs:1458`). The dirty state exists and is
correctly guarded — it simply never reaches the tab strip. The marker is therefore
*unwired*, not *wrong*, and `UIF-TAB-003`'s dirty-close guard (`spec.md:1354`)
cannot be implemented at all until it is.

Conclusion: the slot should not be deleted outright. It should stop costing
layout, and the signal should be connected.

---

## 3. What hoarfrost binds here

The constraints a proposal has to survive, with the authority for each. The app's
current tab is, on several points, **more** spec-conformant than hoarfrost's own
unused `composite/tab_strip` — see §6 before reaching for "just use the composite".

| Constraint | Authority | App today |
|---|---|---|
| Tab width `96..220` | §2.2 `spec.md:109`; §4.4.1; `UIF-TAB-002` `spec.md:1353` | **absent** — shrink-wrap, no clamp |
| Close `[×]` = 24 × 24 hit target, preserved under overflow | §2.2 `spec.md:77`; §4.4.1 slot 4 `spec.md:1335`; `UIF-VER-002` `spec.md:1632` | 33.037 × 24 (padded button) |
| Close separated from tab body by ≥ 2px (`space.2xs`) | `UIF-PRIN-008` `spec.md:48`; §4.4.1 slot 4 says `space.2xs` | 4px (`space.xs`) |
| Leading status slot `12px`, `6px` vector dot, "reserved/collapsed cleanly when saved" | §4.4.1 slot 2 `spec.md:1333` | 12px always reserved, never painted |
| Title `13px`, end-truncated with `…` at 220px, tooltip with full path | §4.4.1 slot 3 `spec.md:1334`; `UIF-PRIN-007` `spec.md:41-44` | 13px ✓; **no truncation, no tooltip** |
| Sibling gap `space.2xs` (2px) of exposed track | §4.4.1 `spec.md:1329` | ✓ 2px |
| Leading inset `space.xs` (4px) | §4.4.1 `spec.md:1328` | ✓ 4px |
| Track height 36px (`size.control.lg`) | §2.2 `spec.md:81`; §4.4.1 `spec.md:1318` | ✓ 36px |
| Tab horizontal padding `space.md` | §2.1 `spec.md:68` ("buttons, inputs, **tabs**, table cells, menu rows") | ✓ 12px |
| Close glyph `14px` (`size.icon.sm`) | §4.4.1 slot 4 `spec.md:1335` | ✓ 14px |
| Icon-only control must carry a tooltip | `UIF-ATOM-001`; `button_widget.rs:611-621` | ✓ `CLOSE_TAB_TOOLTIP` |

Two things the spec does **not** provide, stated so they are not mistaken for
available levers:

- There is no "dead space", "purposeful whitespace" or density principle to cite.
  The nearest normative statements are `UIF-PRIN-002` (compact height against a
  mandatory 24 × 24 target, `spec.md:20`) and §2.1's base-4 grid (`spec.md:61`).
- `layout/density.rs` scales **height only** (`ControlDensity::height`,
  `density.rs:74-80`), over a closed list of five bands that does **not** include
  the tab strip (`density.rs:111-131`). It offers no horizontal lever, and the
  strip's 36px track is off the density ladder entirely.

`token_lint` will not police any of this: a `space::MD → space::SM` swap is
`TokenBound` and passes, and inline padding literals are invisible to Rule B
(`token_lint.rs:96-105` explicitly disclaims semantic correctness). **Passing the
lint is not compliance here.** The real gates are `UIF-VER-002` (hit bounds) and
`UIF-VER-004` (tab-strip geometry snapshots).

---

## 4. Proposals

### P1 — Stop the status slot from costing layout, and wire the signal it marks
**Reclaims 14.000px per tab.**

The spec's slot is `12px` (§4.4.1 slot 2). The tab's left padding is *already*
`space.md` = 12px. **The leading padding already is the leading status slot** —
it is the same width, in the same position, for the same reason. So the slot does
not need its own layout box:

- Delete the reserved `UNSAVED_MARKER_WIDTH` container and `TAB_MARKER_SPACING`.
- Paint the `6px` dot into the existing left padding through the `Stack` the tab
  already uses for its top accent indicator. That pattern is established in the
  same function and is documented as taking no layout (`center.rs:1414-1416`:
  "the indicator rides in a `Stack` and takes no layout").
- Connect the editor's dirty flag to `TabEntry::unsaved` so the dot can actually
  appear, closing the drift the module admits at `center.rs:95-100` and making
  `UIF-TAB-003` implementable.

Why this over the two obvious readings: `UIF-PRIN-003` demands zero layout shift
on state transitions, and the spec's own tab wording — "reserved/collapsed
cleanly when saved" (`spec.md:1333`) — is genuinely ambiguous between them. A
`Stack`-painted dot satisfies both readings at once: the 12px slot exists as
specified, and a dirty toggle moves nothing by 0px. Reserving a real 12px box
would cost 14px on every tab forever to describe a state true on at most one tab
at a time; collapsing it (what hoarfrost's own composite does,
`tab_strip.rs:385-394`) shifts the title the moment the user starts typing.

### P2 — Make the close control the 24 × 24 target the spec describes
**Reclaims 9.037px per tab.**

§4.4.1 slot 4 describes a `24px × 24px` target with a `14px` glyph — a fixed
hit area, not a padded button. The app instead builds
`Button::icon_only(..).compact()`, which applies `button::HORIZONTAL_PADDING`
(`space.md`, 12px per side) and yields 33.037 × 24 for a ~9px glyph.

Build it as a fixed 24 × 24 zero-padding target instead, keeping the tooltip so
`UIF-ATOM-001` still holds. This is exactly what hoarfrost's own composite does
(`tab_strip.rs:412-420`: `.padding(Padding::ZERO)` plus
`Length::Fixed(CLOSE_BUTTON_SIZE)`), and §2.2 `spec.md:77` names the close `×`
explicitly as a `size.hit_target.min` case.

This also dissolves a latent conflict found while measuring: `.compact()` asks for
`HEIGHT_COMPACT` = 28 and `ICON_ONLY_MIN_HEIGHT` = 28, but `outer_size` ends in
`.min(max.height)` and the tab's content band is 28 − 2 × 2 = 24 — so the button
is silently clamped 28 → 24 every frame. A fixed 24 × 24 target states the real
intent and stops two tokens disagreeing in a way nothing reports.

Note the §3.1 icon-only floor of 28 × 28 (`spec.md:268`) does **not** apply here:
`button.rs:174-183` records that `ICON_ONLY_MIN_WIDTH` is the *atom* rule for a
navbar-class control, and §2.2 assigns the close `×` to the 24px hit-target
floor. The tab close is the narrower, spec-named case.

### P3 — Bring the close separation onto its own token
**Reclaims 2.000px per tab.**

`TAB_CLOSE_SPACING` is `space.xs` (4px). §4.4.1 slot 4 specifies `space.2xs`
(2px) separation from the tab activation zone, and `UIF-PRIN-008` sets the
disjointness floor at exactly 2px. So 2px is both the specified value and still
compliant with the floor — this is conformance, not a gamble.

### P4 — Add the `TAB_WIDTH = 96..220` clamp the spec already mandates
**Reclaims up to 70.870px on a long title; costs width on very short ones.**

The app has no clamp at all, which `UIF-TAB-002` (`spec.md:1353`) and §2.2
(`spec.md:109`) both require. The 220px ceiling is where the largest single win
lives: the long-title tab measures **290.870px today, 70.870px past the ceiling.**

Two obligations come attached, and they are the reason this is a proposal rather
than a one-liner:

- **Truncation.** Capping at 220px forces the `…` that §4.4.1 slot 3 mandates.
  iced text wraps rather than ellipsizes, so this needs hoarfrost's own
  `microcopy::truncate` (`truncate_end` / `truncate_middle`,
  `microcopy/truncate.rs:134,163`).
- **Tooltip.** Anything that truncates at layout time must register a tooltip
  with the full path (`UIF-PRIN-007` `spec.md:44`; `check_truncation_tooltip`,
  `microcopy/truncate.rs:286`). The tab title has **no tooltip today** — only the
  close button does. This must be added in the same change, not deferred.
- **Which truncation mode is an open question.** §4.4.1 slot 3 says end-truncate
  for the tab title, while `UIF-PRIN-007` mandates *middle*-truncation for
  filesystem paths and the tab title is a filename. The spec resolves the tab
  case by carrying the full path in the hover tooltip, which argues for
  end-truncation, but the two clauses are in tension and the owner should rule.

---

## 5. Net effect

Per-tab chrome (everything that is not the title):

| | today | proposed | Δ |
|---|---|---|---|
| outside text + close | 42.000 | 26.000 | −16.000 |
| close box | 33.037 | 24.000 | −9.037 |
| **total chrome** | **75.037** | **50.000** | **−25.037 (−33%)** |

Resulting tab widths, with P4's clamp applied:

| title | today | uncapped | final | actual gain |
|---|---|---|---|---|
| `a.md` | 104.306 | 79.269 | **96.000** (floor) | −8.306 |
| `notes.md` | 130.001 | 104.964 | **104.964** | −25.037 |
| `the-quick-brown-fox-jumps-over.md` | 290.870 | 265.833 | **220.000** (ceiling) | −70.870 |

**The honest caveat:** the 96px floor is what limits the win on short names, and
it is hoarfrost's own registered constant. P1–P3 would take `a.md` to 79.269px,
and §2.2 pulls it back to 96. The dead-space recovery is therefore real for
medium and long titles and largely absorbed by the floor for short ones. If the
owner wants the short-tab win too, the floor itself has to be revisited in the
spec — that is a spec amendment, not a code change, and it is out of scope for
this proposal.

---

## 6. Rejected, with reasons

**Adopt `hoarfrost::composite::tab_strip` instead.** Rejected. It is unused by
the app (zero imports in `src/app`) and it is *less* conformant than the code we
have: `TAB_STRIP_HEIGHT` is 32px against §2.2/§4.4.1's 36px (`tab_strip.rs:21`),
the label is `BODY` 14px against the specified 13px (`tab_strip.rs:398`), the
close glyph is a 12px `"×"` character against the specified 14px
`size.icon.sm` (`tab_strip.rs:407-408`), and the dirty dot is the font glyph
`"•"` against §4.4.1 slot 2's explicit "never as a font glyph"
(`tab_strip.rs:388`). Adopting it would trade four conformance wins for one
reuse. It is worth a separate defect against the composite.

**Cut tab padding `space.md` → `space.sm` (would save a further 8px).** Rejected
as written. §2.1 `spec.md:68` binds `space.md` to "buttons, inputs, tabs, table
cells, and menu rows". A `space.sm` compression *is* precedented — segmented
controls compress `space.md → space.sm` (`spec.md:244`) and dropdown triggers may
use `space.sm` compact (`spec.md:443`) — but in both cases the spec frames it as
**container-width-triggered**, part of an atomic `Expanded → Compact` transition,
not a value a host picks at will. Note hoarfrost's own composite already uses
`space::SM` here (`tab_strip.rs:450`), which is drift, not precedent. If the
owner wants this, it should enter as a spec amendment with a width trigger.

**Delete the unsaved marker outright.** Rejected. The editor is real and
`UIF-TAB-003`'s dirty-close guard is specified; the signal is unwired, not
unwanted. P1 removes the cost without removing the capability.

**Reduce the close target to 24 × 24 *and* drop the tooltip.** Rejected —
`UIF-ATOM-001` requires the tooltip on an icon-only control, and
`button_widget.rs:799-804` asserts it at layout time.

---

## 7. Decisions needed from the owner

- **D-1 (P1):** is the `Stack`-painted dot acceptable, or must the 12px slot be a
  real reserved box per the "reserved" reading of `spec.md:1333`? The Stack
  reading costs 0px and shifts 0px; the reserved reading costs 14px/tab.
- **D-2 (P4):** end-truncation or middle-truncation for the tab title, given
  §4.4.1 slot 3 and `UIF-PRIN-007` disagree about filenames?
- **D-3:** confirm wiring the editor's `is_dirty()` to `TabEntry::unsaved` is in
  scope now, or whether P1 ships as layout-only and the wiring lands with the
  `UIF-TAB-003` dirty-close guard.
- **D-4:** is the 96px floor accepted as the limit on short-tab recovery (§5), or
  does the owner want a spec amendment against §2.2?

## 8. Test plan

Per [RUST-005], each change starts from a failing measurement test.

- Pin the current 42.000px chrome and 33.037px close box as the *before* state,
  then re-pin to the proposed figures — the same measurement harness used for
  this document (`Candidate::Container` / `Text` / `Focusable` bounds).
- `UIF-VER-002`: assert the close hit rectangle is ≥ 24 × 24 and disjoint from
  the tab body by ≥ 2px, measured, not asserted from constants.
- P1: assert a dirty toggle moves the title by exactly 0px, and that the dot is
  painted when `unsaved` is true — which requires a construction path that can
  set it, i.e. the wiring in D-3.
- P4: assert the clamp at both ends (a 1-char title lands on 96, a long one on
  220), that the truncated label carries a tooltip, and that `check_truncation_tooltip`
  reports no violation for the strip.
- Re-bless `UIF-VER-004` tab-strip snapshots — token_lint passing proves nothing
  here (§3).

Full gate from repo root: `cargo check --quiet`, `cargo nextest run --workspace`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
`cargo doc --no-deps --quiet`.
