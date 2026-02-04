# Review: Rust Testing and Rollout

## What was implemented
- Added prompt parity tooling (`lf-prompt` + golden tests) to compare Rust prompt assembly against Python.
- Added ops parity tracing in Python and Rust, with a parity test that compares traces.
- Added E2E shell tests (smoke, full cycle, rebase conflict) and wired smoke test into CI.
- Added release workflow to bundle platform `lf` binaries into the Python wheel and dispatch `lf` via `src/loopflow/_bin.py`.
- Updated docs to reflect Rust-first behavior and testing entry points.

## Key choices
- **Trace-based ops parity**: trace JSON is emitted instead of running side effects, keeping tests deterministic.
- **Golden prompts**: keep Python as the source of truth and verify Rust output via fixed fixtures.
- **Rust-first CLI**: `LF_RUST=0` is the only escape hatch; default favors Rust when a bundled binary exists.

## How it fits together
`lf-prompt` and golden fixtures validate prompt assembly parity; ops tracing produces comparable JSON between Python and Rust for `lf ops commit`. CI runs Rust tests, Python tests, and E2E smoke, while the release workflow stages `lf` binaries into the wheel so `_bin.py` can exec the native CLI.

## Risks and bottlenecks
- Ops trace parity currently covers `commit` only; other ops commands still rely on behavioral tests.
- E2E scripts compile `lf`/`loopflow-engine` during execution; CI runtime may be heavier than expected.
- Release workflow ships only `lf` binaries; `lfd` bundling remains an open decision.

## What's not included
- `lfd` packaging or rollout changes.
- Additional parity fixtures beyond the current prompt and commit traces.
- Any new CLI dry-run flag; smoke test still uses `lf-prompt`.
