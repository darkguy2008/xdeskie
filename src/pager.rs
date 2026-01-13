use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, ButtonPressEvent, ConfigureNotifyEvent, ConnectionExt, CreateGCAux, CreateWindowAux,
    EventMask, ExposeEvent, Gcontext, PropertyNotifyEvent, Rectangle, Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;
use x11rb::COPY_DEPTH_FROM_PARENT;

use crate::commands::{move_window, switch_to_desktop};
use crate::state::DesktopState;
use crate::x11::X11Connection;

const DEFAULT_CELL_SIZE: u16 = 32;
const PADDING: u16 = 4;
const BORDER: u16 = 2;
const MIN_CELL_SIZE: u16 = 16;

const PROP_CURRENT: &[u8] = b"_XDESKIE_CURRENT_DESKTOP";

// X11 mouse buttons
const BUTTON_LEFT: u8 = 1;
const BUTTON_RIGHT: u8 = 3;
const BUTTON_SCROLL_UP: u8 = 4;
const BUTTON_SCROLL_DOWN: u8 = 5;


/// Holds the pager window state for recreation
struct PagerWindow {
    win_id: Window,
    gc_id: Gcontext,
    gc_inv_id: Gcontext,
    win_width: u16,
    win_height: u16,
    wm_delete_window: Atom,
}

/// Create a new pager window
fn create_pager_window(
    conn: &impl Connection,
    root: Window,
    screen_width: u16,
    screen_height: u16,
    white_pixel: u32,
    black_pixel: u32,
    num_desktops: u32,
) -> Result<PagerWindow> {
    // Calculate initial window size
    let win_width = num_desktops as u16 * (DEFAULT_CELL_SIZE + PADDING) + PADDING;
    let win_height = DEFAULT_CELL_SIZE + PADDING * 2;

    // Position at bottom center
    let x = (screen_width.saturating_sub(win_width)) / 2;
    let y = screen_height.saturating_sub(win_height + 50);

    let win_id = conn.generate_id()?;
    let gc_id = conn.generate_id()?;
    let gc_inv_id = conn.generate_id()?;

    // Create window as regular WM-managed window
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win_id,
        root,
        x as i16,
        y as i16,
        win_width,
        win_height,
        BORDER,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .background_pixel(white_pixel)
            .border_pixel(black_pixel)
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::STRUCTURE_NOTIFY),
    )?;

    // Create graphics contexts
    conn.create_gc(
        gc_id,
        win_id,
        &CreateGCAux::new()
            .foreground(black_pixel)
            .background(white_pixel),
    )?;

    conn.create_gc(
        gc_inv_id,
        win_id,
        &CreateGCAux::new()
            .foreground(white_pixel)
            .background(black_pixel),
    )?;

    // Set window name
    conn.change_property8(
        x11rb::protocol::xproto::PropMode::REPLACE,
        win_id,
        x11rb::protocol::xproto::AtomEnum::WM_NAME,
        x11rb::protocol::xproto::AtomEnum::STRING,
        b"xdeskie pager",
    )?;

    // Set up WM_DELETE_WINDOW protocol for proper close handling
    let wm_protocols = conn.intern_atom(false, b"WM_PROTOCOLS")?.reply()?.atom;
    let wm_delete_window = conn.intern_atom(false, b"WM_DELETE_WINDOW")?.reply()?.atom;
    conn.change_property32(
        x11rb::protocol::xproto::PropMode::REPLACE,
        win_id,
        wm_protocols,
        x11rb::protocol::xproto::AtomEnum::ATOM,
        &[wm_delete_window],
    )?;

    // Show the window
    conn.map_window(win_id)?;
    conn.flush()?;

    Ok(PagerWindow {
        win_id,
        gc_id,
        gc_inv_id,
        win_width,
        win_height,
        wm_delete_window,
    })
}

