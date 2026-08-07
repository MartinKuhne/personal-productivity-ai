//! `eframe::App` implementation.
//!
//! Owns all mutable state (auth, loaded tree, current selection) and wires
//! the UI to the background workers that perform the blocking network
//! calls. The model is: UI thread fires-and-forgets into an `mpsc` channel;
//! a `std::thread` does the work; the result is delivered back into the
//! next frame's `logic` call via `try_recv`.
//!
//! On Android the activity's current `Intent` is polled every frame so the
//! `msauth://` redirect from the system browser gets picked up and traded
//! for an access token. The polling itself lives behind
//! `cfg(target_os = "android")` and is a no-op on host builds.

use std::collections::HashSet;
use std::sync::mpsc::{self, TryRecvError};

use eframe::egui;

use crate::auth::{build_authorize_url, exchange_code, AuthCallback, PkceSession, TokenSet};
#[cfg(target_os = "android")]
use crate::auth::parse_auth_code_from_uri;
use crate::config::AuthConfig;
use crate::error::AppError;
use crate::file_node::FileNode;
use crate::onedrive::OneDriveClient;

#[cfg(target_os = "android")]
use crate::android;

/// Top-level UI state. Drives which screen is rendered and which
/// background jobs are in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppScreen {
    SignIn,
    AwaitingBrowser,
    Authenticated,
}

pub struct FastMdApp {
    cfg: AuthConfig,
    screen: AppScreen,

    // Auth
    pkce: Option<PkceSession>,
    token: Option<TokenSet>,

    // Tree + selection
    root_folder_input: String,
    root_node: Option<FileNode>,
    expanded: HashSet<String>,
    selection: Option<(String, String)>,
    is_loading_tree: bool,
    is_loading_content: bool,

    // Error display (transient; cleared by user)
    error_message: Option<String>,

    // Background workers
    auth_rx: Option<mpsc::Receiver<Result<TokenSet, String>>>,
    tree_rx: Option<mpsc::Receiver<Result<FileNode, String>>>,
    content_rx: Option<mpsc::Receiver<Result<(String, String), String>>>,

    // Deep-link bookkeeping: the URI of the last intent we acted on, so
    // the poll loop doesn't reprocess the same callback on every frame.
    #[cfg(target_os = "android")]
    last_deep_link: Option<String>,
}

