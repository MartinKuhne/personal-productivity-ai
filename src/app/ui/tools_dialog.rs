//! Tools dialog — modal window that enumerates every tool group as
//! a proper table (UI-051..060).
//!
//! Columns: checkbox, group, kind, tools, prompt char count, actions
//! (⚠ error indicator + Restart link, Authenticate button when
//! applicable).
//!
//! All state changes go through [`crate::agent::tools::registry`]
//! free functions (which lock the global [`ToolRegistry`]) and
//! [`crate::config::save_config`] (which persists the toggle to
//! `config.yaml`).

use crate::agent::tools::registry::{self, InternalToolGroup};
use crate::bus::events::typed::McpAuthEvent;
use crate::config::{McpServerConfig, save_config};
use crate::ui::FastMdApp;
use crate::ui::strings::{
    TOOLS_AUTH_BUTTON, TOOLS_AUTH_RUNNING, TOOLS_CHAR_COUNT_COLUMN, TOOLS_DIALOG_TITLE,
    TOOLS_FORGET, TOOLS_KIND_INTERNAL, TOOLS_KIND_MCP_REMOTE, TOOLS_KIND_MCP_STDIO,
    TOOLS_LIST_COLUMN, TOOLS_RESTART,
};

use eframe::egui;

/// Render the tools dialog. Called every frame while
/// `app.orchestrator.dialogs.tools_dialog_open == true`. The dialog owns its
/// lifecycle: closing sets the flag to `false`. The title-bar
/// `X` button is the primary close affordance; no extra Close
/// button is rendered at the bottom.
///
/// The window's height is computed from the row count: the
/// dialog grows to fit every entry when the screen has room,
/// and is capped at 85% of the screen height otherwise. The
/// Window only honours `default_size` on the first frame; after
/// that, the user's manual resize takes over.
pub fn show_tools_dialog(ctx: &eframe::egui::Context, app: &mut FastMdApp) {
    let mut open = true;
    let title = TOOLS_DIALOG_TITLE;

    app.orchestrator.tool_context.rcu(|bundle| {
        let mut new_bundle = (**bundle).clone();
        new_bundle
            .registry
            .refresh_state(&app.config().to_agent_config());
        new_bundle
    });
    let groups = app.orchestrator.tool_context.load().registry.groups();
    let (default_size, min_size, max_height) =
        compute_dialog_size(ctx.viewport_rect(), groups.len());

    eframe::egui::Window::new(title)
        .id(eframe::egui::Id::new("tools_dialog"))
        .open(&mut open)
        .resizable(true)
        .default_size(default_size)
        .min_size(min_size)
        .max_height(max_height)
        .show(ctx, |ui| {
            render_contents(ui, app);
        });

    if !open {
        app.dialogs_mut().tools_dialog_open = false;
    }
}

/// Compute the default/min/max sizes for the dialog so that
/// every entry is visible when the screen has room.
///
/// The height is `chrome + header + row_count * row_height`,
/// clamped to `[MIN_HEIGHT, min(screen_height * 0.85, MAX_HEIGHT_HARD_CAP)]`.
/// When the screen is too small, the dialog caps at the
/// available height and the user scrolls within the window.
///
/// Takes a `viewport: egui::Rect` rather than a `&Context` so
/// tests can construct one without a full egui run-loop.
pub(crate) fn compute_dialog_size(
    viewport: eframe::egui::Rect,
    row_count: usize,
) -> ([f32; 2], [f32; 2], f32) {
    /// The row height for tool items in the dialog list.
    const ROW_HEIGHT: f32 = 24.0;
    /// The height of the table header row.
    const HEADER_HEIGHT: f32 = 20.0;
    /// Title bar + window border + interior padding above the
    /// table.
    const CHROME_HEIGHT: f32 = 56.0;
    /// Extra padding for the dialog scroll area.
    const EXTRA_PADDING: f32 = 8.0;
    /// The minimum height of the tools dialog.
    const MIN_HEIGHT: f32 = 200.0;
    /// Hard cap so a 4K monitor doesn't produce a 2000-px-tall
    /// dialog. Kept well above the 85%-of-screen cap on common
    /// monitors (918px on 1080p, 1224px on 1440p) so the dialog
    /// grows to fit every row whenever the screen has room; only
    /// genuinely huge screens hit this and fall back to scrolling
    /// inside the window.
    const MAX_HEIGHT_HARD_CAP: f32 = 1200.0;
    /// The minimum width of the tools dialog.
    const MIN_WIDTH: f32 = 520.0;
    /// The default width of the tools dialog.
    const DEFAULT_WIDTH: f32 = 720.0;

    let preferred_height =
        CHROME_HEIGHT + HEADER_HEIGHT + (row_count as f32) * ROW_HEIGHT + EXTRA_PADDING;
    let screen_max = (viewport.height() * 0.85).max(MIN_HEIGHT);
    let max_height = screen_max.min(MAX_HEIGHT_HARD_CAP);
    let default_height = preferred_height.min(max_height).max(MIN_HEIGHT);
    let min_height = MIN_HEIGHT.min(default_height);

    (
        [DEFAULT_WIDTH, default_height],
        [MIN_WIDTH, min_height],
        max_height,
    )
}

