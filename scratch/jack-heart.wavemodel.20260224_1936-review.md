# Review: Interactive sessions in flow execution

## What was implemented

Converged executor-driven interactive steps onto the session API so waves can move through `design → ship → review` without terminal handoffs.

**Rust (executor/sessions):**
- `WaveExecutor` now owns a `SessionManager` and creates real sessions for `WaitInteractive` steps instead of dangling agent records.
- New session watcher: spawns a tokio task that polls session status, auto-commits after interactive completion, and resumes the wave flow.
- Failed interactive sessions now fail the wave run (previously silently advanced).
- `wave_waiting` events include `session_id` so the UI can join immediately.
- `POST /v0/waves` accepts `run: true` for create-and-start in one call.
- `resolve_current_step_name` and `auto_commit_if_dirty` moved to `helpers.rs` (shared between executor and HTTP routes).
- `resolve_step_reference` in `flow.rs` resolves interactive flag from step frontmatter during flow expansion.
- Built-in `design-ship-review` flow added.

**Swift (Concerto):**
- `StartWaveView` changed from design-prompt-first to wave-name-first. Calls `createAndRunWave` which uses `POST /v0/waves` with `run: true` and `flow: "design-ship-review"`.
- `RepoState` tracks `waitingSessionIds` from `wave_waiting` events and wires `ChatState` to join existing sessions via `joinSession`.
- `ChatState.joinSession(_:)` resets stream state and connects to an existing session (no create call).
- `WaveDetailPanel` uses `activeChatState` to show the chat view when an interactive session is active.
- `WaveEvent` now carries optional `sessionId`.
- `LocalWaveService.createWave` accepts `flow` and `run` parameters.

## Key choices

- **Executor owns session lifecycle, UI joins.** The executor creates the session and watches for terminal state. Concerto joins via `session_id` from the event. This keeps flow progression resilient to UI disconnects.
- **Polling loop for session status.** `wait_for_terminal_session_status` polls every 250ms. Simpler than adding a dedicated notification channel from SessionManager, and sessions typically last minutes.
- **Auto-start via empty input.** When the executor creates a session for a wave step, it sends an empty input so the harness delivers the step prompt as the first turn. This removes the need for the user to "kick" the session.
- **`wait_for_wave_start_settle` in create handler.** When `run: true`, the response waits up to 500ms for the wave to settle (e.g., hit `Waiting` state) so the client gets useful status instead of transient `Running`.
- **Step reference resolution during expansion.** A bare step name in a flow YAML now resolves its `interactive` flag from frontmatter. Previously the flag was lost, causing `design` steps in flows to run as non-interactive.

## How it fits together

```
StartWaveView → POST /v0/waves {run: true, flow: "design-ship-review"}
                    ↓
         WaveExecutor.execute_run()
                    ↓
         FlowAction::WaitInteractive → create_interactive_session()
                    ↓                        ↓
         wave_waiting(session_id) ←   spawn session watcher
                    ↓                        ↓
         Concerto joins session      watcher polls until terminal
                    ↓                        ↓
         User interacts via chat     auto-commit → advance step → resume
```

## Risks and bottlenecks

- **Daemon restart loses watchers.** Session watchers are tokio tasks in memory. If `lfd` restarts while a run is `Waiting`, the watcher is gone and the run stays stuck. No rehydration path yet (documented in `scratch/questions.md`).
- **Resume contention.** After session ends, the watcher retries for 60s to acquire a scheduler slot. Prolonged contention could fail the resume.
- **Polling overhead.** `wait_for_terminal_session_status` polls at 250ms. Low overhead for single sessions, but would need a subscription model if many concurrent interactive sessions exist.
- **Settle delay in API response.** `wait_for_wave_start_settle` adds up to 500ms latency on `POST /v0/waves` with `run: true`. Acceptable for a user-initiated action but worth noting.

## What's not included

- Multi-session orchestration per interactive step
- Multi-user handoff/coordination
- Daemon restart rehydration for session watchers
- Per-tab step routing in Concerto
- Real-time filesystem-driven wave content refresh
- Removing terminal fallback UI
