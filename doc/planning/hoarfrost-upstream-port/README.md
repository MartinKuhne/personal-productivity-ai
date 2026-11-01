# Hoarfrost externalisation — content that must be ported upstream

> **Status:** Port staging area — assembled 2026-11-01, **nothing here has been
> applied upstream yet**. Executes Phase 0 of
> [`hoarfrost-external-crate-plan.md`](../hoarfrost-external-crate-plan.md).

## Why this folder exists

On 2026-11-01 the `hoarfrost` UI framework (`src/hoarfrost/`, 354 tracked files)
and its showcase app (`src/hoarfrost-demo/`, 52 tracked files) were externalised:
this repo stops carrying them and consumes
<https://github.com/MartinKuhne/hoarfrost> as a **git dependency** instead of a
workspace member. Phase 2 deletes both directories.

A small amount of content exists **only in this repo** and would be destroyed by
that deletion. This folder holds it — verbatim copies plus diff records — until it
has been ported upstream. Once the upstream commits land, this folder can be
deleted.

Nothing under `src/hoarfrost/` or `src/hoarfrost-demo/` was modified to produce
this folder; those sources were read-only inputs. They have **since been
deleted** — Phase 2 removed both directories on 2026-11-01 — so this folder is
now the only copy of that content outside this repo's git history.

## The port list

| File here | Original in-repo path | Destination upstream | Action |
|---|---|---|---|
| [`hoarfrost-AGENTS.md`](hoarfrost-AGENTS.md) | `src/hoarfrost/AGENTS.md` (25 lines) | upstream root `AGENTS.md`, `## Hoarfrost Specifics` | **Merge, do not append** — see finding 3. |
| [`demo-BUGS.md`](demo-BUGS.md) | `src/hoarfrost-demo/BUGS.md` (340 lines) | `src/hoarfrost-demo/BUGS.md` | New file, verbatim. |
| [`demo-PLAN.md`](demo-PLAN.md) | `src/hoarfrost-demo/PLAN.md` (198 lines) | `src/hoarfrost-demo/PLAN.md` | New file, verbatim **except** constraints C1/C2 — see finding 4. |
| [`demo-font_fallback.diff.md`](demo-font_fallback.diff.md) | diff of `src/hoarfrost-demo/src/font_fallback.rs` | — | **No action: already upstream.** Regression record only. |
| [`demo-README.diff.md`](demo-README.diff.md) | diff of `src/hoarfrost-demo/README.md` | `src/hoarfrost-demo/README.md` | Restore the trailing `## Bugs found` section once `BUGS.md` lands. |

All three copies are byte-identical to their sources (verified with
`git hash-object`, see §6).

## DO NOT PORT: the demo's `[workspace]` table

`src/hoarfrost-demo/Cargo.toml:12` carries an empty `[workspace]` table with this
comment above it (lines 9–11):

```toml
# This crate is its OWN cargo workspace. Without the empty `[workspace]` table
# cargo walks up to `D:\ppai2\Cargo.toml` and tries to join the fastmd
# workspace, which this demo is deliberately not part of (PLAN C2).
[workspace]
```

**This must not reach upstream.** It is an escape hatch that exists only because
the demo sits *inside* this repo, directly below the `fastmd` workspace root, and
must not be absorbed by it. Upstream, the demo **is** a real workspace member
(`members = ["src/hoarfrost", "src/hoarfrost-demo"]`), and an empty `[workspace]`
table there would make the member its own workspace root while the root manifest
still lists it — a conflicting graph. [INFERENCE] The exact cargo diagnostic was
not captured, because running cargo is out of scope for this task; the point that
matters is that the table is meaningless upstream and wrong to send.

Verified: the committed upstream `src/hoarfrost-demo/Cargo.toml` at `dd7af16` has
no `[workspace]` table, and that table plus its three comment lines are the
**only** difference between the two manifests. Port `BUGS.md` and `PLAN.md`; leave
the manifest alone.

## Upstream has a demo `SPEC.md` that this repo does not

The file lists differ by exactly five files:

| Direction | Files |
|---|---|
| **Local only** (would be lost) | `src/hoarfrost-demo/BUGS.md`, `src/hoarfrost-demo/PLAN.md`, `src/hoarfrost/AGENTS.md` |
| **Upstream only** (not in this repo) | `src/hoarfrost-demo/SPEC.md`, `src/hoarfrost/Cargo.lock` |

`src/hoarfrost-demo/SPEC.md` (12,649 bytes) is upstream's specification for the
demo; this repo never had one, so there is nothing to port for it — but do not
"restore balance" by pushing a local spec over it. The stray
`src/hoarfrost/Cargo.lock` is likewise upstream-only noise and is not this repo's
concern.

## Findings the plan did not anticipate

These were discovered while executing Phase 0 and each one changes what Phase 0
actually requires.

### 1. The `font_fallback.rs` "fix" is already upstream — nothing to port

