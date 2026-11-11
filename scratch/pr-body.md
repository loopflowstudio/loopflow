Planning guidance discarded facts recoverable from code or docs even when they explained the chosen approach. Kickoff now traces affected behavior through relevant docs, implementation, callers and tests, keeping a few findings with source paths and symbols in the active plan. Compress and realign retain useful findings, correct stale ones and remove those that no longer help.

The guidance stays generic and keeps conflicts and open questions visible. Findings guide further reading rather than replace it.

## Validation

- Reviewed the three-file diff; `git diff --check` passed.
- Architecture coverage, Rust test inventory and `cargo fmt --all -- --check` passed.
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(documented_commands::)'` stopped during the CLI build after macOS `syspolicyd` exceeded the runner's host-security CPU threshold for three samples. The documentation tests and Clippy did not run locally; CI must verify them before merge.
