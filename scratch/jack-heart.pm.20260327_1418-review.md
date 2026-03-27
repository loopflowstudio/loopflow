# PM-native ingest review

## What was implemented

Added a PM-aware refresh step to `ingest()` so PM-backed waves pull fresh provider state before picking a roadmap item. If that refresh fails, ingest now warns and continues with the existing local wave mirror instead of failing closed. The branch also adds focused ingest tests for refresh success and refresh failure, and updates user-facing docs to explain the new behavior.

## Key choices

- **Refresh inside `ingest()` instead of in flows.** Manual `lf ops ingest` now behaves correctly for PM-backed waves without depending on a flow-level `pm pull` prelude.
- **Warn and continue on pull failure.** Local wave files remain a usable fallback when credentials or network access are unavailable.
- **Use the main repo root for PM pulls.** This matches how wave directories are resolved from worktrees.
- **Treat warnings as warnings in progress sinks.** CLI and tracing progress implementations now expose warning messages without routing them through error-level logging.
- **Document the PM-native behavior where users learn ingest.** README, wave authoring docs, and the built-in ingest step now all describe the refresh-before-pick behavior.

## How it fits together

`ingest()` already resolves the wave name and main repo root before reading `wave/<name>/`. The new flow inserts a conditional `wave_pm_is_enabled()` check at that point, calls `pm_pull()` into the main repo when PM is configured, and then reuses the existing `list_wave_items()` → `select_wave_item()` → copy-to-scratch path unchanged. Test-only hook injection keeps the production code simple while letting unit tests simulate PM refresh success and failure.

## Risks and bottlenecks

- `build-or-silent` can still do a redundant `pm pull` before `ingest`; this is intentionally accepted for correctness.
- Live provider behavior was not exercised in this headless pass, so credential- and network-specific failures still rely on existing PM plumbing.
- The warning message currently includes its own `warning:` prefix because some progress sinks are plain stderr writers.

## What's not included

- No `--no-refresh` or offline toggle for ingest.
- No changes to PM priority mapping or item identity.
- No flow refactor to remove the already-acceptable redundant `pm pull` in PM-backed flows.

## Validation

- `cargo fmt`
- `cargo test -p loopflow ingest -- --nocapture`
- `cargo test -p loopflow --test golden_prompt -- --nocapture`
- `cargo clippy -p loopflow -- -D warnings`

Manual live-provider verification from the design doc was not run in this headless pass.
