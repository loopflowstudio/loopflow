# Event Emission + CollapsePRs Endpoint

## Problem

WebSocket clients (Concerto, web UI) get partial visibility into wave execution. The EventHub and Event types exist. Constructor methods exist. WebSocket delivery works. But events are only emitted from HTTP route handlers -- the executor and triggers are silent. Three event types are defined but never sent: `WaveWaiting`, `AgentStarted`, `AgentEnded`.

The result: a connected client sees wave creation, deletion, and HTTP-initiated starts/stops. It doesn't see the executor pause for interactive input, agents spawning, agents completing, or trigger-initiated wave starts. The UI can poll, but that defeats the purpose of the WebSocket.

CollapsePRs already has an endpoint (`POST /v0/waves/:id/collapse`). That item from the roadmap is done.

## Approach

Thread the `EventHub` into `WaveExecutor` and the trigger infrastructure. Emit events at every state transition point where the data model changes.

**No new event types.** The existing `Event` enum covers every transition. The work is purely wiring.

### Emission points

| Event | Where | Current | Change |
|-------|-------|---------|--------|
| `WaveStarted` | `run_wave_handler` | Emitted | Keep |
| `WaveStarted` | `continue_wave_handler` | Emitted | Keep |
| `WaveStarted` | `loop_ticker` / trigger spawn | **Missing** | Add via `spawn_run_task_with_slot` |
| `WaveStopped` | `stop_wave_handler` | Emitted | Keep |
| `WaveWaiting` | `executor::execute` (WaitInteractive branch) | **Missing** | Add |
| `WaveWaiting` | (implicit: run completes as Failed from executor) | N/A | Covered by polling |
| `AgentStarted` | `executor::run_step` (after `start_agent`) | **Missing** | Add |
| `AgentStarted` | `executor::run_fork` (after `start_agent` in branch task) | **Missing** | Add |
| `AgentEnded` | `executor::run_step` (after `end_agent`) | **Missing** | Add |
| `AgentEnded` | `executor::run_fork` (after `end_agent` in branch task) | **Missing** | Add |
| `WaveUpdated` | `executor::execute` (Complete branch, sets Idle) | **Missing** | Add |
| `WaveUpdated` | `executor::fail_run` (sets Failed) | **Missing** | Add |
| `WorktreeUpdated` | `hooks/git` handler | Emitted | Keep |

### Constructor methods to add

`Event` already has constructors for `wave_created`, `wave_updated`, `wave_deleted`, `wave_started`, `wave_stopped`, `worktree_updated`. Missing:

- `Event::wave_waiting(wave_id, wave_run_id, step)`
- `Event::agent_started(agent_id, step, worktree)`
- `Event::agent_ended(agent_id, status)`

### Wiring

1. **Add `EventHub` to `WaveExecutor`** -- new field, passed at construction. The executor already holds `store`, `scheduler`, `output`. Adding `event_hub` follows the same pattern.

2. **Emit in executor methods:**
   - `run_step`: emit `AgentStarted` after `store.start_agent()`, emit `AgentEnded` after `store.end_agent()`
   - `execute` WaitInteractive branch: emit `WaveWaiting` after setting status
   - `execute` Complete branch: emit `WaveUpdated` (wave went Idle)
   - `fail_run`: emit `WaveUpdated` (wave went Failed)

3. **Emit in fork tasks** (inside `run_fork`): same pattern as `run_step` -- `AgentStarted` and `AgentEnded`. The fork tasks run on spawned tokio tasks, but they already hold clones of other shared state. Clone `event_hub` into each fork task.

4. **Add `EventHub` to trigger infrastructure:**
   - `spawn_run_task_with_slot` takes `EventHub` and emits `WaveStarted` when spawning.
   - Alternatively, emit from `loop_ticker` / `watch_poller` / `cron_poller` at the call site. Simpler: emit in `spawn_run_task_with_slot` since all triggers go through it.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Store-level event emission (emit on every `update_wave`) | Automatic, no emission points to maintain | Over-emits (every field change triggers event), couples store to event system, store trait becomes async |
| Event sourcing (store events as source of truth) | Full audit trail, replay | Massive architecture change for minimal gain at this stage |
| Polling-only (remove WebSocket) | Simpler | Defeats real-time UI; Concerto already depends on WebSocket |

## Key decisions

**Emit at the semantic level, not the storage level.** `WaveWaiting` means "wave entered waiting state" -- it's emitted once at the point where the executor decides to wait. Not every time someone calls `update_wave_run`. This keeps events meaningful and avoids noise.

**Fire-and-forget stays.** EventHub drops events when no one is listening. This is correct -- events are for live clients, not audit trails. The store is the source of truth. WebSocket reconnect gets a fresh snapshot.

**Fork tasks clone EventHub.** `EventHub` is a thin `broadcast::Sender` wrapper. Clone is cheap (Arc increment). Fork tasks already clone `store`, `runner`, `output`, `scheduler`.

**Triggers emit through `spawn_run_task_with_slot`.** Single emission point for all trigger-originated runs. The function already exists as the shared path.

## Scope

- In scope: Wire `EventHub` into executor and triggers, emit all defined event types, add missing constructor methods
- Out of scope: New event types, event persistence, event filtering, WebSocket protocol changes

## Done when

```bash
# All existing tests pass
cargo test --all

# New test: executor emits events at each state transition
cargo test -p loopflow executor_emits

# Manual: connect WebSocket, run a wave, see WaveStarted/AgentStarted/AgentEnded/WaveUpdated events
# Manual: connect WebSocket, run interactive step, see WaveWaiting event
# Manual: enable loop stimulus, see trigger-originated WaveStarted events
```
