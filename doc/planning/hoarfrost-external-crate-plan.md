# Plan: consume `hoarfrost` as an external library crate

> **Status:** **Executed** — §9 records what landed against this plan, and §10
> records the same-day switch from the git `rev` pin to the crates.io release,
> which supersedes D1. No file under `src/hoarfrost/` or `src/hoarfrost-demo/` was
> modified to produce this document; every figure was read from the tree or from
> a throwaway scratch crate outside the repo.
> **Date:** 2026-11-01
> **Baseline:** branch `feature/iced-ui` @ `bb810ca`, working tree clean.
> **Objective:** delete the in-repo copy of the framework and consume
> <https://github.com/MartinKuhne/hoarfrost> as a **library dependency** of
> `fastmd` instead of as a workspace member.
> **Upstream baseline surveyed:** `MartinKuhne/hoarfrost` @ `dd7af16`
> ("Merge pull request #1 from MartinKuhne/chore/maintenance", upstream date
> 2026-10-31 (UTC 2026-11-01)).
> **Guardrails:** `RUST-043` (no `SPEC.md` edits), `RUST-042`
> (`ARCHITECTURE_C4.md` must be updated in the same change), `RUST-002`
> (`cargo nextest`).
>
> Every claim below carries a `file:line` citation from the surveyed tree, or is
> marked as a measured result from the scratch-crate probes in §3.

---

## 1. Verified blast radius

| Carrier | Count | Evidence |
|---|---|---|
| Workspace member | 1 | `Cargo.toml:16` `"src/hoarfrost",` |
| Path dependency | 1 | `Cargo.toml:113` `hoarfrost = { path = "src/hoarfrost" }` — **no features enabled** |
| `Cargo.lock` entry | 1 | `Cargo.lock:2219` `name = "hoarfrost"`, no `source` key (workspace member) — regenerated, never hand-edited |
| Tracked framework files | 354 | `git ls-files src/hoarfrost` — includes `spec.md`, `AGENTS.md`, `bin/gallery.rs` |
| Tracked demo files | 52 | `git ls-files src/hoarfrost-demo` — its own cargo workspace (`src/hoarfrost-demo/Cargo.toml:12`), path dep `../hoarfrost` **with `test-helpers`** (`src/hoarfrost-demo/Cargo.toml:15`) |
| App source files mentioning `hoarfrost` | 74 (64 with a `use hoarfrost` line) | `git grep -l hoarfrost -- src/app`; modules imported: `binding`, `bridge`, `composite`, `focus`, `geometry`, `state`, `surface`, `token`, `token_lint`, `validation`, `widget` |
| Cross-crate intra-doc links | 97 | `git grep -n "](hoarfrost::" -- src/app` (first: `src/app/ui/persisted.rs:72`) |
| `UIF-*` citations in app code | 452 (52 distinct) | authority is `src/hoarfrost/spec.md`, which leaves the repo |
| CI carriers | 5 | `.github/workflows/ci.yml:42,76,115,138` (`--workspace --all-features`), `.github/workflows/coverage.yml:38-39` (two `src/hoarfrost` ignore regexes) |
| Doc carriers | 4 | `AGENTS.md:89` (component routing row), `doc/technical-context/ARCHITECTURE_C4.md:514-564+` (Level-3 diagram), `ICED_PRO_REFACTOR_AUDIT.md:143`, `doc/planning/*` (historical records) |

### 1.1 What does **not** break (checked, not assumed)

* **No filesystem dependency on hoarfrost's sources.** The app uses
  `hoarfrost::token_lint` purely as a library:
  `src/app/ui_iced/token_guard_tests.rs:28` imports `redact_comments_and_strings`
  and `source_files`, and calls `source_files(&root)` with the **app's own**
  `ui_iced` root (`token_guard_tests.rs:63-68`, `:728`).
  `src/app/ui_iced/token_lint_gate.rs:61` uses `scan_tree` with an
  app-supplied `Scope`. `crate_scope()`
  (`src/hoarfrost/token_lint.rs:681-687`) returns static metadata only.
  The only function that touches hoarfrost's own tree is `crate_root()`
  (`src/hoarfrost/token_lint.rs:974-976`, `env!("CARGO_MANIFEST_DIR")`), reached
  solely by `audit_crate()` (`:980`) — which no file under `src/app` calls.
