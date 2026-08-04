//! Per-frame update for `FastMdApp` — the work that runs every frame
//! before the panels are drawn: apply persisted window/font on the first
//! frame, drain the config bus and the background `Task` channel, process
//! file events, drive the editor overlay / modals / panels, and snapshot
//! the current persisted-UI state for the next `save`.
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
//!    window rect, font scale, and panel widths so the next
//!    `eframe::App::save` can persist them.

use eframe::egui;

use super::FastMdApp;

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

        // Apply persisted window size/position on first frame
        if !self.persisted_window_applied {
            if let (Some(w), Some(h)) = (
                self.persisted_ui_state.window_width,
                self.persisted_ui_state.window_height,
            ) {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            }
            if let (Some(x), Some(y)) = (
                self.persisted_ui_state.window_x,
                self.persisted_ui_state.window_y,
            ) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
            }
            self.persisted_window_applied = true;
        }

        // Apply persisted font size scale on first frame
        if !self.persisted_font_applied {
            if let Some(scale) = self.persisted_ui_state.font_size_scale {
                ctx.set_pixels_per_point(ctx.pixels_per_point() * scale);
            }
            self.persisted_font_applied = true;
        }

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

    /// Update persisted UI state with current window size, position, font scale, and panel widths.
    fn update_persisted_ui_state(&mut self, ctx: &egui::Context) {
        // Update panel widths from layout
        self.persisted_ui_state.left_panel_width = self.layout.left_panel_width;
        self.persisted_ui_state.right_panel_width = self.layout.right_panel_width;

        // Update window size and position from viewport
        ctx.input(|i| {
            let viewport = i.viewport();
            // Use inner_rect for window size
            if let Some(inner_rect) = viewport.inner_rect {
                self.persisted_ui_state.window_width = Some(inner_rect.width());
                self.persisted_ui_state.window_height = Some(inner_rect.height());
            }
            // Use outer_rect for window position
            if let Some(outer_rect) = viewport.outer_rect {
                self.persisted_ui_state.window_x = Some(outer_rect.min.x);
                self.persisted_ui_state.window_y = Some(outer_rect.min.y);
            }
        });

        // Update font size scale (relative to default)
        let current_ppp = ctx.pixels_per_point();
        let default_ppp = 1.0; // Default pixels per point
        if (current_ppp - default_ppp).abs() > f32::EPSILON {
            self.persisted_ui_state.font_size_scale = Some(current_ppp / default_ppp);
        } else {
            self.persisted_ui_state.font_size_scale = None;
        }
    }
}
