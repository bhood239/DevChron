# DevChron

A vibrant, high-craft Pomodoro timer and offline focus analytics suite for your
terminal. Built with Rust and [ratatui](https://ratatui.rs), made for
Hyprland/Wayland, and happy anywhere else a truecolor terminal runs.

## TokyoNight-inspired colour system

The interface borrows the visual language of Neovim + LazyVim: deep navy/charcoal
structure, cool blue-violet text, restrained separators, and vivid syntax-like accents.
It is not a literal clone of LazyVim; colors are assigned by semantic role.

| Role | Colour | Hex |
|---|---|---|
| Coding | Warm orange | `#FF9E64` |
| Learning | Bright cyan | `#2AC3DE` |
| Reading | Violet | `#BB9AF7` |
| All-category yearly grid | Contribution green | `#39D353` |
| Today marker | Solar yellow | `#F0D75E` |
| Success / completion | Soft green | `#9ECE6A` |
| Information | TokyoNight blue | `#7AA2F7` |
| Paused / skipped | Muted slate | `#565F89` |
| Reset / alarm | Signal red-pink | `#F7768E` |
| Section headings | Ice blue | `#7DCFFF` |
| Main text | Cool lavender-white | `#C0CAF5` |
| Dividers | Deep blue-slate | `#292E42` |

### Yearly focus grid behavior

- **All selected:** one unified green intensity ramp, similar to contribution heatmaps.
- **Coding / Learning / Reading selected:** the grid switches to the selected category's orange,
  cyan, or violet intensity ramp.
- **Today:** always receives a separate solar-yellow marker, regardless of filter or activity.
- The legend follows the active filter: green ramp for All, category ramp for a selected category.

## Highlights

- **A ring you can read from across the room.** A smooth perceptual-gradient
  Braille ring with cardinal dial ticks and a highlighted leading edge fills as
  the phase runs, with vertical-gradient block digits inside a single rounded
  frame.
- **Focus Categories & Phase Control.** Switch categories (`Coding`, `Learning`,
  `Reading`) on the fly with `C` (or `Shift+Tab`) and jump directly to `Focus`,
  `Short break`, or `Long break` with `F`, `B`, and `L`. Each category and break
  phase drives its own distinct ring and digit gradient.
- **Vibrant, color-coded offline Analytics.** Press `G` (or `D`) to open the
  dedicated Analytics dashboard: a 365-day focus grid colored by dominant
  category, 6 multi-hue hero cards, 24-hour & chronotype stacked bars, weekday
  rhythm, Category × Weekday matrix, per-category telemetry (`Today`, `7-day`,
  `Avg/sess`, `Active` days, `Peak hr`), and a scrollable recent-sessions log
  (`J`/`K`) with interactive category filtering (`C`/`Tab`)—100% offline.
- **Three modes.** Classic 25/5/15, Session 50/10/30, Extended 75/15/45.
  Switch with `1` `2` `3` or `Tab`.
- **A pleasant alarm.** A soft four-note bell chime rings at 00:00 and repeats
  until you press `A`. It is built into the binary, so there is nothing to
  download.
- **Unified multi-hue spectrum palette.** Tuned for dark and transparent
  terminals (using foreground colours only so Hyprland blur works naturally),
  distributing distinct semantic hues across categories (`Coding` Coral,
  `Learning` Aqua, `Reading` Violet), modes, streaks (Trophy Gold), completion
  rates (Vivid Emerald), and status badges.
- **Honest timing.** The countdown is computed from the system clock, so
  frame hiccups or a suspended laptop can't make it drift, and the main loop
  redraws only when the displayed second changes (~0% idle CPU).

## Install

### Arch Linux (AUR)

```bash
yay -S devchron
```

### From this source tree (no root needed)

```bash
./scripts/install.sh              # builds, then installs to ~/.local
./scripts/install.sh --uninstall  # removes what it installed
```

### With Cargo (Linux & macOS)

```bash
git clone https://github.com/bhood239/DevChron.git
cd DevChron
cargo install --path .
```

## Usage

```bash
devchron
```

### Keys

| Key | Action |
|---|---|
| `Space` / `P` | Start / pause |
| `F` / `B` / `L` | Switch directly to Focus, Short break, or Long break |
| `C` / `Shift+Tab` | Cycle Focus category (`Coding` → `Learning` → `Reading`) |
| `R` | Reset the current phase |
| `S` | Skip to the next phase |
| `A` | Stop the alarm (only active while it rings) |
| `G` / `D` | Open / close the Analytics page |
| `C` / `Tab` (on Analytics) | Filter Analytics by category (`All` → `Coding` → `Learning` → `Reading`) |
| `J` / `K` / `↑` / `↓` (on Analytics) | Scroll Recent sessions log |
| `1` `2` `3` | Pick a mode (`Classic`, `Session`, `Extended`) |
| `Tab` | Next mode |
| `Y` / `Enter`, `N` / `Esc` | Confirm / cancel a phase or mode switch |
| `+` / `-` | Add / remove 5 minutes |
| `M` | Minimal mode |
| `H` / `?` | Help (any key closes it) |
| `Q` / `Esc` | Quit (`Esc` closes Analytics/dialogs first) |

## License

MIT. See [LICENSE](LICENSE).
