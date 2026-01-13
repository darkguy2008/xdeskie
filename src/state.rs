use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::x11::X11Connection;

const PROP_CURRENT: &[u8] = b"_XDESKIE_CURRENT_DESKTOP";
const PROP_COUNT: &[u8] = b"_XDESKIE_NUM_DESKTOPS";

const DEFAULT_DESKTOP_COUNT: u32 = 4;

/// Persistent state for virtual desktop management.
///
/// Tracks which desktop each window belongs to and synchronizes
/// with X11 root window properties for cross-instance communication.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DesktopState {
    /// Current desktop (0-indexed internally)
    pub current: u32,
    /// Total number of desktops
    pub desktops: u32,
    /// Window ID (as string) -> desktop number (0=sticky, 1+=specific)
    pub windows: HashMap<String, u32>,
    /// Windows hidden by the application itself (not by desktop switch)
    #[serde(default)]
    pub app_hidden: HashSet<String>,
}

impl DesktopState {
    /// Load state from file, or create default.
    pub fn load() -> Result<Self> {
        let path = Self::state_path()?;

        if !path.exists() {
            return Ok(Self::default_state());
        }

        let content = fs::read_to_string(&path)?;
        let state: DesktopState = serde_json::from_str(&content)?;
        Ok(state)
    }

    fn default_state() -> Self {
        DesktopState {
            current: 0,
            desktops: DEFAULT_DESKTOP_COUNT,
            windows: HashMap::new(),
            app_hidden: HashSet::new(),
        }
    }

    /// Save state to file.
    pub fn save(&self) -> Result<()> {
        let path = Self::state_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Sync state from X properties (for cross-instance communication).
    pub fn sync_from_x(&mut self, x11: &X11Connection) -> Result<()> {
        if let Some(current) = x11.get_root_property(PROP_CURRENT)? {
            self.current = current;
        }
        if let Some(count) = x11.get_root_property(PROP_COUNT)? {
            self.desktops = count;
        }
        Ok(())
    }

    /// Write state to X properties.
    pub fn sync_to_x(&self, x11: &X11Connection) -> Result<()> {
        x11.set_root_property(PROP_CURRENT, self.current)?;
        x11.set_root_property(PROP_COUNT, self.desktops)?;
        Ok(())
    }

    /// Get desktop for a window, assigning to current desktop if new.
    ///
    /// Returns the desktop number (0=sticky, 1+=specific desktop).
    pub fn get_window_desktop(&mut self, window_id: u32, current_desktop: u32) -> u32 {
        let key = window_id.to_string();

        if let Some(&desktop) = self.windows.get(&key) {
            return desktop;
        }

        // New window: assign to current desktop (stored as 1-indexed)
        let assigned = current_desktop + 1;
        self.windows.insert(key, assigned);
        assigned
    }

    /// Set desktop for a window.
    pub fn set_window_desktop(&mut self, window_id: u32, desktop: u32) {
        self.windows.insert(window_id.to_string(), desktop);
    }

    /// Check if window should be visible on the given desktop.
    ///
    /// The desktop parameter is 0-indexed.
    /// Returns false for app-hidden windows regardless of desktop.
    pub fn is_visible_on(&self, window_id: u32, desktop: u32) -> bool {
        let key = window_id.to_string();

        if self.app_hidden.contains(&key) {
            return false;
        }

        match self.windows.get(&key) {
            Some(&win_desktop) => {
                // 0 = sticky (visible everywhere)
                // 1+ = specific desktop (convert to 0-indexed for comparison)
                win_desktop == 0 || win_desktop == desktop + 1
            }
            None => true, // Unknown windows visible until assigned
        }
    }

    /// Mark window as hidden by the application itself.
    pub fn set_app_hidden(&mut self, window_id: u32, hidden: bool) {
        let key = window_id.to_string();
        if hidden {
            self.app_hidden.insert(key);
        } else {
            self.app_hidden.remove(&key);
        }
    }

    /// Check if window is hidden by the application.
    pub fn is_app_hidden(&self, window_id: u32) -> bool {
        self.app_hidden.contains(&window_id.to_string())
    }

    /// Remove windows that no longer exist from state.
    pub fn cleanup_dead_windows(&mut self, live_windows: &[u32]) {
        let live_set: HashSet<String> = live_windows.iter().map(|id| id.to_string()).collect();
        self.windows.retain(|k, _| live_set.contains(k));
        self.app_hidden.retain(|k| live_set.contains(k));
    }

    fn state_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Cannot find config directory"))?;
        Ok(config_dir.join("xdeskie").join("state.json"))
    }
}
