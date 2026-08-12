//! Per-frame rendering of the editor overlay, modals, and the five top-level
//! panels for `FastMdApp`. The methods here are pure delegation: each
//! calls into the appropriate `crate::ui::panels::*` or
//! `crate::ui::modals::*` helper with the right `FastMdApp` field, after
//! collecting the dialog flags that decide which windows to show.
//!
//! Order matters:
//! - [`FastMdApp::show_editor_overlay`] runs first so the editor window
//!   is open before the panels draw the file-tree / markdown body. If
//!   the editor closed this frame we also clear `tab_manager.loaded_path`
//!   so the centre panel reloads from disk on the next frame.
//! - [`FastMdApp::show_modals`] runs second so dialogs float above the
//!   panels.
//! - [`FastMdApp::render_panels`] runs last. Panel order is preserved
//!   from the previous monolithic implementation: top → bottom → right
//!   → left → center.

use eframe::egui;

use crate::bus::events::file::FileEventProducer;
use crate::ui::panels::{
    show_bottom_panel, show_center_panel, show_left_panel, show_right_panel, show_top_panel,
};

use super::FastMdApp;

impl FastMdApp {
    pub(super) fn show_editor_overlay(&mut self, ui: &mut egui::Ui) {
        let producer = FileEventProducer::new(self.orchestrator.file_event_bus.clone());
        // The editor opens its own top-level `egui::Window` from
        // the context pulled out of `ui`. After it returns we
        // check whether the buffer was closed (either by a
        // successful save or a manual cancel) and clear the
        // loaded path so the centre panel reloads the file on
        // the next frame.
        let was_open = self.orchestrator.text_buffer.is_open;
        let _ = crate::ui::editor_egui::show_text_editor(
            ui,
            &mut self.orchestrator.text_buffer,
            &producer,
        );
        if was_open && !self.orchestrator.text_buffer.is_open {
            self.orchestrator.tab_manager.loaded_path = None;
        }
    }

    pub(super) fn show_modals(&mut self, parent_ui: &mut egui::Ui) {
        // egui 0.35: modal dialogs are still rendered through
        // `egui::Window`, which can take the context directly. We
        // pull the `Context` off the root `Ui` so the existing
        // `show_*_modal` helpers (which take `&Context`) keep working.
        let ctx = parent_ui.ctx();
        if self.orchestrator.dialogs.move_dialog_open {
            crate::ui::modals::show_move_modal_dialog(
                &mut self.orchestrator.dialogs,
                &self.orchestrator.content_libraries,
                &self.orchestrator.file_processor,
                &self.orchestrator.file_event_bus,
                ctx,
            );
        }
        if self.orchestrator.dialogs.create_dir_dialog_open {
            crate::ui::modals::show_create_dir_dialog(
                &mut self.orchestrator.dialogs,
                &mut self.orchestrator.file_processor,
                &mut self.orchestrator._watcher,
                &self.orchestrator.file_event_bus,
                ctx,
            );
        }
        if self.orchestrator.dialogs.rename_dialog_open {
            let selection = &mut self.orchestrator.selection;
            crate::ui::modals::show_rename_dialog(crate::ui::modals::RenameDialogCtx {
                dialog_manager: &mut self.orchestrator.dialogs,
                file_event_bus: &self.orchestrator.file_event_bus,
                loaded_path: &mut self.orchestrator.tab_manager.loaded_path,
                selected_file: &mut selection.selected_file,
                selected_dir: &mut selection.selected_dir,
                tabs: &mut self.orchestrator.tab_manager.tabs,
                file_processor: &mut self.orchestrator.file_processor,
                tag_manager: &mut self.orchestrator.tag_manager,
                expanded_dirs: &mut selection.expanded_dirs,
                ctx,
            });
        }
        if self.orchestrator.dialogs.create_document_dialog_open {
            crate::ui::modals::show_create_document_dialog(
                &mut self.orchestrator.dialogs,
                &self.orchestrator.file_event_bus,
                ctx,
            );
        }

        crate::ui::background_logs::show_background_logs_window(self, ctx);

        crate::ui::agent_debug_window::show_agent_debug_window(self, ctx);

        if self.orchestrator.dialogs.tools_dialog_open {
            crate::ui::tools_dialog::show_tools_dialog(ctx, self);
        }

        if self.orchestrator.dialogs.batch_dialog_open {
            let mut dialog_config = self.orchestrator.dialogs.batch_dialog_config.clone();

            let prev_selected = dialog_config
                .selected_dir_idx
                .and_then(|i| dialog_config.available_dirs.get(i).cloned());
            dialog_config.available_dirs = self.orchestrator.directory_tracker.dirs_sorted();
            dialog_config.selected_dir_idx = prev_selected
                .as_ref()
                .and_then(|p| dialog_config.available_dirs.iter().position(|d| d == p));

            if let Some(result) =
                crate::ui::batch_dialog::show_batch_modal(self, ctx, &mut dialog_config)
            {
                match result {
                    crate::app::batch::types::BatchDialogResult::Process(config) => {
                        if self.orchestrator.dialogs.batch_handle.is_none() {
                            let prompt_text = dialog_config
                                .available_prompts
                                .get(dialog_config.selected_prompt_idx.unwrap_or(0))
                                .map(|p| p.content.clone())
                                .unwrap_or_default();

                            let (coordinator, cancel_flag) =
                                crate::app::batch::coordinator::BatchCoordinator::new(
                                    config,
                                    self.orchestrator.config.clone(),
                                    self.orchestrator.tx.clone(),
                                    self.orchestrator.file_event_bus.clone(),
                                    prompt_text,
                                    std::sync::Arc::new(crate::utils::clock::SystemClock),
                                );
                            let handle = coordinator.execute();
                            self.orchestrator.dialogs.batch_handle = Some(handle);
                            self.orchestrator.dialogs.batch_cancel_flag = Some(cancel_flag);
                        }
                    }
                    crate::app::batch::types::BatchDialogResult::Cancel => {
                        self.orchestrator.dialogs.batch_dialog_open = false;
                        dialog_config.available_prompts.clear();
                        dialog_config.selected_prompt_idx = None;
                    }
                }
            }
            self.orchestrator.dialogs.batch_dialog_config = dialog_config;
        }
    }

    pub(super) fn render_panels(&mut self, parent_ui: &mut egui::Ui) {
        // egui 0.35: each `*Panel` allocates itself from a parent
        // `&mut Ui`; pass the root `Ui` from `App::ui` straight
        // through. The order is preserved from 0.27: top → bottom →
        // right → left → center. Panels must be allocated directly from
        // the parent_ui container, not nested within child_ui scopes.
        show_top_panel(self, parent_ui);
        show_bottom_panel(self, parent_ui);
        show_right_panel(self, parent_ui);
        show_left_panel(self, parent_ui);
        show_center_panel(self, parent_ui);
    }
}
