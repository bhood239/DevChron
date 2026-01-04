# DevChron – Rust TUI Pomodoro Timer

A lightweight, visually stunning TUI Pomodoro timer for Hyprland/Wayland built with Rust, ratatui, and tokio. The app will feature configurable timers, session tracking, desktop notifications, and status bar integration through a JSON-based approach that's compositor-agnostic.

## Steps

1. **Initialize project structure** with `cargo init` and configure `Cargo.toml` with dependencies: ratatui (0.26), crossterm (0.27), tokio (async runtime), notify-rust (notifications), serde + toml (config), clap (CLI), directories (XDG paths), anyhow (errors), and chrono (timestamps).

2. **Implement core timer logic** in `src/timer/` with `pomodoro.rs` (Pomodoro state machine tracking focus/short break/long break phases), `state.rs` (Running/Paused/Completed states), and `session.rs` (session counter, cycles tracking). Use tokio intervals for accurate 1-second ticks.

3. **Build configuration system** in `src/config/` with `settings.rs` defining serde-deserializable `Settings` struct (focus: 25m, short break: 5m, long break: 15m, cycles: 4, notification/sound/hyprland toggles), default values, and TOML loading from `~/.config/devchron/config.toml` using the `directories` crate.

4. **Create ratatui UI** in `src/ui/` with `theme.rs` (Nord/Catppuccin/Classic color schemes that change per timer phase), `widgets.rs` (custom timer display with large clock and animated `Gauge` progress bar), `layout.rs` (responsive centered layout with rounded borders), and `render.rs` (efficient rendering logic triggered only on events/ticks).

5. **Implement event handling** in `src/events/` with `keyboard.rs` mapping shortcuts (Space: pause/resume, R: reset, S: skip, Q: quit, H: help, +/-: adjust time), using crossterm's `event::poll()` with tokio::select! to multiplex timer ticks and input events, ensuring proper terminal cleanup via Drop trait.

6. **Add integrations** with `src/notification/manager.rs` (notify-rust desktop notifications on phase completion), and `src/hyprland/status.rs` (write JSON status to `~/.cache/devchron/status.json` containing current phase, time remaining, session count for external status bar scripts to read).

7. **Package for AUR** with `PKGBUILD` defining package metadata, cargo build process, installation paths (`/usr/bin/devchron`, `/usr/share/devchron/default.toml`), runtime dependency on dbus, and post-install message with Waybar integration example (`"exec": "cat ~/.cache/devchron/status.json"`).

## Further Considerations

1. **Theme selection**: Should users switch themes via CLI flag (`--theme nord`), config file setting, or runtime keyboard shortcut (T key)? Recommend config file with CLI override.

2. **Sound alerts**: Include optional sound support via `rodio` crate behind feature flag? Low priority given notify-rust handles system sounds through notification hints.

3. **Session persistence**: Save session history to `~/.local/share/devchron/history.json` for statistics tracking across restarts? Would enable "Today: 2h 30m" display in UI.

4. **Waybar script**: Should we include a reference `waybar-devchron.sh` wrapper script in the repo that formats the JSON status file nicely, or just document the integration in README?

## Project Structure

```
devchron/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Entry point, CLI parsing, app initialization
│   ├── lib.rs                  # Library exports
│   ├── app.rs                  # Core application state and logic
│   ├── ui/
│   │   ├── mod.rs              # UI module coordinator
│   │   ├── layout.rs           # Layout definitions
│   │   ├── widgets.rs          # Custom widgets (timer display, progress bars)
│   │   ├── theme.rs            # Color schemes and styling
│   │   └── render.rs           # Rendering logic
│   ├── timer/
│   │   ├── mod.rs              # Timer module coordinator
│   │   ├── pomodoro.rs         # Pomodoro logic (focus/break cycles)
│   │   ├── state.rs            # Timer states (Running, Paused, Completed)
│   │   └── session.rs          # Session tracking and statistics
│   ├── config/
│   │   ├── mod.rs              # Config module coordinator
│   │   ├── settings.rs         # Settings struct with serde
│   │   └── default.rs          # Default configuration values
│   ├── notification/
│   │   ├── mod.rs              # Notification module
│   │   └── manager.rs          # Desktop notification handling
│   ├── hyprland/
│   │   ├── mod.rs              # Hyprland integration
│   │   └── status.rs           # Status bar updates
│   ├── events/
│   │   ├── mod.rs              # Event handling
│   │   ├── handler.rs          # Event dispatcher
│   │   └── keyboard.rs         # Keyboard shortcuts
│   └── error.rs                # Custom error types
├── config/
│   └── default.toml            # Default configuration file
├── assets/
│   └── icon.png                # Notification icon
├── PKGBUILD                    # AUR package definition
└── README.md
```

## Technology Stack

