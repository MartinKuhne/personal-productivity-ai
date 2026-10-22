//! The unified fixed + context menu engine (`UIF-FMENU-*`, `UIF-CMENU-*`).
//!
//! Fixed menus (anchored to a Navbar trigger) and context menus (anchored to
//! the pointer) share one surface, one row grid and one set of selection
//! rules, so they are one engine rather than two.
//!
//! * [`model`] — the menu tree as pure data: rows, groups, selection and
//!   close policy (`INC-14`).
//! * `view` — the menu card and the §4.9.2 row state matrix (`INC-15`).
//! * `tree` — the cascade controller: mutual exclusion and keyboard
//!   navigation (`INC-16`).

pub mod model;

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