* **The app never enables a hoarfrost feature** (`Cargo.toml:113`), so the
  `test-helpers` harness is not part of the app's build today. The one mention
  of it in app code is a comment explaining why it is *not* used
  (`src/app/ui_iced/menu_surface_tests.rs:190`).
* **No app test reads `src/hoarfrost/spec.md`** — `git grep "spec\.md" -- src/app`
  is empty. The `UIF-*` citations are prose, not a parsed contract.
* **No `-p hoarfrost` invocation** exists in CI; every job is `--workspace`.

---

## 2. Upstream state

`MartinKuhne/hoarfrost` @ `dd7af16`, **one branch (`main`), no tags**, not on
crates.io (`GET /api/v1/crates/hoarfrost` → 404).

Root layout: virtual workspace — `members = ["src/hoarfrost", "src/hoarfrost-demo"]`,
`default-members = ["src/hoarfrost"]` — plus its own `.cargo/config.toml`,
`.config/nextest.toml`, `.github/workflows/ci.yml`, `rustfmt.toml`, `LICENSE`,
root `AGENTS.md`, root `Cargo.lock`, and a stray `src/hoarfrost/Cargo.lock`.

### 2.1 Diff local `src/hoarfrost` → upstream `src/hoarfrost`

19 files differ; the content is the local tree **plus** a branding scrub and a
warning fix. Nothing was invented upstream. The four deltas that matter:

| Delta | Where | Consequence |
|---|---|---|
| `DEFAULT_LICENSED` drops `"FastMD"`, adds `"GitHub"` | `src/hoarfrost/microcopy/case_rule.rs:61` | **Public const, behaviour change.** A host label containing `FastMD` now classifies as `WordCase::Mixed` and fails the sentence-case check. No app file references `DEFAULT_LICENSED` today (verified), so this is latent, not live. |
| Gallery guard marker `.fastmd` → `.hoarfrost` | `src/hoarfrost/bin/gallery.rs:39` | The gallery's "never write into a config root" guard stops refusing `.fastmd*` paths. Only affects running the gallery binary, which moves upstream with the crate. |
| `AGENTS.md` deleted upstream | local `src/hoarfrost/AGENTS.md` (25 lines) | The component rules (strings catalogue, re-export contract, quality gate) lose their home. Needs a destination: upstream root `AGENTS.md`, or a `doc/planning` note. |
| `spec_coverage.rs` prose now cites `ui_iced/attributions.rs` | `src/hoarfrost/spec_coverage.rs:2347-2351` | Prose only; upstream already describes the *renamed* host file, which is ahead of this repo's wording in places. |

### 2.2 Diff local `src/hoarfrost-demo` → upstream `src/hoarfrost-demo`

3 files differ (`Cargo.toml` — local adds the `[workspace]` escape hatch,
`README.md`, `src/font_fallback.rs`), local-only `BUGS.md` + `PLAN.md`,
upstream-only `SPEC.md`. The demo is **upstream's crate**; keeping a second copy
here after the switch would be a fork with a dangling `../hoarfrost` path.

**Direction note (corrected after Phase 0).** "Differ" is not "local is ahead".
For `src/font_fallback.rs` the upstream blob is the **newer** text and the local
copy is the older one — see Phase 0.1. The `README.md` delta runs the other way:
upstream cites `BUGS.md` and `PLAN.md` that it does not have.

---

## 3. Measured feasibility (scratch crates, outside the repo)

1. **A git dependency on a package that lives in a repo subdirectory resolves.**
   A scratch crate with `hoarfrost = { git = "...", rev = "dd7af16..." }` produced:
   `source = "git+https://github.com/MartinKuhne/hoarfrost?rev=dd7af16...#dd7af16..."`,
   `version = "0.2.0"`, deps `iced`, `iced_futures`, `iced_renderer`, `iced_runtime`.
   Cargo walks the fetched repo's virtual workspace and finds the member. No
   upstream restructuring is required.
