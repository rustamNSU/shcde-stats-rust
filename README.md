# SHC:DE Monitor

Desktop monitor for **Stronghold Crusader: Definitive Edition** streams and solo practice.

The app shows live match statistics in a compact stream overlay and includes a separate game ticks timer for eco training. The timer is useful when you slow the game down or pause while practicing build orders, because it recalculates elapsed time from game ticks using the game speed you set.

## Download

For normal use, download the latest `shcde-monitor.exe` from the GitHub Releases page.

You do not need Rust, Cargo, source code, or any developer files when using the release build.

## Requirements

- Windows.
- Stronghold Crusader: Definitive Edition installed.
- SHC:DE should run in borderless/windowed mode if you want the overlay or timer visible above the game.

Exclusive fullscreen can hide normal always-on-top windows. Use borderless/windowed mode and enable `Always on top` in the monitor.

## Quick Start

1. Start Stronghold Crusader: Definitive Edition.
2. Run `shcde-monitor.exe`.
3. Wait until the app connects to the running game.
4. Set the title and description.
5. Configure player names, colors, and visible player slots.
6. Choose the overlay columns you want on stream.
7. Press `Show overlay`.
8. Move the overlay into position.
9. Turn the overlay frame off when it is ready.
10. Open the timer window if you need game-time tracking or eco practice timing.

## Features

- Automatic connection to the running SHC:DE process.
- Eight player slots with custom names, colors, and visibility.
- Configurable compact overlay columns.
- Adjustable overlay font size and panel opacity.
- Movable/framed overlay mode for positioning.
- Clean frameless overlay mode.
- Game ticks timer with adjustable game speed, font size, opacity, and frame mode.
- Timer display for ticks, in-game date, selected resource totals, and recalculated time.
- Solo eco training support on slower game speed or pause.
- Persistent user settings.

## Overlay

The compact overlay is designed to sit above the game:

- player names use the configured player colors,
- data columns can be enabled or disabled,
- font size and panel opacity are adjustable,
- the frame can be enabled for positioning and disabled for a clean view,
- always-on-top is controlled from the settings window.

Default visible overlay columns are:

- Gold,
- Population,
- Army,
- Food effect,
- Tax effect,
- Horse archers,
- Camel lancers,
- Knights.

Additional optional columns include total effect, fear factor, shooters, army killed/lost, and produced/acquired resource totals.

## Timer

The game ticks timer is a separate small window for streams and solo eco practice. It can show:

- total ticks,
- current in-game day,
- month,
- year,
- elapsed time,
- selected game speed,
- selected resource totals for one player.

Game speed is configurable in steps of 5 from `40` to `90`. The default is `50`.

The elapsed time is calculated from game ticks and the speed selected in the monitor. That means you can train eco at slower game speed, or pause to think, while still seeing the equivalent time for the speed you want to measure against.

## Settings

User settings are saved automatically in the Windows roaming AppData folder:

```text
%APPDATA%\shcde-monitor\config.json
```

The app restores settings on the next launch. If a setting is missing, the app uses a default value and writes the completed config back quietly.

Saved settings include:

- title and description,
- player names,
- player colors,
- visible player slots,
- overlay columns,
- shooter options,
- font sizes,
- opacity values,
- refresh interval,
- overlay/timer window positions and frame modes.

## Developer Notes

These sections are only needed if you build the app from source.

### Repository Structure

```text
.
|-- Cargo.toml          # Rust package and feature configuration
|-- build.rs            # Slint build step and Windows icon embedding
|-- README.md
|-- AGENTS.md           # Agent/developer collaboration notes
|-- assets/             # App icon and UI assets
|-- instructions/
|   `-- gui.md          # UI behavior notes
|-- skills/
|   `-- rust.md         # Rust development rules for this repo
|-- src/
|   |-- main.rs         # Application startup and window wiring
|   |-- config.rs       # Persistent user settings
|   |-- obfuscation.rs  # Private build constant helpers
|   |-- engine/         # Live stats collection and aggregation
|   `-- ui/             # Rust <-> Slint bridge
`-- ui/
    `-- app.slint       # Main UI, overlay window, and timer window
```

### Build Requirements

- Rust stable toolchain.
- Local `.user` file for private build constants.
- Local `.memory` file for private memory constants.

Install Rust from:

```text
https://rustup.rs/
```

Check the toolchain:

```powershell
rustc --version
cargo --version
```

The `.user` and `.memory` files are ignored by git and are only used while compiling. They are not needed next to the final executable.

### Build From Source

Development build:

```powershell
cargo build
```

Release build:

```powershell
cargo build --release
```

The release executable is created at:

```text
target/release/shcde-monitor.exe
```

The Windows binary is configured as a GUI application, so it should not open an extra console window.

### Development Commands

Compile check:

```powershell
cargo check
```

Format Rust code:

```powershell
cargo fmt
```

Run tests:

```powershell
cargo test
```
