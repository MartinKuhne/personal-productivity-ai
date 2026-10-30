//! The unified fixed + context menu engine (`UIF-FMENU-*`, `UIF-CMENU-*`).
//!
//! Fixed menus (anchored to a Navbar trigger) and context menus (anchored to
//! the pointer) share one surface, one row grid and one set of selection
//! rules, so they are one engine rather than two.
//!
//! * [`model`] — the menu tree as pure data: rows, groups, selection and
//!   close policy (`INC-14`).
//! * [`view`] — the §4.9.2 row state matrix and the focusable menu row on
//!   the §4.8.1 shared row grid (`INC-15`). The card chrome, the cascade
//!   and cross-row keyboard navigation are not here — see the module docs.
//! * [`cascade`] — the cascade controller: which cards are open, mutual
//!   exclusion, sibling replacement and keyboard navigation (`INC-16`).
//!   Openness lives there, never in the model.
//! * [`row_action`] — the row's action engine: `ActionPending`, duplicate
//!   activation suppression, `on_cancel_request()`, and which row kinds can
//!   genuinely be reverted (`INC-36`).
//! * [`aim`] — the 150ms diagonal aiming corridor: the value-driven
//!   timer that stops a pointer crossing a sibling row from collapsing
//!   the branch the user is aiming at (`UIF-PRIN-008`, `UIF-FMENU-002`).
//!
//! [`row_action`] lives beside the model rather than inside it, because the
//! cascade clones models and `sync` overwrites local changes — a busy flag on
//! a row would be erased by the next refresh.

/// Minimum width of a menu surface card in logical pixels (§2.2, §4.8.1).
pub const MENU_CARD_WIDTH_MIN: f32 = 224.0;

/// Maximum width of a menu surface card in logical pixels (§2.2, §4.8.1).
pub const MENU_CARD_WIDTH_MAX: f32 = 320.0;

pub mod aim;
pub mod cascade;
pub mod model;
pub mod row_action;
pub mod view;

pub use aim::AimCorridor;
pub use aim::AimDecision;
pub use aim::AimTarget;
pub use aim::grace_ms;
pub use cascade::CascadeOutcome;
pub use cascade::FocusTransition;
pub use cascade::IgnoredReason;
pub use cascade::MenuCascade;
pub use cascade::MenuKey;
pub use cascade::MenuTreeKind;
pub use cascade::OpenCard;
pub use cascade::SubmenuTrigger;
pub use model::Activation;
pub use model::ActivationEffect;
pub use model::ClosePolicy;
pub use model::LeadingSlot;
pub use model::Menu;
pub use model::MenuId;
pub use model::MenuViolation;
pub use model::RadioGroupId;
pub use model::RadioGroupInfo;
pub use model::Row;
pub use model::RowGroup;
pub use model::RowKind;
pub use model::TrailingSlot;
pub use model::submenu_id;
pub use row_action::RevertKind;
pub use row_action::RowActionEngine;
pub use row_action::RowEmission;
pub use row_action::RowSettlement;
pub use row_action::row_revert;
pub use row_action::row_tracks_action;
pub use view::MenuRow;
pub use view::RowBase;
pub use view::RowEvent;
pub use view::RowState;
pub use view::RowStyle;
pub use view::SlotKind;
pub use view::derive_row_state;
pub use view::resolve_row_style;
