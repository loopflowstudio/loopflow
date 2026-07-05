# Gate Review: jack-heart.goals.20260705_1100

## What was implemented

Moved shared wave plumbing out of daemon-owned modules. Chat turns/types now live under `chat`, vendor harness code lives under `harness`, GOAL.md frontmatter parsing lives under `engine::wave_config`, repo-root lookup lives under `engine::repo`, wave event subscription lives under `wave::subscription`, and wave/run worktree naming is centralized in `engine::worktrees`.

Removed the live container-mode service surface from `lfd`. Config now rejects removed mode/executor environment variables, native sqlite is the only documented service shape, compose service management is gone, and Concerto dev tooling no longer advertises a dead `lfd --docker` path.

## Key choices

Kept the public `lf q worker run` path as known follow-up debt rather than doing the placement-flag rewrite inside this PR. The branch resolves the worst dependency ownership issues first; ordinary `lf --dispatch/--stack/--fork` placement remains M2 scope.

Preserved the existing lfd HTTP route/DTO shape while moving reusable component ownership. Concerto can keep reading current responses while backend authority moves away from daemon-owned helpers.

Left deeper postgres code in `lfdb` as explicit substrate debt. Container mode and `LFD_DATABASE_URL` are rejected at the lfd surface, but the full sqlite-only lfdb cleanup is not completed here.

## How it fits together

Commands call shared components; components no longer reach upward into command or daemon modules. `wave` owns listener/resident concerns, `engine` owns repo/config/worktree conventions, `harness` owns vendor process adaptation, and `lfd` narrows back toward gatekeeper/query behavior.

## Risks and bottlenecks

The branch still carries the old worker grammar in `lf q worker run`; that keeps the demo live but means placement work remains split from the final desired CLI grammar. Deeper postgres and dual-backend cleanup is also still present in source, though no longer exposed as the native service path.

Asana roadmap validation was blocked locally because the stored Asana token is expired. This was already recorded in `scratch/questions.md`; validation proceeded from scratch notes and `wave/goals/MEMORY.md`.

## What's not included

No full `step` to `skill` rename. No sqlite-only lfdb substrate rewrite. No `lf q worker run` removal or synchronous placement flags. No live two-process wave demo, because it requires authenticated vendor execution and spends tokens.

## Validation

- `cargo fmt --check` passed.
- `cargo nextest run --all` passed: 1229 passed, 4 skipped.
- `cargo test -p loopflow` passed.
- `cargo clippy -- -D warnings` passed.
- `uv run pytest python/tests/test_install_script.py` passed: 9 passed.
- `cd website && uv run python dev.py test` passed: 61 passed, 3 skipped.
- `uv run python -m py_compile scripts/concerto-dev.py` passed.
- `uv run python scripts/concerto-dev.py --help`, `lfd --help`, and `run-debug --help` show the native-only dev surface.
- Dependency-direction smoke checks passed for `wave`: no `crate::lf::commands`, `crate::lfd::http::routes::wave_config`, `crate::lfd::executor`, or `crate::lfd::conversations` imports.
