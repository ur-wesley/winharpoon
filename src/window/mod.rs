pub mod enumerate;
pub mod focus;
pub mod identity;
pub mod process_name;

pub use enumerate::{enumerate_windows, get_foreground_window, WindowInfo};
pub use focus::{capture_stack_snapshot, foreground_is_fullscreen, restore_stack_snapshot, StackSnapshot};
