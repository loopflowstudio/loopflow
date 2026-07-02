---
requires: existing code
produces: code changes
---
Let a wave dispatch a flow-against-a-task as its own attachable lfd session, on demand.

Design context: `scratch/waveagent-sessions.md` ("The one real primitive:
dispatch-through-lfd" and "The object model"). This is the **keystone** unit, scoped
to the minimal real primitive. Do NOT build the supervisor, heartbeat, fan-out branch
isolation, or the object-model rename here.

## Goal

Today a wave run is created only by the ticker/triggers and always renders the wave's
GOAL prompt (`build_wave_run_command`, `rust/loopflow/src/lfd/executor/wave/mod.rs`).
Add the ability to dispatch a **specific flow against a specific task** as its own
tmux-backed `TerminalSession`, initiated on demand via the HTTP API. This is the
mechanism by which a wave's looping agent will later spawn steerable subagents — each
its own attachable session — instead of shelling out inside its own pane.

Reuse the existing run + session + launch machinery. This unit adds a *new reason* to
create a run (a flow+task dispatch) and a *new command shape* for it (run the flow
against the task, not the goal). It does not reinvent worktree/branch/session creation.

## Workflow

1. **Distinguish a dispatch run from a goal run.**
   - `WaveRunSnapshot` (`rust/loopflow/src/lfd/types/wave.rs`) currently carries
     `flow` (+ repo/direction/area). Add an optional `task: Option<String>` field to
     the snapshot (this is internal wire/store state, not a no-defaults DTO — but
     follow the existing serde pattern of the neighbouring fields; if the snapshot
     round-trips through the store, add the column/serialization consistently with how
     `flow` is handled). When `task` is `Some`, the run is a flow-dispatch; when
     `None`, it is a goal iteration (existing behavior).

2. **Build the dispatch command.**
   - In `build_wave_run_command` (or a sibling), branch on `run.snapshot.task`:
     - `Some(task)` → build `lf <flow>: <task>` via the existing
       `build_lf_step_command(flow, ...)` helper (`executor/helpers.rs`), passing the
       task as the inline message. Return a terminal-step label like
       `dispatch:<flow>`.
     - `None` → existing goal rendering, unchanged.
   - Keep the goal path byte-for-byte the same; only add the `Some(task)` branch.

3. **HTTP endpoint** `POST /v0/waves/{name_or_id}/dispatch`:
   - Request body: `{ "flow": String, "task": String }` (both required).
   - Handler resolves the wave, creates a `WaveRun` whose snapshot has
     `flow = <flow>`, `task = Some(<task>)`, and enqueues/launches it exactly like an
     ordinary run so the executor creates its worktree + tmux `TerminalSession`. The
     resulting session must be listable via `GET /v0/terminal-sessions?wave_id=<id>`
     and attachable (so `lfq sessions` / `lfq attach` see it).
   - Register the route in `rust/loopflow/src/lfd/http/mod.rs` next to the other
     `/v0/waves/{...}` routes.
   - Return the created run (or its id + the terminal session id) as JSON.

4. **Tests.**
   - Unit: `build_wave_run_command` with a `task: Some(...)` snapshot produces an
     `lf <flow>: <task>` command (and the `None` case still renders the goal).
   - Route/integration: `POST /v0/waves/{id}/dispatch` creates a run whose snapshot
     carries the task and yields a terminal session tied to the wave. Follow the
     existing wave-route test patterns in `routes/waves.rs` (use the in-memory store /
     executor fakes already used there).

## What matters

- Reuse existing run/worktree/session/launch code. The only genuinely new pieces are:
  the `task` field on the snapshot, the `Some(task)` command branch, and the endpoint.
- The goal-iteration path stays exactly as-is.
- The dispatched session is a normal `TerminalSession` — attachable, listable by wave.

## Guardrails

- Do NOT touch: `parse_trigger`, `resolve_wave_source_wave_id`,
  `resolve_wave_id_in_repo`, or any trigger/cron code — leave them exactly as they
  are. Do NOT delete or "trim" any existing functions. Add only.
- Do NOT rename `WaveRun`, add a `Dispatch` type, add a supervisor, or touch fan-out
  branch logic.
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test -p loopflow` must pass. If
  the snapshot change touches DTO fixtures or the store schema, update them
  consistently and run the affected tests.
- Keep the diff tight and additive.

## Output

`POST /v0/waves/{id}/dispatch {flow, task}` launches an attachable session running the
flow against the task, with tests. The wave loop can now dispatch work as its own
session instead of running it inline.
