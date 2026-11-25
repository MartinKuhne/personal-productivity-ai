# Tab Strip Dead Width — Reduction Proposal

**Status: PARTIALLY LANDED - D-1 and D-3 answered and implemented in `4afe450`
(marker removed outright, horizontal padding ruled to `space.xs`). D-2 and D-4
remain open; P2, P3 and P4 are not implemented.**
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

**The tab strip is read-only by construction, and the marker has no reachable
state.** The editor is **not a tab and never was**. Tracing it:

- `ui_iced/editor.rs` renders `editor_modal` — a **modal overlay surface**,
  registered as `SURFACE_ID_EDITOR = "overlay.editor"` and documented as "the
  inline editor modal (`Modal`)" (`app.rs:1552-1553`). It is mounted through the
  overlay host's z-ordered `OverlayLayer::Editor` (`app.rs:1597-1599`), not
  through the pane grid.
- It is *opened from* a tab, never *as* one: the tab's context menu emits
  `UserCommand::OpenInEditor(path)` (`center.rs:1130-1141`), the reducer queues
  `UserFileOperation::OpenEditor`, and the result lands on
  `state.overlay.editor` (`update.rs:838`).
- The tab's own document view is a **read-only render**. `document_preview`
  (`center.rs:1593`) and the render handed to `center_panel_rendered` draw
  markdown, the YAML front-matter table and FTWA tables. No tab ever hosts an
  editable buffer.
- `inline_editor_enabled` (CONFIG-001) does **not** mean "docked in the pane". It
  selects FastMD's modal editor versus the **external system editor**
  (`user_file_operation.rs:236-237`) — and in the external case the app holds no
  dirty signal whatsoever.

So can a tab ever be dirty *while the strip is visible and interactive*? Following
the modal host: hoarfrost's `Modal` is a scrim whose "bounds are the viewport",
with a focus trap and card-level click absorption (`surface/modal.rs:1,31-37`),
and `guard_leave` (`UIF-MOD-002`) refuses `Escape`, the scrim and `Cancel` while
dirty, permitting only an explicit discard (`app.rs:1452-1458`). Therefore:

| state | is the dot observable? |
|---|---|
| editor open, dirty | no — the scrim covers the viewport and focus is trapped |
| editor closed after save | no — the buffer is clean |
| editor closed after discard | no — the buffer was explicitly thrown away |
| external system editor | no — the app has no dirty signal at all |

**There is no reachable state in which a tab is dirty and the marker could be
seen.** It is not merely unwired; under this app's modal editing model it is
structurally unobservable. `UIF-TAB-003`'s dirty-close guard (`spec.md:1354`) is
subsumed by the modal's own `UIF-MOD-002` guard: a tab cannot be reached to be
closed while its editor is up and dirty.

Conclusion: the marker should be **removed**, not reserved and not wired. §4.4.1
slot 2 is not implementable in this architecture, and the drift note at
`center.rs:95-100` should say so with the reason instead of promising that "the
reducer populates it when dirty tracking lands".

> Correction to an earlier draft of this document: it asserted "the app is not
> read-only" and cited `editor.rs` as if the tab's document were editable. That
> was wrong — `editor.rs` is a modal surface, not a tab, and the tab strip is
> read-only by construction. The consequence is that P1 got *stronger*, not
> weaker: the field should go rather than be connected.

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

### P1 — Remove the unsaved marker from the tab
**Reclaims 14.000px per tab.**

Per §2 the marker has no reachable state, so the honest change is deletion rather
than rearrangement:

- Drop the reserved `UNSAVED_MARKER_WIDTH` container and `TAB_MARKER_SPACING`
  from `tab_widget`.
- Retire `TabEntry::unsaved`, `unsaved_dot`, `UnsavedDot`, `UNSAVED_DOT_SIZE`,
  `UNSAVED_DOT_RADIUS` and `tab_unsaved_dot_color` — a field that is written
  `false` in one place and read in one place is not state, it is decoration.
- Replace the drift note at `center.rs:95-100` with the actual finding: §4.4.1
  slot 2 is **not implementable under the app's modal editing model**, because the
  only thing that could set dirty is a scrimmed modal that cannot be dismissed
  while dirty. That is a reasoned `UIF-PRIN` deviation, not an unfinished task.

