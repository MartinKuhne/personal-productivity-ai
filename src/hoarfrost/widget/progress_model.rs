//! The declarative `ProgressModel` a host owns and hands to the progress
//! indicators each frame — §3.9's mode, status, label and cancellation
//! (`UIF-ATOM-009`).
//!
//! A progress indicator is the framework's other non-interactive atom,
//! with one exception this module has to account for: §3.9 bolts a
//! **stop button** onto a cancellable bar. That button is a real control
//! with a real callback, so unlike [`badge_model`](crate::widget::badge_model)
//! this model does feed an interaction — but the interaction belongs to
//! the stop button, not to the bar. The bar itself never changes because
//! a pointer crossed it.
//!
//! # The host owns the clock
//!
//! Nothing in this module reads a clock, and nothing in
//! [`crate::widget::progress`] does either. `completed` is a number the
//! host already has, and the indeterminate segment's position arrives at
//! paint time as an **injected phase**. That split is the whole reason
//! the §3.9 cells are testable: a resolver that called
//! `Instant::now()` would produce a different snapshot on every run and
//! a headless audit could not pin any of them.
//!
//! The same rule covers motion. §3.9 says the indeterminate segment
//! renders "static striped" when the OS `reduced_motion` setting is on.
//! This crate never reads the OS setting — a widget that queried the
//! platform would be untestable and would change under the host's feet.
//! `reduced_motion` is a `bool` the host passes in, and the resolver
//! honours it.
//!
//! # Why `total == 0` is not an error
//!
//! A determinate model with `total: 0` is a real thing that happens: the
//! work exists and its size has not been discovered yet. Treating it as
//! an error would mean every host writing a progress bar also writes a
//! guard for a case that is not a bug. [`ProgressModel::fraction`]
//! returns `0.0` for it, which is also what an empty bar looks like, so
//! the visual answer is the honest one.
//!
//! # Why the label is `Option<&'static str>`
//!
//! §3.9 prints `label: Option<String>`. The `&'static str` half is a
//! tightening rather than a loosening: a progress label is a chosen word
//! ("Indexing", "Uploading"), and a model carrying an owned `String`
//! invites a host to format a new one every frame — allocation in a
//! paint path for a string that never changes.
//!
//! Unit tests live in the sibling `progress_model_tests.rs` sidecar.

/// How far along the work is — or whether that question has an answer.
///
/// The two variants are not two sizes of the same thing. A determinate
/// bar answers "how far"; an indeterminate one answers "still going".
/// Collapsing them into a single `f32` with a sentinel would mean every
/// consumer testing for the sentinel, and a sentinel that arrives by
/// accident is a bar that stops moving for no visible reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressMode {
    /// The work is running and its extent is unknown or unmeasured.
    ///
    /// Renders the `35%`-width sliding segment, or the static striped
    /// segment under injected reduced motion.
    Indeterminate,

    /// The work has a known extent and a position within it.
    Determinate {
        /// Units finished. May exceed `total` if the host over-delivers;
        /// [`ProgressModel::fraction`] clamps rather than panicking.
        completed: u64,
        /// Units of work. `0` means the extent is not yet known — see
        /// the [module docs](self).
        total: u64,
    },
}

/// What the running work is doing, as far as the user is told.
///
/// Three positions, and they are not a severity scale: `Paused` is not
/// worse than `Active`, it is a different fact. The colours §3.9 assigns
/// them are deliberately not the status palette's alarm ladder — Paused
/// borrows `status.warning` because it needs to be *seen*, not because
/// it is a warning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProgressStatus {
    /// Running normally.
    #[default]
    Active,
    /// Stopped by the host, expected to resume.
    Paused,
    /// Stopped by failure.
    Error,
}

