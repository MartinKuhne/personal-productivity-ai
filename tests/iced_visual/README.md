# Tier 3 / tier 4 — iced visual tests ([TASK-025])

Tier 1 is the per-module `*_tests.rs` sidecar set (pure model). Tier 2 is
`src/app/ui_iced/interaction_tests.rs` (headless `iced_test::Simulator`
driving the real view + reducer and asserting **messages**).

This directory holds the artifacts for the two tiers that need a **real
recording pass by a human**:

| Tier | What it asserts | Tool | Artifacts |
|---|---|---|---|
| 3 | pixels do not drift | `iced_test::simulator(..).snapshot(&theme)` → `Snapshot::matches_hash` / `matches_image` | `baselines/*` |
| 4 | a recorded user session still replays | `iced_test::run(program, "…/ice")` | `ice/*.ice` |

The harness itself is `tests/iced_visual_harness.rs`.

## Directory layout

```text
tests/
  iced_visual_harness.rs    Program adapter + tier-3/4 entry points
  iced_visual/
    README.md               this file
    baselines/              tier-3 baselines  (app_view.sha256, top_band.sha256, …)
    ice/                    tier-4 recorded sessions (*.ice)
```

`baselines/` and `ice/` ship empty (`.gitkeep` only). **No binary baseline
is committed by TASK-025** because none was recorded — the recording tests
are `#[ignore]`d until someone runs a recording pass.

## The `Program` adapter

`iced_test::screenshot` and `iced_test::run` require a
`iced_test::program::Program`, but FastMD is built from the free functions
`build_state` / `update` / `view` plus `window_settings`
(`src/app/ui_iced/app.rs`). `FastMdProgram` in the harness adapts those
four directly, so the harness renders exactly what `launch()` renders.

It uses `iced_test::futures::backend::null::Executor`, so **no
subscription runs** during a recording: no network, no file watcher, no
real user configuration. State comes from `build_state` over
`InMemoryConfigStorage` + `InMemoryUiStateStorage` ([RUST-006]).

## Recording tier-3 baselines

Baselines are **sha256 of the rendered RGBA frame** (small, diff-friendly).
`Snapshot::matches_hash` *creates* a missing baseline and returns `true`,
which would silently turn "no baseline" into "pass" — so the harness gates
recording behind an env var and asserts the file exists before comparing.

```powershell
# 1. record (or refresh) every baseline
$env:FASTMD_RECORD_ICED_BASELINES = "1"
cargo test -p fastmd --test iced_visual_harness -- --ignored

# 2. review the new .sha256 files in the diff, commit them
Remove-Item Env:FASTMD_RECORD_ICED_BASELINES

# 3. from now on the same tests assert against the committed hashes
cargo nextest run -p fastmd --test iced_visual_harness -- --ignored
```

Recording entry points currently defined:

| Test | Subject |
|---|---|
| `test_record_app_view_baseline` | the whole `view(&AppState)` |
| `test_record_top_band_baseline` | `top_bar(&state.models.top)` |

Add a new baseline by copying one of those two and calling
`compare_or_record(&snapshot, "<name>")`.

### Baseline caveats (read before trusting a green tier 3)

* The hash is **renderer-specific**: `Snapshot` writes
  `<name>-<renderer>.sha256`. A machine that falls back from `wgpu` to
  `tiny-skia` produces a different file and will not match. Pin the
  backend with `$env:ICED_TEST_BACKEND = "tiny-skia"` for reproducible,
  GPU-free baselines.
* Any change to fonts, theme tokens or layout changes every baseline. That
  is expected; re-record deliberately, never reflexively.
* Tier 3 is a *drift detector*, not a correctness proof. Tier 2 is where
  behaviour is asserted.

## Recording tier-4 `.ice` sessions

`.ice` files are recorded interaction scripts that `iced_test::run`
replays against the real program.

Recording requires iced's `tester` feature (F12 opens the recorder):

```toml
# Cargo.toml — TEMPORARY, only while recording. Do not leave enabled.
iced = { version = "0.14.0", features = [ "...", "tester" ] }
```

1. Add `tester` to the `iced` feature list.
2. Run the app under the harness program (`FastMdProgram`) rather than the
   production `launch()`, so the session records against in-memory state.
3. Press **F12**, perform the interaction, save the `.ice` file.
4. Drop the file in `tests/iced_visual/ice/`.
5. Remove the `tester` feature again.

`test_ice_replay_directory_is_wired` already executes **every** `.ice`
file in that directory, so recorded sessions start being enforced the
moment they are committed — no harness change needed.

> **TASK-025 did not enable `tester`.** It was not needed to stand the
> harness up, and leaving a recording feature compiled into the shipped
> `iced` dependency is a cost the task deliberately avoided. Recording a
> session is therefore still a manual step that needs a short-lived
> manifest change.

## Why tiers 3 and 4 are skeletons

* A tier-3 baseline is only meaningful once a human has looked at the
  rendered frame and agreed it is correct. TASK-025 cannot make that call,
  and committing an unreviewed hash is worse than committing none.
* A tier-4 `.ice` file cannot be produced headlessly — the recorder is an
  interactive overlay in a real window.

Both tiers are wired, compile, and run; they just have no artifacts yet.
