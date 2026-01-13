mod cli;
mod commands;
mod popup;
mod state;
mod x11;

use anyhow::{anyhow, Result};
use clap::Parser;

use cli::{Args, Command};
use commands::{
    list_desktops, list_windows, move_window, parse_window_id, print_current_desktop,
    set_desktop_count, switch_to_desktop,
};
use commands::desktop::{switch_next, switch_prev};
use state::DesktopState;
use x11::X11Connection;

fn main() -> Result<()> {
    let args = Args::parse();
    let x11 = X11Connection::new()?;
    let mut state = DesktopState::load()?;

    state.sync_from_x(&x11)?;

    run_command(args.command, &x11, &mut state)
}

fn run_command(command: Command, x11: &X11Connection, state: &mut DesktopState) -> Result<()> {
    match command {
        Command::Switch { desktop } => handle_switch(x11, state, desktop),
        Command::Next => handle_next(x11, state),
        Command::Prev => handle_prev(x11, state),
        Command::Move { window, desktop } => handle_move(x11, state, &window, desktop),
        Command::SetDesktops { count } => handle_set_desktops(x11, state, count),
        Command::List => {
            list_desktops(state);
            Ok(())
        }
        Command::Current => {
            print_current_desktop(state);
            Ok(())
        }
        Command::Windows => list_windows(x11, state),
        Command::Identify => handle_identify(x11, state),
    }
}

fn handle_switch(x11: &X11Connection, state: &mut DesktopState, desktop: u32) -> Result<()> {
    if desktop == 0 || desktop > state.desktops {
        return Err(anyhow!(
            "Invalid desktop {}. Valid range: 1-{}",
            desktop,
            state.desktops
        ));
    }

    let target = desktop - 1;
    switch_to_desktop(x11, state, target)?;
    println!("Switched to desktop {}", desktop);

    Ok(())
}

fn handle_next(x11: &X11Connection, state: &mut DesktopState) -> Result<()> {
    let next = switch_next(x11, state)?;
    println!("Switched to desktop {}", next + 1);
    Ok(())
}

fn handle_prev(x11: &X11Connection, state: &mut DesktopState) -> Result<()> {
    let prev = switch_prev(x11, state)?;
    println!("Switched to desktop {}", prev + 1);
    Ok(())
}

fn handle_move(
    x11: &X11Connection,
    state: &mut DesktopState,
    window: &str,
    desktop: u32,
) -> Result<()> {
    let window_id = parse_window_id(window, x11)?;
    move_window(x11, state, window_id, desktop)?;

    if desktop == 0 {
        println!("Window 0x{:x} is now sticky (all desktops)", window_id);
    } else {
        println!("Moved window 0x{:x} to desktop {}", window_id, desktop);
    }

    Ok(())
}

fn handle_set_desktops(x11: &X11Connection, state: &mut DesktopState, count: u32) -> Result<()> {
    set_desktop_count(x11, state, count)?;
    println!("Set desktop count to {}", count);
    Ok(())
}

fn handle_identify(x11: &X11Connection, state: &DesktopState) -> Result<()> {
    popup::show_desktop_popup(x11, state.current)?;
    Ok(())
}
