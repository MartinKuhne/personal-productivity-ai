//! About dialog — application identity, build metadata, license, and third-party attributions.
//!
//! Displayed as an `egui::Window` when `Dialogs.about_dialog_open` is true.
//! Build metadata and license are embedded at compile time (`env!` and
//! `include_str!`). Attributions are rendered from the static
//! `DIRECT_DEPENDENCIES` catalog.
//!
//! Unit tests live in the sibling `about_dialog_tests.rs` sidecar.

use crate::ui::FastMdApp;
use crate::ui::attributions::DIRECT_DEPENDENCIES;
use crate::ui::strings;
use eframe::egui;

/// Full MIT license text bundled at compile time.
const LICENSE_TEXT: &str = include_str!("../../../LICENSE");

/// Git branch captured at compile time via `build.rs` (`cargo:rustc-env`).
const BUILD_BRANCH: &str = env!("BUILD_BRANCH");

/// Full 40-character commit hash captured at compile time.
const BUILD_COMMIT_HASH: &str = env!("BUILD_COMMIT_HASH");

/// Short 7–8 character commit hash for display.
const BUILD_COMMIT_SHORT_HASH: &str = env!("BUILD_COMMIT_SHORT_HASH");

/// Build date in `YYYY-MM-DD` format.
const BUILD_DATE: &str = env!("BUILD_DATE");

/// Current application version, from the `fastmd` package manifest.
pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Seconds the copy confirmation stays visible after a click.
const COPY_CONFIRM_TTL_SECS: f64 = 3.0;

/// Builds the hover tooltip for the commit hash: full hash plus copy hint.
///
/// Pure and unit-tested; see `about_dialog_tests.rs`.
/// Covers spec FR-006 (hover reveals full hash) with FR-015
/// (no inline literals — prefix and hint come from `strings`).
pub(crate) fn commit_tooltip_text() -> String {
    format!(
        "{} {}\n{}",
        strings::ABOUT_FULL_COMMIT_PREFIX,
        BUILD_COMMIT_HASH,
        strings::ABOUT_COPY_COMMIT_TOOLTIP
    )
}

/// Copies the full commit hash to the clipboard and records the copy time
/// so the confirmation label shows. No-op when build metadata is unknown
/// (there is nothing meaningful to copy). Side effects isolated here for
/// testability.
pub(crate) fn copy_commit_hash_to_output(ctx: &egui::Context) {
    if is_build_metadata_unknown() {
        return;
    }
    ctx.copy_text(BUILD_COMMIT_HASH.to_owned());
    let now = ctx.input(|input| input.time);
    ctx.data_mut(|data| {
        data.insert_persisted(egui::Id::new("about_commit_copied_at"), now);
    });
}

/// Returns `true` when a copy happened within [`COPY_CONFIRM_TTL_SECS`].
pub(crate) fn should_show_copy_confirmation(ctx: &egui::Context) -> bool {
    let copied_at: Option<f64> =
        ctx.data_mut(|data| data.get_persisted(egui::Id::new("about_commit_copied_at")));
    match copied_at {
        Some(at) => ctx.input(|input| input.time) - at < COPY_CONFIRM_TTL_SECS,
        None => false,
    }
}

/// Returns `true` when compile-time git metadata was unavailable and
/// `build.rs` fell back to `"unknown"`. Callers render a non-interactive
/// fallback instead of hover/copy affordances (spec FR-007).
pub(crate) fn is_build_metadata_unknown() -> bool {
    BUILD_COMMIT_HASH == "unknown"
}

/// Decides whether the About dialog should open automatically at startup
/// (spec FR-016): `None` (fresh state, or state written before the flag
/// existed) or a version mismatch auto-shows; a match stays quiet.
/// Pure and unit-tested; see `about_dialog_tests.rs`.
pub(crate) fn should_auto_show_about(recorded: Option<&str>, current: &str) -> bool {
    match recorded {
        None => true,
        Some(version) => version != current,
    }
}

