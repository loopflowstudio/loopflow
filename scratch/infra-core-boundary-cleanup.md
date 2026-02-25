# 01: Core Boundary Cleanup

## Goal

Reduce regression blast radius in three hotspot seams:

- `lfd/store/mod.rs`
- `lfd/executor/docker.rs`
- `engine/agent.rs` harness command switching

This pass is structural (deconcentration), not a behavior change pass.

## What is now true on this branch

### Harness command construction

- `build_model_command()` dispatches directly via `match` to per-harness builder functions (`build_claude_command`, `build_codex_command`, `build_gemini_command`, `build_opencode_command`) — all in `engine/agent.rs`.
- No trait, no separate modules. The initial `HarnessCommandBuilder` trait extraction was compressed back: functions are simpler than a trait with one method and one impl per module.
- `apply_harness_env()` handles model-specific env var setup (Gemini system prompt, OpenCode config).
- Unknown-model fallback behavior preserved: falls back to Claude with the full model string as variant.
- Regression coverage: `build_model_command_falls_back_to_claude_for_unknown_model`, `build_model_command_uses_codex_default_for_bare_codex_model`.

### Docker executor decomposition

- Monolithic `lfd/executor/docker.rs` was replaced by lifecycle modules:
  - `docker/mod.rs` (orchestration)
  - `docker/image.rs`
  - `docker/workspace.rs`
  - `docker/recovery.rs`
  - `docker/io.rs`
  - `docker/tests.rs`
- Each submodule uses explicit imports (no `use super::*`), making cross-module dependencies visible.
- Labels, mounts, and startup recovery contracts are preserved.
- Executor behavior remains behind the same `AgentExecutor` surface.

### Store capability cleanup

- `SessionStore` is now a first-class store capability.
- `Store` now exposes capability accessors:
  - `wave_state()`
  - `execution()`
  - `sessions()`
  - `admin()`
- Session-facing paths now use capability routing, reducing forwarding boilerplate.

## Validation run

The following passed on this branch:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow build_model_command`
- `cargo test -p loopflow store_`
- `cargo test -p loopflow docker_`

## Remaining gaps and risks

- **Store deconcentration is partial**: backend `match` dispatch still exists inside trait impls; backend-port adapter extraction is not complete.
- **Docker recovery remains complex**: decomposition improves reviewability, but reconciliation logic is still high-risk if metadata conventions drift.
- **Known flaky unrelated test**: full `cargo test -p loopflow` intermittently hit `wave_worktree_tests::wave_rename_renames_branch`; targeted reruns passed.

## Follow-up decisions

1. Should a follow-up complete the store backend-port shape (`SqliteStoreBackend` / `PostgresStoreBackend`) and remove remaining `Store` forwarding/match surface?
2. Should Docker startup-recovery tests that currently soft-skip without Docker be split into a Docker-required suite?

## Out of scope in this pass

- Prompt budgeting/rendering changes
- Trigger model changes (polling/webhooks)
- Flow language changes
- Session API contract changes
- Persisted field renames (for example `provider_session_id`)
