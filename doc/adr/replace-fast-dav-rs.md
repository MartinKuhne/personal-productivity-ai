# Replace `fast-dav-rs` with Native `reqwest`-Backed Client

Status: accepted
Date: 2026-09-04

## Context

The `fastmd-agent` crate previously adopted `fast-dav-rs` (v0.4.4) to implement
CalDAV (RFC 4791) and CardDAV (RFC 6352) client integration for calendar and
contact synchronization.

`fast-dav-rs` is licensed under the **GNU Lesser General Public License v3.0 (LGPL-3.0)**.
Because the Rust toolchain statically links dependencies by default into the resulting
binary, LGPL-3.0 imposes copyleft requirements that restrict proprietary distribution,
require relinking capabilities for downstream users, and violate permissive open-source
compliance constraints for the project.

Additionally, `fast-dav-rs` relied on `hyper-util` connection pooling which suffered
from TCP keep-alive socket race conditions when communicating with servers over
sequential requests, necessitating complex retry logic and test workarounds (as documented
in `src/agent/lib/dav/cal_tests.rs`).

A non-LGPL replacement is required to maintain all CalDAV and CardDAV capabilities
without copyleft encumbrance.

## Decision

Replace `fast-dav-rs` with a project-native, lightweight CalDAV and CardDAV client
implemented directly inside [`src/agent/lib/dav/`](../../src/agent/lib/dav/)
backed by [`reqwest`](https://crates.io/crates/reqwest) and
[`roxmltree`](https://crates.io/crates/roxmltree) (both dual-licensed under **MIT / Apache-2.0**).

The implementation:
- Uses the existing `reqwest::Client` in `fastmd-agent` configured with `rustls`.
- Dispatches standard HTTP methods (`GET`, `PUT`, `DELETE`) as well as WebDAV protocol
  extension methods (`PROPFIND`, `REPORT`).
- Uses dedicated XML builders and `roxmltree` parsers in [`xml.rs`](../../src/agent/lib/dav/xml.rs)
  for collection discovery, `current-user-principal`, `calendar-home-set`, `calendar-query`,
  and `sync-collection` / `addressbook-query`.
- Preserves the exact public API of [`DavClient`](../../src/agent/lib/dav/client.rs) and the
  `cal` / `card` protocol adapters, ensuring zero breaking changes to the agent tool registry
  and UI layers.

### Alternatives considered

| Option | License | Scope | Outcome |
|--------|---------|-------|---------|
| **Native `reqwest` + `roxmltree` (chosen)** | **MIT / Apache-2.0** | Tailored CalDAV + CardDAV | Zero external DAV dependency risk, single HTTP stack, fixes connection pool races, 100% permissive. |
| `libdav` | ISC | CalDAV + CardDAV + WebDAV | Permissive, but requires secondary `hyper 1.x`/`hyper-rustls`/`tower-http` stack alongside `reqwest`. |
| `kaldav` | MIT | CalDAV only | Rejected: Does not support CardDAV (RFC 6352). |
| `caldav-utils` | MIT | CalDAV utilities only | Rejected: CalDAV only, tightly coupled to `icalendar`/`rrule`, lacks CardDAV. |
| `io-webdav` | MIT / Apache-2.0 | Sans-I/O WebDAV/CalDAV/CardDAV | Rejected: Experimental coroutine library requiring complex custom transport and state driving. |
| `reqwest_dav` | MIT / Apache-2.0 | File WebDAV (RFC 4918) only | Rejected: No support for CalDAV or CardDAV `REPORT` queries. |
| `minicaldav` | GPL-3.0-or-later | CalDAV | Rejected: Copyleft-encumbered. |
| `rustydav` | GPL-3.0 | WebDAV | Rejected: Copyleft-encumbered. |
| `vstorage` | EUPL-1.2 | CalDAV / CardDAV | Rejected: Copyleft-encumbered. |

## Consequences

- The `fast-dav-rs` crate and its LGPL-3.0 license are completely eliminated from the
  dependency graph.
- No new heavy networking crates (`hyper`, `hyper-util`, `hyper-rustls`, `tower-http`)
  are added; only the lightweight, permissive `roxmltree` XML parser is retained.
- The HTTP transport is unified on `reqwest`, simplifying TLS, timeout, and connection
  management across all external service integrations in `fastmd-agent`.
- All CalDAV and CardDAV unit and integration tests continue to run hermetically against
  wiremock stubs with improved connection stability.
