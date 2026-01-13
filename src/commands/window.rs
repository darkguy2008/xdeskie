use anyhow::{anyhow, Result};

use crate::state::DesktopState;
use crate::x11::X11Connection;

/// Parse a window ID from string.
///
/// Accepts:
/// - "active" - returns the currently focused window
/// - "0x1234" - hexadecimal window ID
/// - "1234" - decimal window ID
pub fn parse_window_id(s: &str, x11: &X11Connection) -> Result<u32> {
    if s.eq_ignore_ascii_case("active") {
        return x11.get_active_window();
    }

    let id = if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16)?
    } else {
        s.parse()?
    };

    Ok(id)
}

/// Move a window to a specific desktop.
///
/// Desktop 0 makes the window sticky (visible on all desktops).
pub fn move_window(
    x11: &X11Connection,
    state: &mut DesktopState,
    window_id: u32,
    desktop: u32,
) -> Result<()> {
    if desktop > state.desktops {
        return Err(anyhow!(
            "Invalid desktop {}. Valid range: 0-{} (0=sticky)",
            desktop,
            state.desktops
        ));
    }

    state.set_window_desktop(window_id, desktop);
    state.set_app_hidden(window_id, false);

    // Update visibility: show if sticky or on current desktop
    let should_show = desktop == 0 || desktop == state.current + 1;
    if should_show {
        x11.map_window(window_id)?;
    } else {
        x11.unmap_window(window_id)?;
    }

    state.save()?;

    Ok(())
}

/// List all windows and their desktop assignments.
pub fn list_windows(x11: &X11Connection, state: &mut DesktopState) -> Result<()> {
    let infos = x11.get_all_window_info()?;

    // Ensure all windows are tracked and detect app-hidden
    for info in &infos {
        let key = info.id.to_string();
        let is_new = !state.windows.contains_key(&key);
        state.get_window_desktop(info.id, state.current);

        if is_new && !info.is_mapped {
            state.set_app_hidden(info.id, true);
        }
    }

    let window_ids: Vec<u32> = infos.iter().map(|i| i.id).collect();
    state.cleanup_dead_windows(&window_ids);
    state.save()?;

    println!("Windows (current desktop: {}):", state.current + 1);

    for info in &infos {
        let desktop = state.windows.get(&info.id.to_string()).copied().unwrap_or(0);
        let desktop_str = format_desktop(desktop);
        let status = format_window_status(state, info);

        println!(
            "  0x{:08x}  desktop {}  {:.40}{}",
            info.id, desktop_str, info.name, status
        );
    }

    Ok(())
}

fn format_desktop(desktop: u32) -> String {
    if desktop == 0 {
        "sticky".to_string()
    } else {
        desktop.to_string()
    }
}

fn format_window_status(state: &DesktopState, info: &crate::x11::WindowInfo) -> &'static str {
    if state.is_app_hidden(info.id) {
        " [app-hidden]"
    } else if !info.is_mapped {
        " [hidden]"
    } else {
        ""
    }
}