2. **`cargo doc --no-deps` stays warning-free, but cross-crate links go dead.**
   A probe item documented as
   ``[`Palette`](hoarfrost::token::color::Palette)`` built with **0 warnings**,
   and rendered as `<a href="hoarfrost::token::color::Palette">Palette</a>` — a
   relative link to a page that does not exist. Today `cargo doc --workspace
   --all-features --no-deps` documents hoarfrost as a member, so the repo's 97
   links resolve. After the switch the gate still passes and the links silently
   rot. **Deferred by decision (§6, D3)** — not a blocker for the switch.
3. **`[patch]` works for the dev loop.** Adding
   `[patch."https://github.com/MartinKuhne/hoarfrost"] hoarfrost = { path = "..." }`
   to the scratch crate made cargo re-resolve to the local checkout
   (`Adding hoarfrost v0.2.0 (...\hoarfrost-upstream\src\hoarfrost)`).
   The equivalent `--config` one-liner is quoting-hostile under PowerShell
   (the inner quotes are stripped before cargo sees them), so the manifest block
   or a user-level `paths` override is the documented route.

---

## 4. Consumption mechanism — options

| Option | How | Verdict |
|---|---|---|
| **A. Git dependency, pinned by `rev`** | `hoarfrost = { git = "https://github.com/MartinKuhne/hoarfrost", rev = "dd7af16..." }` | **Chosen.** Zero duplicated source, exact reproducibility via the pinned commit + `Cargo.lock`, works today with no upstream work (§3.1). Costs: builds need network access to GitHub; `cargo publish` of `fastmd` is impossible while the dep is a git source (deferred, §6 D4); and a bump is a SHA edit rather than a readable version. |
| A′. Same, pinned by `tag` | `tag = "v0.2.0"` | Strictly nicer to read, but it costs an upstream tag that does not exist yet. Rejected for now (§6, D1); switching `rev =` to `tag =` later is a one-line change with no other effect. |
| B. Git submodule at `src/hoarfrost` | keep `path = "src/hoarfrost"`, make the directory a submodule | Keeps `--workspace` builds and the path dep untouched, but the framework is *still* a workspace member — it does not achieve "library crate". Submodule friction (clone `--recursive`, forgotten `git submodule update`) for no gain. |
| C. Vendored copy + sync script | keep a copy, sync from upstream | This is the status quo with extra steps; the copy drifts. Rejected. |
| D. Publish to crates.io | `hoarfrost = "0.2"` | The right end state if `fastmd` is ever published or if offline builds matter. **Deferred by decision (§6, D4).** |

**Decision:** Option A, `rev`-pinned at `dd7af16dc0d1031adcd5b90e8c789b6bf6af2900`.

---

## 5. Execution phases

Each phase is a separate commit and ends with the quality gate (§7) green.

### Phase 0 — carry the local-only content upstream

Nothing here blocks Phase 1 (the `rev` pin needs no upstream work), but items 1
and 2 are **hard preconditions for Phase 2**, because they are the only content
that exists here and not upstream. Deleting first loses them.

1. **Demo, local-only files:** `src/hoarfrost-demo/BUGS.md` and
   `src/hoarfrost-demo/PLAN.md`. Port to `MartinKuhne/hoarfrost`. Do **not**
   port the demo's `[workspace]` table (`src/hoarfrost-demo/Cargo.toml:12`) —
   that is the escape
   hatch that keeps it out of *this* repo's workspace; upstream it is a member.
   Upstream also has a `SPEC.md` for the demo that this repo does not.
   **Correction (Phase 0, 2026-11-01) — `font_fallback.rs` is NOT port content.**
   This item originally listed the `src/hoarfrost-demo/src/font_fallback.rs` fix
   (`Path::new(&system_root)` → `PathBuf::from(system_root)`, plus the branding
   wording) as local content to carry upstream. The direction is **backwards**:
   measured against `dd7af16`, upstream already contains all three changes — the
   committed blob `c53c046ef7e5cba01180202745bd7cb3fc12e226` has
   `use std::path::PathBuf;` (no unused `Path`), `PathBuf::from(system_root)` and
   the branding scrub — and the local copy was the *older* text. **Already
   upstream, nothing to port**; the deletion in Phase 2 loses nothing here. The
   diff was kept as a regression record in
   `doc/planning/hoarfrost-upstream-port/demo-font_fallback.diff.md`, with the
   note that the call-site and import changes are coupled. That staging folder
   was removed on 2026-11-01 (see §10); the file is recoverable from git
   history at `b649ce3`.
