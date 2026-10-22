//! Font bootstrap for the FastMD user interface.
//!
//! Registers the font inventory held by `crate::ui::fonts` — the embedded
//! Phosphor icon font and the platform symbol-fallback font — through
//! `iced::font::load(bytes)`, which adds them to the cosmic-text runtime
//! font system. The byte-level inventory is shared
//! (`crate::ui::fonts::{PHOSPHOR_REGULAR_FONT, load_fallback_font}`), so
//! there is exactly one font inventory ([RUST-023]).
//!
//! Per-widget font *selection* (`iced::Font::MONOSPACE` / family names)
//! happens in the render code and is out of scope here.
//!
//! Unit tests live in the sibling `fonts_tests.rs` sidecar.

/// Family name under which the embedded Phosphor icon font becomes
/// addressable in iced (`iced::Font::with_name`) once loaded.
pub const PHOSPHOR_FONT_NAME: &str = "phosphor";

/// Returns the font collections to register with the iced runtime at boot.
///
/// Byte inventory, in registration order:
/// 1. the embedded Phosphor Regular icon font (always present),
/// 2. the first discoverable system symbol-fallback font, if any
///    (delegated to `crate::ui::fonts::load_fallback_font`, unchanged).
pub fn font_collections() -> Vec<std::borrow::Cow<'static, [u8]>> {
    let mut collections = vec![std::borrow::Cow::Borrowed(
        crate::ui::fonts::PHOSPHOR_REGULAR_FONT,
    )];
    if let Some(bytes) = crate::ui::fonts::load_fallback_font() {
        collections.push(std::borrow::Cow::Owned(bytes));
    }
    collections
}

/// Formats the outcome of one `iced::font::load` attempt.
///
/// Returns `None` when the collection registered cleanly, `Some(message)`
/// describing the failure otherwise. Failures are non-fatal: iced keeps its
/// default font set and the app remains usable; callers log the
/// message as an ERROR ([NFR-001]).
pub fn font_load_error_message(result: &Result<(), iced::font::Error>) -> Option<String> {
    result
        .err()
        .map(|error| format!("font::load error: {error:?}"))
}

/// Builds the boot task that registers every FastMD font collection with the
/// iced runtime font system.
///
/// Each collection is loaded via `iced::font::load(bytes)` (
/// `iced_runtime-0.14.0/src/font.rs:11`, re-exported as `iced::font::load`
/// through `iced-0.14.0/src/lib.rs:576-580`); the loads are combined with
/// `iced::Task::batch` (`iced_runtime-0.14.0/src/task.rs:88-109`), so the
/// batched task emits ONE outcome per collection (the `Task` pilot/App
/// `boot` maps into its `Message` type; the `(State, Task)` boot contract is
/// verified at `iced-0.14.0/src/application.rs:551-566`).
pub fn font_boot_task() -> iced::Task<Option<String>> {
    let loads = font_collections()
        .into_iter()
        .map(|bytes| {
            // `Task::perform` takes a Future in 0.14 (`task.rs:48-56`); for a
            // `Task` outcome (what `font::load` returns) `.map` is the
            // equivalent adapter (`iced_runtime-0.14.0/src/task.rs:111-117`).
            iced::font::load(bytes).map(|result| font_load_error_message(&result))
        })
        .collect::<Vec<_>>();
    iced::Task::batch(loads)
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
