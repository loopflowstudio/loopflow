# Codex: queue / interrupt / steer — mechanisms + adoption

Researched against `openai/codex` `main`, `codex-rs/` crates. Two protocol
surfaces: the legacy **SQ/EQ** (`protocol` crate + `core`) and the newer
**app-server v2 RPC** (`app-server-protocol/src/protocol/v2/`). Both sit on the
same `core` session engine. `exec --json` emits the `exec_events` shape.

## 1. Submission / op queue (SQ/EQ)

Explicit **Submission Queue (SQ) → Event Queue (EQ)** duplex, correlated by `id`.

- `codex-rs/protocol/src/protocol.rs`:
  - `Submission { id, op: Op, client_user_message_id, trace }` — SQ entry.
  - `Op` variants: `UserInput { items, final_output_json_schema, thread_settings }`,
    `Interrupt` ("Abort current task without terminating background terminal
    processes… sends `EventMsg::TurnAborted`"), `CleanBackgroundTerminals`,
    `Shutdown`.
  - `Event { id, msg: EventMsg }` — EQ entry; `id` echoes the submission.
  - `EventMsg` (serde `tag="type"`, snake_case): `TurnStarted`, `TurnComplete`,
    `AgentMessage`, `UserMessage`, `ExecCommandBegin`, `ExecCommandOutputDelta`,
    `TurnAborted`, `ShutdownComplete`, …

Single consumer loop over the SQ dispatches each `Op`. State in
`core/src/state/session.rs` + `core/src/session/session.rs`. Input arriving while
busy does **not** block: `UserInput` is routed to the running turn's queue (§3),
not a competing task.

## 2. Interrupting

`Op::Interrupt` (or v2 `turn/interrupt { thread_id, turn_id }`) drives abort via
`core/src/tasks/mod.rs` + `tasks/lifecycle.rs`. Running turn stored as a
`RunningTask` inside `ActiveTurn`:
```
RunningTask { done, handle: AbortOnDropHandle::new(handle), kind, task,
              cancellation_token, turn_context, ... }
```
Abort sequence (reason `TurnAbortReason::Interrupted`):
1. `cancellation_token.cancel()` — cooperative signal (each step gets a `child_token()`).
2. Wait up to **~100ms** on `task.done.notified()` for graceful unwind.
3. `task.handle.abort()` — forceful Tokio abort if not finished.
4. `session_task.abort()` — task-specific teardown.
5. Write an **interrupted-turn history marker** so the transcript closes out
   consistently, then emit `EventMsg::TurnAborted`.

Design point: interrupt is **task-scoped, not process-scoped** — background
terminals survive (`CleanBackgroundTerminals` is a separate explicit kill).
`AbortOnDropHandle` guarantees the tokio task dies. Cancellation is
cooperative-first (token) with a hard-abort fallback.

## 3. Steering (inject mid-turn)

Codex supports steering **without interrupting** via v2 `turn/steer`:
```
TurnSteerParams { thread_id, input, expected_turn_id, client_user_message_id,
                  additional_context, ... }
TurnSteerResponse { turn_id }
```
`expected_turn_id` is an optimistic-concurrency guard: you steer *the turn you
think is running*; if it already completed, the steer is rejected/redirected.

Mechanism (`core/src/session/inject.rs`, `input_queue.rs`):
- `inject_if_running(input) -> Result<(), Vec<ResponseItem>>` grabs the
  `active_turn` lock; if a turn exists, appends items as
  `TurnInput::ResponseItem` to that turn's `pending_input`; else returns the
  input back as `Err` (caller starts a fresh turn).
- Two queues: turn-local `TurnInputQueue.pending_input.items` (steered text) and
  a session-scoped mailbox `VecDeque<InterAgentCommunication>` (multi-agent).

**Timing (important):** steered input is applied at the **next step boundary, not
mid-stream**. Task loop (`core/src/tasks/regular.rs`):
```
loop {
    let last = run_turn(sess, ctx, ..., next_input, ..., cancel.child_token()).await?;
    if !sess.input_queue.has_pending_input(&sess.active_turn).await {
        return Ok(last);   // only now → TurnComplete
    }
    next_input = Vec::new();
}
```
A single user-visible **task keeps re-entering `run_turn` as long as steered
input keeps arriving**; `TurnComplete` fires once, when the queue drains. There
is **no true token-level mid-stream injection**; granularity = between model
requests / tool-call iterations.

TUI: you can always type while the agent works; the composer submits as a steer
if a turn is active, else as a new turn (`tui/src/chatwidget/input_queue.rs`,
`interrupts.rs`).

## 4. Streaming protocol shapes

`exec --json` (`exec/src/exec_events.rs`) — align ChatTurn/ConversationItem to:
```
ThreadEvent (tag="type"):
  thread.started {thread_id} | turn.started {} | turn.completed {usage}
  | turn.failed {error} | item.started/updated/completed (ThreadItem {id, details}) | error
ThreadItemDetails (tag="type", snake_case):
  agent_message | reasoning | command_execution | file_change |
  mcp_tool_call | collab_tool_call | web_search | todo_list | error
```
Item lifecycle `started → (updated…) → completed`, each carrying a **stable item
id** — consumer keeps a map keyed by id, mutates in place. `turn.completed`
carries token `usage`. App-server v2 adds `TurnDiffUpdated`, `TurnPlanUpdated`.
Input side: `exec` is one-shot; interactive input is the app-server RPC
(`turn/start`, `turn/steer`, `turn/interrupt`, `thread/injectItems`).

## What to adopt for WaveRuntime

- **Mirror SQ/EQ.** `POST /messages` inbox = the SQ; give every inbound message a
  typed op (`UserInput | Interrupt | Steer`), correlate by id. SSE/`GET
  /conversation` = the EQ. Adopt **`item.started/updated/completed` + stable item
  id** verbatim (already what our `codex exec --json` subagent emits). Add
  `turn.started`/`turn.completed{usage}` bookends.
- **Steer-as-queue, not steer-as-restart.** Busy turn appends to a `pending_input`
  queue; task loop re-enters and consumes; one `turn.completed` when empty.
  "Type while it works" for free. Our boundary is between subagent *invocations*
  (codex exec is one-shot).
- **`expected_turn_id` optimistic concurrency** on steer — cheap, prevents the
  land-just-as-it-finished race.
- **Cooperative-cancel-then-hard-abort interrupt, task-scoped.** CancellationToken
  (child per step) into the supervisor; ~100ms grace then kill. **Write an
  interrupt marker + emit `turn.aborted` before idle** — never a half-open item.
  Keep "abort turn" and "tear down wave" distinct ops.
- **Skip** the mailbox/multi-agent queue unless waves talk to each other.
- **Caveat / fork:** our subagent is `codex exec` (one-shot per invocation) →
  coarser steer than Codex's in-process loop (only between invocations, not
  between tool calls). If finer steering matters → drive `codex app-server`
  (`turn/steer`) against a long-lived thread instead of re-spawning `exec`.

### Sources
`codex-rs/protocol/src/protocol.rs`; `core/src/session/input_queue.rs`,
`inject.rs`; `core/src/tasks/{mod,lifecycle,regular}.rs`;
`core/src/context/turn_aborted.rs`; `app-server-protocol/src/protocol/v2/turn.rs`;
`app-server/tests/suite/v2/{turn_steer,turn_interrupt,thread_inject_items}.rs`;
`exec/src/exec_events.rs`; `tui/src/chatwidget/{input_queue,input_submission,interrupts}.rs`.