2. **Component agent rules:** the 25 lines of `src/hoarfrost/AGENTS.md`
   (strings-catalogue isolation, the re-export-keeps-the-old-path contract, the
   quality gate) exist only here. **Correction (Phase 0, 2026-11-01) — this is a
   merge, not a rehome.** Upstream did not delete the rules wholesale: its root
   `AGENTS.md` already carries a condensed `## Hoarfrost Specifics` section with
   rule `[HF-001]` (the strings-catalogue isolation rule) and its own
   `### Quality Gate`. Appending the 25 lines would duplicate `[HF-001]` and add
   a second, conflicting gate, so the work is to **merge** them into that section.
   The genuinely missing piece — the real port target — is the **re-export /
   back-compat contract** (define each label in `strings.rs` and re-export it from
   the module that originally owned it, so a host keeps the old path), together
   with the definition of "user-facing", the `strings.rs` doc-comment rule, the
   `microcopy/` boundary and the two component gate checks (every cited `UIF-*`
   exists in `spec.md`; re-exported labels stay importable at their original
   path). Details in the port staging README,
   `doc/planning/hoarfrost-upstream-port/README.md` finding 3 — that folder was
   removed on 2026-11-01 (see §10) and is recoverable from git history at
   `b649ce3`.
3. **Accepted as-is, no action:** the `DEFAULT_LICENSED` swap (§2.1) is an
   intentional upstream decision and no `src/app` file reads that const; the
   gallery guard now refuses `.hoarfrost*` rather than `.fastmd*`, which only
   affects running the gallery binary — and that binary leaves this repo in
   Phase 2 anyway.

### Phase 1 — switch the dependency (local, one commit)

1. `Cargo.toml:16`: delete the `"src/hoarfrost",` member.
2. `Cargo.toml:113`: replace the path dep with

   ```toml
   hoarfrost = { git = "https://github.com/MartinKuhne/hoarfrost", rev = "dd7af16dc0d1031adcd5b90e8c789b6bf6af2900" }
   ```

   and rewrite the comment above it — it currently explains the INT-1 token
   ownership in terms of the local directory. The replacement comment MUST name
   the pinned commit's upstream date (2026-10-31 (UTC 2026-11-01) — the commit's
   own author/committer date is 2026-10-31, which is already 2026-11-01 in UTC)
   and state that **a bump is an
   edit to this `rev`**, since a SHA pin carries no readable version and
   `cargo update` will not move it on its own.
3. Let cargo regenerate `Cargo.lock` (`cargo update -p hoarfrost` or any build).
   Never hand-edit it. The entry gains
   `source = "git+https://github.com/MartinKuhne/hoarfrost?rev=dd7af16…#dd7af16…"`.
4. `cargo check --workspace --all-features`. Expect the only compile-level risk to
   be the §2.1 `DEFAULT_LICENSED` change, which no app file uses.

### Phase 2 — remove the local copies

1. `git rm -r src/hoarfrost` (354 files) — after Phase 0.2 has merged the
   component `AGENTS.md` rules into upstream's root `AGENTS.md`.
2. `git rm -r src/hoarfrost-demo` (52 files) — **decided: it leaves with the
   framework** (§6, D2). Its `path = "../hoarfrost"` dep cannot survive the
   deletion, and upstream already owns this crate. Precondition: Phase 0.1 has
   ported `BUGS.md` and `PLAN.md` (the `font_fallback.rs` change needs no port —
   it was already upstream, see Phase 0.1).
3. Confirm nothing else referenced them: `git grep -n "src/hoarfrost"` should
   return only historical `doc/planning/*` records and `ICED_PRO_REFACTOR_AUDIT.md`.
4. Note for the commit message: hoarfrost's git history stays in this repo's past
   commits; upstream history is a squashed copy ("add the code"), so
   `git log -- src/hoarfrost` remains the only annotated history of that work.

### Phase 3 — the two-repo development loop

Framework work does not stop — `src/app` is its only real consumer, so the loop
must be documented, not improvised:

