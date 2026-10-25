//! The `UIF-PRIN-001` / `UIF-VER-001` token gate run over the app's own
//! iced UI tree.
//!
//! # Why this module exists
//!
//! [`iced_pro::token_lint`] shipped as a self-audit: it read
//! iced-pro's `widget/`, `surface/` and `composite/` and nothing else.
//! That left the half of the interface this crate actually draws entirely
//! ungated — no rule reached a hard-coded padding, radius, stroke or size
//! in `src/app/ui_iced`, and no rule reached a colour constructor outside
//! [`Palette`](crate::ui::theme::Palette), which `src/app/AGENTS.md` §4a
//! requires but nothing enforced.
//!
//! This module is that gate. It is **not a second scanner**. The whole
//! rule set — redaction, the colour-constructor patterns, the visual-name
//! markers, the scalar classifier, the registry semantics, the
//! stale-allow-list check — is iced-pro's, reached through
//! [`scan_tree`](iced_pro::token_lint::scan_tree) with an app-supplied
//! [`Scope`](iced_pro::token_lint::Scope). A parallel scanner starts
//! identical and then drifts: fix a false negative in one and the other
//! keeps reporting what it always reported, and two numbers disagreeing
//! about the same tree is worse than one number wrong in a known way.
//!
//! # The app's scope
//!
//! * **Rule A** — colour constructors are legal only in
//!   [`APP_COLOUR_DEFINITION_FILE`]. Everywhere else in the crate is a
//!   finding. This is §4a expressed as a machine check rather than a
//!   convention.
//! * **Rule B** — a `pub const … : f32` whose name marks it as a visual
//!   measurement, anywhere under [`APP_VISUAL_SCOPE_PREFIXES`], must read
//!   a token rather than a bare number.
//!
//! # What this gate cannot catch
//!
//! Inherited verbatim from the scanner, and worth restating because a gate
//! that overstates itself is worse than no gate:
//!
//! * **Non-`pub` constants, locals and inline literals.** A `12.0` typed
//!   straight into a `Padding {..}` is invisible. Rule B reads
//!   declarations, not expressions.
//! * **Indirection.** `const X: f32 = Y;` where `Y` is itself a magic
//!   number reads as token-bound; only the definition site is examined.
//! * **Semantic correctness.** The scan proves a constant *names a
//!   token*, not that it named the *right* one.
//! * **Colours reached by other means** — composed channel by channel, or
//!   carried in from a host — never trip Rule A.
//!
//! For the sub-floor and on-scale questions specifically — "is this size
//! legal", not "does this size come from a token" — see the sibling
//! `token_guard_tests` module (INT-7c, `#[cfg(test)]`-only, so it cannot
//! be linked from here), which evaluates the render inputs at runtime
//! rather than reading their declarations. The two gates are
//! complementary: this one watches where a number came from, that one
//! watches what the number is.
//!
//! Unit tests live in the sibling `token_lint_gate_tests.rs` sidecar.

use std::path::{Path, PathBuf};

use iced_pro::token_lint::{self, AllowEntry, Audit, SurfaceGeometryConstant};

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// The one file in this crate allowed to define colours.
///
/// [`Palette`](crate::ui::theme::Palette) and
/// [`EditorColors`](crate::ui::theme::EditorColors) live
/// here, and `src/app/AGENTS.md` §4a puts every numeric colour in the
/// crate behind them. The rule is the same shape iced-pro applies to its
/// own `token/color.rs`: a colour either has a token or it does not exist.
pub const APP_COLOUR_DEFINITION_FILE: &str = "ui/theme.rs";

/// Path prefixes Rule B inspects.
///
/// `ui_iced/` is the whole iced rendering layer, scoped by directory
/// rather than by content — the same way iced-pro scopes `widget/` and
/// `surface/`. A file under it is inspected whether or not it happens to
/// carry a colour literal, so a file cannot escape the rule by being
/// quiet about colours.
pub const APP_VISUAL_SCOPE_PREFIXES: [&str; 1] = ["ui_iced/"];