/// Applies the first-run auto-show decision to startup state (spec FR-016).
/// When [`should_auto_show_about`] holds for the recorded version, opens
/// the dialog and records `current` so later starts stay quiet.
/// Returns `true` when the dialog was auto-opened.
pub(crate) fn apply_first_run_auto_show(
    persisted: &mut crate::ui::PersistedUiState,
    dialogs: &mut crate::ui::Dialogs,
    current: &str,
) -> bool {
    if should_auto_show_about(persisted.about_shown_for_version.as_deref(), current) {
        dialogs.about_dialog_open = true;
        persisted.about_shown_for_version = Some(current.to_owned());
        true
    } else {
        false
    }
}

/// Shows the About dialog. Called every frame while
/// `app.orchestrator.dialogs.about_dialog_open == true`. The dialog owns its
/// lifecycle: closing via the title-bar `X` sets the flag to `false`.
pub fn show_about_dialog(ctx: &egui::Context, app: &mut FastMdApp) {
    let mut open = app.dialogs().about_dialog_open;

    egui::Window::new(strings::ABOUT_DIALOG_TITLE)
        .id(egui::Id::new("about_dialog"))
        .open(&mut open)
        .resizable(true)
        .default_size([620.0, 580.0])
        .min_size([480.0, 400.0])
        .show(ctx, |ui| {
            render_contents(ui, ctx);
        });

    if !open {
        app.dialogs_mut().about_dialog_open = false;
    }
}

/// Renders the dialog body — header, build metadata, license, and attributions.
fn render_contents(ui: &mut egui::Ui, ctx: &egui::Context) {
    // Header — app name and copyright.
    ui.heading(egui::RichText::new(strings::ABOUT_APP_NAME).strong());
    ui.label(strings::ABOUT_COPYRIGHT);
    ui.separator();

    // Build metadata row.
    ui.horizontal(|ui| {
        ui.label(strings::ABOUT_BRANCH_LABEL);
        ui.label(BUILD_BRANCH);
        ui.separator();
        ui.label(strings::ABOUT_COMMIT_LABEL);
        if is_build_metadata_unknown() {
            // Graceful fallback (spec FR-007): nothing to reveal or copy.
            ui.label(BUILD_COMMIT_SHORT_HASH);
        } else {
            let commit_response = ui
                .add(
                    egui::Label::new(BUILD_COMMIT_SHORT_HASH)
                        .selectable(false)
                        .sense(egui::Sense::click()),
                )
                .on_hover_text(commit_tooltip_text());
            if commit_response.clicked() {
                copy_commit_hash_to_output(ctx);
            }
        }
        // Always allocated; visibility toggles so widget ids stay stable.
        // Shows the copy confirmation (spec FR-006) right after a click.
        ui.add_visible(
            should_show_copy_confirmation(ctx),
            egui::Label::new(strings::ABOUT_COPIED_NOTIFICATION),
        );
        ui.separator();
        ui.label(strings::ABOUT_DATE_LABEL);
        ui.label(BUILD_DATE);
    });

    ui.separator();

    // License section.
    ui.strong(strings::ABOUT_LICENSE_HEADER);
    egui::ScrollArea::vertical()
        .id_salt("about_license_scroll")
        .max_height(140.0)
        .show(ui, |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(LICENSE_TEXT);
            });
        });

    ui.separator();

    // Attributions section.
    ui.strong(strings::ABOUT_ATTRIBUTIONS_HEADER);
    ui.horizontal(|ui| {
        ui.label(strings::ABOUT_COL_CRATE);
        ui.label(strings::ABOUT_COL_AUTHORS);
        ui.label(strings::ABOUT_COL_REPO);
    });
    egui::ScrollArea::vertical()
        .id_salt("about_attributions_scroll")
        .max_height(240.0)
        .show(ui, |ui| {
            for attr in DIRECT_DEPENDENCIES {
                ui.push_id((attr.name, "attribution_row"), |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(attr.name);
                        ui.label(attr.authors);
                        ui.hyperlink_to(attr.github_url, attr.github_url);
                    });
                });
            }
        });
}

#[cfg(test)]
#[path = "about_dialog_tests.rs"]
mod about_dialog_tests;
