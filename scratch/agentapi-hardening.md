# AgentAPI Phase 04: Hardening

## Status

Completed on branch `jack-heart.agentapi.20260225_1122`.

This file is the canonical summary for AgentAPI hardening work. It replaces separate planning and review notes.

## Remaining follow-up

- Decide whether successful session completion should also trigger immediate waiting-step advancement, or continue relying on scheduler tick progression after session unregister.

## Goal

Eliminate reliability gaps in interactive sessions: event loss before persistence, crash/stop lifecycle leaks, and wave runs stalling while sessions are active.

## Implemented

- Replaced harness→bridge delivery with unbounded `mpsc` (single producer/single consumer) so events are not dropped before store persistence.
- Kept broadcast fan-out for bridge→SSE clients, but added SSE lag recovery using store backfill on `RecvError::Lagged`.
- Hardened Claude stop/crash behavior:
  - drain reader before final shutdown completion,
  - avoid stale completion races during stop,
  - complete in-flight tool items as failed on abnormal exit.
- Wired session lifecycle into scheduler occupancy tracking:
  - register on session creation when `wave_run_id` is present,
  - unregister on terminal session paths (stop/fail/end).
- Added `Starting`-state stop handling so sessions can fail fast without hanging on incomplete startup.
- Added provider conformance replay tests with recorded traces:
  - Claude: normal turn, crash mid-tool, multi-tool,
  - Codex: normal turn, error path.
- Added startup cleanup behavior so stop-during-starting does not leak provider processes.

## Decisions to preserve

- Use unbounded `mpsc` for harness→bridge correctness; bounded channels risk subprocess backpressure and dropped provider output.
- Use store-backed SSE gap repair instead of best-effort live buffering.
- Keep wave resume behavior scheduler-driven (unregister + loop ticker) rather than tightly coupling sessions to direct wave callbacks.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all -- --skip lfd::executor::docker::tests::docker_startup_rehydrates_running_agents_and_cleans_orphans --skip lfd::executor::docker::tests::docker_startup_lost_agent_does_not_flip_terminal_run_wave_status`
- `cargo test -p loopflow lfd::sessions::tests::`
- `cargo test -p loopflow lfd::http::routes::sessions::tests::`
- `cargo test -p loopflow lfd::sessions::harness::conformance_tests::`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

All commands above passed in this environment.

## Known limits

- Harness→bridge queue is intentionally unbounded; memory grows with unconsumed burst volume.
- Wave advancement is tick-driven after session unregister; not immediate callback-driven.
- SSE recovery depends on store read availability during lag repair.
- Docker startup tests still require `/var/run/docker.sock` in the executing environment.