### Core Dependencies
- **ratatui** (0.26): TUI framework with powerful widget system
- **crossterm** (0.27): Terminal manipulation and event handling
- **tokio** (1.x): Async runtime for non-blocking timer + event loop
- **notify-rust** (4.x): Desktop notifications via D-Bus
- **serde** (1.x) + **toml** (0.8): Configuration serialization
- **clap** (4.x): CLI argument parsing
- **directories** (5.x): XDG Base Directory compliance
- **anyhow** (1.x): Error handling
- **chrono** (0.4): Time/date handling

### Optional Features
- **rodio** (0.17): Sound alerts (behind feature flag)

## Visual Design

### Theme Color Schemes

**Nord Theme** (Recommended default)
- Focus: `Color::Rgb(191, 97, 106)` (aurora red)
- Short Break: `Color::Rgb(163, 190, 140)` (aurora green)
- Long Break: `Color::Rgb(129, 161, 193)` (frost blue)
- Background: `Color::Rgb(46, 52, 64)` (polar night)
- Paused: `Color::Rgb(108, 117, 125)` (gray)

**Catppuccin Mocha**
- Focus: `Color::Rgb(243, 139, 168)` (pink)
- Short Break: `Color::Rgb(166, 227, 161)` (green)
- Long Break: `Color::Rgb(137, 180, 250)` (blue)
- Background: `Color::Rgb(30, 30, 46)` (base)

**Classic Tomato**
- Focus: `Color::Rgb(220, 53, 69)` (red)
- Short Break: `Color::Rgb(40, 167, 69)` (green)
- Long Break: `Color::Rgb(0, 123, 255)` (blue)

### Layout Design

```
╭─ DevChron ──────────────────────────╮
│ Session 1/4        Focus Time       │
├─────────────────────────────────────┤
│                                     │
│              25:00                  │
│                                     │
│   ████████████████████░░░░░░░░░    │
│                                     │
│   Today: 2h 30m | Total: 24h 15m   │
│                                     │
├─────────────────────────────────────┤
│ [Space] Pause [R] Reset [S] Skip   │
│ [Q] Quit      [H] Help              │
╰─────────────────────────────────────╯
```

## Features

### Core Functionality
- ✅ Pomodoro timer (default: 25 minutes)
- ✅ Short break timer (default: 5 minutes)
- ✅ Long break timer (default: 15 minutes)
- ✅ Configurable durations via TOML config
- ✅ Start/Pause/Reset controls
- ✅ Skip to next phase
- ✅ Visual countdown with progress bar
- ✅ Session counter (cycles before long break)
- ✅ Desktop notifications on phase completion

### Keyboard Shortcuts
- `Space`: Start/Pause timer
- `R`: Reset current timer
- `S`: Skip to next phase
- `Q` / `Esc`: Quit application
- `H` / `?`: Toggle help screen
- `+` / `-`: Adjust time (while paused)

### Integrations
- **Desktop Notifications**: notify-rust with D-Bus (Wayland-compatible)
- **Status Bar**: JSON status file for Waybar/other status bars
- **Configuration**: XDG-compliant config location (`~/.config/devchron/config.toml`)

## Configuration

### Default Settings
```toml
# ~/.config/devchron/config.toml

[timer]
focus_duration = 25        # minutes
short_break_duration = 5   # minutes
long_break_duration = 15   # minutes
cycles_before_long_break = 4

[notifications]
enabled = true
sound_enabled = false

[ui]
theme = "nord"  # nord, catppuccin, classic

[integrations]
hyprland_status_bar = true
```

## Hyprland/Waybar Integration

### Status JSON Format
```json
{
  "phase": "focus",
  "time_remaining": "23:45",
  "session": "2/4",
  "is_running": true,
  "percentage_complete": 7
}
```

### Waybar Configuration
```json
"custom/pomodoro": {
  "exec": "jq -r '.phase + \" \" + .time_remaining' ~/.cache/devchron/status.json",
  "interval": 1,
  "format": "🍅 {}",
  "on-click": "pkill -USR1 devchron"
}
```

## AUR Packaging

### Installation Paths
- Binary: `/usr/bin/devchron`
- Default config: `/usr/share/devchron/default.toml`
- Documentation: `/usr/share/doc/devchron/`
- License: `/usr/share/licenses/devchron/`

### Runtime Dependencies
- `dbus`: Desktop notification support

### Build Dependencies
- `rust`
- `cargo`

## Implementation Priorities

### Phase 1: Core Timer (MVP)
1. Project initialization and dependencies
2. Basic timer state machine
3. Minimal TUI with countdown display
4. Start/pause/reset keyboard controls
5. Simple configuration loading

### Phase 2: Visual Polish
1. Theme system implementation
2. Progress bar with animations
3. Responsive layout
4. Session counter display
5. Help screen

### Phase 3: Integrations
1. Desktop notifications
2. Status JSON file generation
3. Config file management
4. Error handling improvements

### Phase 4: Packaging & Documentation
1. PKGBUILD creation
2. README with screenshots
3. Example Waybar integration
4. AUR submission
