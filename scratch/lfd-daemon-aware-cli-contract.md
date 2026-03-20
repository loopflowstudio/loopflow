---
linear_id: b00f983a-47b2-4ea8-b357-e45e0d183aa3
---
# Daemon-Aware CLI Contract

## Problem

`lfd` can't observe what `lf` is doing. Today `lfd` works around this by bypassing `lf` entirely — it spawns agents directly and reimplements flow logic (step sequencing, xor routing, fork/synthesize) in the wave executor. This creates two problems:

1. **Flow logic diverges.** `lf` has xor, loop, and, fork-to-worktree. The wave executor has its own step loop. They will drift.
2. **CLI-started runs are invisible.** A human or agent running `lf implement` in a daemon-managed worktree produces zero telemetry. `lfd` has no idea it happened.

The fix: make `lf` the execution unit that `lfd` observes, rather than a tool `lfd` replaces.

## Approach

Add a lightweight reporting client to `lf` that fires lifecycle events to `lfd` over HTTP when daemon env vars are present. `lfd` pre-assigns identity for runs it starts; `lf` self-registers for runs started independently. Events are fire-and-forget — `lf` never blocks on delivery.

### Detection contract

`lf` checks three env vars at startup:

| Var | Set by | Purpose |
|-----|--------|---------|
| `LFD_URL` | `lfd` or user | Base URL of the daemon HTTP API (e.g. `http://127.0.0.1:2486`) |
| `LFD_TOKEN` | `lfd` or user | Bearer token for auth |
| `LFD_RUN_ID` | `lfd` only | Pre-assigned run ID for daemon-started runs |

If `LFD_URL` is absent, `lf` behaves exactly as it does today. No daemon path, no overhead. The detection is a single env var check at the top of `main()`.

If `LFD_URL` is present but `LFD_RUN_ID` is absent, this is a CLI-started run in a daemon-managed environment. `lf` calls `POST /v0/runs/register` to get an ID, then proceeds with events.

If both are present, `lf` uses the pre-assigned run ID directly. No registration handshake.

Optional context vars `lfd` can also set:

| Var | Purpose |
|-----|---------|
| `LFD_WAVE_ID` | Associate this run with a wave |
| `LFD_WAVE_RUN_ID` | Associate with a specific wave run |
| `LFD_SESSION_ID` | Associate with a session |

These are correlation IDs only. `lf` passes them through in events but doesn't interpret them.

### Contract version

The first env var check also reads `LFD_CONTRACT_VERSION` (default: `1`). The event schema below is version 1. If `lf` sees a version it doesn't understand, it logs a warning and disables reporting (safe fallback, not crash).

### Events

Six event types. All are POST requests to `LFD_URL/v0/runs/{run_id}/events` with JSON bodies. All carry a `timestamp` (RFC3339) and `type` field.

```
POST /v0/runs/{run_id}/events
Authorization: Bearer {LFD_TOKEN}
Content-Type: application/json

{ "type": "...", "timestamp": "...", ... }
```

| Type | When | Extra fields |
|------|------|-------------|
| `run.started` | `lf` begins executing | `command` (full argv), `step` or `flow` (resolved name), `worktree` (path), `wave_id`, `wave_run_id` (if set) |
| `step.started` | A step begins (within a flow, fired per step) | `step` (name), `index` (position in flow) |
| `step.completed` | A step finishes | `step`, `exit_code`, `index` |
| `run.waiting` | Interactive wait point — agent needs human input | `step`, `session_id` (if applicable) |
| `run.completed` | `lf` exits successfully | `exit_code` (0) |
| `run.failed` | `lf` exits with error | `exit_code`, `error` (message) |

For single-step runs (`lf implement`), only `run.started` and `run.completed`/`run.failed` fire — no `step.*` events.

For flow runs (`lf build`), `step.started`/`step.completed` fire for each step in the flow, bracketed by `run.started` and `run.completed`/`run.failed`.

### Delivery semantics

**Fire-and-forget.** `lf` sends each event via a non-blocking HTTP POST. If the request fails (connection refused, timeout, 5xx), `lf` logs a debug message and continues. No retry. No queue. No buffering.

This is the right tradeoff because:
- Events are observability, not correctness. `lfd` can reconcile from process exit codes if events are lost.
- `lf` must never block on `lfd`. A slow or crashed daemon must not degrade CLI performance.
- The failure mode is "less telemetry" not "broken execution."

Implementation: a background `tokio::spawn` task per event, with a 2-second timeout on the HTTP POST. The `lf` process doesn't join these tasks — if `lf` exits before an event delivers, the event is lost. That's fine.

### Registration endpoint

For CLI-started runs (no `LFD_RUN_ID`):

```
POST /v0/runs/register
Authorization: Bearer {LFD_TOKEN}
Content-Type: application/json

{
  "worktree": "/path/to/worktree",
  "command": ["lf", "implement"],
  "wave_id": null,
  "wave_run_id": null
}

Response 201:
{
  "run_id": "uuid-v4"
}
```

This is the only synchronous call. It must complete before `lf` starts executing so that all subsequent events carry the assigned ID. If it fails, `lf` disables reporting and proceeds without daemon awareness. Timeout: 1 second.

### `lfd` changes

**New HTTP routes:**
- `POST /v0/runs/register` — create a run record, return ID
- `POST /v0/runs/{run_id}/events` — ingest a lifecycle event