/// App-owned layout bounds registered as fixed literals.
///
/// The app-tree analogue of
/// [`SURFACE_GEOMETRY_CONSTANTS`](iced_pro::token_lint::SURFACE_GEOMETRY_CONSTANTS):
/// each entry carries the authority for a number no token carries. The
/// registry is the *only* way a bare literal on a visually-named constant
/// passes Rule B short of an [`APP_ALLOW_LIST`] entry, and an entry that
/// stops firing fails the build, so a bound cannot outlive its reason.
///
/// Only figures the spec itself fixes belong here. A number the app chose
/// for its own reasons is not a spec-backed bound, and dressing it as one
/// would claim an authority the number does not have — those go in
/// [`APP_ALLOW_LIST`] instead.
pub const APP_GEOMETRY_CONSTANTS: &[SurfaceGeometryConstant] = &[
    SurfaceGeometryConstant {
        file: "ui_iced/app.rs",
        name: "WINDOW_WIDTH_POINTS",
        spec: "§4.1 — Main Window `WINDOW_DEFAULT = 1000 x 700`, width",
    },
    SurfaceGeometryConstant {
        file: "ui_iced/app.rs",
        name: "WINDOW_HEIGHT_POINTS",
        spec: "§4.1 — Main Window `WINDOW_DEFAULT = 1000 x 700`, height",
    },
    SurfaceGeometryConstant {
        file: "ui_iced/menu_surface.rs",
        name: "CARD_WIDTH",
        spec: "§3.4/§3.7/§3.10/§4.8 — `MENU_CARD_WIDTH = 224..320`, lower bound",
    },
];

