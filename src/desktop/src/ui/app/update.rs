//! Per-frame update for `FastMdApp` — the work that runs every frame
//! before the panels are drawn: apply persisted font on the first frame,
//! drain the config bus and the background `Task` channel, process file
//! events, drive the editor overlay / modals / panels, and snapshot the
//! current persisted-UI state for the next `save`.
//!
//! The flow is:
//! 1. [`FastMdApp::update_ui`] orchestrates one frame.
//! 2. [`FastMdApp::process_file_events_and_repaint`] decides whether the
//!    frame needs an immediate repaint (events arrived, indexing still
//!    running, or raw input was delivered) or whether to throttle to the
//!    configured repaint interval.
//! 3. [`FastMdApp::handle_deferred_actions`] flushes side effects the
//!    panel callbacks asked to happen on the next frame — e.g. starting
//!    a queued agent session, joining a finished batch.
//! 4. [`FastMdApp::update_persisted_ui_state`] snapshots the current
//!    font scale and panel widths so the next `eframe::App::save` can
//!    persist them. Window rect is handled by eframe's built-in
//!    `persistence` feature.

use eframe::egui;

use super::{FONT_SCALE_MAX, FONT_SCALE_MIN, FastMdApp, sanitise_font_scale};

impl FastMdApp {
    /// Purpose: Drive one frame of the app.
    /// Inputs: `ui` - The root [`egui::Ui`] supplied by eframe.
    /// Outputs: None.
    /// Purity: Impure (mutates `self`, paints to `ui`).
    /// Preconditions: None.
    /// Postconditions: The root view has been rendered for this frame.
    ///
    /// egui 0.35 changed `App::update` to `App::ui`, and the
    /// `eframe::App` entry point now hands us a `&mut egui::Ui`
    /// rather than a `&Context`. We use the inner `Ui` to draw
    /// all panels, and pluck out the [`egui::Context`] for the
    /// non-rendering bookkeeping (file-event drain, repaint
    /// scheduling, etc).
    pub fn update_ui(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "profiling")]
        puffin::GlobalProfiler::lock().new_frame();

        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let ctx = ui.ctx();

        // Apply persisted font size scale on the first frame. The
        // window rect is handled by eframe's built-in `persistence`
        // feature (see `with_app_id` in `main.rs` and the
        // `persistence` feature on the `eframe` dep in
        // `Cargo.toml`).
        self.apply_persisted_font_scale(ctx);

        self.orchestrator.drain_config_bus();
        self.process_file_events_and_repaint(ctx);
        self.orchestrator.drain_background_channel();
        self.orchestrator.handle_file_selection();
        self.show_editor_overlay(ui);
        self.show_modals(ui);
        self.render_panels(ui);
        self.handle_deferred_actions();

        // Update persisted UI state with current values for saving on exit
        self.update_persisted_ui_state(ui.ctx());

        #[cfg(feature = "profiling")]
        {
            egui::Window::new("Profiler")
                .vscroll(true)
                .resizable(true)
                .default_size([400.0, 300.0])
                .show(ui.ctx(), |ui| {
                    puffin_egui::profiler_ui(ui);
                });
        }
    }

    fn process_file_events_and_repaint(&mut self, ctx: &egui::Context) {
        if self.orchestrator.process_file_events()
            || !self.orchestrator.file_processor.indexing_finished
            || !ctx.input(|i| i.raw.events.is_empty())
        {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(self.orchestrator.repaint_interval);
        }
    }

    fn handle_deferred_actions(&mut self) {
        if let Some(prompt) = self.orchestrator.submit_prompt.take() {
            self.orchestrator.start_agent_session(prompt);
        }

        if let Some(handle) = self.orchestrator.dialogs.batch_handle.take() {
            if handle.thread.is_finished() {
                let result = handle.join();
                self.orchestrator.dialogs.batch_cancel_flag = None;
                tracing::info!("Batch completed: {:?}", result);
            } else {
                self.orchestrator.dialogs.batch_handle = Some(handle);
            }
        }
    }

    /// Update persisted UI state with current font scale and panel widths.
    ///
    /// Window size/position are persisted by eframe's built-in
    /// `persistence` feature — we do not duplicate that here.
    fn update_persisted_ui_state(&mut self, ctx: &egui::Context) {
        // Update panel widths from layout
        self.persisted_ui_state.left_panel_width = self.layout.left_panel_width;
        self.persisted_ui_state.right_panel_width = self.layout.right_panel_width;

        // Update font size scale (relative to the OS-reported
        // baseline captured on the first frame — see
        // [`Self::apply_persisted_font_scale`]).
        self.persist_font_scale(ctx);
    }

    /// Capture the OS-reported `pixels_per_point` as the baseline
    /// and apply any persisted user-chosen scale on top of it.
    ///
    /// Must run on the first frame of the session, **before** any
    /// widget paints, so the rest of the UI sees the scaled ppp.
    /// After this call the persisted scale is treated as a
    /// multiplier on top of the baseline; never as the absolute
    /// ppp.
    ///
    /// The baseline is the OS-reported value at the start of the
    /// session and is **not persisted** — the OS re-reports it on
    /// every launch (and may differ between monitors on a
    /// multi-DPI Windows setup).
    ///
    /// Records the chosen scale into [`Self::applied_font_scale`]
    /// so [`Self::persist_font_scale`] can write a stable value
    /// even though egui 0.35 defers `set_pixels_per_point` until
    /// the next `begin_pass` (so `ctx.pixels_per_point()` still
    /// returns the pre-apply value within the same frame).
    ///
    /// `pub(in crate::ui::app)` so the regression tests in the
    /// sibling `tests` submodule can drive the same first-frame
    /// apply + persist pair that `update_ui` runs in production.
    pub(in crate::ui::app) fn apply_persisted_font_scale(&mut self, ctx: &egui::Context) {
        if self.persisted_font_applied {
            return;
        }
        let baseline = ctx.pixels_per_point();
        self.os_baseline_ppp = Some(baseline);
        let scale = self
            .persisted_ui_state
            .font_size_scale
            .and_then(sanitise_font_scale)
            .unwrap_or(1.0);
        self.applied_font_scale = scale;
        if scale != 1.0 {
            ctx.set_pixels_per_point(baseline * scale);
        }
        self.persisted_font_applied = true;
    }

    /// Persist the font scale that was actually applied on the
    /// first frame (see [`Self::applied_font_scale`]) back into
    /// [`Self::persisted_ui_state`]. This is a pure
    /// "remember what we did" write — it does **not** re-read
    /// `ctx.pixels_per_point()` to recompute the multiplier,
    /// because egui 0.35 defers `set_pixels_per_point` until the
    /// next `begin_pass` and recomputing from the still-old ppp
    /// would silently reset the persisted value to `None` on the
    /// same frame the scale was applied.
    ///
    /// A near-unity applied scale (≈ 1.0) is stored as `None` so
    /// a fresh install doesn't carry a redundant `1.0` and risk
    /// a future bug misinterpreting it. Out-of-range or
    /// non-finite applied scales are also stored as `None` so a
    /// single corrupt entry self-heals on the next launch.
    ///
    /// `pub(in crate::ui::app)` so the regression tests in the
    /// sibling `tests` submodule can call persist directly.
    pub(in crate::ui::app) fn persist_font_scale(&mut self, _ctx: &egui::Context) {
        if !self.applied_font_scale.is_finite()
            || self.applied_font_scale < FONT_SCALE_MIN
            || self.applied_font_scale > FONT_SCALE_MAX
        {
            self.persisted_ui_state.font_size_scale = None;
            return;
        }
        if (self.applied_font_scale - 1.0).abs() < 1e-3 {
            self.persisted_ui_state.font_size_scale = None;
        } else {
            self.persisted_ui_state.font_size_scale = Some(self.applied_font_scale);
        }
    }
}