1. Clone alongside: `git clone https://github.com/MartinKuhne/hoarfrost ../hoarfrost`.
2. Build against the working copy by adding, **locally and uncommitted**,
   `[patch."https://github.com/MartinKuhne/hoarfrost"]` +
   `hoarfrost = { path = "../hoarfrost/src/hoarfrost" }` to the root
   `Cargo.toml` (verified §3.3). A user-level `paths = [...]` in
   `%USERPROFILE%\.cargo\config.toml` is the commit-free alternative.
3. Land hoarfrost changes upstream, then bump the `rev` in `Cargo.toml:113` to the
   new commit SHA. Until the bump, `cargo nextest run --workspace` in this repo
   tests the *pinned* framework, not the working copy — say so in the PR body of
   any cross-repo change. Because a `rev` carries no version, the bump commit
   message MUST name what moved (old → new SHA, upstream PR/commit subject).
4. Hoarfrost's own CI (`.github/workflows/ci.yml` upstream: check, nextest,
   clippy, doc) is now the only place its test suite runs.

### Phase 4 — CI and tooling

1. `.github/workflows/coverage.yml:38-39`: delete both `src/hoarfrost` ignore
   regexes (dead paths).
2. Re-baseline the coverage number: hoarfrost's ~350 files leave the
   `--workspace` denominator, so the reported percentage moves for a reason that
   has nothing to do with the app. Record the before/after in the PR.
3. `ci.yml` needs no edit — every job is `--workspace`, and hoarfrost simply
   stops being part of it. Confirm `cargo clippy --workspace --all-features
   -- -D warnings` no longer lints hoarfrost (dependencies are not linted), and
   that the app's own gates that *call into* hoarfrost still pass:
   `src/app/ui_iced/token_lint_gate.rs` and `token_guard_tests.rs`.
4. Confirm CI can reach GitHub over HTTPS — a git dependency is fetched at build
   time, so a firewalled or air-gapped runner fails to resolve the graph at all.
   Whether `Swatinem/rust-cache@v2` restores `~/.cargo/git` (where git
   checkouts live) is unverified; the first CI run after the switch is the test,
   and a cold fetch of this one repo is cheap either way.

### Phase 5 — documentation

1. `AGENTS.md:89`: drop the `src/hoarfrost` routing row (the file it links to is
   gone). If hoarfrost-specific rules still need to be visible to agents working
   on `src/app`, keep a short "external framework" note pointing at upstream.
2. `doc/technical-context/ARCHITECTURE_C4.md` (`RUST-042`, mandatory in the same
   change): the Level-3 `hoarfrost` diagram at `:514-564` describes an
   in-repo crate. Retitle it as an **external container**, replace
   "`src/hoarfrost/spec.md`" with the upstream URL, and keep the *integration*
   contract prose — the ownership split, D-2/D-8 decisions and the
   "no `fastmd` type appears in this API" statement are still true and still
   belong here. Per `doc/technical-context/AGENTS.md`, every diagram node must
   map to a real directory, so the crate's internals move out of the component
   diagrams and into a container-level node.
3. `UIF-*` traceability: 452 citations in `src/app` now point at a spec that
   lives in another repo. Add one sentence to `AGENTS.md` (spec-traceability
   section) naming the upstream `spec.md` as the authority for `UIF-*`.
4. Historical records — `doc/planning/hoarfrost-tasks.md`,
   `hoarfrost-handoff.md`, `hoarfrost-rename-plan.md`, `ui-defects-ledger.md`,
   `ICED_PRO_REFACTOR_AUDIT.md` — contain now-dead `src/hoarfrost/...` links.
   They are dated design records; add a one-line banner at the top of each
   ("paths refer to the pre-externalisation tree") rather than rewriting them.
5. The 97 cross-crate doc links: **deferred (D3)** — no action in this change.
   Record the measured behaviour (§3.2) in the PR body so the dead links are a
   known state rather than a discovered one.

### Phase 6 — verification

See §7.

---

## 6. Decisions

