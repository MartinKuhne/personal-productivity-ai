//! Centralized user intent executor.
//!
//! Drains the `UserCommand` bus and mutates `AppOrchestrator` state.
//! This decouple UI from direct state mutations.

use crate::bus::events::user_command::UserCommand;
use crate::orchestrator::AppOrchestrator;

impl AppOrchestrator {
    /// Apply one user command. Called from `drain_user_command_bus`.
    ///
    /// Owns *all* side effects of user intent. UI panels publish; this is
    /// the single place that mutates orchestrator state in response.
    #[tracing::instrument(skip(self), name = "executor.apply_user_command", level = "debug")]
    pub fn apply_user_command(&mut self, cmd: UserCommand) {
        match cmd {
            UserCommand::RunAgent(prompt) => self.start_agent_session(prompt),
            UserCommand::ShowModels => {
                self.agent.set_status("Done".to_string());
                let models_response =
                    crate::ui::panels::bottom::format_models_list(&self.config.models);
                self.agent.set_response(models_response);
                self.agent_panel_state.show_results = true;
            }
            UserCommand::ShowDeprecatedModelMessage => {
                self.agent.set_status("Error".to_string());
                self.agent
                    .set_response(crate::ui::strings::DEPRECATED_MODEL_MESSAGE.to_string());
                self.agent_panel_state.show_results = true;
            }
            UserCommand::CancelAgent => {
                self.agent.cancel();
            }
            UserCommand::QueueAgentPrompt(prompt) => {
                self.agent.queue_prompt(prompt);
            }
            UserCommand::CloseAllTabs => {
                self.tabs.tabs.clear();
                *self.selection.selected_file_mut() = None;
            }
            UserCommand::CloseTab(idx) => {
                if idx < self.tabs.tabs.len() {
                    self.tabs.tabs.remove(idx);
                }
                if let Some(selected) = self.selection.selected_file_mut().clone() {
                    if !self.tabs.tabs.contains(&selected) {
                        *self.selection.selected_file_mut() = self.tabs.tabs.last().cloned();
                    }
                } else if !self.tabs.tabs.is_empty() {
                    *self.selection.selected_file_mut() = self.tabs.tabs.last().cloned();
                } else {
                    *self.selection.selected_file_mut() = None;
                }
            }
            UserCommand::CloseOtherTabs(idx) => {
                if idx < self.tabs.tabs.len() {
                    let keep = self.tabs.tabs[idx].clone();
                    self.tabs.tabs.clear();
                    self.tabs.tabs.push(keep.clone());
                    *self.selection.selected_file_mut() = Some(keep);
                }
            }
            UserCommand::ScrollToHeader(entry_id) => {
                self.tabs.scroll_to_header_id = Some(entry_id);
            }
            UserCommand::OpenBatchDialog => {
                self.dialogs.batch_dialog_open = true;
            }
            UserCommand::OpenToolsDialog => {
                self.dialogs.tools_dialog_open = true;
            }
            UserCommand::OpenAboutDialog => {
                self.dialogs.about_dialog_open = true;
            }
            UserCommand::ToggleBackgroundLogs(show) => {
                self.background_manager.lock().unwrap().show_background_logs = show;
            }
            UserCommand::ToggleAgentDebugWindow(show) => {
                self.agent_panel_state.show_debug_window = show;
            }
            UserCommand::ToggleAgentDebugWindowShortcut => {
                self.agent_panel_state.show_debug_window =
                    !self.agent_panel_state.show_debug_window;
            }
            UserCommand::SelectChatModel(model_name) => {
                let mut new_config = self.config.clone();
                if new_config.selected_chat_model.as_deref() != Some(model_name.as_str()) {
                    new_config.selected_chat_model = Some(model_name);
                    self.agent.set_agent_config(new_config.to_agent_config());
                    self.config = new_config;
                }
            }
            UserCommand::ChangeTableWidthStrategy(strategy) => {
                let new_value = strategy.to_config();
                let mut new_config = self.config.clone();
                if new_config.table_width_strategy != new_value {
                    new_config.table_width_strategy = new_value.to_string();
                    if let Err(e) = self.config_storage.save_config(&new_config) {
                        tracing::error!(
                            error = %e,
                            "failed to persist AppConfig after table-width strategy change"
                        );
                    }
                    self.config = new_config;
                }
            }

            UserCommand::ConfirmMove { file, destination } => {
                let mut new_path = destination.clone();
                new_path.push(file.file_name().unwrap());
                if let Err(e) = std::fs::rename(&file, &new_path) {
                    tracing::error!(
                        name = "ui.file.move_failed",
                        source = %file.display(),
                        destination = %new_path.display(),
                        error = %e,
                        "Failed to move file. Likely cause: permission denied or file in use. Operator should check file locks."
                    );
                } else {
                    let producer = crate::bus::events::file::FileEventProducer::new(
                        self.file_event_bus.clone(),
                    );
                    producer.publish_rename(&file, &new_path);
                    if self.tabs.loaded_path.as_ref() == Some(&file) {
                        self.tabs.loaded_path = Some(new_path.clone());
                    }
                    if self.selection.selected_file.as_ref() == Some(&file) {
                        self.selection.selected_file = Some(new_path.clone());
                    }
                    if self.selection.selected_dir.as_ref() == Some(&file) {
                        self.selection.selected_dir = Some(new_path.clone());
                    }
                    for tab in self.tabs.tabs.iter_mut() {
                        if *tab == file {
                            *tab = new_path.clone();
                        }
                    }
                    self.file_processor.remove_file(&file);
                    if self.file_processor.contains_dir(&file) {
                        self.file_processor.remove_dir(&file);
                        self.file_processor.add_dir(new_path.clone());
                    }
                    let ext = new_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "md" || ext == "markdown" {
                        self.file_processor.add_file(new_path.clone());
                    }
                    let tags = crate::utils::tags::extract_tags_from_file(&new_path);
                    self.tags.remove_file(&file);
                    self.tags.add_tags(new_path.clone(), tags);
                    if self.selection.expanded_dirs.remove(&file) {
                        self.selection.expanded_dirs.insert(new_path.clone());
                    }
                }
            }
            UserCommand::SelectFile { path, multi } => {
                if multi {
                    if self.selection.selected_files.contains(&path) {
                        self.selection.selected_files.remove(&path);
                        if self.selection.selected_file == Some(path.clone()) {
                            self.selection.selected_file =
                                self.selection.selected_files.iter().next().cloned();
                        }
                    } else {
                        self.selection.selected_files.insert(path.clone());
                        self.selection.selected_file = Some(path.clone());
                    }
                } else {
                    self.selection.selected_files.clear();
                    self.selection.selected_files.insert(path.clone());
                    self.selection.selected_file = Some(path.clone());
                }

                self.selection.selected_dir = path.parent().map(|p| p.to_path_buf());

                // Also open the file tab
                self.tabs.open_tab(path);
            }
            UserCommand::SelectDirectory {
                path,
                toggle_expand,
            } => {
                self.selection.tree_dirty = true;
                if toggle_expand {
                    if self.selection.expanded_dirs.contains(&path) {
                        self.selection.expanded_dirs.remove(&path);
                    } else {
                        self.selection.expanded_dirs.insert(path.clone());
                    }
                }

                self.selection.selected_dir = Some(path.clone());
                // Opening a directory clears file selection
                self.selection.selected_files.clear();
                self.selection.selected_file = None;
            }
            UserCommand::OpenInEditor(path) => {
                if self.inline_editor_enabled {
                    if let Ok(file_content) = std::fs::read_to_string(&path) {
                        let _is_pdf_backed = self.pdf_backing_tracker.is_pdf_backed(&path);
                        self.text_buffer.open(
                            &path,
                            &file_content,
                            Some(&self.pdf_backing_tracker),
                        );
                        if !self.tabs.tabs.contains(&path) {
                            self.tabs.tabs.push(path.clone());
                        }
                        self.tabs.loaded_path = Some(path);
                    }
                } else {
                    crate::ui::open_in_system_editor(&path);
                }
            }
            UserCommand::ShowInExplorer(path) => {
                crate::ui::show_in_file_explorer(&path);
            }
            UserCommand::CopyPath(path) => {
                let path_str = path.to_string_lossy().to_string();
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(path_str);
                } else {
                    tracing::error!("Failed to initialize clipboard for CopyPath");
                }
            }
            #[cfg(feature = "pdf-export")]
            UserCommand::SaveAsPdf(path) => {
                let path_to_export = path;
                let default_dir = path_to_export.parent().map(|p| p.to_path_buf());
                let default_name = path_to_export
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("document");

                let target_path = rfd::FileDialog::new()
                    .set_directory(default_dir.as_deref().unwrap_or(std::path::Path::new(".")))
                    .set_file_name(default_name)
                    .add_filter("PDF document", &["pdf"])
                    .save_file();

                if let Some(target) = target_path {
                    let mut job =
                        crate::export::pdf::SaveAsPdfJob::from_path(path_to_export.clone());
                    job.output_path = Some(target);
                    let _ = crate::export::pdf::execute_save_as_pdf_blocking(
                        job,
                        Some(self.tx.clone()),
                    );
                }
            }
            #[cfg(not(feature = "pdf-export"))]
            UserCommand::SaveAsPdf(_) => {}
            UserCommand::Rename(path) => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                self.dialogs.file_to_rename = Some(path.clone());
                self.dialogs.rename_new_name =
                    crate::ui::tree::flatten::initial_rename_value(&path, name);
                self.dialogs.rename_dialog_open = true;
            }
            UserCommand::Move(path) => {
                self.dialogs.file_to_move = Some(path);
                self.dialogs.move_dialog_open = true;
            }
            UserCommand::Delete(path) => {
                if let Err(e) = crate::utils::recycle_bin::delete(&path) {
                    tracing::error!(
                        name = "ui.file.delete_failed",
                        path = %path.display(),
                        error = %e,
                        "Failed to delete to trash"
                    );
                } else {
                    self.file_event_bus
                        .publish(crate::bus::events::file::FileEvent::removed_one(path));
                }
            }
            UserCommand::CreateDirectory { parent } => {
                self.dialogs.create_dir_parent = Some(parent);
                self.dialogs.create_dir_dialog_open = true;
            }
            UserCommand::CreateDocument { parent } => {
                self.dialogs.create_document_parent = Some(parent);
                self.dialogs.create_document_dialog_open = true;
            }
            UserCommand::RunSkillPrompt {
                content,
                target_dir,
                target_file,
            } => {
                if let Some(f) = target_file {
                    self.selection.selected_files.clear();
                    self.selection.selected_files.insert(f.clone());
                    self.selection.selected_file = Some(f);
                } else if let Some(d) = target_dir {
                    self.selection.selected_dir = Some(d);
                    self.selection.selected_files.clear();
                    self.selection.selected_file = None;
                }
                self.start_agent_session(content);
            }
            UserCommand::MergePrompt(files) => {
                let prompt = crate::ui::tree::handlers::build_merge_prompt(
                    &self.config.content_libraries,
                    &files.into_iter().collect(),
                );
                self.start_agent_session(prompt);
            }
            UserCommand::ConfirmCreateDirectory { parent, name } => {
                if !crate::utils::path::is_safe_basename(&name) {
                    tracing::warn!(
                        name = "ui.directory.invalid_name",
                        name_input = %name,
                        "User attempted to create directory with invalid characters. Operation skipped. Operator should advise user of valid names."
                    );
                } else {
                    let new_dir_path = parent.join(&name);
                    if let Err(e) = std::fs::create_dir_all(&new_dir_path) {
                        tracing::error!(
                            name = "ui.directory.create_failed",
                            path = %new_dir_path.display(),
                            error = %e,
                            "Failed to create new directory. Likely cause: permission denied or invalid path. Operator should verify permissions on parent directory."
                        );
                    } else {
                        self.file_processor.add_dir(new_dir_path.clone());
                        let producer = crate::bus::events::file::FileEventProducer::new(
                            self.file_event_bus.clone(),
                        );
                        producer.publish_dir_discovered(&new_dir_path);
                        if let Some(watcher) = &mut self._watcher {
                            use notify::Watcher;
                            let _ = watcher.watch(&new_dir_path, notify::RecursiveMode::Recursive);
                        }
                    }
                }
            }
            UserCommand::ConfirmCreateDocument { parent, name } => {
                if !crate::utils::path::is_safe_basename(&name) {
                    tracing::warn!(
                        name = "ui.file.invalid_name",
                        name_input = %name,
                        "User attempted to create document with invalid characters. Operation skipped. Operator should advise user of valid names."
                    );
                } else {
                    match crate::ui::modals::write_new_document(&parent, &name) {
                        Ok(new_path) => {
                            let producer = crate::bus::events::file::FileEventProducer::new(
                                self.file_event_bus.clone(),
                            );
                            producer.publish_discovered(&new_path);
                        }
                        Err(e) => tracing::error!(
                            name = "ui.file.create_failed",
                            parent = %parent.display(),
                            error = %e,
                            "Failed to create new document. Likely cause: permission denied or disk full. Operator should verify directory permissions."
                        ),
                    }
                }
            }
            UserCommand::ConfirmRename {
                path: file,
                new_name,
            } => {
                if !crate::utils::path::is_safe_basename(&new_name) {
                    tracing::warn!(
                        name = "ui.file.invalid_rename",
                        name_input = %new_name,
                        "User attempted to rename file with invalid characters. Operation skipped. Operator should advise user of valid names."
                    );
                } else {
                    let ext = file
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| format!(".{}", e))
                        .unwrap_or_default();
                    let new_name_with_ext = format!("{}{}", new_name, ext);
                    let mut new_path = file.clone();
                    new_path.set_file_name(&new_name_with_ext);
                    if let Err(e) = std::fs::rename(&file, &new_path) {
                        tracing::error!(
                            name = "ui.file.rename_failed",
                            source = %file.display(),
                            destination = %new_path.display(),
                            error = %e,
                            "Failed to rename file. Likely cause: permission denied or file in use. Operator should check file locks."
                        );
                    } else {
                        let producer = crate::bus::events::file::FileEventProducer::new(
                            self.file_event_bus.clone(),
                        );
                        producer.publish_rename(&file, &new_path);
                        if self.tabs.loaded_path.as_ref() == Some(&file) {
                            self.tabs.loaded_path = Some(new_path.clone());
                        }
                        if self.selection.selected_file.as_ref() == Some(&file) {
                            self.selection.selected_file = Some(new_path.clone());
                        }
                        if self.selection.selected_dir.as_ref() == Some(&file) {
                            self.selection.selected_dir = Some(new_path.clone());
                        }
                        for tab in self.tabs.tabs.iter_mut() {
                            if *tab == file {
                                *tab = new_path.clone();
                            }
                        }
                        self.file_processor.remove_file(&file);
                        if self.file_processor.contains_dir(&file) {
                            self.file_processor.remove_dir(&file);
                            self.file_processor.add_dir(new_path.clone());
                        }
                        let ext = new_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        if ext == "md" || ext == "markdown" {
                            self.file_processor.add_file(new_path.clone());
                        }
                        let tags = crate::utils::tags::extract_tags_from_file(&new_path);
                        self.tags.remove_file(&file);
                        self.tags.add_tags(new_path.clone(), tags);
                        if self.selection.expanded_dirs.remove(&file) {
                            self.selection.expanded_dirs.insert(new_path.clone());
                        }
                    }
                }
            }

            UserCommand::SetToolGroupEnabled { id, enabled } => {
                let mut new_config = self.config.clone();
                match &id {
                    crate::agent::tools::registry::ToolGroupId::Internal(g) => match g {
                        crate::agent::tools::registry::InternalToolGroup::Filesystem => {
                            new_config.tool_groups.filesystem = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::Web => {
                            new_config.tool_groups.web = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::Browser => {
                            new_config.tool_groups.browser = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::Email => {
                            new_config.tool_groups.email = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::Contacts => {
                            new_config.tool_groups.contacts = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::Calendar => {
                            new_config.tool_groups.calendar = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::CsvDb => {
                            new_config.tool_groups.csv_db = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::Weather => {
                            new_config.tool_groups.weather = enabled
                        }
                        crate::agent::tools::registry::InternalToolGroup::Trello => {
                            new_config.tool_groups.trello = enabled
                        }
                    },
                    crate::agent::tools::registry::ToolGroupId::Mcp(name) => {
                        if let Some(entry) = new_config.mcp_servers.get_mut(name) {
                            entry.enabled = enabled;
                        }
                    }
                }
                if let Err(e) = self.config_storage.save_config(&new_config) {
                    tracing::error!(
                        error = %e,
                        "failed to persist AppConfig after tool-group toggle"
                    );
                }
                self.config = new_config;
                self.tool_context.rcu(|bundle| {
                    let new_bundle = (**bundle).clone();
                    // Just mutating registry to match, wait, set_group_enabled takes agent config
                    // Let's pass the new config converted
                    let mut agent_cfg = self.config.to_agent_config();
                    new_bundle
                        .registry
                        .set_group_enabled(&mut agent_cfg, &id, enabled);
                    new_bundle
                });
            }
            UserCommand::ClearToolGroupError(id) => {
                self.tool_context.rcu(|bundle| {
                    let mut new_bundle = (**bundle).clone();
                    new_bundle.registry.clear_error(&id);
                    new_bundle
                });
            }
            UserCommand::StartMcpAuth(name) => {
                self.dialogs.set_oauth_in_progress(&name);

                // Spawn auth flow
                let server_name = name.clone();
                let tx = self.tx.clone();
                let mgr = self.tool_context.load().registry.mcp_manager();
                std::thread::spawn(move || {
                    let error = match crate::agent::tools::blocking::block_on(async {
                        mgr.authenticate(&server_name).await
                    }) {
                        Ok(()) => {
                            tracing::info!(server = %server_name, "OAuth flow completed");
                            None
                        }
                        Err(e) => {
                            tracing::error!(server = %server_name, error = %e, "OAuth flow failed");
                            Some(e)
                        }
                    };
                    let _ = tx.send(
                        crate::bus::events::typed::McpAuthEvent::Completed { server_name, error }
                            .into(),
                    );
                });
            }
            UserCommand::ForgetMcpAuth(name) => {
                self.tool_context
                    .load()
                    .registry
                    .mcp_manager()
                    .mark_needs_auth(&name, false);
            }
            UserCommand::StartBatch(config) => {
                if self.dialogs.batch_handle.is_none() {
                    let prompt_text = self
                        .dialogs
                        .batch_dialog_config
                        .available_prompts
                        .get(
                            self.dialogs
                                .batch_dialog_config
                                .selected_prompt_idx
                                .unwrap_or(0),
                        )
                        .map(|p| p.content.clone())
                        .unwrap_or_default();
                    let (coordinator, cancel_flag) =
                        crate::agent::batch::coordinator::BatchCoordinator::new(
                            config,
                            self.config.clone(),
                            self.tx.clone(),
                            self.file_event_bus.clone(),
                            prompt_text,
                            std::sync::Arc::new(crate::utils::clock::SystemClock),
                        );
                    let handle = coordinator.execute();
                    self.dialogs.batch_handle = Some(handle);
                    self.dialogs.batch_cancel_flag = Some(cancel_flag);
                }
            }
            UserCommand::CancelBatch => {
                self.dialogs.batch_dialog_open = false;
                self.dialogs.batch_dialog_config.available_prompts.clear();
                self.dialogs.batch_dialog_config.selected_prompt_idx = None;
            }
            UserCommand::SelectTagFilter(tag) => {
                self.tags.selected_tag = tag;
            }
            UserCommand::ClearAgentSession => {
                self.agent_panel_state.show_results = false;
                self.agent_panel_state.scroll_to_id = None;
                self.agent.clear_history();
                self.agent.set_response(String::new());
                self.agent.set_thinking(String::new());
                self.agent_transcript.reset();
                if self.agent.state().running {
                    self.agent.cancel();
                }
            }
            UserCommand::ApplyTaskToggles { toggles } => {
                for (idx, checked) in toggles {
                    crate::ui::render::apply_task_toggle(
                        &mut self.agent_transcript.content,
                        idx,
                        checked,
                    );
                }
            }
            UserCommand::ClearCommandInput => {
                self.agent_panel_state.command_input.clear();
            }
            UserCommand::ToggleDebugWindow(show) => {
                self.agent_panel_state.show_debug_window = show;
            }
            UserCommand::ClearDebugEntries => {
                self.agent.state_mut().debug_entries.clear();
            }
            UserCommand::SetDebugJsonRows(rows) => {
                self.agent_panel_state.debug_json_rows = rows;
            }
            UserCommand::SetDebugSearchText(text) => {
                self.agent_panel_state.debug_search_text = text;
            }
            UserCommand::SetDebugAutoScroll(auto) => {
                self.agent_panel_state.debug_auto_scroll = auto;
            }
        }
    }
}
