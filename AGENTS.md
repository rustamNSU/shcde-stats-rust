# AGENTS.md
See `skills/rust.md` for Rust development guidelines.

## Project Purpose

This repo is a Rust + Slint desktop overlay for Stronghold Crusader: Definitive Edition streams and solo practice.

The final product should:

- collect live match stats,
- aggregate player and match data for display,
- display a clean stream overlay,
- support full settings mode, compact overlay mode, and timer mode,
- stay read-only for normal public use.

## Important Reference Files

Use these files as instructions and reference material:
instructions/gui.md
  GUI behavior notes for the main settings window, overlay window, and timer window.
  Keep this file user-facing and free of address maps or reverse-engineering notes.

## Recommended structure:
src/
  engine/
    live stats collection and aggregation
  ui/
    bridge.rs
  main.rs
ui/
  app.slint


## Collaboration Rules
- Keep patches focused.
- Do not rewrite architecture without a clear reason.
- Do not add public address maps, table layouts, debug prompts, or write-tool instructions.
- Document assumptions in comments when platform behavior is uncertain.
- Prefer simple working code over clever UI/window hacks.