| # | Decision | Consequence |
|---|---|---|
| **D1** | Pin by **`rev`**, not `tag`. | No upstream work gates Phase 1. Pinned at `dd7af16dc0d1031adcd5b90e8c789b6bf6af2900`. A bump is a SHA edit that must be explained in the commit message, and `cargo update` will not move it on its own. Upgrading to `tag =` later is a one-line change. |
| **D2** | `src/hoarfrost-demo` **leaves with the framework**. | Verified safe: `git grep -n "hoarfrost-demo" -- ':!src/hoarfrost-demo'` is empty — no CI job, no cargo alias, no doc references it. Its local-only content must be ported upstream first (Phase 0.1), and the `[workspace]` escape hatch must **not** be ported with it. |
| **D3** | The 97 `](hoarfrost::…)` doc links are **not handled in this change**. | The doc gate stays green while the links stop resolving (§3.2). Accepted debt, revisited once hoarfrost has published docs. |
| **D4** | Publishing `fastmd` to crates.io is **not a near-term goal**. | The git dependency is therefore not a blocker. If publishing ever returns, hoarfrost must be on crates.io first (Option D) or the release recipe needs `cargo vendor`. |
| **D5** | Offline / air-gapped builds are not a requirement. | CI must reach GitHub over HTTPS at build time (Phase 4.4); `cargo vendor` is not part of any recipe today. |

---

## 7. Quality gate (every phase)

From the repo root:

```
cargo check --workspace --quiet
cargo nextest run --status-level fail --show-progress none
cargo clippy --workspace --all-features -- -D warnings
cargo fmt --check
cargo doc --workspace --all-features --no-deps --quiet
```

Plus, once in Phase 1/2:

* `cargo tree -p hoarfrost` shows the `rev` pin of D1 — and no path source:

  ```
  hoarfrost v0.2.0 (https://github.com/MartinKuhne/hoarfrost?rev=dd7af16dc0d1031adcd5b90e8c789b6bf6af2900#dd7af16d)
  ```

  This wording is corrected: the plan originally expected
  `hoarfrost v0.2.0 (git+…?tag=v0.2.0)`, which contradicts D1 (pin by `rev`, not
  `tag`) and could never have matched, because upstream has no tags (§2). The
  output above is what was actually observed after Phase 1.
* `git grep -n "src/hoarfrost"` returns only historical records:
  `doc/planning/*` and `ICED_PRO_REFACTOR_AUDIT.md`. No build, CI or source file
  may still name the path.
* `cargo nextest run -p fastmd token_lint` / `token_guard` suites pass — these
  are the app-side gates that reach into the framework's lint engine.
* A clean `target/`-less build (`cargo clean && cargo check`) proves the git
  checkout is fetched from scratch, not served by a stale local path.

---

## 8. Explicitly out of scope

* Any edit to `src/hoarfrost/**` or `src/hoarfrost-demo/**` (this document was
  written without touching them).
* Any `SPEC.md` change (`RUST-043`).
* Migrating app code onto newer framework APIs — the switch is source-identical
  apart from the §2.1 deltas.
* Publishing to crates.io (Option D).

---

## 9. Execution record — 2026-11-01

What was actually executed against this plan, and where reality diverged from it.

| Phase | Status | What landed |
|---|---|---|
| **0 — carry local content upstream** | **staged, then un-staged; NOT applied upstream** | The port content was assembled into `doc/planning/hoarfrost-upstream-port/` (verbatim copies + diff records + findings). No upstream commit was ever made from it. The staging folder was **removed on 2026-11-01**: its six files are recoverable from git history at `b649ce3`. Checked against the local clone at `c1ffee9` that day — upstream `AGENTS.md` already carries a condensed `## Hoarfrost Specifics` (`[HF-001]`), while `src/hoarfrost-demo/BUGS.md`, `src/hoarfrost-demo/PLAN.md` and the demo README's `## Bugs found` section are still missing upstream. |
| **1 — switch the dependency** | done | `Cargo.toml`: dropped the `"src/hoarfrost"` workspace member and replaced the path dep with the `rev`-pinned git dep (D1); `Cargo.lock` regenerated by cargo, never hand-edited. |
| **2 — remove the local copies** | done | Deleted **354** tracked files under `src/hoarfrost/` and **52** under `src/hoarfrost-demo/`. |
| **3 — two-repo loop** | documented | Recorded in `AGENTS.md` (external-framework section): clone alongside, `[patch]` block locally and uncommitted, bump = edit the `rev`. |
| **4 — CI and tooling** | done | `.github/workflows/coverage.yml`: removed both now-dead `src/hoarfrost` ignore regexes. |
| **5 — documentation** | done | `AGENTS.md` rewritten (routing row dropped, external-framework section added, `UIF-*` authority named); `doc/technical-context/ARCHITECTURE_C4.md` rewritten so `hoarfrost` is an external container and every remaining diagram node maps to a real `src/app/` directory or module. Historical records carry a "paths refer to the pre-externalisation tree" banner. |

