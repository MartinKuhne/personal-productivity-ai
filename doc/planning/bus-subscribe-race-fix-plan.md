# Plan: Fix `test_bus_subscribers_see_discovered_events` race

Status: proposal
Date: 2026-07-27
Owner: Mavis (Mavis)

## Symptom

```
---- background_task::tests::test_bus_subscribers_see_discovered_events stdout ----

thread 'background_task::tests::test_bus_subscribers_see_discovered_events' (3818)
panicked at src/background_task.rs:276:9:
assertion `left == right` failed
  left: 0
 right: 1
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

test result: FAILED. 689 passed; 1 failed; 3 ignored; 0 measured;
              0 filtered out; finished in 4.56s
```

The failing assertion is `assert_eq!(tag_events.len(), 1);` at line 276 of
`src/desktop/src/background_task.rs` inside the
`test_bus_subscribers_see_discovered_events` test. The test expects exactly one
`Discovered` event for `a.md`, but the reader drained zero events.

## Root cause — `tokio::sync::broadcast` late-subscriber race

The test does:

```rust
let task = Task::new(config);                // (1) spawns indexer thread
let tag_reader = task.file_event_bus.subscribe();   // (2)
let tree_reader = task.file_event_bus.subscribe();  // (3)
// ... wait for Finished, then drain ...
```

`Task::new` (in `src/background_task.rs:25-49`) immediately spawns a thread
that calls `Indexer::scan_libraries`, which walks the library tree and calls
`Bus::publish` via `flush_batch` (`src/background/indexer.rs:103-180`). The bus
is a `tokio::sync::broadcast` channel (`src/background/file_events.rs:55-90`).

A new `subscribe()` from a `tokio::sync::broadcast` receiver only sees events
sent **after** it subscribed. Events sent before the subscription are silently
dropped. The race window:

| step | thread A (test)              | thread B (indexer)            |
| ---- | ---------------------------- | ----------------------------- |
| 1    | `Task::new` returns          | starts `scan_libraries`       |
| 2    | `subscribe()` ←─ races here  | walks dir, calls `publish`    |
| 3    |                              | publishes `Discovered{a.md}`  |

For a 1-file tempdir, the walk + publish in thread B takes a few hundred
microseconds to a few milliseconds. If the test thread is preempted between
`Task::new` returning and the first `subscribe()` (e.g. by another test thread
holding the OS scheduler under load), thread B completes the publish first and
the receiver arrives too late.

**Why the analogous 3-file test (`test_initial_scan_publishes_discovered_events`)
typically passes:** walking three files adds enough work that the publish is
pushed just past the test's `subscribe()` calls *most of the time*, but it is
the same race and will also fail intermittently under heavier load. The fact
that only the 1-file variant failed in the reported run is timing luck.

**Reproduction in this environment:** running `cargo test --lib
background_task::tests::test_bus_subscribers_see_discovered_events --test-threads=1`
five times in a row passes every time; running the full suite five times in a
row also passes here. The race window is real but small; on the CI machine
that produced the failure, scheduler pressure from the surrounding 689 tests
moved the test thread past its `subscribe` calls only after the indexer had
already published.

The same race exists in production. `src/ui/app.rs:380-381` and the existing
test scaffolding subscribe to the bus **after** `Task::new`, so the UI is
potentially missing the first batch of `Discovered` events on startup. This
plan addresses the test directly and is structured so the production call
sites can adopt the same fix without re-doing the work.

## Fix options

### Option A — Pre-built bus (recommended)

Make the bus injectable so the consumer can subscribe before any thread is
spawned.

1. Add a second constructor:

   ```rust
   impl Task {
       pub fn new(config: AppConfig) -> Self {
           let bus = Bus::new();
           Self::new_with_bus(config, bus)
       }

       pub fn new_with_bus(config: AppConfig, file_event_bus: Bus<FileEvent>) -> Self {
           // ... existing body, but use the supplied bus instead of Bus::new() ...
       }
   }
   ```

2. Rework the failing test (and the 3-file variant) to pre-build and pre-
   subscribe:

   ```rust
   let bus = Bus::<FileEvent>::new();
   let tag_reader = bus.subscribe();
   let tree_reader = bus.subscribe();
   let task = Task::new_with_bus(config, bus);
   ```

   Because the indexer thread isn't spawned until `new_with_bus` constructs the
   `Task`, the `subscribe()` calls in the test are guaranteed to land first.
   The race disappears entirely.

3. Migrate `src/ui/app.rs:380-381` to use `new_with_bus` so the production
   `FileEventProcessor` also receives the initial scan events. Document this
   in `doc/technical-context/ARCHITECTURE_C4.md` and add a `REQ-xxx` entry
   in `SPEC.md` capturing the contract: "consumers must subscribe to the
   file-event bus before the first `FileEvent` is published; production
   wires this up via `Task::new_with_bus`."

**Pros**
- Removes the race at the source. No timing assumptions, no sleeps, no
  barriers.
- Same fix improves production (UI no longer misses initial scan events).
- Mechanical change. `Task::new` stays as a thin wrapper, so the public API
  is preserved.
- Total diff: ~30 lines in `background_task.rs`, ~6 lines in the test,
  ~3 lines in `app.rs`, plus doc/SPEC updates.

**Cons**
- Touches three production call sites (the constructor itself + the two
  `subscribe`-after-`Task::new` sites in tests/`app.rs`).

### Option B — Test-only synchronization

Add a barrier or channel the test can use to wait for the indexer to *start*
before subscribing. This would require either:

