//! Font bootstrap for the FastMD user interface.
//!
//! Registers the font inventory held by `crate::ui::fonts` — the embedded
//! Phosphor icon font and the platform symbol-fallback font — into the
//! process-wide iced font system. The byte-level inventory is shared
//! (`crate::ui::fonts::{PHOSPHOR_REGULAR_FONT, load_fallback_font}`), so
//! there is exactly one font inventory ([RUST-023]).
//!
//! # Why registration is synchronous
//!
//! `iced::font::load(bytes)` (`iced_runtime-0.14.0/src/font.rs:11-16`) is
//! `task::oneshot(|channel| Action::LoadFont { bytes, channel })`: the bytes
//! are **not** in the font database when the call returns. They are added
//! only once the iced scheduler polls the task and the winit event loop
//! reaches `Action::LoadFont` (`iced_winit-0.14.1/src/lib.rs:1777-1784`),
//! which is after `boot` has returned and, on a loaded machine, after the
//! first frame has already resolved glyphs. Any glyph shaped before the task
//! lands is shaped against a database that does not contain the FastMD
//! collections.
//!
//! That is the same hazard that made the startup `ConfigArrived` publish
//! synchronous — see the note in `app::build_state`, which records that a
//! boot task "can land seconds after `boot` returns (behind the
//! font-registration boot task)". Registration therefore follows the same
//! rule: it happens inside [`register_fonts`], which `build_state` calls
//! before it returns, so the faces are in the system before the first frame
//! resolves anything.
//!
//! Registration is idempotent: `REGISTRATION` latches the outcome, so
//! the production boot plus every headless test fixture add each face
//! exactly once. That latch is load-bearing — `FontSystem::load_font`
//! de-duplicates only `Cow::Borrowed` bytes, by pointer
//! (`iced_graphics-0.14.0/src/text.rs:150-164`), so the `Cow::Owned`
//! fallback would otherwise re-parse and re-add its faces on every call.
//!
//! Per-widget font *selection* (`iced::Font::MONOSPACE` / family names)
//! happens in the render code and is out of scope here.
//!
//! Unit tests live in the sibling `fonts_tests.rs` sidecar.

use std::borrow::Cow;
use std::sync::OnceLock;

use iced::advanced::graphics::text::font_system;

/// Family name under which the embedded Phosphor icon font becomes
/// addressable in iced (`iced::Font::with_name`) once loaded.
///
/// This MUST match the family name the TTF itself declares (name records 1, 4
/// and 6 all read `Phosphor`, capital `P`). cosmic-text resolves a requested
/// family against the declared name case-sensitively, so a lowercase key
/// silently misses the embedded face and falls through to the system fallback
/// — the icon still draws, which is exactly why the mismatch is invisible
/// until the glyphs are compared.
pub const PHOSPHOR_FONT_NAME: &str = "Phosphor";

/// Latches the outcome of the one font registration this process performs.
///
/// `None` inside the `Some(_)` means "registered cleanly"; see
/// [`register_fonts`].
static REGISTRATION: OnceLock<Option<String>> = OnceLock::new();

/// Panic message for the shared iced font-system write lock.
const FONT_SYSTEM_LOCK: &str = "the shared iced font system lock is not poisoned";

/// Returns the font collections to register with the iced runtime font
/// system.
///
/// Byte inventory, in registration order:
/// 1. the embedded Phosphor Regular icon font (always present),
/// 2. the first discoverable system symbol-fallback font, if any
///    (delegated to `crate::ui::fonts::load_fallback_font`, unchanged).
#[must_use]
pub fn font_collections() -> Vec<Cow<'static, [u8]>> {
    let mut collections = vec![Cow::Borrowed(crate::ui::fonts::PHOSPHOR_REGULAR_FONT)];
    if let Some(bytes) = crate::ui::fonts::load_fallback_font() {
        collections.push(Cow::Owned(bytes));
    }
    collections
}

/// Formats the outcome of one `iced::font::load` attempt.
///
/// Returns `None` when the collection registered cleanly, `Some(message)`
/// describing the failure otherwise. Failures are non-fatal: iced keeps its
/// default font set and the app remains usable; callers log the
/// message as an ERROR ([NFR-001]).
#[must_use]
pub fn font_load_error_message(result: &Result<(), iced::font::Error>) -> Option<String> {
    result
        .err()
        .map(|error| format!("font::load error: {error:?}"))
}

/// Registers every FastMD font collection **synchronously** in the
/// process-wide iced font system.
///
/// Writes straight into `iced::advanced::graphics::text::font_system()`
/// rather than deferring through `iced::font::load`, so the faces are
/// present the moment this returns — see the module note for why the
/// deferred form cannot guarantee that. The whole registration runs inside
/// `REGISTRATION`'s `get_or_init`, so it happens exactly once per
/// process: a second caller blocks until the first finishes and then
/// receives the same outcome instead of re-adding faces.
///
/// Returns `None` when every collection contributed at least one face, and
/// `Some(message)` naming the collections that contributed none. Failures
/// are non-fatal ([NFR-001]): iced keeps the faces it already had and the
/// app stays usable, just without the missing collection.
#[must_use]
pub fn register_fonts() -> Option<String> {
    REGISTRATION.get_or_init(register_collections).clone()
}

/// Whether [`register_fonts`] has already run in this process.
///
/// Lets a test tell a pristine font system apart from a registered one.
#[must_use]
pub fn registration_attempted() -> bool {
    REGISTRATION.get().is_some()
}

/// One pass over [`font_collections`], adding each collection to the shared
/// font system and reporting any that produced no faces.
fn register_collections() -> Option<String> {
    let mut system = font_system().write().expect(FONT_SYSTEM_LOCK);

    let mut empty: Vec<String> = Vec::new();
    for (index, bytes) in font_collections().into_iter().enumerate() {
        let before = system.raw().db().faces().count();
        system.load_font(bytes);
        if system.raw().db().faces().count() == before {
            empty.push(format!("collection {index}"));
        }
    }

    if empty.is_empty() {
        None
    } else {
        Some(format!(
            "font registration added no faces for {}",
            empty.join(", ")
        ))
    }
}

/// Builds the boot-report task carrying the font-registration outcome.
///
/// The registration itself is **not** deferred: [`register_fonts`] runs
/// here, synchronously, during `boot`. Its already-known outcome is handed
/// to `iced::Task::done` (`iced_runtime-0.14.0/src/task.rs:39-44`) purely
/// so the `BootMessage::FontLoadFinished` report still rides the message
/// channel into `update`, where a `Some(_)` becomes an ERROR log
/// ([NFR-001]). Only the *report* is asynchronous; the faces are in the
/// font system before `boot` returns.
pub fn font_boot_task() -> iced::Task<Option<String>> {
    iced::Task::done(register_fonts())
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
