//! The closed design-token set behind Perceptual Harmony Through
//! Tokenization (`UIF-PRIN-001`).
//!
//! Every visual number the framework uses — spacing, sizing, corner radius
//! and stroke weight — is declared inside this directory and nowhere else.
//! Nothing outside `token/` may define a visual numeric literal: a widget
//! reads the semantic token that names what the number is *for*, never the
//! number itself. A literal in a widget file is a spec violation, not a
//! style choice.
//!
//! The three modules are deliberately kept separate and are **not**
//! glob-re-exported into `token::*`. Short rung names collide across the
//! scales — `sm`, `md` and `lg` mean 8px of padding in one module and a
//! 32px control height in another — so every call site stays explicitly
//! disambiguated: [`space::MD`], [`size::CONTROL_MD`],
//! [`shape::RADIUS_WIDGET`].
//!
//! * [`space`] — the base-4 spatial grid and spacing scale (`UIF-TOK-001`)
//! * [`size`] — control heights, hit targets and icon sizes (`UIF-TOK-002`)
//! * [`shape`] — corner radii and stroke weights (`UIF-TOK-003`)
//!
//! Token values are `f32` logical pixels: the unit iced lays out in, and
//! the unit the spec's tables are written in. This subsystem holds
//! constants only — it computes nothing, adapts nothing and scales nothing.
//! Behaviour arrives later, with the widgets that consume the tokens.

pub mod shape;
pub mod size;
pub mod space;