impl FastMdApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let cfg = AuthConfig::load_bundled().unwrap_or_else(|e| {
            tracing::error!("failed to load bundled auth config: {e}");
            // Fall back to a stub so the app still launches; sign-in will
            // fail with a clear error when the user clicks the button.
            AuthConfig {
                client_id: AuthConfig::PLACEHOLDER_CLIENT_ID.to_string(),
                redirect_uri: "msauth://com.fastmd.android.egui/signature_hash_here".to_string(),
                authorization_user_agent: "DEFAULT".to_string(),
                authorities: vec![],
            }
        });

        Self {
            cfg,
            screen: AppScreen::SignIn,
            pkce: None,
            token: None,
            root_folder_input: "./Wiki".to_string(),
            root_node: None,
            expanded: HashSet::new(),
            selection: None,
            is_loading_tree: false,
            is_loading_content: false,
            error_message: None,
            auth_rx: None,
            tree_rx: None,
            content_rx: None,
            #[cfg(target_os = "android")]
            last_deep_link: None,
        }
    }

    // -----------------------------------------------------------------
    // Sign-in flow
    // -----------------------------------------------------------------
    fn begin_sign_in(&mut self) {
        match PkceSession::generate() {
            Ok(pkce) => {
                let url = match build_authorize_url(&self.cfg, &pkce) {
                    Ok(u) => u,
                    Err(e) => {
                        self.error_message = Some(e.to_string());
                        return;
                    }
                };

                // Android: dispatch an Intent.ACTION_VIEW through the
                // system browser. Host (cargo test / desktop preview): open
                // the URL in the default browser via `webbrowser`.
                #[cfg(target_os = "android")]
                let launch_result = crate::android::open_in_browser(url.as_str())
                    .map_err(|e| format!("open browser: {e}"));
                #[cfg(not(target_os = "android"))]
                let launch_result = webbrowser::open(url.as_str())
                    .map_err(|e| format!("open browser: {e}"));

                if let Err(e) = launch_result {
                    self.error_message = Some(e);
                    return;
                }

                self.pkce = Some(pkce);
                self.screen = AppScreen::AwaitingBrowser;
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("PKCE generate: {e}"));
            }
        }
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn complete_sign_in(&mut self, code: String) {
        let cfg = self.cfg.clone();
        let pkce = match self.pkce.take() {
            Some(p) => p,
            None => {
                self.error_message = Some("internal: no PKCE session".to_string());
                return;
            }
        };

        let (tx, rx) = mpsc::channel();
        self.auth_rx = Some(rx);
        self.screen = AppScreen::AwaitingBrowser;

        std::thread::spawn(move || {
            let result = exchange_code(&cfg, &pkce, &code)
                .map_err(|e: AppError| e.to_string());
            let _ = tx.send(result);
        });
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn handle_auth_callback(&mut self, callback: AuthCallback) {
        // Validate the state matches what we sent. Microsoft puts the state
        // in the redirect query; if it doesn't match, treat it as a CSRF
        // attempt and refuse to exchange.
        let matches = self
            .pkce
            .as_ref()
            .map(|p| p.state == callback.state)
            .unwrap_or(false);
        if !matches {
            self.error_message = Some("auth state mismatch (CSRF check failed)".to_string());
            self.pkce = None;
            return;
        }
        self.complete_sign_in(callback.code);
    }

    // -----------------------------------------------------------------
    // Tree + content
    // -----------------------------------------------------------------
    fn load_folder(&mut self) {
        let token = match self.token.as_ref() {
            Some(t) if !t.is_expired() => t.access_token.clone(),
            _ => {
                self.error_message = Some("not signed in (or token expired)".to_string());
                return;
            }
        };
        let folder = self.root_folder_input.clone();
        let (tx, rx) = mpsc::channel();
        self.tree_rx = Some(rx);
        self.is_loading_tree = true;
        self.root_node = None;
        self.expanded.clear();
        self.selection = None;

        std::thread::spawn(move || {
            let client = OneDriveClient::new(&token);
            let result = client
                .fetch_root_tree(&folder)
                .and_then(|raw| {
                    crate::file_node::FileTreeProcessor::process_tree(raw)
                        .ok_or_else(|| AppError::Graph("tree empty after filter".to_string()))
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn on_file_click(&mut self, file: &FileNode) {
        let Some(download_url) = file.download_url.clone() else {
            self.error_message = Some(format!("file '{}' has no download URL", file.name));
            return;
        };
        let token = match self.token.as_ref() {
            Some(t) if !t.is_expired() => t.access_token.clone(),
            _ => {
                self.error_message = Some("not signed in (or token expired)".to_string());
                return;
            }
        };
        let name = file.name.clone();
        let (tx, rx) = mpsc::channel();
        self.content_rx = Some(rx);
        self.is_loading_content = true;

        std::thread::spawn(move || {
            let client = OneDriveClient::new(&token);
            let result = client
                .fetch_file_content(&download_url)
                .map(|content| (name, content))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    // -----------------------------------------------------------------
    // Polling
    // -----------------------------------------------------------------
    fn drain_channels(&mut self) {
        if let Some(rx) = &self.auth_rx {
            match rx.try_recv() {
                Ok(Ok(token)) => {
                    self.token = Some(token);
                    self.auth_rx = None;
                    self.screen = AppScreen::Authenticated;
                }
                Ok(Err(e)) => {
                    self.auth_rx = None;
                    self.screen = AppScreen::SignIn;
                    self.error_message = Some(format!("auth: {e}"));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.auth_rx = None;
                    self.screen = AppScreen::SignIn;
                    self.error_message = Some("auth worker disconnected".to_string());
                }
            }
        }

        if let Some(rx) = &self.tree_rx {
            match rx.try_recv() {
                Ok(Ok(node)) => {
                    self.tree_rx = None;
                    self.is_loading_tree = false;
                    self.root_node = Some(node);
                }
                Ok(Err(e)) => {
                    self.tree_rx = None;
                    self.is_loading_tree = false;
                    self.error_message = Some(format!("load folder: {e}"));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.tree_rx = None;
                    self.is_loading_tree = false;
                    self.error_message = Some("tree worker disconnected".to_string());
                }
            }
        }

        if let Some(rx) = &self.content_rx {
            match rx.try_recv() {
                Ok(Ok(selection)) => {
                    self.content_rx = None;
                    self.is_loading_content = false;
                    self.selection = Some(selection);
                }
                Ok(Err(e)) => {
                    self.content_rx = None;
                    self.is_loading_content = false;
                    self.error_message = Some(format!("download: {e}"));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.content_rx = None;
                    self.is_loading_content = false;
                    self.error_message = Some("content worker disconnected".to_string());
                }
            }
        }
    }

    #[cfg(target_os = "android")]
    fn poll_deep_link(&mut self) {
        // Read the activity's current intent URI via JNI. If it's a new
        // `msauth://...` URI (different from the one we last saw), parse
        // the auth code and kick off the token exchange.
        let uri = match android::current_intent_uri() {
            Ok(Some(uri)) => uri,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!("deep-link poll: {e}");
                return;
            }
        };
        if uri.is_empty() || self.last_deep_link.as_deref() == Some(uri.as_str()) {
            return;
        }
        self.last_deep_link = Some(uri.clone());
        match parse_auth_code_from_uri(&uri) {
            Ok(callback) => self.handle_auth_callback(callback),
            Err(e) => self.error_message = Some(e.to_string()),
        }
    }

    // -----------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------
    fn render_authenticated(&mut self, ui: &mut egui::Ui) {
        let total = ui.available_size();
        let left_w = (total.x * 0.4).max(220.0);

        // --- Left pane: folder input + tree ---
        egui::Panel::left("left_pane")
            .resizable(false)
            .default_size(left_w)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Root folder")
                                .color(egui::Color32::from_rgb(0xC0, 0xC0, 0xC8)),
                        );
                    });
                    ui.add(
                        egui::TextEdit::singleline(&mut self.root_folder_input)
                            .hint_text("./Wiki")
                            .desired_width(ui.available_width()),
                    );
                    let load_btn = egui::Button::new("Load Folder")
                        .fill(egui::Color32::from_rgb(0x00, 0x78, 0xD4))
                        .min_size(egui::vec2(0.0, 32.0));
                    if ui.add(load_btn).clicked() {
                        self.load_folder();
                    }
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if self.is_loading_tree {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Loading tree…");
                                });
                            } else if let Some(node) = &self.root_node {
                                let clicked =
                                    crate::ui::file_tree::render(ui, node, &mut self.expanded);
                                for file in clicked {
                                    self.on_file_click(&file);
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("No folder loaded yet")
                                        .italics()
                                        .color(egui::Color32::from_rgb(0x80, 0x80, 0x88)),
                                );
                            }
                        });
                });
            });

        // --- Right pane: viewer ---
        egui::CentralPanel::default().show(ui, |ui| {
            if self.is_loading_content {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Downloading…");
                });
            } else {
                crate::ui::file_viewer::render(ui, &self.selection);
            }
        });
    }
}

