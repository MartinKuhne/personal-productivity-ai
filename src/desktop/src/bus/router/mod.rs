//! Bus-side plumbing — multiplexers and generic channel-drain workers.
//!
//! The actual worker *implementations* (PDF converter, image vision,
//! indexer) live in `app/background/`. This module is for the
//! infrastructure they sit on: per-format fan-out (`bus_router`) and
//! generic channel-driven workers (`worker`).

pub mod bus_router;
pub mod worker;

pub use bus_router::BusRouter;
pub use worker::{ChannelWorker, spawn_path_worker};
