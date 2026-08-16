//! Messaging subsystem — transports, event payloads, and bus-side
//! routing for the whole crate.
//!
//! Every cross-thread event in the desktop app flows through a `Bus<T>`
//! defined here. Producers (background workers, the watcher, the
//! initial library scan) and consumers (the tag manager, the directory
//! tree, the UI, the indexer) all import from this module so the
//! message-passing surface is discoverable in one place.
//!
//! ## Layout
//!
//! - [`core`] — the transport primitive: `Bus<T>` / `BusReader<T>`.
//! - [`events`] — every event payload that flows over a bus or
//!   channel (`FileEvent`, `BackgroundEvent`, `ConfigArrived`, …).
//! - [`router`] — bus-side plumbing (per-format fan-out, generic
//!   channel-drain workers).
//! - [`config`] — the configuration-arrival bus constructor and
//!   subscriber timeout.
//!
//! See the architecture review (`doc/planning/application-architecture-review.md`,
//! P1-6) for the background-event migration that split the legacy
//! `BackgroundMessage` god-enum into per-domain sub-enums
//! (`AgentEvent`, `FsEvent`, `ProcessEvent`) under [`events::typed`].

pub mod config;
pub mod core;
pub mod events;
pub mod router;