pub fn render_contents(ui: &mut eframe::egui::Ui, app: &mut FastMdApp) {
    // Snapshot the group view under a fresh refresh so we don't
    // hold any lock across UI rendering.
    app.orchestrator.tool_context.rcu(|bundle| {
        let mut new_bundle = (**bundle).clone();
        new_bundle
            .registry
            .refresh_state(&app.config().to_agent_config());
        new_bundle
    });
    let groups = app.orchestrator.tool_context.load().registry.groups();

    if groups.is_empty() {
        ui.label("No tool groups registered.");
        return;
    }

    use egui_extras::{Column, TableBuilder};

    // Use a ScrollArea to ensure the table can scroll when content
    // exceeds the window height. The table itself will use the full
    // available height from the ScrollArea.
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false])
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto().at_least(28.0)) // checkbox
                .column(Column::remainder().at_least(140.0).clip(true)) // group
                .column(Column::auto().at_least(72.0)) // kind
                .column(Column::remainder().at_least(120.0)) // tools
                .column(Column::auto().at_least(80.0)) // prompt char count
                .column(Column::remainder().at_least(140.0)) // actions
                .header(22.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("");
                    });
                    header.col(|ui| {
                        ui.strong("Group");
                    });
                    header.col(|ui| {
                        ui.strong("Kind");
                    });
                    header.col(|ui| {
                        ui.strong(TOOLS_LIST_COLUMN);
                    });
                    header.col(|ui| {
                        ui.strong(TOOLS_CHAR_COUNT_COLUMN);
                    });
                    header.col(|ui| {
                        ui.strong("Actions");
                    });
                })
                .body(|mut body| {
                    // Internal groups first, then MCP servers — both in
                    // their own kind buckets so the table is visually
                    // grouped.
                    let (internals, mcp): (Vec<_>, Vec<_>) = groups
                        .into_iter()
                        .partition(|g| g.kind == registry::ToolGroupKind::Internal);
                    for group in internals.into_iter().chain(mcp) {
                        body.row(26.0, |mut row| {
                            render_row(&mut row, app, &group);
                        });
                    }
                });
        });
}

