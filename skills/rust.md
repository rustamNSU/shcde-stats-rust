# Rust Development Rules for This Repo

These rules apply to agents writing Rust code for the SHC eco-simulator core engine and Slint GUI.

## Core Principles

1. Keep code simple and predictable.
2. Prefer readability over cleverness.
3. Ship small, safe changes.
4. Avoid unnecessary abstraction and premature optimization.

## Code Style

1. Follow standard Rust formatting and linting:
   - `cargo fmt`
   - `cargo clippy --all-targets --all-features -D warnings` (when practical)
2. Use clear names:
   - Types: `PascalCase`
   - Functions/variables/modules: `snake_case`
   - Constants: `UPPER_SNAKE_CASE`
3. Keep functions short and focused on one job.
4. Prefer explicit, straightforward control flow over deeply nested logic.
5. Avoid “magic numbers”; extract meaningful constants.
6. Keep modules cohesive: each module should have a clear responsibility.

## Complexity Rules

1. Do not write “smart” code that is hard to read.
2. Do not over-engineer:
   - No extra traits/generics/macros unless they clearly reduce duplication and improve clarity.
   - No architecture layers that do not provide concrete value now.
3. Prefer composition and plain data structures over complicated inheritance-like patterns.
4. If a solution needs a long explanation, rewrite it simpler.

## Error Handling

1. Do not use `.unwrap()` / `.expect()` in normal runtime paths.
2. Use `Result` and propagate errors with context.
3. Use panics only for truly unrecoverable programmer errors.
4. Error messages must be actionable and specific.

## Comments and Documentation

1. Do not comment every line.
2. Add comments only when code intent is not obvious.
3. Prefer self-explanatory names and small functions over verbose comments.
4. Keep comments short and factual; remove stale comments during edits.
5. Add doc comments for public APIs and tricky domain logic.

## Testing

1. Add/adjust tests for behavior changes.
2. Prefer focused unit tests for core engine logic.
3. Add integration tests for important flows or boundaries.
4. Tests should be deterministic and readable.

## Performance and Memory

1. Measure first, optimize second.
2. Avoid unnecessary allocations and clones in hot paths.
3. Keep data layout and ownership simple unless profiling proves a bottleneck.

## Engine-Specific Guidance (Eco-Simulator Core)

1. Keep simulation rules deterministic.
2. Separate domain logic from I/O, UI, and persistence.
3. Use strong types for domain concepts where it improves correctness.
4. Keep update/tick logic easy to trace and test.

## Slint GUI Guidance

1. Keep business logic in Rust, not embedded in complex UI callbacks.
2. Keep `.slint` files mostly declarative and clean.
3. UI state changes should be explicit and predictable.
4. Avoid tight coupling between GUI widgets and core simulation internals.

## Change Hygiene for Agents

1. Touch only files needed for the task.
2. Do not refactor unrelated areas.
3. Preserve existing behavior unless change is requested.
4. Summarize what changed and why in plain language.
