# GUI Notes

This file describes the user-facing Slint interface for SHC:DE Monitor.

Keep this document focused on layout and behavior only. Do not store address maps, table layouts, reverse-engineering notes, or debug/write instructions here.

## Main Window

The main window is the settings and monitoring surface.

It should provide:

- title and description fields,
- player slot visibility controls,
- player display names,
- player color selection,
- overlay settings,
- timer settings,
- visible column controls,
- a full data table for checking the current match state.

The first settings column should stay readable at the default window size. The stats table should handle many columns with horizontal scrolling instead of forcing the settings column off screen.

## Overlay Window

The overlay window is intended to sit above the game.

It should:

- be compact,
- support a movable framed mode for positioning,
- support a clean frameless mode,
- use a semi-transparent panel background,
- keep text fully opaque and sharp,
- fit its width and height to enabled players and columns,
- update when player visibility or column settings change.

Player names should use the configured player color. If a color is too dark for the overlay background, keep the text readable.

## Timer Window

The timer window is separate from the main overlay.

It should support stream display and solo eco training. The user may slow the game down or pause while practicing, so elapsed time should be calculated from game ticks using the game speed selected in the monitor.

It should show:

- game ticks,
- day,
- month,
- year,
- elapsed time,
- selected game speed,
- selected player resource totals.

Timer settings should include:

- game speed from `40` to `90`,
- step size `5`,
- font size,
- opacity,
- movable frame,
- show/hide window controls.

Month names should use short labels such as `Jan`, `Feb`, and `Mar`.

## General UI Rules

- Prefer clear alignment over decorative layout.
- Keep controls compact but readable.
- Avoid text overflow in buttons and table cells.
- Use persistent user settings for manual UI choices.
- Do not make the user reconfigure the overlay on every launch.