- exposing a "scan started" hook in the indexer (production change), or
- spawning `Task::new` on a helper thread and using a `std::sync::Barrier`
  to interleave subscribe before publish (works only if the spawned thread
  signals *after* the publish — but the publish is what we want to receive,
  so this becomes a chicken-and-egg problem), or
- polling for the first `FileParsed` message on `task.rx` before subscribing
  to the bus — this only works because the indexer sends `FileParsed` after
  batching the `Discovered` event, but the order of those two sends across
  threads is not guaranteed and would itself need a synchronization point.

**Pros:** keeps `Task::new` unchanged.

**Cons:** still leaves the production race in `ui/app.rs`; introduces a
test-only seam that future readers must puzzle through; the polling
approach is itself a race.

### Option C — Buffer events until first subscribe

Make `Bus` itself retain unpublished events in a small ring buffer and
deliver them to the next `subscribe()` call. This would change the bus
contract and affect every existing test, every call site, and the channel
capacity math (now bounded by time-since-first-subscribe, not buffer size).
It also changes semantics: a subscriber that comes online 30 seconds after
startup would receive a snapshot from 30 seconds ago, which may not be
what consumers want.

**Pros:** fixes the race transparently everywhere; no call-site changes
needed in user code.

**Cons:** largest blast radius; semantic change to the bus; would need a
new `Bus` variant or a new method to opt in, plus a full test rewrite and
SPEC update.

### Option D — Loosen the test

Change the assertion to `>= 0` and document the race. **Rejected.** This
turns a deterministic contract into a probabilistic one and gives future
readers no signal that something is wrong. The test currently exists to
prove the bus wiring works; weakening it to "sometimes works" defeats the
purpose.

## Recommended approach: Option A

The race is real, the fix is small, and the same fix improves the
production wiring. Option A is the only one that:

1. Makes the test deterministic.
2. Removes the same race from production code.
3. Keeps the existing public API (`Task::new`) working.
4. Fits the codebase's bounded-subsystem rules (see `src/desktop/AGENTS.md`
   §5): `Task` is the orchestrator and already owns the bus; exposing a
   `new_with_bus` constructor is a small, well-scoped addition.

## Implementation steps

Following the repo's test-first issue-fixing workflow (`AGENTS.md` §10) and
quality gate (`src/desktop/AGENTS.md` §6).

1. **Reproduce the failure locally (test-first).**
   - Add a `--test-threads=8` (or just let the default parallel runner
     hammer it) loop that runs the full `background_task` test module until
     the failing test is observed.
   - If the race is hard to surface, write a *focused* regression test that
     exercises the same pattern as the failing one and runs it under a
     stress loop; this is the closest deterministic test we can get without
     injecting sleeps. The Option A fix should make both deterministic.

2. **Add `Task::new_with_bus`.**
   - In `src/desktop/src/background_task.rs`, extract the body of
     `Task::new` into `new_with_bus(config, bus)`, and have `new` call
     `new_with_bus(config, Bus::new())`.
   - Update the `//!` module doc to mention the constructor and the
     "subscribe-before-spawn" contract.

3. **Update the failing test and the analogous 3-file test.**
   - `test_bus_subscribers_see_discovered_events` (line 240+): build the
     bus, subscribe twice, then call `Task::new_with_bus(config, bus)`.
   - `test_initial_scan_publishes_discovered_events` (line ~187): same
     change.
   - `test_initial_scan_publishes_pdf_discovered_to_bus` (line ~285): same
     change. (Even though it currently passes, it has the same race.)

4. **Update the production call site.**
   - `src/ui/app.rs:380-381`: build the bus, subscribe for
     `FileEventProcessor`, then call `Task::new_with_bus(config.clone(), bus)`.
     Add a `REQ-xxx` to `SPEC.md` documenting the new constructor and the
     "consumers must subscribe before `new_with_bus`" contract.

5. **Update `ARCHITECTURE_C4.md`.**
   - Add a short note in the `background/` subsystem description that
     `Task` accepts a pre-built bus via `new_with_bus` so consumers can
     subscribe before the indexer thread is spawned.

6. **Quality gate** (from `src/desktop/AGENTS.md` §6):
   - `cargo check` (no warnings)
   - `cargo test` — full suite, three back-to-back runs, all green
   - `cargo clippy -- -D warnings`
   - `cargo fmt --check`
   - `cargo doc --no-deps --quiet`

7. **Regression coverage.**
   - The new test pattern (subscribe *before* spawn) is the regression
     coverage: any future change that moves the spawn back before the
     subscribe will re-introduce the race and the test will fail.

## Risks and follow-ups

- **No expected behavior change for end users.** Initial scan events now
  reach the UI on first frame instead of being silently dropped; the UI
  path is already designed to handle `Discovered` events (`FileEventProcessor`).
  The change should be a strict improvement.
- **Other call sites.** A repo-wide grep for `Task::new` (planned in step
  0.5 of the implementation) will surface any other test or code path that
  uses the old pattern; they should be migrated in the same PR or, at
  minimum, follow-up issues filed.
- **The `Test::cancel` and `test_background_task_indexing` tests** also
  rely on `Task::new` but don't subscribe to the bus, so they are
  unaffected. They will keep using `Task::new`.

## Out of scope

- Changing the bus to a replay-friendly channel (Option C). The race is
  better fixed at the producer/consumer contract boundary.
- Refactoring the indexer to publish events lazily or to flush batches on
  a timer. The current eager batched publish is correct; only the
  subscribe timing needs to change.
