//! The closed design-token set behind Perceptual Harmony Through
//! Tokenization (`UIF-PRIN-001`).
//!
//! Every visual and temporal number the framework uses — colour,
//! spacing, sizing, corner radius, stroke weight, type and duration — is
//! declared inside this directory and nowhere else.
//! Nothing outside `token/` may define a visual numeric literal: a widget
//! reads the semantic token that names what the number is *for*, never the
//! number itself. A literal in a widget file is a spec violation, not a
//! style choice.
//!
//! The seven modules are deliberately kept separate and are **not**
//! glob-re-exported into `token::*`. Short rung names collide across the
//! scales — `sm`, `md` and `lg` mean 8px of padding in one module and a
//! 32px control height in another — so every call site stays explicitly
//! disambiguated: [`space::MD`], [`size::CONTROL_MD`],
//! [`shape::RADIUS_WIDGET`], [`typography::BODY`], [`time::NORMAL`],
//! [`color::Palette::dark`].
//!
//! * [`color`] — the semantic colour tokens: surfaces, borders, accents,
//!   text roles, status colours and the inverted editor set
//!   (`UIF-TOK-005`)
//! * [`space`] — the base-4 spatial grid and spacing scale (`UIF-TOK-001`)
//! * [`size`] — control heights, hit targets and icon sizes (`UIF-TOK-002`)
//! * [`shape`] — corner radii and stroke weights (`UIF-TOK-003`)
//! * [`typography`] — the type scale: size, line height, weight and
//!   tracking per rung, plus the 12px absolute text floor
//!   (`UIF-TOK-004`)
//! * [`contrast`] — the WCAG 2.1 luminance and contrast-ratio solver
//!   that measures the tokens against the mandated thresholds
//!   (`UIF-TOK-006`, `UIF-VER-001`)
//! * [`time`] — the duration scale and its easing curves: hover delays,
//!   debounce windows, transition durations, and the reduced-motion
//!   fallback that resolves them (`UIF-TOK-007`)
//!
//! Token values are `f32` logical pixels — the unit iced lays out in,
//! and the unit the spec's tables are written in — except in [`color`],
//! where a token is an `iced::Color`, and in [`time`], where a duration
//! is whole `u64` milliseconds because that is the resolution the
//! host-injected clock is read at. This subsystem holds the
//! token values and pure derivations of them (a type rung's line-height
//! ratio, a weight's rank, the WCAG contrast ratio between two colours,
//! the eased fraction a motion rung implies at a given elapsed time)
//! — it adapts nothing, scales nothing to DPI, and resolves no style.
//! Behaviour arrives later, with the widgets that consume the tokens.

pub mod color;
pub mod contrast;
pub mod shape;
pub mod size;
pub mod space;
pub mod time;
pub mod typography;
