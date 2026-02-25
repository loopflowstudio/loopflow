# AgentAPI Phase 04: Hardening

## Problem

Interactive sessions have three reliability gaps that affect real users:

1. **Silent event loss.** When the harness event bridge lags (broadcast buffer overflow), events are dropped *before* reaching the store. This is permanent data loss — no reconnect recovers it. A fast-typing agent producing many `TextDelta` events in a burst can overflow the 256-entry buffer while the bridge is blocked on a slow store write.

2. **Crash state leaks.** Claude's process-per-turn model means normal process exits happen constantly. But when a crash occurs mid-tool, partial state is silently dropped: the accumulated `input_json_delta` vanishes, in-flight tool items never complete, and the `stop()` race can emit a stale `TurnCompleted(Completed)` alongside the stop flow.

3. **Sessions are islands.** Session end doesn't trigger wave advancement. Wave runs block while a session is active (loop ticker checks `has_active_session`) but never resume when the session ends. The scheduler's `register_session`/`unregister_session` methods exist but aren't called.

Secondary concerns: SSE lag recovery for slow clients (recoverable via reconnect, but jarring), concurrent client fan-out verification, and provider conformance tests.

## Approach

### 1. Event bridge: mpsc replaces broadcast for harness→bridge

The harness event bridge is a single consumer. Using `tokio::broadcast` (which drops on overflow) is wrong — use an unbounded `tokio::sync::mpsc` channel instead. The bridge is the only reader; the harness is the only writer. Backpressure isn't meaningful here because the bridge must persist every event.

**Changes:**
- `HarnessEventChannel`: switch from `broadcast::channel(256)` to `mpsc::unbounded_channel()` for the harness→bridge path.
- Keep `broadcast::channel(LIVE_EVENT_BUFFER)` for bridge→SSE clients (this fan-out does need broadcast semantics).
- The harness implementations (`claude.rs`, `codex.rs`) switch from `events.send()` (broadcast) to `events.send()` (mpsc unbounded) — same call signature, different channel type.

This eliminates the data loss bug entirely. The bridge can't lag behind the harness because mpsc unbounded never drops.

### 2. SSE lag backfill from store

When the SSE broadcast receiver gets `RecvError::Lagged`, the handler currently skips lost events. Instead: backfill from the store.

**Changes to `stream_session_events_handler`:**
```
Err(RecvError::Lagged(_)) => {
    // Fetch events from store starting after last_seq
    let missed = store.list_session_events(&session_id, Some(last_seq)).await;
    for event in missed {
        if event.seq <= last_seq { continue; }
        last_seq = event.seq;
        tx.send(Ok(session_event_sse(&event))).await;
    }
    // Continue receiving from broadcast — cursor is now at oldest buffered message
}
```

The backfill reads from the store (which has all events thanks to fix #1), streams them to the client, then resumes from the broadcast channel. Dedup by `last_seq` handles overlap between store and broadcast.

### 3. Claude crash cleanup: drain before stop

The `stop()` race: kill the child → reader sees EOF → emits stale `TurnCompleted` → abort reader. Fix: drain the reader *before* emitting stop events, and complete in-flight tools on abnormal exit.

**Changes to `claude.rs`:**

**a) `stop()` — drain reader first:**
```rust
async fn stop(&mut self) -> Result<()> {
    self.shutdown_requested.store(true, Ordering::SeqCst);
    if let Some(mut child) = self.child.take() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    // Wait for reader to finish (it checks shutdown_requested)
    // rather than aborting it
    if let Some(task) = self.reader_task.take() {
        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
    }
    // stderr can be aborted — it's diagnostic only
    if let Some(task) = self.stderr_task.take() {
        task.abort();
    }
    self.turn_in_progress.store(false, Ordering::SeqCst);
    Ok(())
}
```

The reader task already checks `shutdown_requested` and breaks. After the child is killed, stdout hits EOF, the reader sees no more lines, checks the flag, and exits. Waiting up to 2 seconds (with timeout) ensures clean drain. Only abort as fallback if it doesn't exit.

**b) Reader task — complete in-flight tools on abnormal exit:**

When the reader loop ends without `saw_turn_completed` and `shutdown` wasn't requested (abnormal exit), emit `ItemCompleted` with `Failed` status for every tool in `state.active_tools` before emitting `TurnCompleted(Failed)`. This gives clients a clean item lifecycle even on crashes.

### 4. Wave integration: session lifecycle hooks

Wire session end into wave advancement via the scheduler.

**Changes:**

**a) Register/unregister sessions in scheduler:**
- `create_session()`: after runtime creation, call `scheduler.register_session(wave_id)` if `wave_run_id` is set.
- `stop_session()` / `mark_session_failed()`: call `scheduler.unregister_session(wave_id)`.
- `recover_orphaned_sessions()`: no scheduler call needed (sessions never registered at startup).