/// Render one row of the table across its five columns.
fn render_row(
    row: &mut egui_extras::TableRow,
    app: &mut FastMdApp,
    group: &registry::ToolGroupState,
) {
    use registry::{ToolGroupId, ToolGroupKind};

    let id = group.id.clone();
    let prompt = ""; // char count against empty prompt — informational.

    // Column 1 — enable checkbox
    row.col(|ui| {
        let mut enabled = group.enabled;
        if ui.checkbox(&mut enabled, "").changed() {
            let mut new_config = app.config().clone();
            match &id {
                ToolGroupId::Internal(g) => match g {
                    InternalToolGroup::Filesystem => new_config.tool_groups.filesystem = enabled,
                    InternalToolGroup::Web => new_config.tool_groups.web = enabled,
                    InternalToolGroup::Browser => new_config.tool_groups.browser = enabled,
                    InternalToolGroup::Email => new_config.tool_groups.email = enabled,
                    InternalToolGroup::Contacts => new_config.tool_groups.contacts = enabled,
                    InternalToolGroup::Calendar => new_config.tool_groups.calendar = enabled,
                    InternalToolGroup::CsvDb => new_config.tool_groups.csv_db = enabled,
                    InternalToolGroup::Weather => new_config.tool_groups.weather = enabled,
                    InternalToolGroup::Trello => new_config.tool_groups.trello = enabled,
                },
                ToolGroupId::Mcp(name) => {
                    if let Some(entry) = new_config.mcp_servers.get_mut(name) {
                        entry.enabled = enabled;
                    }
                }
            }
            if let Err(e) = save_config(&new_config) {
                tracing::error!(
                    error = %e,
                    "failed to persist AppConfig after tool-group toggle"
                );
            }
            *app.config_mut() = new_config;
        }
    });

    // Column 2 — group display name (with optional ⚠ hover-tooltip
    // for the most recent error)
    row.col(|ui| {
        let response = ui.label(egui::RichText::new(&group.display_name).strong());
        if let Some(err) = &group.last_error {
            response.on_hover_text(format!("{:?}: {}", err.kind, err.message));
        }
    });

    // Column 3 — kind label
    row.col(|ui| {
        let text = match group.kind {
            ToolGroupKind::Internal => TOOLS_KIND_INTERNAL,
            ToolGroupKind::McpStdio => TOOLS_KIND_MCP_STDIO,
            ToolGroupKind::McpRemote => TOOLS_KIND_MCP_REMOTE,
        };
        ui.label(text);
    });

    // Column 4 — comma-separated tool names
    row.col(|ui| {
        ui.add(egui::Label::new(group.tool_names.join(", ")).wrap());
    });

    // Column 5 — prompt char count + parallel-safe chip
    row.col(|ui| {
        let char_count: usize = group
            .tool_names
            .iter()
            .filter_map(|n| {
                app.orchestrator
                    .tool_context
                    .load()
                    .registry
                    .tool_char_count(n, &app.config().to_agent_config(), prompt)
            })
            .sum();
        ui.label(format!("{char_count}"));
    });

    // Column 5 — actions: ⚠ + Restart link (if error), Authenticate
    // button (if eligible MCP), otherwise empty.
    row.col(|ui| {
        ui.horizontal(|ui| {
            if let Some(err) = &group.last_error {
                ui.label(egui::RichText::new("⚠").color(egui::Color32::from_rgb(220, 130, 0)))
                    .on_hover_text(format!("{:?}: {}", err.kind, err.message));
                if ui.small_button(TOOLS_RESTART).clicked() {
                    app.orchestrator.tool_context.rcu(|bundle| {
                        let mut new_bundle = (**bundle).clone();
                        new_bundle.registry.clear_error(&id);
                        new_bundle
                    });
                }
            }

            if let ToolGroupId::Mcp(name) = &id
                && group.needs_auth
                && let Some(entry) = app.config().mcp_servers.get(name)
                && needs_authentication(entry.config())
            {
                let in_progress = app.orchestrator.dialogs.is_oauth_in_progress(name);
                if in_progress {
                    // Flow is running — show a disabled label so the
                    // user knows we are working. Button is not
                    // interactive to prevent double-spawning.
                    ui.add_enabled(false, egui::Button::new(TOOLS_AUTH_RUNNING));
                } else {
                    if ui.button(TOOLS_AUTH_BUTTON).clicked() {
                        app.dialogs_mut().set_oauth_in_progress(name);
                        spawn_auth_flow(
                            name.clone(),
                            app.orchestrator.tx.clone(),
                            ui.ctx().clone(),
                            app.orchestrator.tool_context.load().registry.mcp_manager(),
                        );
                    }
                }
                if ui.small_button(TOOLS_FORGET).clicked() {
                    app.orchestrator
                        .tool_context
                        .load()
                        .registry
                        .mcp_manager()
                        .mark_needs_auth(name, false);
                }
            }
        });
    });
}

fn needs_authentication(cfg: &McpServerConfig) -> bool {
    match cfg {
        McpServerConfig::Stdio { .. } => false,
        McpServerConfig::Sse { headers, .. } => !headers
            .keys()
            .any(|k| k.eq_ignore_ascii_case("authorization")),
    }
}

/// Spawn a background thread that runs the OAuth flow for the given
/// MCP server. On completion the thread sends an `McpAuthEvent::Completed`
/// over the UI channel so `app.rs` can clear the in-progress flag and,
/// if the flow failed, record an error on the group row.
/// `ctx.request_repaint()` is called after sending so egui picks up
/// the state change without waiting for the next timer tick.
fn spawn_auth_flow(
    server_name: String,
    tx: crate::bus::events::typed::BackgroundEventSender,
    ctx: eframe::egui::Context,
    mgr: std::sync::Arc<crate::agent::lib::mcp::McpClients>,
) {
    std::thread::spawn(move || {
        let error = match mgr.authenticate(&server_name) {
            Ok(()) => {
                tracing::info!(server = %server_name, "OAuth flow completed");
                None
            }
            Err(e) => {
                tracing::error!(server = %server_name, error = %e, "OAuth flow failed");
                Some(e)
            }
        };
        let _ = tx.send(McpAuthEvent::Completed { server_name, error }.into());
        ctx.request_repaint();
    });
}
