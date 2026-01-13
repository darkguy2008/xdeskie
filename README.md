# xdeskie

A virtual desktop manager for X11 compatible with TWM.

By DARKGuy

## What is this?

A lightweight virtual desktop manager designed for TWM and other minimalist X11 window managers that don't support EWMH (Extended Window Manager Hints). Made 100% with Claude.

Features:
- Virtual desktop switching via window mapping/unmapping
- Sticky windows (visible on all desktops)
- Persistent state across sessions
- Visual desktop identification popup
- Works with TWM and similar minimal WMs

## Building

```bash
make
```

## Installation

```bash
sudo make install
```

Or install to a custom location:
```bash
make PREFIX=~/.local install
```

## Uninstall

```bash
sudo make uninstall
```

## Usage

```bash
# Switch to desktop 3
xdeskie switch 3

# Cycle to next/previous desktop
xdeskie next
xdeskie prev

# Show current desktop number in a popup
xdeskie identify

# Run pager toolbar (persistent, stays open)
xdeskie gui &

# Move active window to desktop 2
xdeskie move active 2

# Make a window sticky (visible on all desktops)
xdeskie move 0x1400007 0

# Set number of desktops
xdeskie set-desktops 4

# List all desktops
xdeskie list

# Print current desktop number
xdeskie current

# List all windows and their desktop assignments
xdeskie windows
```

## Commands

| Command | Description |
|---------|-------------|
| `switch <N>` | Switch to desktop N (1-indexed) |
| `next` | Switch to next desktop (wraps around) |
| `prev` | Switch to previous desktop (wraps around) |
| `identify` | Show current desktop number in a centered popup window |
| `gui` | Run a persistent pager toolbar (click to switch desktops) |
| `move <window> <desktop>` | Move window to desktop (0 = sticky) |
| `set-desktops <count>` | Set number of virtual desktops |
| `list` | List all desktops |
| `current` | Print current desktop number |
| `windows` | List all windows and their desktop assignments |

### Window Specifiers

The `move` command accepts window IDs in these formats:
- `active` - the currently focused window
- `0x1234567` - hexadecimal window ID
- `1234567` - decimal window ID

## Keybindings with TWM

Add to your `.twmrc`:

```
"1" = mod4 : all : !"xdeskie switch 1 && xdeskie identify"
"2" = mod4 : all : !"xdeskie switch 2 && xdeskie identify"
"3" = mod4 : all : !"xdeskie switch 3 && xdeskie identify"
"4" = mod4 : all : !"xdeskie switch 4 && xdeskie identify"
"Right" = mod4 : all : !"xdeskie next && xdeskie identify"
"Left" = mod4 : all : !"xdeskie prev && xdeskie identify"
```

## Files

- `~/.config/xdeskie/state.json` - Persistent state file

## License

MIT