impl ProgressStatus {
    /// Whether the status describes work that is still moving.
    ///
    /// Only [`ProgressStatus::Active`] is. A paused bar and a failed bar
    /// both stop, and a renderer that animated either would be claiming
    /// the work is progressing when it is not.
    #[must_use]
    pub const fn is_moving(self) -> bool {
        matches!(self, Self::Active)
    }
}

/// The declarative progress indicator a host owns (`UIF-ATOM-009`).
///
/// No method takes `&mut self`. Progress happens in the host; the model
/// is replaced, not mutated, which is what makes a frame's worth of
/// progress comparable against a recorded one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressModel {
    mode: ProgressMode,
    status: ProgressStatus,
    label: Option<&'static str>,
    cancellable: bool,
}

impl ProgressModel {
    /// A determinate bar for `completed` of `total`, in `status`.
    #[must_use]
    pub fn determinate(completed: u64, total: u64) -> Self {
        Self {
            mode: ProgressMode::Determinate { completed, total },
            status: ProgressStatus::Active,
            label: None,
            cancellable: false,
        }
    }

    /// An indeterminate bar in `status`.
    #[must_use]
    pub fn indeterminate() -> Self {
        Self {
            mode: ProgressMode::Indeterminate,
            status: ProgressStatus::Active,
            label: None,
            cancellable: false,
        }
    }

    /// Return a copy in `status`.
    #[must_use]
    pub fn with_status(mut self, status: ProgressStatus) -> Self {
        self.status = status;
        self
    }

    /// Return a copy carrying `label`.
    #[must_use]
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    /// Return a copy whose stop button exists (`true`) or does not
    /// (`false`).
    ///
    /// The flag decides whether the control is *there*, not whether it
    /// is enabled. A non-cancellable bar paints no stop button at all,
    /// so there is nothing for a user to discover is inert.
    #[must_use]
    pub fn with_cancellable(mut self, cancellable: bool) -> Self {
        self.cancellable = cancellable;
        self
    }

    /// Return a copy whose mode is `mode`.
    #[must_use]
    pub fn with_mode(mut self, mode: ProgressMode) -> Self {
        self.mode = mode;
        self
    }

    /// Return a copy with `completed` units finished, keeping the
    /// current `total`.
    ///
    /// A no-op on an indeterminate model rather than a panic: the host
    /// that advances a bar it does not know the extent of is asking a
    /// question the model cannot answer, and the answer is "nothing
    /// changed".
    #[must_use]
    pub fn with_completed(mut self, completed: u64) -> Self {
        if let ProgressMode::Determinate { total, .. } = self.mode {
            self.mode = ProgressMode::Determinate { completed, total };
        }
        self
    }

    /// The mode.
    #[must_use]
    pub fn mode(&self) -> ProgressMode {
        self.mode
    }

    /// The status.
    #[must_use]
    pub fn status(&self) -> ProgressStatus {
        self.status
    }

    /// The label, if any.
    #[must_use]
    pub fn label(&self) -> Option<&'static str> {
        self.label
    }

    /// Whether the trailing stop button is present.
    #[must_use]
    pub fn cancellable(&self) -> bool {
        self.cancellable
    }

    /// Whether this bar has no known extent.
    #[must_use]
    pub fn is_indeterminate(&self) -> bool {
        matches!(self.mode, ProgressMode::Indeterminate)
    }

    /// The completed fraction in `0.0..=1.0`, or `None` when the bar is
    /// indeterminate.
    ///
    /// Clamped at both ends: over-delivery reads as a full bar rather
    /// than a fill that has left its track, and `total == 0` reads as an
    /// empty bar rather than a division error.
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        match self.mode {
            ProgressMode::Indeterminate => None,
            ProgressMode::Determinate { total: 0, .. } => Some(0.0),
            ProgressMode::Determinate { completed, total } => {
                let raw = completed as f64 / total as f64;
                Some(if raw > 1.0 { 1.0 } else { raw as f32 })
            }
        }
    }
}

#[cfg(test)]
#[path = "progress_model_tests.rs"]
mod progress_model_tests;
