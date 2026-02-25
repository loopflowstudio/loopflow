# Interactive sessions in flow execution (current state)

Converged executor-driven interactive steps onto the session API so waves can move through `design → ship → review` without terminal handoffs.

## Current behavior

- `WaitInteractive` steps now create real sessions via `SessionManager`.
- Wave runs enter `Waiting` with a stored `session_id`.
- `wave_waiting` events now include optional `session_id`.
- Concerto joins existing sessions instead of creating new ones when a waiting wave provides `session_id`.
- Session completion now auto-commits, advances step index, and resumes the run.
- Failed interactive sessions now fail the run/wave instead of silently advancing.
- `POST /v0/waves` now supports `run: true` for create-and-start.
- Built-in flow now includes `design-ship-review`.

## Architecture and ownership

- **Executor owns lifecycle**: create session, watch terminal state, advance run.
- **UI is participant/viewer**: Concerto joins and interacts with executor-owned sessions.
- **Session lifecycle unchanged**: `starting → active → ending → ended/failed`.
- **Harness starts immediately** on session creation; users join in-progress via replay.

## Scope boundaries

### In scope (done)

- Rust executor/session integration for interactive steps
- WebSocket payload update for `wave_waiting(session_id)`
- Concerto join-existing-session path
- Interactive-step auto-commit/auto-resume
- Default onboarding flow: `design → ship → review`

### Out of scope (still)

- Multi-session orchestration per interactive step
- Multi-user handoff/coordination
- Removing terminal fallback UI completely
- Per-tab step routing changes in Concerto
- Real-time filesystem-driven wave content refresh
- DB schema changes for wave content

## Remaining risks and follow-ups

- **Daemon restart gap**: session watchers are in-memory; restart during `Waiting` has no rehydration path.
- **Resume contention**: resume still depends on reacquiring scheduler capacity.
- **Local UI-test reliability**: headless `xcodebuild test -scheme Concerto` can time out on automation bootstrap.

## Why this matters

This enables mixed interactive + automated flows to run as one continuous wave, with transcript-backed chat UX and reconnect support, while keeping flow progression resilient to UI disconnects.
