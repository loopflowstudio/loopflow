# Event Emission Wiring

Wired `EventHub` into `WaveExecutor` and all three trigger pollers (loop, watch, cron) so that every wave/agent state transition emits a real-time event to WebSocket subscribers.

## Architecture

```
HTTP handlers (existing) ──emit──▶ EventHub ──broadcast──▶ WebSocket clients
WaveExecutor (new)       ──emit──┘
Trigger pollers (new)    ──emit──┘
```

EventHub is a thin `broadcast::Sender<Event>` wrapper. Clone is cheap (Arc). Fork tasks clone it alongside store, runner, output, scheduler.

## Emission points

| Event | Where | Source |
|-------|-------|--------|
| `WaveStarted` | `run_wave_handler`, `continue_wave_handler` | HTTP handlers (existing) |
| `WaveStarted` | `spawn_run_task_with_slot` | Triggers (new) |
| `WaveStopped` | `stop_wave_handler` | HTTP handler (existing) |
| `WaveWaiting` | `executor::execute` (WaitInteractive branch) | Executor (new) |
| `AgentStarted` | `executor::run_step`, `executor::run_fork` | Executor (new) |
| `AgentEnded` | `executor::run_step`, `executor::run_fork` | Executor (new) |
| `WaveUpdated` | `executor::execute` (Complete), `executor::fail_run` | Executor (new) |
| `WaveUpdated` | `execute_run_inner` (unexpected error) | Triggers (new) |
| `WorktreeUpdated` | `hooks/git` handler | HTTP handler (existing) |

## Key decisions

**Emit at semantic boundaries, not storage boundaries.** Events fire when the executor makes a state decision (wait, complete, fail), not on every `update_wave_run` call. Keeps events meaningful, avoids noise.

**Single emission point for trigger-originated starts.** All triggers route through `spawn_run_task_with_slot`, which emits `WaveStarted`. No per-trigger duplication.

**`AgentStatus` passed directly to `agent_ended`.** Constructor accepts the enum and calls `as_str()` internally. Type-safe at call sites, string in the JSON wire format.

**Fire-and-forget preserved.** EventHub drops events when no one is listening. Events are for live clients; the store is the source of truth.

## Alternatives considered

| Approach | Why not |
|----------|---------|
| Store-level emission (emit on every `update_wave`) | Over-emits, couples store to event system, store trait becomes async |
| Event sourcing (events as source of truth) | Massive architecture change for minimal gain |
| Polling-only (remove WebSocket) | Defeats real-time UI; Concerto already depends on WebSocket |

## Risks

- **Broadcast channel capacity** (1024): Client falls behind → `Lagged` error. Acceptable — clients reconnect and get a fresh snapshot from the store.
- **No event for executor panics**: A tokio task panic would be silent. The recovery loop handles stuck agents operationally.

## Not included

- No new event types (existing enum covers all transitions)
- No event persistence or filtering (events are ephemeral)
- No WebSocket protocol changes (existing subscribers receive new events automatically)
- No executor-level tests for event emission (unit tests cover serialization and EventHub delivery)
