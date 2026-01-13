use anyhow::Result;
use std::thread;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, Gcontext, Window,
    WindowClass,
};
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;
use x11rb::COPY_DEPTH_FROM_PARENT;

use crate::x11::X11Connection;

const POPUP_ATOM: &[u8] = b"_XDESKIE_POPUP";
const POPUP_SIZE: u16 = 60;
const DISPLAY_DURATION_MS: u64 = 1000;

/// Show a popup window displaying the current desktop number.
///
/// Uses X atom coordination to ensure only one popup exists at a time.
/// When called rapidly, the previous popup is destroyed and timer resets.
pub fn show_desktop_popup(x11: &X11Connection, desktop: u32) -> Result<()> {
    // Destroy any existing popup window
    if let Some(old_win) = x11.get_root_property(POPUP_ATOM)? {
        let _ = x11.destroy_window(old_win);
    }

    // Create the popup window
    let (win_id, gc_id) = create_popup_window(x11)?;

    // Store window ID in atom for coordination
    x11.set_root_property(POPUP_ATOM, win_id)?;

    // Draw the desktop number
    draw_desktop_number(x11, win_id, gc_id, desktop)?;

    // Wait and then cleanup
    thread::sleep(Duration::from_millis(DISPLAY_DURATION_MS));

    // Destroy window and remove atom
    x11.destroy_window(win_id)?;
    x11.delete_root_property(POPUP_ATOM)?;

    Ok(())
}

fn create_popup_window(x11: &X11Connection) -> Result<(Window, Gcontext)> {
    let conn = x11.conn();
    let root = x11.root();
    let (screen_width, screen_height) = x11.screen_size();
    let (white_pixel, black_pixel) = x11.screen_pixels();

    // Center the window
    let x = (screen_width.saturating_sub(POPUP_SIZE)) / 2;
    let y = (screen_height.saturating_sub(POPUP_SIZE)) / 2;

    let win_id = x11.generate_id()?;
    let gc_id = conn.generate_id()?;

    // Create popup window with override_redirect to bypass WM
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win_id,
        root,
        x as i16,
        y as i16,
        POPUP_SIZE,
        POPUP_SIZE,
        2,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .background_pixel(white_pixel)
            .border_pixel(black_pixel)
            .override_redirect(1)
            .event_mask(EventMask::EXPOSURE),
    )?;

    // Create graphics context for drawing
    conn.create_gc(
        gc_id,
        win_id,
        &CreateGCAux::new()
            .foreground(black_pixel)
            .background(white_pixel),
    )?;

    // Set window name
    conn.change_property8(
        x11rb::protocol::xproto::PropMode::REPLACE,
        win_id,
        x11rb::protocol::xproto::AtomEnum::WM_NAME,
        x11rb::protocol::xproto::AtomEnum::STRING,
        b"xdeskie",
    )?;

    // Show the window
    conn.map_window(win_id)?;
    conn.flush()?;

    Ok((win_id, gc_id))
}

fn draw_desktop_number(x11: &X11Connection, win_id: Window, gc_id: Gcontext, desktop: u32) -> Result<()> {
    let conn = x11.conn();

    // Draw the desktop number centered
    // Desktop is 0-indexed internally, display as 1-indexed
    let text = format!("{}", desktop + 1);

    // Approximate text centering (rough calculation for default font)
    // X11 default font is roughly 6x13 pixels per character
    let char_width = 8;
    let char_height = 13;
    let text_width = text.len() as i16 * char_width;
    let text_x = (POPUP_SIZE as i16 - text_width) / 2;
    let text_y = (POPUP_SIZE as i16 + char_height) / 2;

    conn.image_text8(win_id, gc_id, text_x, text_y, text.as_bytes())?;
    conn.flush()?;

    Ok(())
}
