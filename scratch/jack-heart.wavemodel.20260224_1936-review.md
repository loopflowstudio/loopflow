# Interactive sessions convergence review

## What was implemented

- Wave executor now creates real sessions for `WaitInteractive` steps instead of relying on terminal launch conventions.
- `wave_waiting` events now carry optional `session_id`, and Concerto consumes it to join existing sessions.
- Interactive session completion now auto-commits and resumes flow execution; failed sessions now fail the wave run instead of advancing.
- `POST /v0/waves` now accepts `run: true` to create and start in one request.
- Concerto Start Wave flow now asks for a wave name and starts `design-ship-review` directly.
- Concerto Wave detail now prefers chat sessions tied to waiting waves and joins by provided `session_id`.
- Added built-in `design-ship-review` flow and docs updates for the new flow.

## Key choices

- **Executor owns lifecycle**: session creation + run advancement stay in lfd, not UI, so flow progress is resilient to UI disconnects.
- **No new session states**: reused `starting/active/ending/ended/failed` instead of adding a special waiting-for-user state.
- **Immediate harness start**: interactive steps begin on session creation; users join in-progress via replay.
- **Failure semantics tightened**: a `failed` interactive session now marks the run/wave failed (with test coverage) rather than silently continuing.

## How it fits together

Executor hits an interactive step, creates a session via `SessionManager`, marks the run/wave waiting, and emits `wave_waiting(session_id)`. Concerto receives that event, binds chat state to the existing session, and streams transcript events. When the session ends, lfd watcher logic commits changes, advances step index, and reacquires scheduler capacity to continue the same run.

## Risks and bottlenecks

- Session watchers are in-memory tasks; lfd restart during `Waiting` currently has no watcher rehydration path.
- Resume path still depends on scheduler slot reacquisition; prolonged contention can delay continuation.
- `xcodebuild test -scheme Concerto` can fail in headless/local automation environments due UI-test automation bootstrap timeout, even when package/unit tests pass.

## What's not included

- No multi-session orchestration per interactive step.
- No session handoff / multi-user coordination.
- No full replacement removal of terminal fallback UI (`InteractiveSessionView` remains available).
- No auto-close/delete policy for completed waves in this phase.

## Wave alignment

This directly advances the wave goal of design-first onboarding and unified interactive execution (`design → ship → review`) while keeping prompt assembly shared between session and executor paths.