**b) Session end callback:**
- Add `on_session_ended(session_id, status, wave_run_id)` method to `SessionManager` (or as a hook the bridge task calls after terminal status).
- When a session reaches `Ended`/`Failed` with a `wave_run_id`:
  1. Unregister from scheduler.
  2. If `Ended` (success): trigger wave step advancement via existing executor machinery.
  3. If `Failed`: log and leave the wave run for the loop ticker to retry or escalate.

The loop ticker's existing `has_active_session` check means wave execution resumes naturally once the session is unregistered.

### 5. End-during-starting guard

`stop_session()` should handle the `Starting` state. Currently it checks for terminal states and returns early, but `Starting` could mean the harness startup task hasn't finished yet. The harness might not be ready to stop.

**Change:** When status is `Starting`, set status to `Failed` directly (skip `Ending` → harness.stop() → `Ended` flow). The startup task will see the status change and bail. Emit `StatusChanged(Failed)` event.

### 6. Provider conformance tests

Add trace-replay tests that feed recorded provider output through the mapping modules.

**Structure:**
```
rust/loopflow/src/lfd/sessions/harness/
    testdata/
        claude_normal_turn.ndjson
        claude_crash_mid_tool.ndjson
        claude_multi_tool.ndjson
        codex_normal_turn.jsonl
        codex_error.jsonl
```

Each test:
1. Read a recorded trace file.
2. Feed lines through `claude_mapping::process_line` or the codex mapping equivalent.
3. Assert the sequence of `SessionEvent`s matches expected output.

These are unit tests on the mapping modules — no subprocess spawning, no network. They catch NDJSON edge cases that hand-written unit tests miss because they use real provider output.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Bounded mpsc for harness→bridge | Backpressure slows the harness | Harness subprocess can't be slowed — it writes to stdout and blocks if pipe is full, which kills the child |
| Timer-based replay/live promotion (keep current) | No server change needed | The sentinel already exists; the client should trust it. Document the protocol, don't work around it |
| Epoch-based orphan recovery | More robust than status-scan | Current status-scan works because sessions are per-daemon-instance. Epochs add complexity for zero benefit when there's one lfd per repo |
| Kill reader task immediately on stop() | Simpler code | Stale events leak through — the whole point of this fix |
| Explicit session→wave callback via trait | More extensible | Over-engineering; scheduler register/unregister + loop ticker check is sufficient |

## Key decisions

**Unbounded mpsc for harness→bridge.** The harness produces a bounded number of events per turn (proportional to agent output). Memory growth is bounded by turn duration. The alternative — bounded channel with backpressure — risks killing the child process when the pipe buffer fills. Unbounded is the right choice here.

**Drain reader on stop, don't abort.** The 2-second timeout is a safety net. In practice, after `child.kill()` + `child.wait()`, stdout is immediately EOF'd and the reader exits in microseconds. The timeout catches pathological cases without blocking shutdown.

**Scheduler-based wave integration, not event-driven.** A callback-style `on_session_ended` could trigger wave advancement directly, but that couples sessions to wave execution. Instead: unregister from scheduler, let the loop ticker's natural polling pick up the change. Simpler, decoupled, testable. The latency cost (one tick interval) is acceptable — waves don't need sub-second advancement.

**No replay-complete protocol changes.** The `session.replay_completed` sentinel already exists and works. Concerto's timer-based promotion heuristic is a client bug, not a server bug. Document the sentinel in the API spec and fix the client to trust it.

## Scope

**In scope:**
- Harness→bridge channel change (mpsc)
- SSE lag backfill from store
- Claude stop() drain + in-flight tool cleanup
- Scheduler session registration/unregistration
- Session end → wave advancement (via scheduler unregister)
- End-during-starting guard
- Provider conformance tests (trace replay)

**Out of scope:**
- Server-side bounded buffering for long sessions (UI already handles this)
- Provider-layer unification for CLI reuse (future phase)
- DiffUpdated provider normalization (cosmetic, not reliability)
- Concerto client-side replay promotion fix (client-side, separate PR)
- Provider auth interruption (requires provider-specific error taxonomy — future)
- Concurrent client load testing (verify manually; no automated harness needed yet)

## Done when

- `cargo test -p loopflow` passes with new tests
- Trace-replay tests cover normal turn, crash-mid-tool, and multi-tool scenarios for both Claude and Codex
- Manual test: start session, kill lfd, restart lfd → orphaned session shows Failed with error event
- Manual test: start session, two Concerto clients connected → both see same events, no corruption
- Manual test: stop session during active turn → no stale TurnCompleted events, in-flight tools show Failed
- Manual test: session with `wave_run_id` ends → wave run resumes on next tick
- SSE handler recovers from broadcast lag by backfilling from store (unit test with artificial lag)

## Implementation order

1. **Harness→bridge mpsc** — fixes data loss, unblocks everything else
2. **Claude crash cleanup** — drain + in-flight tool completion
3. **SSE lag backfill** — client reliability
4. **Wave integration** — scheduler wiring
5. **End-during-starting guard** — edge case
6. **Provider conformance tests** — regression safety net
