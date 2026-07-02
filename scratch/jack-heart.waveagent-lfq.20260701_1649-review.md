# Gate Review — jack-heart.waveagent-lfq.20260701_1649

## What was implemented

Added the goal prompt primitive and made `wave/<name>/goal.md` the authored wave intent surface. Waves now carry required `goal` and `metrics` fields across Rust, Python, Swift, DTO fixtures, migrations, and API clients; lfd renders the selected goal with the Loopflow operating prompt, available flows, the roadmap handle, and metrics before launching a wave run.

Added `lfq sessions [wave]` and `lfq attach <session_id>`. The sessions command lists live terminal sessions, derives a role from `wave_run_id` and `source`, and flags sessions with unresolved interactive attention. The attach command posts to the terminal-session attach endpoint and replaces the process with `tmux attach -t <name>`.

Polished docs so the user-facing examples teach `goal.md` frontmatter instead of the old wave YAML config path, and added the new `lfq sessions` / `lfq attach` commands to the CLI references.

## Key choices

- `goal` is a required wave field and defaults to `ship-roadmap` for newly created or migrated waves.
- `metrics` is mirrored as a required DTO field so Rust, Python, and Swift stay in lockstep.
- `goal.md` frontmatter seeds durable wave intent (`goal`, `primary_flow`, `mode`, `workers`, `metrics`, `agent`, `pm`). Crons and triggers remain live lfd state instead of repo-authored `goal.md` fields.
- `lfq sessions` does not add a `needs-input` terminal status. It reuses unresolved `AttentionItem(kind=interactive)` records and their `terminal_session_id` context.
- The Python terminal-session client preserves the requested `statuses` API shape, maps live-status filters to `active_only=true`, and locally filters the returned payload because the Rust route does not expose a `statuses` query.
- `lfq attach` uses exact session ids for this unit. Short id prefix resolution is intentionally not included.

## How it fits together

Wave creation reads `wave/<name>/goal.md` frontmatter into the internal `WaveConfig`, then stores the resolved intent on the wave record. Wave execution loads the goal prompt via the same repo/user/builtin resolution model as other loopflow language primitives and renders it through the existing `lf : ...` launch path.

Terminal sessions are already owned by lfd. The new Python client wrappers expose list, attention, and attach calls; the CLI composes those read models into a small table and delegates attachment to tmux.

## Risks and bottlenecks

- Goal prompts still travel through the existing inline `lf : ...` path. Extremely large goals may need a file-backed launch path later.
- `lfq attach` requires tmux on `PATH` and exact session ids.
- `needs-input` depends on interactive attention items continuing to carry `context.terminal_session_id`.
- Concerto Xcode UI tests were not run in this headless gate because no rendering environment is available.

## What's not included

- No Asana live roadmap read/write implementation.
- No hosted 24/7 looping-agent supervisor, heartbeat, or crash relaunch logic.
- No budget/spend enforcement.
- No branch/PR isolation for parallel workers beyond the current worker controls.
- No removal of the still-live wave `area` and `direction` API/UI fields.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 164 tests.
- `cd website && uv run python dev.py test` passed after doc updates: 61 passed, 3 skipped.
- `swift test --package-path swift` passed: 5 XCTest tests and 336 Swift Testing tests.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` passed: 16 tests.
- `uv run pytest tests/regression/ -v` passed: 4 tests.
- `docker version` passed.
- `cargo test -p loopflow docker_` passed: 11 docker-filtered tests.
- `git diff --check` passed.
