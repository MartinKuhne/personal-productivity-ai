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
//! * `tree` — the cascade controller: mutual exclusion and keyboard
//!   navigation (`INC-16`).

pub mod model;
pub mod view;

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
pub use view::MenuRow;
pub use view::RowBase;
pub use view::RowEvent;
pub use view::RowState;
pub use view::RowStyle;
pub use view::SlotKind;
pub use view::derive_row_state;
pub use view::resolve_row_style;
