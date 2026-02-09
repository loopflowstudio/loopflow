# Event Emission Wiring — Review

## What was implemented

Wired `EventHub` into `WaveExecutor` and all three trigger pollers (loop, watch, cron) so that every wave/agent state transition emits a real-time event to WebSocket subscribers.

### Changes

- Added `event_hub: EventHub` field to `WaveExecutor`, passed at construction
- Added three `Event` constructors: `wave_waiting`, `agent_started`, `agent_ended`
- Added `AgentStatus::as_str()` for clean serialization in `agent_ended`
- Emit `AgentStarted`/`AgentEnded` in both `run_step` and `run_fork` (fork branch tasks)
- Emit `WaveWaiting` when executor enters interactive wait
- Emit `WaveUpdated` on run completion (Idle) and failure (Failed)
- Emit `WaveStarted` in `spawn_run_task_with_slot` — single emission point for all trigger-originated runs
- Threaded `EventHub` through `Scheduler::start_loops`, `spawn_loop_ticker`, `spawn_watch_poller`, `spawn_cron_poller`
- Deleted roadmap item `02c-grpc-events.md` (ingested to scratch)

### Files changed

| File | Change |
|------|--------|
| `types/event.rs` | 3 constructors + tests |
| `types/agent.rs` | `as_str()` method |
| `executor.rs` | EventHub field, emit at 6 points |
| `triggers/common.rs` | EventHub param, WaveStarted + WaveUpdated emission |
| `triggers/loop_ticker.rs` | Thread EventHub |
| `triggers/watch.rs` | Thread EventHub |
| `triggers/cron.rs` | Thread EventHub |
| `scheduler.rs` | Thread EventHub through `start_loops` |
| `bin/lfd.rs` | Pass EventHub to executor and loops |
| `events.rs` | Integration test |

## Key choices

**Emit at semantic boundaries, not storage boundaries.** Events fire when the executor makes a state decision (wait, complete, fail), not on every `update_wave_run` call. This keeps events meaningful and avoids noise.

**Single emission point for trigger-originated starts.** All triggers route through `spawn_run_task_with_slot`, which emits `WaveStarted`. No per-trigger duplication.

**`AgentStatus` passed directly to `agent_ended`.** Instead of a string, the constructor accepts the enum and calls `as_str()` internally. Type-safe at call sites, string in the JSON wire format.

**Fire-and-forget preserved.** EventHub drops events when no one is listening. Events are for live clients; the store is the source of truth.

## How it fits together

```
HTTP handlers (existing) ──emit──▶ EventHub ──broadcast──▶ WebSocket clients
WaveExecutor (new)       ──emit──┘
Trigger pollers (new)    ──emit──┘
```

The EventHub is a thin `broadcast::Sender<Event>` wrapper. Clone is cheap (Arc). Fork tasks clone it alongside store, runner, output, scheduler.

## Risks and bottlenecks

- **Broadcast channel capacity** (1024): If a client falls behind by 1024 events, it'll get a `Lagged` error. This is acceptable — clients reconnect and get a fresh snapshot from the store.
- **No event for executor panics**: If `executor.execute()` returns `Err`, `execute_run_inner` now emits `WaveUpdated`. But a tokio task panic would be silent. The recovery loop handles stuck agents, so this is covered operationally.

## What's not included

- No new event types. The existing `Event` enum already covered all transitions.
- No event persistence or filtering. Events are ephemeral, for live clients only.
- No WebSocket protocol changes. Existing subscribers automatically receive the new events.
- No executor-level tests for event emission (would require mocking the full execution pipeline). The unit tests cover serialization and EventHub delivery.
