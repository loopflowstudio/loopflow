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

- Gate owns the full affected suites and materialized migration checks, including
  `default_and_nested_commands_use_the_installed_cli_and_main_home` in the
  disposable OS-account harness. The local source CLI was observed forwarding
  `home id --json` to the installed main Home with stale control pins.
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

Checks: `cargo test -p loopflow --lib store::migrations::tests:: -- --test-threads=4` passed (79); focused published-CLI selection and WAL-lock regressions passed; `cargo test -p loopflow --test one_home_tests --test global_commands` passed (10, one disposable-account proof deferred to gate); `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all --check` and `git diff --check` passed; prior installer Python (8) and Swift observation (8) passes remain applicable.
