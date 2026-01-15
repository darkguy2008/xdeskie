use anyhow::{anyhow, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConfigureWindowAux, ConnectionExt, GetWindowAttributesReply, MapState, PropMode,
    StackMode, Window,
};
use x11rb::rust_connection::RustConnection;

pub struct X11Connection {
    conn: RustConnection,
    root: Window,
    screen_num: usize,
}

#[derive(Debug)]
pub struct WindowInfo {
    pub id: u32,
    pub name: String,
    pub is_mapped: bool,
}

impl X11Connection {
    pub fn new() -> Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        Ok(Self { conn, root, screen_num })
    }

    /// Get reference to the X11 connection
    pub fn conn(&self) -> &RustConnection {
        &self.conn
    }

    /// Get the root window ID
    pub fn root(&self) -> Window {
        self.root
    }

    /// Get screen dimensions
    pub fn screen_size(&self) -> (u16, u16) {
        let screen = &self.conn.setup().roots[self.screen_num];
        (screen.width_in_pixels, screen.height_in_pixels)
    }

    /// Get screen pixels (white and black)
    pub fn screen_pixels(&self) -> (u32, u32) {
        let screen = &self.conn.setup().roots[self.screen_num];
        (screen.white_pixel, screen.black_pixel)
    }

    /// Get the currently focused window
    pub fn get_active_window(&self) -> Result<u32> {
        let reply = self.conn.get_input_focus()?.reply()?;
        let focus = reply.focus;

        // If focus is root or None, return error
        if focus == self.root || focus == 0 {
            return Err(anyhow!("No window focused"));
        }

        Ok(focus)
    }

    /// Map (show) a window
    pub fn map_window(&self, window: u32) -> Result<()> {
        self.conn.map_window(window)?;
        self.conn.flush()?;
        Ok(())
    }

    /// Unmap (hide) a window
    pub fn unmap_window(&self, window: u32) -> Result<()> {
        self.conn.unmap_window(window)?;
        self.conn.flush()?;
        Ok(())
    }

    /// Get window attributes to check if mapped
    pub fn get_window_attributes(&self, window: u32) -> Result<GetWindowAttributesReply> {
        Ok(self.conn.get_window_attributes(window)?.reply()?)
    }

    /// Check if a window is currently mapped (visible)
    pub fn is_window_mapped(&self, window: u32) -> Result<bool> {
        let attrs = self.get_window_attributes(window)?;
        Ok(attrs.map_state == MapState::VIEWABLE)
    }

    /// Get all top-level windows (children of root that are real application windows)
    pub fn get_toplevel_windows(&self) -> Result<Vec<u32>> {
        let reply = self.conn.query_tree(self.root)?.reply()?;
        let mut windows = Vec::new();

        for &child in &reply.children {
            // Check if this is a real application window
            if self.is_application_window(child)? {
                windows.push(child);
            }
        }

        Ok(windows)
    }

    /// Check if window is a real application window or TWM frame containing one
    fn is_application_window(&self, window: u32) -> Result<bool> {
        let attrs = match self.conn.get_window_attributes(window)?.reply() {
            Ok(a) => a,
            Err(_) => return Ok(false),
        };

        // Skip override_redirect windows (menus, tooltips, etc.)
        if attrs.override_redirect {
            return Ok(false);
        }

        // Get geometry to filter out tiny hidden windows
        let geom = match self.conn.get_geometry(window)?.reply() {
            Ok(g) => g,
            Err(_) => return Ok(false),
        };

        // Skip tiny windows (1x1 hidden windows used by toolkits)
        if geom.width <= 10 && geom.height <= 10 {
            return Ok(false);
        }

        // Check if this window has WM_CLASS (direct application window)
        if self.has_wm_class(window)? {
            return Ok(true);
        }

        // Check if this is a TWM frame (has a child with WM_CLASS)
        // TWM reparents app windows into frames
        if let Ok(reply) = self.conn.query_tree(window)?.reply() {
            for &child in &reply.children {
                if self.has_wm_class(child)? {
                    return Ok(true); // This is a TWM frame containing an app
                }
            }
        }

        Ok(false)
    }

    /// Check if window has WM_CLASS property set
    fn has_wm_class(&self, window: u32) -> Result<bool> {
        let reply = self.conn
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1)?
            .reply()?;
        Ok(reply.length > 0)
    }

    /// Get window name (try _NET_WM_NAME first, then WM_NAME)
    /// For TWM frames, looks at child windows for the name
    pub fn get_window_name(&self, window: u32) -> Result<String> {
        // Try to get name from this window first
        if let Some(name) = self.get_window_name_direct(window)? {
            return Ok(name);
        }

        // If this is a TWM frame, try to get name from child app window
        if let Ok(reply) = self.conn.query_tree(window)?.reply() {
            for &child in &reply.children {
                if let Some(name) = self.get_window_name_direct(child)? {
                    return Ok(name);
                }
            }
        }

        Ok(format!("0x{:x}", window))
    }

    /// Get window name directly from a window (not checking children)
    fn get_window_name_direct(&self, window: u32) -> Result<Option<String>> {
        // Try _NET_WM_NAME first (UTF-8)
        let net_wm_name = self.conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let utf8_string = self.conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;

        let reply = self.conn
            .get_property(false, window, net_wm_name, utf8_string, 0, 256)?
            .reply()?;

        if reply.length > 0 {
            return Ok(Some(String::from_utf8_lossy(&reply.value).to_string()));
        }

        // Fallback to WM_NAME (Latin-1)
        let reply = self.conn
            .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 256)?
            .reply()?;

        if reply.length > 0 {
            return Ok(Some(String::from_utf8_lossy(&reply.value).to_string()));
        }

        Ok(None)
    }

    /// Get info about all toplevel windows
    pub fn get_all_window_info(&self) -> Result<Vec<WindowInfo>> {
        let windows = self.get_toplevel_windows()?;
        let mut infos = Vec::new();

        for id in windows {
            let name = self.get_window_name(id).unwrap_or_else(|_| format!("0x{:x}", id));
            let is_mapped = self.is_window_mapped(id).unwrap_or(false);
            infos.push(WindowInfo { id, name, is_mapped });
        }

        Ok(infos)
    }

    /// Store a value in X property on root window
    pub fn set_root_property(&self, name: &[u8], value: u32) -> Result<()> {
        let atom = self.conn.intern_atom(false, name)?.reply()?.atom;
        self.conn.change_property(
            PropMode::REPLACE,
            self.root,
            atom,
            AtomEnum::CARDINAL,
            32,
            1,
            &value.to_ne_bytes(),
        )?;
        self.conn.flush()?;
        Ok(())
    }

    /// Get a value from X property on root window
    pub fn get_root_property(&self, name: &[u8]) -> Result<Option<u32>> {
        let atom = self.conn.intern_atom(false, name)?.reply()?.atom;
        let reply = self.conn
            .get_property(false, self.root, atom, AtomEnum::CARDINAL, 0, 1)?
            .reply()?;

        if reply.format != 32 || reply.length == 0 {
            return Ok(None);
        }

        let values: Vec<u32> = reply
            .value32()
            .ok_or_else(|| anyhow!("Invalid property"))?
            .collect();

        Ok(values.into_iter().next())
    }

    /// Delete a property from root window
    pub fn delete_root_property(&self, name: &[u8]) -> Result<()> {
        let atom = self.conn.intern_atom(false, name)?.reply()?.atom;
        self.conn.delete_property(self.root, atom)?;
        self.conn.flush()?;
        Ok(())
    }

    /// Destroy a window
    pub fn destroy_window(&self, window: u32) -> Result<()> {
        self.conn.destroy_window(window)?;
        self.conn.flush()?;
        Ok(())
    }

    /// Generate a new window ID
    pub fn generate_id(&self) -> Result<u32> {
        Ok(self.conn.generate_id()?)
    }

    /// Get the current stacking order of all toplevel windows (bottom to top)
    pub fn get_stacking_order(&self) -> Result<Vec<u32>> {
        let reply = self.conn.query_tree(self.root)?.reply()?;
        let mut windows = Vec::new();

        // query_tree returns children in bottom-to-top stacking order
        for &child in &reply.children {
            if self.is_application_window(child)? {
                windows.push(child);
            }
        }

        Ok(windows)
    }

    /// Restack windows to match the given order (bottom to top)
    pub fn restack_windows(&self, order: &[u32]) -> Result<()> {
        if order.len() < 2 {
            return Ok(());
        }

        // Stack each window above the previous one
        for i in 1..order.len() {
            self.conn.configure_window(
                order[i],
                &ConfigureWindowAux::new()
                    .sibling(order[i - 1])
                    .stack_mode(StackMode::ABOVE),
            )?;
        }
        self.conn.flush()?;
        Ok(())
    }
}
