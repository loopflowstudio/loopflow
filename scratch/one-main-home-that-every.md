# One main Home — LOO-342

Accepted direction: Jack Heart, 2026-09-30. Every ordinary CLI, Task worker,
Flow step and agent-issued command uses the installed CLI and `~/.lf`.
An explicit `LF_HOME` selects disposable experimental data. No implicit copy,
retained development installation, upgrade or recovery contract remains.

## Implementation

Resolve the default executable before command effects. Source builds without an
explicit experimental Home forward to the installed CLI. Resolve data independently
of artifact receipts: `LF_HOME` or `~/.lf`, with nested commands inheriting that
choice. Installation continues to update the main Home through the existing
published migration boundary. A custom Home initializes once and validates its
exact schema afterward; schema changes require a new experiment.

## Delete — do not maintain

- `store/branch_data.rs`: source-specific directories, snapshot seeding and authority scrubbing.
- Local development promotion, `--from-build`, `--fresh`, `--reuse-home`, local preflight and retained-pair recovery.
- `ops/task_destination.rs` and its per-operation cross-store dispatch consumers.
- Receipt-based selection of Home paths and retained development handoffs.
- Installer `local --use` and exclusive development-promotion fixtures.
- Experimental-store draft append/adoption/backup paths and their upgrade fixtures.
- Separate authority, observation and current-Home resolvers; all consumers use the same Home and execution context.

Preserve published artifact verification, promotion recovery, the main database
migration boundary, explicit experimental isolation, and ordinary Task/Session
identity and continuation. No automatic data merging or Task transfer.

## Remaining

- Publication and published installation remain the caller's later Flow steps.
- Finish retiring the stores recorded in `home-retirement.json` after their live
  processes settle. Older running builds can still recreate implicit side Homes
  until the published cutover; no process was killed or active database removed.

## Review findings resolved

- Release harnesses removed ordinary Home variables and depended on hidden pins.
  Provider tools and tmux workers now inherit the same explicit Home.
- Forwarding into an older installed CLI could revive inherited control pins.
  Default dispatch now supplies the main Home and removes the retired pins.
- Relative experimental paths could change meaning after a child changed its
  working directory. Startup canonicalizes both paths before child execution.
- The Task resume regression no longer needs a development installation fixture;
  it now runs in the ordinary suite using an explicit experiment.
- Experimental initialization still carried draft append, release adoption and
  backup machinery. It now initializes an empty schema once and validates later
  opens; header-only files and concurrent first opens remain supported.
- Home aliases and duplicate execution-context resolvers obscured the single
  destination. Their callers now use the same Home, database and CLI resolvers.
- Startup and child launches now share published-artifact selection, including
  the fallback from a retired development selection. Diagnostics give one
  disposable-Home instruction instead of contradictory recovery advice.
- Gate corrected PR fixtures that selected only `LF_DB_PATH`, provider fixtures
  that still read retired `LF_CONTROL_HOME`, and a Flow fixture that expected PATH
  to override the selected CLI. The fixtures now exercise explicit `LF_HOME` and
  `LF_BIN`, retaining the schema-refusal and completed-effect assertions.
- The populated historical status fixture now applies published migrations
  explicitly before reading the result; ordinary experimental opens no longer
  serve as its upgrade boundary.
- Testing guidance and the migration-authority comment now describe one-time
  experimental initialization. Infrastructure memory retains Jack's post-release
  cleanup direction and the five dated retirement paths before scratch clearing.

Checks (gate, 2026-09-30): `cargo nextest run --all --no-fail-fast --build-jobs 4 --test-threads 4` completed 2,010 cases (1,995 passed, 15 stale-fixture failures, 13 skipped); after repairs, `cargo nextest run -p loopflow --test flow_tests --test land_tests --test pr_tests --test status_tests --no-fail-fast --build-jobs 4 --test-threads 4` passed all 87 (2 skipped); `uv run python scripts/test_task_installation.py` passed all 4 disposable-account proofs after correcting its explicit executable fixture; `uv run pytest python/tests/` passed 312; `cd website && uv run python dev.py test` passed 78 (3 skipped); `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone` built the app and passed 304 tests without WindowServer; `cargo clippy --all-targets --jobs 4 -- -D warnings`, `cargo fmt --all -- --check`, architecture and Swift-boundary scripts, `check_migrations.py` (58 released migrations unchanged), and `git diff --check` passed; disposable-copy `canonicalize_migrations.py 0.12.29 --materialize-for-tests` found no drafts, so the tested schema already matches the materialized graph. Hosted CI owns the final candidate matrix; live installation and retirement remain later operations.