**Contingency, and the reason the layout work still matters.** If a *docked*
(non-modal) editor ever lands — and the `inline` naming in CONFIG-001 suggests
that was once the intent — the dot becomes observable again, and then it should
return **without taking layout**: the tab's left padding is *already* `space.md`
= 12px, which is exactly §4.4.1 slot 2's `12px` leading status slot, same width,
same position. Painting a `6px` dot into that padding through the `Stack` the tab
already uses for its top accent indicator satisfies both readings of the spec's
ambiguous "reserved/collapsed cleanly when saved": the slot exists as specified,
and a dirty toggle moves 0px. The pattern is established in the same function and
is documented as taking no layout (`center.rs:1414-1416`: "the indicator rides in
a `Stack` and takes no layout").

Reserving a real 12px box would cost 14px on every tab forever to describe a state
that is unreachable today; collapsing it (what hoarfrost's own composite does,
`tab_strip.rs:385-394`) would shift the title the moment a user starts typing.
Removing it avoids both, and the `Stack` route is the recorded answer for when —
and only when — a non-modal editor makes it meaningful.

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

**Wire the editor's `is_dirty()` through to `TabEntry::unsaved`.** Rejected, and
this document originally recommended it — see the correction in §2. Wiring would
populate a field that can never be *seen*: the only thing that sets dirty is a
scrimmed, focus-trapped modal that refuses to close while dirty, so the strip is
never visible-and-interactive with a dirty buffer. Connecting it would add a
cross-subsystem dependency (`overlay` → tab view-model) to light an indicator no
one can observe, and would leave `UIF-TAB-003` still unimplemented because the
modal's own `UIF-MOD-002` guard already prevents the close it describes.

**Reduce the close target to 24 × 24 *and* drop the tooltip.** Rejected —
`UIF-ATOM-001` requires the tooltip on an icon-only control, and
`button_widget.rs:799-804` asserts it at layout time.

---

## 7. Decisions needed from the owner

- **D-1 (P1): ANSWERED - remove it outright.** Implemented in `4afe450`. The
  field, its slot, its dot, its radius and its role-locked colour are all gone,
  not merely un-laid-out.
- **D-2 (P4): OPEN.** End-truncation or middle-truncation for the tab title,
  given §4.4.1 slot 3 and `UIF-PRIN-007` disagree about filenames?
- **D-3: ANSWERED by implication.** The owner ruled full removal rather than
  "slot stops costing layout, field retained", which is the answer that only
  makes sense if a docked editor is *not* on the roadmap. Recorded as such:
  `UIF-TAB-003`'s dirty-close guard is satisfied-by-substitution through
  `UIF-MOD-002`, and the contingency for a future docked editor is left in the
  `panels/center.rs` module header so the reasoning survives with the code. If a
  docked editor does land, this ruling has to be revisited, not just re-opened.
- **D-4: OPEN.** Is the 96px floor accepted as the limit on short-tab recovery
  (§5), or does the owner want a spec amendment against §2.2?

### Ruling that overrode this document

The owner additionally ruled **`TAB_PADDING_X` down to 4px (`space.xs`)**. §6
rejected narrowing the tab padding, and §3 records that spec.md:68 binds
`space.md` to tabs by name - so this is an owner override of a written spec
binding, not a reading of it. It is implemented because the owner outranks the
spec here, and it is flagged as drift in the constant's own doc comment per
[RUST-041]. The rung is still on the base-4 grid, so `UIF-TOK-001`'s "no off-rung
spacing in layout code" rule is honoured even though the role binding is not.

Measured effect, from laid-out bounds rather than arithmetic: chrome outside the
title and close button fell **42.000 -> 12.000px** per tab. The marker accounted
for 14 of that (12 slot + 2 spacing) and the padding for 16 (8 per side). P2,
P3 and P4 would have reached 50.000 total non-title width by a different route;
the ruling recovered comparable space on the padding alone.

## 8. Test plan

Per [RUST-005], each change starts from a failing measurement test.

- Pin the current 42.000px chrome and 33.037px close box as the *before* state,
  then re-pin to the proposed figures — the same measurement harness used for
  this document (`Candidate::Container` / `Text` / `Focusable` bounds).
- `UIF-VER-002`: assert the close hit rectangle is ≥ 24 × 24 and disjoint from
  the tab body by ≥ 2px, measured, not asserted from constants.
- P1: assert the strip renders no unsaved-marker slot at all — i.e. the measured
  chrome drops by exactly 14.000px — and that removing `TabEntry::unsaved` leaves
  no dead references. If D-3 comes back as "docked editor is on the roadmap", the
  replacement assertion is that a dirty toggle moves the title by exactly **0px**,
  which is what makes the `Stack` route the required implementation rather than a
  stylistic preference.
- P4: assert the clamp at both ends (a 1-char title lands on 96, a long one on
  220), that the truncated label carries a tooltip, and that `check_truncation_tooltip`
  reports no violation for the strip.
- Re-bless `UIF-VER-004` tab-strip snapshots — token_lint passing proves nothing
  here (§3).

Full gate from repo root: `cargo check --quiet`, `cargo nextest run --workspace`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
`cargo doc --no-deps --quiet`.
