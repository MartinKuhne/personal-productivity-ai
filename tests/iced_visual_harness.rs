//! Tier-3 / tier-4 iced visual-test harness ([TASK-025]).
//!
//! Tier 2 (`src/app/ui_iced/interaction_tests.rs`) asserts *messages*.
//! This harness asserts *pixels* (tier 3) and replays *recorded sessions*
//! (tier 4). Both need a real recording pass by a human, so everything that
//! consumes a committed artifact is `#[ignore]`d until that pass happens.
//!
//! # Layout
//!
//! ```text
//! tests/
//!   iced_visual_harness.rs      <- this file: the Program adapter + entry points
//!   iced_visual/
//!     README.md                 <- recording instructions (read this first)
//!     baselines/                <- tier-3 sha256 / PNG baselines (recorded, committed)
//!     ice/                      <- tier-4 recorded `.ice` session files (committed)
//! ```
//!
//! # The `Program` adapter
//!
//! `iced_test::screenshot` and `iced_test::run` take a
//! [`program::Program`], not the free `build_state` / `update` / `view`
//! functions the app is actually built from. [`FastMdProgram`] is a thin
//! adapter over those three functions plus the production
//! [`window_settings`], so the harness renders exactly what `launch()`
//! renders — no parallel reimplementation of the app.
//!
//! The adapter uses `backend::null::Executor`: no subscriptions run, so a
//! recording never touches the network or the real user configuration.
//! State comes from `build_state` over `InMemoryConfigStorage` +
//! `InMemoryUiStateStorage` ([RUST-006]).
//!
//! # Running
//!
//! ```sh
//! # tier 3/4 default: only the wiring check runs
//! cargo nextest run -p fastmd --test iced_visual_harness
//!
//! # record / refresh baselines (deliberate, reviewed in the diff)
//! $env:FASTMD_RECORD_ICED_BASELINES = "1"
//! cargo test -p fastmd --test iced_visual_harness -- --ignored
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fastmd::bus::core::Bus;
use fastmd::bus::events::ConfigArrived;
use fastmd::config::{AppConfig, InMemoryConfigStorage};
use fastmd::ui::PersistedUiState;
use fastmd::ui_iced::app::{AppState, build_state, window_settings};
use fastmd::ui_iced::persisted::InMemoryUiStateStorage;
use fastmd::ui_iced::update::update;
use fastmd::ui_iced::view::view;
use iced::Task;
use iced_test::program::{self, Program as _};
use iced_test::simulator;
use iced_test::simulator::Snapshot;

/// Root of the tier-3/4 artifacts, relative to the crate root.
const VISUAL_DIR: &str = "tests/iced_visual";

/// Tier-3 baseline directory.
const BASELINE_DIR: &str = "tests/iced_visual/baselines";

/// Tier-4 recorded `.ice` session directory.
const ICE_DIR: &str = "tests/iced_visual/ice";

/// Env var that switches the harness from *compare* to *record*.
const RECORD_ENV: &str = "FASTMD_RECORD_ICED_BASELINES";

/// A [`program::Program`] adapter over the production
/// `build_state` / `update` / `view` trio ([TASK-025]).
///
/// `boot` builds a framework-free [`AppState`] over in-memory storage, so
/// recording never reads or writes a real user path ([RUST-006]).
#[derive(Debug, Clone, Copy, Default)]
pub struct FastMdProgram;

impl program::Program for FastMdProgram {
    type State = AppState;
    type Message = fastmd::ui_iced::message::Message;
    type Theme = iced::Theme;
    type Renderer = iced::Renderer;
    type Executor = iced_test::futures::backend::null::Executor;

    fn name() -> &'static str {
        "fastmd"
    }

    fn settings(&self) -> iced::Settings {
        iced::Settings::default()
    }

    fn window(&self) -> Option<iced::window::Settings> {
        Some(window_settings(&PersistedUiState::default()))
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        let state = build_state(
            AppConfig::default(),
            &Bus::<ConfigArrived>::new(),
            Arc::new(InMemoryConfigStorage::new()),
            Arc::new(InMemoryUiStateStorage::new()),
            PersistedUiState::default(),
        );
        (state, Task::none())
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        update(state, message)
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        _window: iced::window::Id,
    ) -> iced::Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        view(state)
    }
}