/// Enumerated exceptions to the two rules.
///
/// Every entry carries its own reason, and
/// [`scan_tree`](iced_pro::token_lint::scan_tree) reports any entry that
/// suppresses nothing, so this list cannot quietly grow stale as the tree
/// moves under it.
///
/// These are **not** exemptions granted because a rule is inconvenient.
/// Each is a number that no token carries at its current value, where
/// binding it to the nearest rung would be a rendering change rather than
/// a token binding. Landing the gate was not licence to re-tune the Logs
/// panel or re-solve the table widths; it was licence to *name* every
/// place that still needs a decision, so the next pass inherits a list
/// instead of an unmeasured surface.
pub const APP_ALLOW_LIST: &[AllowEntry] = &[
    AllowEntry {
        key: "ui_iced/about.rs:LICENSE_MAX_HEIGHT",
        reason: "Scroll-viewport cap on the licence text in the About modal. \
                 §4.6.2 fixes the modal's width (440px) but says nothing about \
                 how much of the licence is visible before scrolling, and no \
                 `size::` rung carries 200. A layout bound, not a spacing or \
                 type decision, and deliberately not registered as geometry \
                 because the spec never set the number.",
    },
    AllowEntry {
        key: "ui_iced/agent_debug.rs:CARD_WIDTH",
        reason: "The Agent Debug card is 600 wide against §4.7.1's movable \
                 default of 680x440 — a deliberate deviation, not an \
                 oversight: its rows are single-line headers and a JSON pane, \
                 and the extra 80px buys nothing. Registered nowhere because \
                 the spec sets a *different* number, and pretending otherwise \
                 would hide the deviation behind a §-reference.",
    },
    AllowEntry {
        key: "ui_iced/logo.rs:LOGO_MARK_SIZE",
        reason: "Geometry of the brand mark itself. `src/app/AGENTS.md` §4a \
                 already exempts `logo.rs` because the mark's tints are local \
                 to the artwork and not part of the UI palette; the same \
                 reasoning covers its size, which is drawn to the mark's own \
                 proportions rather than to the interface scale. A rung here \
                 would resize the artwork, not the UI.",
    },
    AllowEntry {
        key: "ui_iced/logs.rs:CELL_PADDING_Y",
        reason: "`1.0` is on no `space` rung — `space.2xs` is 2.0 and there \
                 is nothing between it and zero. Binding to `2xs` would move \
                 every log row by a pixel, which is a density re-tune of the \
                 Logs panel rather than a token binding. Named here so the \
                 Logs pass inherits an enumerated list instead of an \
                 unmeasured surface.",
    },
    AllowEntry {
        key: "ui_iced/logs.rs:PANEL_PADDING",
        reason: "`6.0` sits exactly between `space.xs` (4) and `space.sm` (8) \
                 and belongs to neither. INT-7c broke that tie on role \
                 elsewhere — `SM` for sibling gaps, `XS` for inner padding — \
                 but the viewer's outer padding is neither, and either choice \
                 changes the panel's whole measure. Left as-is and named, not \
                 guessed at.",
    },
    AllowEntry {
        key: "ui_iced/panels/top.rs:MENU_MIN_WIDTH",
        reason: "Floor on the navbar dropdown's width so a short label cannot \
                 collapse the menu below a clickable size. §4.1 fixes the \
                 navbar band (36) and the control rung (28) but sets no menu \
                 floor; §3.4's `MENU_CARD_WIDTH = 224..320` governs the \
                 menu-surface card, not this trigger-driven popup. 180 is the \
                 app's own floor and no rung carries it.",
    },
    AllowEntry {
        key: "ui_iced/render/yaml.rs:YAML_KEY_COLUMN_WIDTH",
        reason: "Preferred width of the front-matter key column. A column \
                 width is a solved layout quantity, not a spacing rung, and \
                 §2.1's spacing scale has no entry that means \"how wide a \
                 YAML key column wants to be\". Carried over from INT-7c, \
                 which classified it app-owned for the same reason.",
    },
    AllowEntry {
        key: "ui_iced/render/yaml.rs:YAML_MIN_KEY_WIDTH",
        reason: "Lower clamp on the key column so a long value cannot squeeze \
                 the keys out of readability. A minimum is a solver bound, \
                 not a measurement drawn from the scale; binding it to a \
                 spacing rung would conflate \"gap between things\" with \
                 \"least room a thing needs\".",
    },
    AllowEntry {
        key: "ui_iced/render/yaml.rs:YAML_MIN_VALUE_WIDTH",
        reason: "Lower clamp on the value column, the mirror of \
                 `YAML_MIN_KEY_WIDTH`. Same reasoning: a solver bound is not \
                 a spacing rung, and 40 appears on no `space::` or `size::` \
                 scale. Kept as a named constant rather than inlined so the \
                 two clamps stay visibly paired.",
    },
    AllowEntry {
        key: "ui_iced/render/yaml.rs:YAML_WIDTH_CHROME",
        reason: "Allowance reserved for the frame around the two columns \
                 (borders plus the divider) when solving the pair against a \
                 container. It is a chrome budget computed from the widget's \
                 own decoration, not a gap chosen from a scale, so no token \
                 expresses it. INT-7c recorded the same classification.",
    },
    AllowEntry {
        key: "ui_iced/table_width.rs:CELL_PADDING_X",
        reason: "Horizontal cell padding consumed as an *input* by the Fair \
                 Table Width Algorithm. Changing 5.0 to any rung re-solves \
                 every column in every markdown table in the document view, \
                 which is a layout change of a different order than binding \
                 a constant to a token. The FTWA solver is the authority for \
                 this number, not the spacing scale.",
    },
    AllowEntry {
        key: "ui_iced/table_width.rs:CELL_PADDING_Y",
        reason: "Vertical cell padding fed to the same FTWA row-height solve. \
                 `1.0` is on no rung, and as with `CELL_PADDING_X` the number \
                 is a solver input whose change propagates through every \
                 table's measured geometry. Enumerated rather than rounded, \
                 so a future FTWA pass changes it deliberately.",
    },
];

/// The app's scan scope, built from the constants above.
#[must_use]
pub fn app_scope() -> token_lint::Scope<'static> {
    token_lint::Scope {
        colour_definition_file: APP_COLOUR_DEFINITION_FILE,
        visual_scope_prefixes: &APP_VISUAL_SCOPE_PREFIXES,
        surface_geometry: APP_GEOMETRY_CONSTANTS,
    }
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// The directory reported paths are relative to — `src/app`.
///
/// The `fastmd` package's manifest is the workspace root (`[lib] path =
/// "src/app/lib.rs"`), so `CARGO_MANIFEST_DIR` is the repo root and the
/// crate's sources hang off `src/app/`.
#[must_use]
pub fn app_source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("app")
}

/// Run the gate over the app's iced UI tree.
#[must_use]
pub fn scan_app() -> Audit {
    token_lint::scan_tree(&app_source_root(), &app_scope(), APP_ALLOW_LIST)
}

/// Render the gate's result as a plain-text report.
#[must_use]
pub fn render_app(audit: &Audit) -> String {
    token_lint::render_scoped("fastmd/ui_iced", APP_COLOUR_DEFINITION_FILE, audit)
}
