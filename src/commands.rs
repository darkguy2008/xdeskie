pub mod desktop;
pub mod window;

pub use desktop::{list_desktops, print_current_desktop, set_desktop_count, switch_to_desktop};
pub use window::{list_windows, move_window, parse_window_id};