/// `true` when the harness should record rather than compare.
fn recording() -> bool {
    matches!(
        std::env::var(RECORD_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Absolute-ish path to a tier-3 baseline under [`BASELINE_DIR`].
fn baseline_path(name: &str) -> PathBuf {
    Path::new(BASELINE_DIR).join(name)
}

/// Compares a rendered [`Snapshot`] against its committed baseline, or
/// records it when [`RECORD_ENV`] is set ([TASK-025]).
///
/// Recording is opt-in on purpose: `Snapshot::matches_hash` *creates* a
/// missing baseline and returns `true`, which would turn a missing baseline
/// into a silent pass. Callers must therefore assert the baseline exists
/// before comparing.
fn compare_or_record(snapshot: &Snapshot, name: &str) {
    let path = baseline_path(name);

    if recording() {
        std::fs::create_dir_all(&path).ok();
        let hash_file = path.with_extension("sha256");
        std::fs::remove_file(&hash_file).ok();
        snapshot
            .matches_hash(&path)
            .unwrap_or_else(|e| panic!("recording {name:?} failed: {e}"));
        println!("RECORDED baseline {}", hash_file.display());
        return;
    }

    let hash_file = path.with_extension("sha256");
    assert!(
        hash_file.exists(),
        "tier-3 baseline {} is missing. Record it with: \
         ${RECORD_ENV} = \"1\"; cargo test -p fastmd --test iced_visual_harness -- --ignored",
        hash_file.display()
    );

    snapshot
        .matches_hash(&path)
        .unwrap_or_else(|e| panic!("baseline {name:?} comparison failed: {e}"));
}

/// Wiring check: the tier-4 `.ice` directory is readable by
/// `iced_test::run` and every `.ice` file in it executes.
///
/// Passes on an empty directory. Once a human records sessions into
/// [`ICE_DIR`] this same test replays them against [`FastMdProgram`] with
/// no change, which is the point of the harness.
#[test]
fn test_ice_replay_directory_is_wired() {
    let dir = Path::new(ICE_DIR);
    assert!(
        dir.is_dir(),
        "{ICE_DIR} must exist for tier-4 replay (see {VISUAL_DIR}/README.md)"
    );

    let recorded: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read ice dir")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "ice"))
        .collect();

    let result = iced_test::run(FastMdProgram, dir);

    if recorded.is_empty() {
        assert!(
            result.is_ok(),
            "an empty .ice directory must replay cleanly, got {result:?}"
        );
    } else {
        result.unwrap_or_else(|error| {
            panic!(
                "replaying {} recorded session(s) failed: {error:?}",
                recorded.len()
            )
        });
    }
}

/// Tier-3 example: the whole-app view against a sha256 baseline.
///
/// Needs a recording pass before it can assert anything, so it is ignored
/// by default. See [`VISUAL_DIR`]/README.md.
#[test]
#[ignore = "tier 3: needs a recorded baseline — run with FASTMD_RECORD_ICED_BASELINES=1 -- --ignored"]
fn test_record_app_view_baseline() {
    let state = boot_state();
    let mut ui = simulator(view(&state));
    let snapshot = ui
        .snapshot(&iced::Theme::Dark)
        .expect("snapshot renders under the software renderer");
    compare_or_record(&snapshot, "app_view");
}

/// Tier-3 example: the top band against a sha256 baseline.
#[test]
#[ignore = "tier 3: needs a recorded baseline — run with FASTMD_RECORD_ICED_BASELINES=1 -- --ignored"]
fn test_record_top_band_baseline() {
    let state = boot_state();
    let mut ui = simulator(fastmd::ui_iced::panels::top::top_bar(&state.models.top));
    let snapshot = ui
        .snapshot(&iced::Theme::Dark)
        .expect("snapshot renders under the software renderer");
    compare_or_record(&snapshot, "top_band");
}

/// A booted [`AppState`] for the recording entry points.
fn boot_state() -> AppState {
    FastMdProgram.boot().0
}

/// Keeps [`VISUAL_DIR`] referenced from code so the documented layout and
/// the actual layout cannot drift silently.
#[test]
fn test_visual_artifact_directories_exist() {
    assert!(Path::new(VISUAL_DIR).is_dir(), "{VISUAL_DIR} missing");
    assert!(Path::new(BASELINE_DIR).is_dir(), "{BASELINE_DIR} missing");
    assert!(Path::new(ICE_DIR).is_dir(), "{ICE_DIR} missing");
}
