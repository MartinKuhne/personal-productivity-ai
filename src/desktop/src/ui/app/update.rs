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
use crate::ui::os_shell;

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
        self.orchestrator.drain_agent_event_bus();

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::A)) {
            self.orchestrator.agent_panel_state.show_debug_window =
                !self.orchestrator.agent_panel_state.show_debug_window;
        }

        self.orchestrator.handle_file_selection();
        self.show_editor_overlay(ui);
        self.show_modals(ui);
        self.render_panels(ui);
        self.handle_deferred_actions();

        // Update persisted UI state with current values for saving on exit
        self.update_persisted_ui_state(ui.ctx());

        // Drain egui platform output commands (notably
        // `OutputCommand::OpenUrl` emitted by `egui::Link`
        // widget clicks in the markdown viewer) and dispatch
        // them to the OS shell. eframe 0.36's native (winit)
        // runtime does **not** process `OutputCommand::OpenUrl`
        // — only the `web` target handles it (see
        // `eframe/src/web/{app_runner.rs,mod.rs}`). Without
        // this drain, hyperlink clicks in the viewer are
        // silently dropped and the URL never reaches the
        // system browser. Doing it at the end of `update_ui`
        // (after `render_panels`) guarantees every click that
        // landed during the current frame is picked up exactly
        // once. The `CopyText` / `CopyImage` siblings on the
        // same `PlatformOutput::commands` list are still
        // processed by eframe's built-in clipboard handling.
        //
        // Re-acquire `ctx` from `ui` here (rather than reusing
        // the binding from the top of the function) so the
        // immutable borrow of `ui` ends as soon as
        // `ctx.output(...)` returns; the earlier `let ctx =
        // ui.ctx();` binding would otherwise keep `ui`
        // borrowed immutably for the entire function and
        // collide with the `&mut ui` reborrow in
        // `self.render_panels(ui)`.
        let commands = ui.ctx().output(|o| o.commands.clone());
        os_shell::dispatch_platform_commands(&commands, os_shell::open_url);
    }

    fn process_file_events_and_repaint(&mut self, ctx: &egui::Context) {
        if self.should_request_immediate_repaint(ctx) {
            ctx.request_repaint();
        }
        // When `should_request_immediate_repaint` returns `false` the
        // frame is fully idle (no file events, indexing finished, no
        // raw input). We deliberately do NOT call
        // `ctx.request_repaint_after(...)` here. A previous revision
        // scheduled a 16 ms repaint in the idle branch as a safety net
        // for the indexing-finished transition, but in practice the
        // next event always arrived from the winit event loop
        // (mouse move, key, focus, resize, window move, …) well
        // before the 16 ms timer — and the timer itself kept the
        // entire `update_ui` closure running at 60 FPS for no
        // visible change, pegging the process at ~5% CPU with no
        // input. egui's reactive model will repaint on the next real
        // input event; the regression tests
        // (`tests::test_idle_app_does_not_request_repaint`,
        // `tests::test_indexing_in_progress_requests_repaint`) lock
        // this contract in.
    }

    /// Decide whether this frame needs an *immediate* repaint.
    ///
    /// Returns `true` when any of the following is true:
    /// - File events arrived on the `file_event_bus` this frame.
    /// - Indexing is still in progress (the toolbar spinner and
    ///   status text need to keep animating).
    /// - Raw input is pending in the egui input state (mouse, key,
    ///   focus, viewport resize, …) that the next frame must flush.
    ///
    /// When this returns `false`, the frame is fully idle and the
    /// caller MUST NOT request any repaint. Letting egui's reactive
    /// model pick the next repaint from real input events is what
    /// keeps the idle CPU at 0%; the historical 60 FPS forced loop
    /// that did the same job is what caused the regression this
    /// decider exists to prevent.
    pub(in crate::ui::app) fn should_request_immediate_repaint(
        &mut self,
        ctx: &egui::Context,
    ) -> bool {
        let events_changed = self.orchestrator.process_file_events();
        let indexing_active = !self.orchestrator.file_processor.indexing_finished;
        let raw_input_pending = !ctx.input(|i| i.raw.events.is_empty());
        events_changed || indexing_active || raw_input_pending
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
