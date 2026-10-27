# Gate receipts (LOO-624, PR #1743 at 1bf12dfcd3)

- Code tree identical to 9780b8bb63 (later commits only add/clear scratch); primary's focused rental/machine_credentials/linear/Fly-migration tests and all-target clippy reused.
- `cargo fmt --all -- --check`: clean (local).
- CI on 1bf12dfcd3: rust-lint, migration-check, python-test, architecture-check, e2e-smoke, loopflow-ui-test passed; rust-test shards, swift-test, macos-26 process test, task-installation still running — deferred to CI, owned by the land --wait-and-fix step. CLI reference check rides the integration shards.
- No stale Fly references in README/docs/rental code. PR body keeps the open gap: automatic give-back after Task completion plus grace not yet seen live; no rental spending authorized.
