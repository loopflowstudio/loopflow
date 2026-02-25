# AgentAPI Phase 04: Hardening — Design Review

## What was implemented

Session reliability hardening across five areas:

1. **Event delivery correctness.** Replaced `broadcast` channel between harness and bridge with unbounded `mpsc`. Events from provider processes are no longer dropped under backpressure — every event reaches the store before being fanned out to SSE clients.

2. **SSE lag recovery.** When a broadcast subscriber falls behind (`RecvError::Lagged`), the SSE handler now backfills missed events from the store using `after_seq` rather than silently skipping them.

3. **Claude crash/stop hardening.** Three fixes: (a) drain the reader task with a 2s timeout before aborting on stop, (b) complete in-flight tool items as `Failed` when the process exits without a result event, (c) handle stop-during-`Starting` by marking the session `Failed` immediately and cleaning up the harness once startup returns.

4. **Scheduler occupancy tracking.** Sessions with a `wave_run_id` register with the scheduler on creation and unregister on any terminal status transition. This prevents waves from stalling while sessions hold a slot.

5. **Conformance replay tests.** Five recorded traces (Claude: normal, crash mid-tool, multi-tool; Codex: normal, error) replayed through the harness mapping layer, asserting canonical event output. Establishes the testing pattern for the OpenCode adapter.

Additionally, direction group expansion was added (`expand_direction_names`) with builtin groups ("roles", "values") and user-defined group directories, plus the `find_direction_path` function now searches subdirectories.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Unbounded `mpsc` for harness→bridge | Provider output must not be dropped. Bounded channels risk backpressure propagating to the subprocess. | Bounded `mpsc` with large capacity — still drops under burst. |
| Store-backed SSE backfill on `Lagged` | SSE clients may lag; store is the source of truth. | Best-effort: skip lagged events — data loss for slow clients. |
| 2s timeout on reader drain before abort | Gives in-flight events time to flush. Hard abort after timeout prevents infinite hangs. | No drain (immediate abort) — loses buffered events. |
| Tick-driven wave advancement (not callback) | Simpler; avoids tight coupling between sessions and wave state machine. | Immediate callback on session terminal — faster but more complex wiring. |
| `drain_failed_items()` on abnormal exit | In-flight tools should surface to UI as failed, not disappear silently. | Only emit `TurnCompleted(Failed)` — loses tool-level visibility. |

## How it fits together

```
Provider process → UnboundedMpsc → Bridge task → Store + Broadcast → SSE clients
                                                      ↑
                                        SSE Lagged? → Store backfill
```

The bridge task is the single writer to the store for a session's events. It also checks for fatal errors and transitions the session state machine. The scheduler integration sits above this: `create_session` registers, any terminal status transition unregisters.

The startup flow has a new race guard: if the session transitions to a terminal state (via `stop_session`) while the harness `start()` is in flight, the startup task detects this on return and cleans up the harness instead of transitioning to `Active`.

## Risks and bottlenecks

- **Unbounded memory.** The `mpsc` channel grows without bound if the bridge task stalls. Acceptable at current session lengths but worth monitoring for multi-hour sessions.
- **Store read on SSE lag.** If the store is slow or unavailable during a lag recovery, the SSE stream may stall or deliver events out of order. The current code falls through gracefully (logs warning, continues).
- **Static `AtomicBool` in tests.** `STARTING_STOP_CALLED` is a process-global static, which means the `stop_session_while_starting` test could conflict with parallel test runs touching the same flag. Currently safe because Rust tests run with `--test-threads=1` for integration tests, but fragile.
- **Direction group expansion** is O(n * m) where n = group count, m = filesystem reads. Fine for the handful of groups that exist today.

## What's not included

- **lfd restart orphan cleanup.** Active sessions become orphans on lfd restart. Events survive in the store, but a startup recovery pass (mark orphaned `active`/`starting` sessions as `failed`) is not implemented yet.
- **Immediate wave advancement.** Terminal sessions unregister from the scheduler, but advancement to the next wave step is tick-driven. An immediate callback could reduce latency.
- **OpenCode adapter.** The conformance test infrastructure is ready; the adapter itself is phase 01 work.
