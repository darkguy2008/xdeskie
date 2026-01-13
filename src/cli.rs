use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xdeskie")]
#[command(about = "Virtual desktop manager for TWM and similar WMs", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Switch to desktop N (1-indexed)
    Switch { desktop: u32 },

    /// Switch to next desktop (wraps around)
    Next,

    /// Switch to previous desktop (wraps around)
    Prev,

    /// Move window to desktop N (0 = sticky/all desktops)
    Move {
        /// Window ID (hex like 0x1234, decimal, or "active")
        window: String,
        /// Target desktop (0 = sticky, 1+ = specific desktop)
        desktop: u32,
    },

    /// Set the number of desktops
    SetDesktops { count: u32 },

    /// List all desktops
    List,

    /// Print current desktop number
    Current,

    /// List all windows and their assigned desktops
    Windows,

    /// Show current desktop number in a popup window
    Identify,
}