Phase 0.1 describes `Path::new(&system_root)` → `PathBuf::from(system_root)`, the
unused `Path` import and the branding wording as local content to **port to**
upstream. Measured against `dd7af16`, **upstream already has all three**; the
local file is the older text. There is no fix to carry. The diff is recorded in
[`demo-font_fallback.diff.md`](demo-font_fallback.diff.md) as a regression record
instead, with the note that the call-site change and the import change are coupled —
applying one without the other leaves an unused-import warning, which the
`clippy -- -D warnings` gate rejects.

### 2. Upstream's demo README already has three dead references

Upstream `src/hoarfrost-demo/README.md` cites `BUGS.md` (lines 167, 206) and
`PLAN.md` (line 274) — files it does not have. Porting the two documents repairs
those links, and the deleted `## Bugs found` section should come back with them
(details in [`demo-README.diff.md`](demo-README.diff.md)).

### 3. Upstream's root `AGENTS.md` already has a partial version of the rules

Upstream `AGENTS.md:50-53` has a `## Hoarfrost Specifics` section containing one
rule, `[HF-001]`, which is a condensed copy of the strings-catalogue isolation
rule, plus a `### Quality Gate` at `:54-60` (workspace-wide cargo commands). So
the component file is **not** missing wholesale — it was condensed. Appending all
25 lines would duplicate `[HF-001]` and add a second, conflicting quality gate.

What is genuinely absent upstream, and is the part worth porting:

* **The re-export / back-compat contract** — define each constant in `strings.rs`
  and re-export it from the module that originally owned it
  (`pub use crate::strings::LABEL_APPLY;` in `widget::color_picker`), so a host
  that referenced the old path does not have to change. This is a promise to
  external consumers and has no upstream equivalent.
* **The definition of "user-facing"** — glyphs used as layout markers, a11y role
  names, token names and gallery fixtures are *not* user-facing and stay with
  their module.
* **The doc-comment rule** — every `pub const` in `strings.rs` names the control
  it labels and cites the `spec.md` section / `UIF-*` ID it implements.
* **The `microcopy/` boundary** — it holds the *rules* for shaping text, not the
  text itself.
* **The two component quality-gate checks** — every cited `UIF-*` exists in
  `spec.md`, and re-exported labels keep their original module path importable.

Suggested shape: extend `## Hoarfrost Specifics` with these as `[HF-002]`… and
leave the existing `### Quality Gate` alone, adding the two `UIF-*`/re-export
checks as bullets under it.

### 4. `demo-PLAN.md` carries constraints that are false upstream

Constraints C1 and C2 of `PLAN.md` are about *this repo*: C1 pins every file to
`D:\ppai2\src\hoarfrost-demo\` and names `src/app/**` as read-only reference; C2
mandates the standalone `[workspace]` table (the very thing §"DO NOT PORT" forbids
porting) and tells the reader to run cargo from `D:\ppai2\src\hoarfrost-demo`.
Porting the file verbatim would ship a hard constraint that contradicts upstream's
own layout. Either reword C1/C2 for the upstream context or add a header note
saying they are historical records of how the demo was built inside `fastmd`.

### 5. Diffing against `D:\hoarfrost`'s working tree gives phantom deltas

`D:\hoarfrost` is a working clone with uncommitted changes, including a modified
`src/hoarfrost-demo/Cargo.toml` that adds a `[lib]`/`[[bin]]` split pointing at an
**untracked** `src/lib.rs`, plus an untracked `src/hoarfrost-gallery/` directory.
A naive `git diff --no-index` against that tree reports a `[lib]`/`[[bin]]` delta
that does not exist upstream. Every diff in this folder was therefore taken
against the **committed `dd7af16` blobs**, and the blob hashes are recorded in
each document so the provenance can be re-checked.

## 6. Provenance and verification

`git hash-object` of each source and its copy — pairs must match:

| Source | Copy | Blob |
|---|---|---|
| `src/hoarfrost/AGENTS.md` | `hoarfrost-AGENTS.md` | `6a4c5b23fd40f240ab86ad2796e50e7a3951e1f4` |
| `src/hoarfrost-demo/BUGS.md` | `demo-BUGS.md` | `6fa5721057c0be07e36691160ada04db6ef5784a` |
| `src/hoarfrost-demo/PLAN.md` | `demo-PLAN.md` | `4a55e3df9f6c21a985c932ef7e330a98bae1f7ce` |

Upstream baselines used for the diffs (`dd7af16dc0d1031adcd5b90e8c789b6bf6af2900`):

| Upstream file | Blob |
|---|---|
| `src/hoarfrost-demo/src/font_fallback.rs` | `c53c046ef7e5cba01180202745bd7cb3fc12e226` |
| `src/hoarfrost-demo/README.md` | `2326ca6bb0332ae01a1713ec007911b5b946cdcd` |

## 7. When this folder can be deleted

After upstream has: (a) merged the missing rules into its root `AGENTS.md`,
(b) taken `src/hoarfrost-demo/BUGS.md` and `src/hoarfrost-demo/PLAN.md` with the
`## Bugs found` README section restored, and (c) confirmed it did **not** receive
a `[workspace]` table. Then Phase 2's `git rm -r src/hoarfrost src/hoarfrost-demo`
loses nothing.
