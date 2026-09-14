//! Five-panel layout — top, bottom, left, right, and center panel submodules and their entry-point functions.

pub(crate) mod bottom;
pub(crate) mod center;
pub(crate) mod left;
pub(crate) mod right;
pub(crate) mod top;

pub use bottom::show_bottom_panel;
pub use center::show_center_panel;
pub use left::show_left_panel;
pub use right::show_right_panel;
pub use top::show_top_panel;
