//! Entry point of the `hoarfrost` showcase application.
//!
//! Deliberately thin: only the iced builder chain and the module graph live
//! here; every gram of state, view and data lives in the modules it
//! declares (`RUST-054` — an entry point composes, it does not implement).
//!
//! Run it with `cargo run` from `src/hoarfrost-demo`.

#[cfg(test)]
mod alignment_audit;
mod app;
mod chrome;
mod data;
mod font_fallback;
#[cfg(test)]
mod glyph_resolution;
#[cfg(test)]
mod id_namespace;
#[cfg(test)]
mod layout_gate;
#[cfg(test)]
mod layout_probe;
mod menu_surface;
mod pane_atoms;
mod pane_composites;
mod previews;
#[cfg(test)]
mod render_atoms;
#[cfg(test)]
mod render_capture;
#[cfg(test)]
mod render_composites;
#[cfg(test)]
mod render_section;
#[cfg(test)]
mod render_stepper;
#[cfg(test)]
mod render_surfaces;
mod section;
mod theme;

/// Opens the demo window.
///
/// The theme is the hoarfrost-token-derived iced theme from
/// [`crate::theme`], so iced's own derived widget styles resolve against
/// the same palette the framework widgets read.
///
/// The symbol fallback font is registered inside
/// [`DemoApp::boot`](app::DemoApp::boot), not here: `boot` is the single
/// init path the window and the headless tests share, and iced needs no
/// window and no renderer before a face can be added to its process-wide
/// font system.
fn main() -> iced::Result {
    iced::application(app::DemoApp::boot, app::DemoApp::update, app::DemoApp::view)
        .title(app::strings::WINDOW_TITLE)
        .theme(|_app: &app::DemoApp| theme::demo_theme())
        .run()
}