**Quality gate after the change** — passes in full: `cargo check`, `cargo nextest
run --workspace` (**4567 tests**), `cargo clippy -- -D warnings`,
`cargo fmt --check`, `cargo doc --no-deps`.

**Unplanned breakage found and fixed.**
`src/app/ui/attributions_tests.rs::completeness_against_cargo_manifests` read the
framework's `Cargo.toml` **at test time** to collect its expected third-party
dependencies, so Phase 2 broke it even though nothing failed to compile. §1.1
checked for a filesystem dependency on hoarfrost's sources and found none in the
app's *runtime* paths — it did not catch a test that parses a manifest. The test
now treats `hoarfrost` as an external git dependency instead of scanning a
manifest that is no longer in the repo.

**Known state after the switch (accepted debt, D3).** The 97 cross-crate
intra-doc links (`](hoarfrost::…)`) in `src/app` now render as **dead anchors**
and rustdoc emits **no warning** for them — measured: 43 generated HTML files in
`target/doc` contain `href="hoarfrost::…"`. The `cargo doc` gate staying green is
therefore not evidence that these links work.

**Stale figure in §1.** The `UIF-*` row of §1 records "452 citations (52
distinct)" as measured at planning time. That count no longer reproduces exactly
(450 matches over 51 distinct ids across `src/app/**/*.rs` at the time of this
record), which is why `AGENTS.md` now states the claim without an exact count.

---

## 10. Execution record — superseded by the crates.io release, same day

D1 was **"pin by `rev`, not `tag`"** because upstream had one branch and no tags,
and because the crate was not on crates.io. Both premises stopped being true on
2026-11-01: `hoarfrost` **0.14.0** was published to crates.io from
`MartinKuhne/hoarfrost`, so the dependency moved from a git `rev` to a registry
version. This section records that switch; §1–§8 above are kept as the design
record of the externalisation itself, and their `dd7af16` citations are the
baseline they were measured against, not current state.

**Verified before switching** (the name collision was the risk, not the API):

| Check | Result |
|---|---|
| Publisher / repository | `MartinKuhne`, repository `https://github.com/MartinKuhne/hoarfrost`, MIT, edition 2024 — the same crate, not a name squat |
| Version jump | `0.2.0` → `0.14.0` is a deliberate renumber to track iced's version; it is not 12 skipped releases |
| Content delta vs `dd7af16` | **26 of 351** `.rs` files differ, essentially all `#[cfg(feature = …)]` gating; **2 files added** (`composite/icon.rs` + sidecar); **nothing removed** |
| Ungated modules the app depends on | `token`, `token_lint`, `surface`, `geometry`, `invariant_gate` carry no feature gate, so the app's token gate still compiles |
| `default` feature | **`default = []`** — a bare `hoarfrost = "0.14.0"` compiles **zero widgets** |

**What changed.**

- `Cargo.toml`: `hoarfrost = { version = "0.14.0", features = [ … ] }`. The
  `features` list is **required**, not cosmetic: with `default = []`, omitting it
  silently compiles no widgets. The list is the **explicit set this app imports**,
  not `all` — derived by sweeping the `hoarfrost::` imports of `src/app`, then
  corrected by the compiler, which caught a grouped `use hoarfrost::composite::{
  form_field, list_tile}` that the text sweep had missed. `form-field` is in the
  list because of that import and nothing else.
- `test-helpers` is requested through `[dev-dependencies]`, not the normal
  dependency, so `hoarfrost::test_helpers` compiles for test targets only and
  never reaches the shipped binary. `menu_surface_tests.rs` then dropped its
  private `shoot()` — 38 lines of headless renderer assembly — in favour of
  `test_helpers::screenshot`, which builds the renderer identically (Fira Sans,
  16 px, `None`). The swap is covered by
  `test_the_app_menu_card_paints_every_row_inside_its_own_rect`, which fails if
  the frame comes back blank, so a harness that stopped painting could not pass
  for the wrong reason.
