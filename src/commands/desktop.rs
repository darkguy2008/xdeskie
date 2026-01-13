use anyhow::{anyhow, Result};

use crate::state::DesktopState;
use crate::x11::X11Connection;

/// Switch to a specific desktop (0-indexed internally).
///
/// This handles:
/// - Detecting newly appeared windows and assigning them to current desktop
/// - Detecting app-hidden windows (windows hidden by the app itself)
/// - Cleaning up dead windows from state
/// - Mapping/unmapping windows based on target desktop visibility
pub fn switch_to_desktop(x11: &X11Connection, state: &mut DesktopState, target: u32) -> Result<()> {
    let infos = x11.get_all_window_info()?;
    let window_ids: Vec<u32> = infos.iter().map(|i| i.id).collect();

    detect_new_windows(state, &infos);
    state.cleanup_dead_windows(&window_ids);
    update_window_visibility(x11, state, &infos, target)?;

    state.current = target;
    state.sync_to_x(x11)?;
    state.save()?;

    Ok(())
}

/// Detect newly appeared windows and handle app-hidden state.
fn detect_new_windows(state: &mut DesktopState, infos: &[crate::x11::WindowInfo]) {
    for info in infos {
        let key = info.id.to_string();
        let is_new = !state.windows.contains_key(&key);

        if is_new {
            // Assign new window to current desktop
            state.get_window_desktop(info.id, state.current);

            // If already hidden on arrival, mark as app-hidden
            if !info.is_mapped {
                state.set_app_hidden(info.id, true);
            }
        }
    }
}

/// Update window visibility based on target desktop.
fn update_window_visibility(
    x11: &X11Connection,
    state: &DesktopState,
    infos: &[crate::x11::WindowInfo],
    target: u32,
) -> Result<()> {
    for info in infos {
        if state.is_visible_on(info.id, target) {
            x11.map_window(info.id)?;
        } else {
            x11.unmap_window(info.id)?;
        }
    }
    Ok(())
}

/// Switch to the next desktop (wraps around).
pub fn switch_next(x11: &X11Connection, state: &mut DesktopState) -> Result<u32> {
    let next = (state.current + 1) % state.desktops;
    switch_to_desktop(x11, state, next)?;
    Ok(next)
}

/// Switch to the previous desktop (wraps around).
pub fn switch_prev(x11: &X11Connection, state: &mut DesktopState) -> Result<u32> {
    let prev = if state.current == 0 {
        state.desktops - 1
    } else {
        state.current - 1
    };
    switch_to_desktop(x11, state, prev)?;
    Ok(prev)
}

/// Set the number of desktops, relocating windows if necessary.
pub fn set_desktop_count(
    x11: &X11Connection,
    state: &mut DesktopState,
    count: u32,
) -> Result<()> {
    if count == 0 {
        return Err(anyhow!("Desktop count must be at least 1"));
    }

    // Move windows from removed desktops to the last valid one
    if count < state.desktops {
        for win_desktop in state.windows.values_mut() {
            if *win_desktop > count {
                *win_desktop = count;
            }
        }
    }

    state.desktops = count;

    // Switch to last valid desktop if current is now invalid
    if state.current >= count {
        let new_current = count - 1;
        switch_to_desktop(x11, state, new_current)?;
    }

    state.sync_to_x(x11)?;
    state.save()?;

    Ok(())
}

/// List all desktops with current marker.
pub fn list_desktops(state: &DesktopState) {
    println!("Desktops: {} (current: {})", state.desktops, state.current + 1);
    for i in 0..state.desktops {
        let marker = if i == state.current { " *" } else { "" };
        println!("  {}{}", i + 1, marker);
    }
}

/// Print the current desktop number (1-indexed).
pub fn print_current_desktop(state: &DesktopState) {
    println!("{}", state.current + 1);
}