/// Run the pager as a persistent floating toolbar.
/// This function runs indefinitely until the process is killed.
/// If the window is destroyed externally, it will be automatically recreated.
pub fn run_pager(x11: &X11Connection, state: &mut DesktopState) -> Result<()> {
    let conn = x11.conn();
    let root = x11.root();
    let (screen_width, screen_height) = x11.screen_size();
    let (white_pixel, black_pixel) = x11.screen_pixels();

    let num_desktops = state.desktops;
    let mut current = state.current;

    // Subscribe to property changes on root window to detect desktop switches
    conn.change_window_attributes(
        root,
        &x11rb::protocol::xproto::ChangeWindowAttributesAux::new()
            .event_mask(EventMask::PROPERTY_CHANGE),
    )?;

    // Get the atom for desktop property
    let current_atom = conn.intern_atom(false, PROP_CURRENT)?.reply()?.atom;

    // Create initial window
    let mut pager = create_pager_window(conn, root, screen_width, screen_height, white_pixel, black_pixel, num_desktops)?;

    // Draw initial state
    draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;

    // Event loop - runs forever
    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::Expose(ExposeEvent { window, count: 0, .. }) if window == pager.win_id => {
                draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
            }
            Event::ConfigureNotify(ConfigureNotifyEvent { window, width, height, .. }) if window == pager.win_id => {
                // Window was resized
                if width != pager.win_width || height != pager.win_height {
                    pager.win_width = width;
                    pager.win_height = height;
                    draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
                }
            }
            Event::DestroyNotify(ev) if ev.window == pager.win_id => {
                // Window was destroyed externally - recreate it
                eprintln!("xdeskie: pager window destroyed, recreating...");
                pager = create_pager_window(conn, root, screen_width, screen_height, white_pixel, black_pixel, num_desktops)?;
                draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
            }
            Event::UnmapNotify(ev) if ev.window == pager.win_id => {
                // Window was unmapped - remap it to keep it visible
                conn.map_window(pager.win_id)?;
                conn.flush()?;
            }
            Event::ButtonPress(ev) if ev.event == pager.win_id => {
                match ev.detail {
                    BUTTON_LEFT => {
                        // Left click - switch to clicked desktop
                        if let Some(target) = get_clicked_desktop(&ev, num_desktops, pager.win_width, pager.win_height) {
                            if target != current {
                                switch_to_desktop(x11, state, target)?;
                                current = target;
                                draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
                            }
                        }
                    }
                    BUTTON_RIGHT => {
                        // Right click - grab pointer and let user click a window to move to this desktop
                        if let Some(target) = get_clicked_desktop(&ev, num_desktops, pager.win_width, pager.win_height) {
                            if let Ok(Some(window_id)) = grab_window_pick(x11) {
                                // Move the selected window to the target desktop (1-indexed for move_window)
                                if let Err(e) = move_window(x11, state, window_id, target + 1) {
                                    eprintln!("xdeskie: failed to move window: {}", e);
                                }
                            }
                            // Redraw pager in case we need to refresh
                            draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
                        }
                    }
                    BUTTON_SCROLL_UP => {
                        // Scroll up - previous desktop (no wrap)
                        if current > 0 {
                            let prev = current - 1;
                            switch_to_desktop(x11, state, prev)?;
                            current = prev;
                            draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
                        }
                    }
                    BUTTON_SCROLL_DOWN => {
                        // Scroll down - next desktop (no wrap)
                        if current < num_desktops - 1 {
                            let next = current + 1;
                            switch_to_desktop(x11, state, next)?;
                            current = next;
                            draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
                        }
                    }
                    _ => {}
                }
            }
            Event::PropertyNotify(PropertyNotifyEvent { atom, .. }) if atom == current_atom => {
                // Desktop changed externally, update display
                if let Some(new_current) = x11.get_root_property(PROP_CURRENT)? {
                    if new_current != current {
                        current = new_current;
                        state.current = current;
                        draw_pager(conn, pager.win_id, pager.gc_id, pager.gc_inv_id, num_desktops, current, pager.win_width, pager.win_height)?;
                    }
                }
            }
            Event::ClientMessage(ev) if ev.window == pager.win_id => {
                // Check for WM_DELETE_WINDOW
                if ev.format == 32 && ev.data.as_data32()[0] == pager.wm_delete_window {
                    // User clicked close button - exit gracefully
                    conn.destroy_window(pager.win_id)?;
                    conn.flush()?;
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}

fn draw_pager(
    conn: &impl Connection,
    win_id: Window,
    gc_id: Gcontext,
    gc_inv_id: Gcontext,
    num_desktops: u32,
    current: u32,
    win_width: u16,
    win_height: u16,
) -> Result<()> {
    // Calculate cell dimensions based on window size
    let (cell_width, cell_height) = calculate_cell_dimensions(num_desktops, win_width, win_height);

    // Clear window with white background
    let clear_rect = Rectangle {
        x: 0,
        y: 0,
        width: win_width,
        height: win_height,
    };
    conn.poly_fill_rectangle(win_id, gc_inv_id, &[clear_rect])?;

    // Calculate starting position to center cells
    let total_cells_width = num_desktops as u16 * (cell_width + PADDING) - PADDING;
    let start_x = (win_width.saturating_sub(total_cells_width)) / 2;
    let start_y = PADDING;

    // Draw each desktop cell
    for i in 0..num_desktops {
        let cell_x = start_x + i as u16 * (cell_width + PADDING);
        let cell_y = start_y;
        let is_current = i == current;

        // Use inverted colors for current desktop
        let (fill_gc, text_gc) = if is_current {
            (gc_id, gc_inv_id)
        } else {
            (gc_inv_id, gc_id)
        };

        // Draw cell background
        let cell = Rectangle {
            x: cell_x as i16,
            y: cell_y as i16,
            width: cell_width,
            height: cell_height,
        };
        conn.poly_fill_rectangle(win_id, fill_gc, &[cell])?;

        // Draw border around cell
        let border = Rectangle {
            x: cell_x as i16,
            y: cell_y as i16,
            width: cell_width,
            height: cell_height,
        };
        conn.poly_rectangle(win_id, gc_id, &[border])?;

        // Draw desktop number (1-indexed for display)
        let text = format!("{}", i + 1);
        let char_width = 6i16;
        let char_height = 13i16;
        let text_width = text.len() as i16 * char_width;
        let text_x = cell_x as i16 + (cell_width as i16 - text_width) / 2;
        let text_y = cell_y as i16 + (cell_height as i16 + char_height) / 2;

        conn.image_text8(win_id, text_gc, text_x, text_y, text.as_bytes())?;
    }

    conn.flush()?;
    Ok(())
}

fn calculate_cell_dimensions(num_desktops: u32, win_width: u16, win_height: u16) -> (u16, u16) {
    // Calculate cell width to fill horizontally
    let available_width = win_width.saturating_sub(PADDING);
    let cell_width = (available_width / num_desktops as u16).saturating_sub(PADDING).max(MIN_CELL_SIZE);

    // Use full height minus padding
    let cell_height = win_height.saturating_sub(PADDING * 2).max(MIN_CELL_SIZE);

    (cell_width, cell_height)
}

fn get_clicked_desktop(ev: &ButtonPressEvent, num_desktops: u32, win_width: u16, win_height: u16) -> Option<u32> {
    let x = ev.event_x as u16;
    let y = ev.event_y as u16;

    let (cell_width, cell_height) = calculate_cell_dimensions(num_desktops, win_width, win_height);

    // Calculate starting position (same as draw_pager)
    let total_cells_width = num_desktops as u16 * (cell_width + PADDING) - PADDING;
    let start_x = (win_width.saturating_sub(total_cells_width)) / 2;
    let start_y = PADDING;

    // Check if click is within cell area vertically
    if y < start_y || y >= start_y + cell_height {
        return None;
    }

    // Find which cell was clicked
    for i in 0..num_desktops {
        let cell_x = start_x + i as u16 * (cell_width + PADDING);
        if x >= cell_x && x < cell_x + cell_width {
            return Some(i);
        }
    }

    None
}

/// Grab the pointer and let user click on a window to select it (like xwininfo)
/// Returns the window ID of the clicked window, or None if cancelled (right-click/escape)
fn grab_window_pick(x11: &X11Connection) -> Result<Option<u32>> {
    let conn = x11.conn();
    let root = x11.root();

    // Create a crosshair cursor for visual feedback
    let cursor_font = conn.generate_id()?;
    conn.open_font(cursor_font, b"cursor")?;

    let cursor = conn.generate_id()?;
    // 34 is the crosshair cursor in the cursor font
    conn.create_glyph_cursor(cursor, cursor_font, cursor_font, 34, 35, 0, 0, 0, 0xFFFF, 0xFFFF, 0xFFFF)?;

    // Grab the pointer on root window with crosshair cursor
    let grab_result = conn.grab_pointer(
        false,
        root,
        (EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE).into(),
        x11rb::protocol::xproto::GrabMode::ASYNC,
        x11rb::protocol::xproto::GrabMode::ASYNC,
        x11rb::NONE,
        cursor,
        x11rb::CURRENT_TIME,
    )?.reply()?;

    if grab_result.status != x11rb::protocol::xproto::GrabStatus::SUCCESS {
        conn.free_cursor(cursor)?;
        conn.close_font(cursor_font)?;
        return Ok(None);
    }

    conn.flush()?;

    // Wait for a button press
    let result = loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::ButtonPress(ev) => {
                if ev.detail == BUTTON_LEFT {
                    // Left click - find the window under cursor
                    // ev.child is the window clicked on (or 0 if root)
                    let window = if ev.child != 0 { ev.child } else { root };
                    break Some(window);
                } else {
                    // Right click or other - cancel
                    break None;
                }
            }
            _ => {}
        }
    };

    // Cleanup
    conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
    conn.free_cursor(cursor)?;
    conn.close_font(cursor_font)?;
    conn.flush()?;

    Ok(result)
}
