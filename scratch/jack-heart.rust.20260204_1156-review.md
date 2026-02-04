# Review: Rust prompt parity harness

## What was implemented
- Added a Rust `lf-prompt` helper binary to emit formatted prompts for parity testing.
- Added Rust golden prompt tests plus Python/Rust prompt parity tests with fixtures and golden files.
- Added e2e shell scripts for `lf ops` full-cycle and rebase-conflict workflows.
- Added a parity/testing design note in `scratch/` and updated `TESTING.md` with Rust/e2e commands.

## Key choices
- Use a dedicated `lf-prompt` binary to avoid coupling parity tests to the main CLI and keep inputs explicit.
- Normalize prompts (paths, line endings) before comparison to keep goldens stable across environments.
- Keep e2e scripts minimal and repo-local by using temp git repos and `cargo run` instead of external tooling.

## How it fits together
Python generates goldens and parity cases; Rust uses `lf-prompt` + `gather_context` to produce equivalent prompts.
The Python parity test compares Python vs Rust outputs, while Rust golden tests compare against expected prompt files.

## Risks and bottlenecks
- `cargo run` in e2e scripts is slow and can be noisy in CI; consider building once and reusing binaries.
- Parity fixtures are small; missing edge cases could hide prompt drift.
- Goldens are generated from Python; if Python behavior changes, goldens need regeneration.

## What's not included
- No CI wiring for Rust/e2e tests yet.
- No additional parity fixtures beyond the basic/direction cases.
- No Rust-side golden regeneration tool; Python remains the source of truth.
