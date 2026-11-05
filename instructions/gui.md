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

### SteamID tab (admin only)

After the existing admin key is accepted, **SteamID** appears beside the settings
tabs. It shows human multiplayer players with their current Steam name in their
game color and full SteamID64. Clicking the ID opens its Steam profile. This page is
separate from the normal statistics and detached windows.

Identities are collected once per second while admin mode is unlocked, including
while other tabs are open. **Refresh Steam IDs** requests an immediate update.
The last match's names, colors, Steam IDs and profile links remain available after
the match ends, the player leaves, or the game closes. Read failures and empty
rosters do not clear them; a new active, loaded match replaces them. On app restart,
the latest locally recorded match restores the table if no match is running.
AI players are omitted. Profile links are checked against the displayed account
before opening in the default browser, so they also work after the match.
Reading this page does not change the game or import names over manual aliases.

Steam IDs use the same clickable text in the SteamID table, game rosters, player
journal and player details. Only the numeric ID is displayed; clicking a player
name opens their journal details, while clicking the ID opens the Steam profile.

### Game records and player journal (admin only)

While admin mode is unlocked, the app automatically records confirmed online
multiplayer matches containing this Steam account and another human Steam
account. Single-player skirmishes and human-versus-bots-only sessions are excluded.
Recording works independently of the selected tab.

One JSON is created in `history/games/` beside the existing configuration as soon
as a match is detected. The app refreshes the latest statistics in memory each
second and replaces that same JSON every five seconds. No timeline or periodic
snapshot collection is stored. Game end, departure or game-process exit finalizes
the record; finalized game files are never rewritten. Closing the monitor saves
the current values without finalizing an ongoing game, allowing it to resume on
the next launch. An abrupt monitor crash leaves its last successful save intact.

Below the SteamID table in the right working area:

- **Games** opens searchable records with setup, P1–P8 identities and statistics.
- **Players** searches SteamID64 and every nickname observed for that account.
- **Update player journal** rebuilds `history/players.json` from the game records.
- **Export games…** exports the filtered games, selected game or selected player's
  games; from the SteamID page it exports the collection.
- **Import games…** previews an archive and adds new record IDs. Existing records,
  including conflicting IDs, are preserved. Independently recorded copies of the
  same real match can have different IDs and remain separate records.
- **Open games folder** opens the local JSON directory.

Player names and dates describe observed game records, not a complete Steam name
history. Unknown results stay unknown. Late attachment is marked as partial
coverage. The admin key gates app features; the local JSON files are not encrypted.

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
