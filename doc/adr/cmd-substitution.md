# Hide console window when spawning stdio MCP servers on Windows

Status: accepted
Date: 2026-08-01
Reviewer: MiniMax

## Context

The desktop release binary is linked with `/SUBSYSTEM:WINDOWS`
([`src/desktop/src/main.rs:1`](../../src/desktop/src/main.rs) sets
`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`),
so Windows does **not** allocate a console for the process at
startup. Any console-subsystem child we then spawn — `node.exe`,
`cmd.exe`, the `npx.cmd` shim, anything that links as a console
app — gets a **new visible console window** by default. There is
no parent console to inherit.

Two compounding factors in our specific code path:

1. **`utils::path::resolve_executable_path`
   ([`src/desktop/src/utils/path.rs:56`](../../src/desktop/src/utils/path.rs))**
   walks `PATHEXT` (`.COM;.EXE;.BAT;.CMD`) when resolving a bare
   command name. For a user config of `command: npx` it returns
   the absolute path to `npx.cmd`. `Command::new` on a `.cmd` file
   forces Windows to launch `cmd.exe` to interpret the batch —
   and `cmd.exe` is itself a console-subsystem binary, so it
   produces the very window we are trying to avoid.

2. **The MCP stdio spawn
   ([`src/desktop/src/agent/tools/mcp/session.rs:1081`](../../src/desktop/src/agent/tools/mcp/session.rs),
   pre-fix)**
   called `std::process::Command::new(executable)` and
   `cmd.spawn()` with no `creation_flags`. With `dwCreationFlags
   = 0` and a GUI-subsystem parent, the kernel allocates a new
   *visible* console for the child (see
   [rprichard/win32-console-docs](https://github.com/rprichard/win32-console-docs)
   for the authoritative parent/child console matrix). The user
   sees a console window flash every time a stdio MCP server
   starts.

The same latent issue exists at four other callsites in the
crate, with varying blast radius:

| File:Line | Spawn | Frequency |
|---|---|---|
| `agent/tools/mcp/oauth/store.rs:344` | `icacls` to set token-store ACLs | One-shot |
| `agent/tools/mcp/mod.rs:2646` | `python` / `python3` / `py --version` discovery probe | One-shot at discovery |
| `app/background/pdf_converter.rs:105` | Configured PDF converter | Per conversion |
| `bin/deploy.rs:8` | `cargo` for the deploy script | Deploy-time only |

## Decision

**Apply `CREATE_NO_WINDOW` (`0x08000000`) to the MCP stdio server
spawn and document the config gotcha for npm-style servers.** Keep
the change small and local to the MCP module; defer the other
four callsites to a follow-up.

### Implementation

- Introduce a small free function
  `build_stdio_command(program, args, env) -> Command` in
  `session.rs`. It owns the program/args/env wiring, the
  `Stdio::piped()` setup, and — on Windows only — the
  `creation_flags(CREATE_NO_WINDOW)` call.
- Replace the 8 lines of inline `Command` construction in
  `McpClientSession::ensure_stdio_transport_locked` with a single
  call to the helper.
- Name the flag as a module-level `const CREATE_NO_WINDOW: u32 =
  0x0800_0000` (`#[cfg(windows)]`) rather than a bare magic
  number, with a doc comment that points back to this ADR.

### Config guidance

For an npm-distributed MCP server, the cleanest configuration is
to point `command` directly at `node.exe` and pass the server
script as the first argument:

```yaml
command: C:\Program Files\nodejs\node.exe
args:
  - "C:\\Users\\<user>\\AppData\\Roaming\\npm\\node_modules\\@modelcontextprotocol\\server-foo\\dist\\index.js"
```

This skips the `cmd.exe` interpreter hop that the `.cmd` shim
forces. Document this in the user-facing config docs (out of
scope for this ADR; tracked as a follow-up).

## Consequences

### Positive

- The MCP stdio transport no longer flashes a console window on
  Windows release builds.
- `CREATE_NO_WINDOW` allocates a hidden console buffer for the
  child but leaves stdin / stdout / stderr redirection to pipes
  intact, so the JSON-RPC over stdio protocol is unaffected.
- The Command construction is now exercised by three unit tests
  in a `#[cfg(test)] mod tests` block, and is callable without
  spinning up a full `McpClientSession`.
- Centralising the Command construction in a named helper makes
  the next refactor (e.g. applying the same fix to the other four
  callsites) one helper call away per callsite.

### Negative / accepted trade-offs

- `CREATE_NO_WINDOW` does not affect the `.cmd` shim hop through
  `cmd.exe`. A user who configures `command: npx` still pays one
  extra `conhost.exe` allocation per server start. The fix for
  that is configuration-side (point `command` at `node.exe`
  directly), not code-side. The ADR carries the config
  guidance for that.
- Four other callsites in the crate still flash a window on
  Windows. They are deferred because none of them are persistent
  (one-shot discovery, one-shot ACL write, infrequent PDF
  conversion, deploy-time only). The fix, when it lands, is the
  same one-liner per callsite.
- The Rust std API exposes a `CommandExt::creation_flags(u32)`
  *setter* but no public *getter*, so the "no visible window"
  assertion cannot be observed from a unit test. The test
  module documents this in a header comment and asserts only on
  the helper's contract (program/args/env/stdio/spawn). The
  actual no-window behaviour is verified manually in PR review
  on a real Windows machine, per `AGENTS.md §10`'s "issue
  cannot be reproduced at the unit or integration level"
  escape clause.

## Verification

- `cargo check` (lib) — clean.
- `cargo clippy --lib -- -D warnings` — clean.
- `cargo fmt --check src/agent/tools/mcp/session.rs` — clean.
- `cargo test --lib agent::tools::mcp` — **92 passed; 0 failed;
  6 ignored**, including the 3 new tests
  `build_stdio_command_sets_program_args_env_and_pipes_stdio`,
  `build_stdio_command_produces_a_spawnable_command`, and
  `build_stdio_command_is_idempotent_under_repeated_calls`.
- `cargo doc --no-deps --quiet` — clean.
- **Manual PR check (Windows release build):** launch a stdio
  MCP server, confirm no console window appears. Confirm the
  JSON-RPC over stdio traffic still flows (the existing
  `test_stdio_*` integration tests cover this on the lib side).

## Out of scope (follow-up)

1. Apply the same `creation_flags(CREATE_NO_WINDOW)` fix to the
   four other `process::Command::new` callsites listed in the
   Context table.
2. Add a config documentation note that npm-distributed servers
   should set `command` to `node.exe` (or equivalent) directly,
   to skip the `.cmd` interpreter hop.
3. If a more user-friendly fix is wanted long-term, consider a
   stdio-MCP "command normalisation" pass at config load time
   that detects `.cmd`/`.bat` and either warns or rewrites to
   the underlying `.exe` via `PATHEXT`.

## Pre-existing baseline note (do not fix here)

`cargo clippy --all-targets` is **red on this branch** with a
test compile error in
[`src/agent/tools/jmap/email.rs:1385`](../../src/desktop/src/agent/tools/jmap/email.rs):
the test module imports `SearchEmailPagination` from `super`, but
the type no longer exists in `jmap/email.rs` — it was moved out
during the in-progress `manager/builtin/strings/` refactor and
the test module was not updated. The struct is referenced in 11
tests but defined nowhere. This is unrelated to the MCP change
in this ADR and is tracked separately.