impl eframe::App for FastMdApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain background workers and poll the deep link. The eframe
        // `logic` hook is the right place for per-frame state changes
        // that need the `Context` (e.g. `request_repaint_after`).
        self.drain_channels();
        #[cfg(target_os = "android")]
        self.poll_deep_link();

        // Keep the UI repainting while background work is in flight so the
        // `try_recv` polls don't have to wait for user input.
        if self.auth_rx.is_some() || self.tree_rx.is_some() || self.content_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Dark background, matching the Kotlin app's `darkColorScheme()`.
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            0.0,
            egui::Color32::from_rgb(0x1E, 0x1E, 0x22),
        );

        match self.screen {
            AppScreen::SignIn => {
                let mut clicked = false;
                crate::ui::sign_in::render(ui, || clicked = true);
                if clicked {
                    self.begin_sign_in();
                }
            }
            AppScreen::AwaitingBrowser => {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.4);
                    ui.label(
                        egui::RichText::new("Complete sign-in in your browser…")
                            .size(18.0)
                            .color(egui::Color32::from_rgb(0xE0, 0xE0, 0xE6)),
                    );
                    ui.add_space(8.0);
                    ui.spinner();
                });
            }
            AppScreen::Authenticated => {
                self.render_authenticated(ui);
            }
        }

        if let Some(msg) = self.error_message.clone() {
            let mut open = true;
            egui::Window::new("Error")
                .open(&mut open)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .resizable(false)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label(&msg);
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        self.error_message = None;
                    }
                });
            if !open {
                self.error_message = None;
            }
        }
    }
}