**WaveExecutor refactor (future):** Once this contract exists, `lfd` can spawn `lf <flow>` instead of spawning agents directly. The wave executor becomes a thin orchestrator: create worktree, set env vars, spawn `lf`, observe events. Flow logic (xor, loop, and) stays in `lf` where it belongs. This is a follow-on change, not part of this milestone.

**Event → internal EventHub bridging:** When `lfd` receives a `step.started` event via HTTP, it translates to the internal `Event::AgentStarted` (or a new `Event::StepStarted` variant). This lets the WebSocket stream and Concerto UI show step-level progress for CLI-started runs, not just daemon-started ones.

### `lf` implementation

New module: `engine/daemon.rs`.

```rust
pub struct DaemonClient {
    url: String,
    token: String,
    run_id: String,
    http: reqwest::Client,
}

impl DaemonClient {
    /// Returns None if LFD_URL is not set (standalone mode).
    pub fn from_env() -> Option<Self> { ... }

    /// Fire-and-forget event delivery.
    pub fn emit(&self, event: DaemonEvent) { ... }
}
```

The `DaemonClient` is created once in `main()` and threaded through to `run_target()` / flow execution. It's `Option<DaemonClient>` everywhere — `None` means standalone mode, no daemon reporting.

The existing `EngineEvent` enum (`engine/event.rs`) is replaced by `DaemonEvent` which carries the HTTP-serializable event payload. Or better: `EngineEvent` is extended with the fields needed and `DaemonClient::emit` serializes it to the HTTP format.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Unix socket instead of HTTP | Lower latency, no port allocation | `lfd` already has an HTTP server. Adding a socket means two transports to maintain. HTTP is ~1ms on localhost. Latency doesn't matter for fire-and-forget events. |
| Stdio side channel (fd 3) | Zero network overhead, works in containers | Only works when `lfd` owns the process. Doesn't work for CLI-started runs. Would need HTTP anyway as fallback, so we'd have two paths. |
| Shared log file + inotify | No daemon needed at all | Parsing is fragile (the exact problem we're solving). Race conditions on concurrent writes. No structured schema enforcement. |
| `lfd` continues spawning agents directly | No changes to `lf` needed | Flow logic stays duplicated. CLI-started runs stay invisible. This is the status quo and it's already showing cracks. |
| Full request-response protocol (tmux-style) | `lfd` can control `lf` execution | Over-coupling. `lf` should be autonomous. `lfd` observes, it doesn't command. The tmux study says: learn from control mode, don't copy it. |

## Key decisions

**HTTP over sockets.** The daemon already listens on HTTP. One transport, one auth model, one set of routes. If we need sockets later (high-frequency events, container networking), the event schema stays the same — only the transport changes.

**Fire-and-forget over reliable delivery.** Lost events are acceptable because `lfd` already tracks process lifecycle (PID, exit code). Events add granularity (step-level progress, resolved flow/step names), not correctness. Reliable delivery would require queuing, retry logic, and backpressure — complexity that doesn't pay for itself.

**Pre-assign for daemon runs, register for CLI runs.** One authority for identity (lfd), zero for conflicts. Daemon-started runs get IDs injected via env. CLI-started runs register once synchronously, then proceed with fire-and-forget events. The registration call is the only synchronous interaction.

**Six events, not twenty.** The minimum set that gives `lfd` step-level visibility without coupling to `lf` internals. `run.started`/`run.completed`/`run.failed` bracket the process. `step.started`/`step.completed` give progress within flows. `run.waiting` signals interactive handoff. No events for internal decisions (xor routing, fork strategy) — those are `lf`'s business.

**`EngineEvent` as the internal type.** Rather than creating a parallel event system, extend the existing `EngineEvent` with the fields needed for HTTP serialization. The daemon client reads `EngineEvent` and serializes to JSON. One event vocabulary, two delivery paths (daemon HTTP, standalone no-op).

## Scope

**In scope:**
- `LFD_URL` / `LFD_TOKEN` / `LFD_RUN_ID` detection in `lf`
- `DaemonClient` module with fire-and-forget HTTP event delivery
- `POST /v0/runs/register` endpoint in `lfd`
- `POST /v0/runs/{run_id}/events` endpoint in `lfd`
- Event → internal EventHub bridging in `lfd`
- `EngineEvent` extension with serializable fields
- Parity tests: `lf implement` with and without `LFD_URL`, same execution semantics
- Contract version header for forward compatibility

**Out of scope:**
- Refactoring WaveExecutor to spawn `lf` instead of agents (follow-on)
- Output streaming from `lf` to `lfd` (existing OutputHub handles this for daemon-spawned processes)
- Interactive session bridging (existing session API handles this)
- Remote/TLS transport
- Event persistence or replay in `lfd` (events flow through EventHub broadcast, not stored separately)

## Done when

- `lf` detects `LFD_URL` and emits lifecycle events over HTTP without changing execution behavior
- `lfd` receives and bridges events to its internal EventHub
- `lf implement` and `lf build` produce identical results with and without `LFD_URL` set
- CLI-started runs in daemon-managed worktrees register and report step progress
- Tests pin the event schema (JSON golden tests) and the env var detection contract
- `cargo test` includes a test that runs `lf` with mock `LFD_URL`, verifies events arrive, and confirms `lf` completes normally even if the mock server is slow or down
