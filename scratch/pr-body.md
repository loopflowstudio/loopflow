## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test -p loopflow
uv run pytest python/tests/test_install_script.py
cd website && uv run python dev.py test
uv run python scripts/concerto-dev.py lfd --help
```

Expected results from gate:

- Rust fmt, clippy, nextest, and `cargo test -p loopflow` pass.
- Python install-script tests pass: 9 passed.
- Website tests pass: 61 passed, 3 skipped.
- `scripts/concerto-dev.py lfd --help` exposes only native sqlite lfd controls.

## Intent

This PR makes the module graph match the wave architecture: commands call components, components do not call command or daemon internals, and `lfd` stops owning shared wave plumbing. It also removes the live container-mode service surface so local lfd is native sqlite only.

## Assumptions

Asana is the roadmap source of truth, but local roadmap lookup is blocked by an expired stored Asana token. The branch was gated from `scratch/` notes and `wave/goals/MEMORY.md`.

The public `lf q worker run` path remains intentionally live for this PR. The ordinary `lf --dispatch/--stack/--fork` placement grammar is documented as follow-up substrate work, not completed here.

## Key decisions

Shared vocabulary moved to neutral owners: `chat` for streamed turns/types, `harness` for vendor runtime adaptation, `engine` for repo/config/worktree conventions, and `wave` for subscription parsing.

Container mode was removed at the lfd service/config surface instead of preserving a compatibility shim. The Concerto dev script no longer advertises or launches the dead Docker/postgres lfd path.

Postgres internals in `lfdb` remain as M2 debt. This PR rejects the removed service entry points but does not pretend to be the sqlite-only substrate rewrite.

## Not included

No full postgres deletion. No `lf q worker run` replacement. No `step` to `skill` rename. No live vendor-backed wave demo during gate.
