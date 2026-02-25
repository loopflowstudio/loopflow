# Core Boundary Cleanup — Review Guide

## What was implemented

- Replaced harness switching in `engine/agent.rs` with a harness-command registry in `engine/harness_commands/`.
  - Added per-harness builders (`claude`, `codex`, `gemini`, `opencode`) implementing a shared `HarnessCommandBuilder` trait.
  - `build_model_command()` and `launch_agent()` now delegate to `builder_for_model()`.
- Added `SessionStore` as a first-class store capability in `lfd/store/mod.rs`.
  - Added capability accessors on `Store`: `wave_state()`, `execution()`, `sessions()`, `admin()`.
  - Routed session-facing `Store` methods through trait dispatch to reduce forwarding duplication.
- Split monolithic Docker executor (`lfd/executor/docker.rs`) into lifecycle modules:
  - `docker/mod.rs` (orchestration)
  - `docker/image.rs` (image lifecycle)
  - `docker/workspace.rs` (workspace/volume prep)
  - `docker/recovery.rs` (startup rehydration/orphan cleanup)
  - `docker/io.rs` (run/log/terminate paths)
  - `docker/tests.rs` (module-local coverage)
- Added a regression test to preserve unknown-model fallback behavior after the harness refactor:
  - `build_model_command_falls_back_to_claude_for_unknown_model`.

## Key choices

1. **Trait+registry over central `match` for harness command construction**
   - Keeps harness-specific behavior in harness-owned files and makes new harness onboarding additive.
2. **Preserve fallback semantics explicitly**
   - Unknown models still route through Claude with the full model string as variant (behavior parity with prior `FallbackClaude`).
3. **Decompose Docker by lifecycle ownership, not by low-level primitives**
   - Image, workspace, recovery, and IO now map directly to failure domains.
4. **Keep SQLite async safety behavior unchanged**
   - SQLite still executes through `run_sqlite`/`spawn_blocking` paths.

## How it fits together

`engine::agent` is now a thin orchestrator: it chooses a harness builder and lets that builder define command args and harness-specific env setup. The Docker executor keeps a single public executor surface while delegating lifecycle concerns to focused modules. The store layer now has an explicit session capability trait, aligning session operations with the existing wave/execution/admin capability pattern.

## Risks and bottlenecks

- **Store deconcentration is partial**: `Store` now exposes capability accessors and a `SessionStore` trait, but backend `match` dispatch still exists inside trait impls; backend-port extraction is not complete yet.
- **Docker recovery remains high-complexity logic**: startup reconciliation is safer to review now, but still sensitive to metadata/label drift across releases.
- **Test stability note**: a full `cargo test -p loopflow` run intermittently surfaced a pre-existing flaky `wave_worktree_tests::wave_rename_renames_branch` failure; targeted reruns passed.

## What's not included

- No change to external API contracts for sessions/waves.
- No prompt budgeting, flow-language, or trigger-model behavior changes.
- No migration of persisted field names (for example, `provider_session_id`).
- No full backend-adapter rewrite (`SqliteStoreBackend`/`PostgresStoreBackend` object-port abstraction) in this pass.

## Wave alignment

- Advances wave goal **“Make extension points trait-based, not switch-based”** via harness registry/builders.
- Advances wave goal **“Maintain architectural compactness as features grow”** via Docker lifecycle split (2,839-line monolith removed).
- Advances wave goal **“Eliminate boilerplate and duplicated patterns”** in session store paths via `SessionStore` capability routing.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow build_model_command`
- `cargo test -p loopflow store_`
- `cargo test -p loopflow docker_`

All commands above passed on this branch.