- `Cargo.lock`: regenerated by cargo, never hand-edited. `hoarfrost` now carries
  `source = "registry+…"` and a checksum instead of a git URL; the resolved
  dependency graph is otherwise unchanged.
- `AGENTS.md`: the external-framework section now describes a registry dep — bump
  by editing the version, `cargo update -p hoarfrost` moves within the
  requirement, and the two-repo loop uses `[patch.crates-io]` instead of
  `[patch."https://github.com/MartinKuhne/hoarfrost"]`. Added the caveat that a
  patch override is only accepted while the clone's `version` still satisfies the
  requirement.
- `ARCHITECTURE_C4.md`: external container re-described as a crates.io
  dependency at `0.14.0`; the build-shape paragraph now states the feature-gating
  contract.
- `src/app/ui/attributions_tests.rs`: **the derivation rule was wrong, not just
  its data.** It selected lock packages whose `source` began with `"git+"`, so
  the moment the framework became registry-sourced it derived an **empty** set and
  three tests failed. The rule now keys on the package **name**
  (`external_framework_dependency_names(lock, package)` with a `FRAMEWORK_PACKAGE`
  constant), because the attribution obligation is identical whichever source the
  framework comes from. A source-shaped rule was the bug; the source switch is
  what exposed it. New anti-vacuity test
  `lock_derivation_ignores_the_source_scheme` pins that the same package yields
  the same dependency set from a `git+` block and a registry block.
  Mutation-proved: breaking the parser's name match fails four tests, including
  `completeness_against_cargo_manifests`, which is a **two-way** set equality and
  therefore fires when the derived set shrinks as well as grows.

**Newly possible, and taken.** The published crate ships its headless capture
harness behind `test-helpers`, so the duplicated `shoot()` is gone (above). What
was deliberately **not** folded in: this file's `PIXEL_SCALE`, `INK_THRESHOLD` and
`ink_in_rect()`. Those are the thresholds the app's own assertions are written
against, and pinning them locally means an upstream retune cannot silently move a
green test. Upstream exports equivalents; using them would couple this suite to a
number it does not own.

**Quality gate after this switch** — passes in full: `cargo check`, `cargo nextest
run --workspace --all-features`, `cargo clippy --workspace --all-features
--all-targets -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps`.

### Follow-ups on the same branch

* **Dead cross-crate doc links.** 96 links of the form `[label](hoarfrost::path::Item)`
  across 24 files rendered as dead `href="hoarfrost::..."` anchors (82 in the
  generated HTML). rustdoc never warns about an unresolved cross-crate link - it has
  no doc JSON for a registry dependency to resolve against - so a clean `cargo doc`
  was never evidence they worked. All of them, plus one reference-style definition in
  `panels/center.rs`, are now plain code spans naming the full path.
* **Vacuous framework-parity assertions.** An `assert_eq!(APP_CONST, token::X)` where
  `APP_CONST` is *defined as* `token::X` cannot fail: both sides are the same token
  and a framework bump moves them together. **65** such assertions were deleted
  across `layout_tests`, `right_tests`, `menu_surface_tests`, `top_tests`,
  `bottom_tests`, `center_tests`, `left_tests`, `token_guard_tests`,
  `view_tests` and `hamburger_tests`. Kept: literal pins (`== 10.0`, `== 36.0`,
  `!= 28.0`), derived-value checks, measured widget bounds, and genuine cross-token
  checks (`ROW_INDENT_PER_DEPTH == space::LG`, `ROW_MIN_HEIGHT == size::HIT_TARGET_MIN`,
  `scroll_view::TRACK_WIDTH_RESERVED == size::SPLITTER_GRAB`). Test names and doc
  comments that claimed framework parity now describe what remains.
* `doc/planning/hoarfrost-upstream-port/` **deleted.** The Phase 0 staging folder was
  never applied upstream. Its content is recoverable from git history at `b649ce3`,
  and the three files it existed to protect (`src/hoarfrost/AGENTS.md`,
  `src/hoarfrost-demo/BUGS.md`, `src/hoarfrost-demo/PLAN.md`) remain in this repo's
  history from before the Phase 2 deletion. The three references above were rewritten
  to point at history rather than a deleted path.
