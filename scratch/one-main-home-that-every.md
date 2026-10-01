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

Checks: `cargo test` focused Home/global/harness/frontier/install/Task-resume checks passed (28); `uv run pytest python/tests/test_install_script.py -q` passed (8); `swift test --package-path swift --filter ActiveRunsObservationTests` passed (8); `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all --check`, changed-file Ruff and `git diff --check` passed; disposable-account routing proof deferred to gate.
