# OpenCode: control-flow mechanics

All paths in `sst/opencode` @ `dev`. Recently rebuilt on the [Effect](https://effect.website)
runtime; core primitive is a per-session `Runner` state machine.

## The load-bearing primitive: `Runner`
`packages/opencode/src/effect/runner.ts` — a single-slot fiber supervisor:
```
State = Idle | Running{run} | Shell{shell} | ShellThenRun{shell, run}
```
- `ensureRunning(work)` — **the queue/coalesce primitive.** If `Idle`, forks
  `work` and returns a `Deferred`. If already `Running`, does **not** start a
  second fiber — returns `awaitDone(existing.run.done)`, so every caller attaches
  to the one in-flight run.
- `cancel` — for `Running`: `Fiber.interrupt(run.fiber)`, fail the `Deferred`
  with a `Cancelled` tagged error, flip to `Idle`.
- `onIdle`/`onBusy`/`onInterrupt` fire on transitions. `onInterrupt`: cancelled
  runs resolve to a **value** (the last assistant message), not an exception.

Per-session registry: `session/run-state.ts` — `Map<SessionID, Runner>`, plus
`assertNotBusy` (throws `BusyError`), `cancel` (also cancels matching background
jobs transitively via `parentSessionId`), `ensureRunning`.

## 1. Queueing
`session/prompt.ts` — **no separate server-side queue. The message log (SQLite)
IS the queue.**
- `prompt(input)` → `createUserMessage` (writes user turn to DB) → `loop`.
- `loop` = `state.ensureRunning(sessionID, ..., runLoop(sessionID))`.
- `runLoop` = `while(true)`: each iteration **re-reads all messages from the DB**,
  continues while there's an unanswered user turn (`lastUser.id > lastAssistant.id`),
  `break`s only when the latest assistant finished with no pending tools and no
  newer user message.

A message sent mid-turn: `createUserMessage` appends to DB; `ensureRunning` sees
`Running` and coalesces; the loop picks it up on its **next iteration** (after the
current step's LLM+tools finish). No interrupt, no loss. Append-to-log + re-read
beats an in-memory queue: the queue can't desync from the transcript. UX-level
queue (pin, send-now, cancel-queued) lives **client-side** in the TUI, not the
server. Open tension (#16102): drain the queue at top of `loop()` and inject
wrapped as `[USER_MID_TASK_MESSAGE]` rather than natural re-read (→ §3).

## 2. Interrupting / aborting
- `POST /session/:sessionID/abort` → handler is one line: `promptSvc.cancel(sessionID)`.
- `cancel` → `Runner.cancel` → `Fiber.interrupt`. LLM stream wrapped
  `Effect.onInterrupt(() => { aborted=true; controller.abort() })` (`processor.ts:648`),
  real `AbortController` to the AI SDK/fetch. Tools get their own AbortController.
- **Partial-turn finalization is explicit.** `finalizeInterruptedAssistant`: if
  the assistant message has no `completed` time, stamp an `AbortError`
  (`aborted:true`) into `msg.error`, set `time.completed`, persist. Orphaned tool
  calls → `state.status="error", metadata.interrupted=true`, later ignored by
  `isOrphanedInterruptedTool`. Aborted turn = a **well-formed, closed message** in
  the log; transcript stays replayable. Session → `Idle`, emits idle event.

## 3. Steering
- **Shipped (implicit):** because the loop re-reads the transcript every step, a
  user message landing during a run *is* mid-run redirection — the model sees
  "actually do X" as the next user turn before the next LLM call. Granularity =
  one step.
- **Proposed (explicit, #16102):** drain queued messages at top of `loop()` and
  splice into the next model call wrapped `[USER_MID_TASK_MESSAGE]…` — guidance
  *as context* mid-todolist, not a fresh user turn. Copy this for "nudge without
  ending the turn."
- No true token-stream interruption; steering resolution = step boundary.

## 4. Session model & client sync
- Session state = ordered messages, each with typed `parts` (text/reasoning/tool/
  agent), streamed. `session/message-v2.ts`.
- **Single global SSE stream:** `GET /event` (`text/event-stream`) — one
  connection multiplexes all sessions; clients filter by `sessionID`.
- Events: `message.updated`, `message.part.updated`, `message.part.delta`
  (token-level), `message.removed`; `session.status {busy|idle}` and a dedicated
  `session.idle` (the "turn finished" signal clients gate the composer on);
  `session.error`, `session.updated`, diffs.
- **Event-sourced projection:** DB is truth; SSE carries deltas; reconnect →
  `GET /session/:id/message` (paginated) to rebuild, then resume. Deltas are
  additive patches, not whole-message replacements.
- Two send modes: `POST /:id/message` (blocks until loop completes — synchronous
  request/response over the whole turn) vs `POST /:id/prompt_async` (forks the
  loop, returns immediately; watch SSE).

## What to adopt for WaveRuntime
1. **Make the thread the queue.** `POST /messages` = append-to-log; the progress
   loop re-reads at each step boundary. The "inbox" = a message appended to the
   thread. Can't desync from `GET /conversation`. Real in-memory queue only for
   UX affordances (client-side).
2. **`Runner`/`ensureRunning` coalescing** for the progress agent — one
   supervised task; a new message attaches to the live run, never spawns a dup.
   Explicit state enum, not `bool running`.
3. **Cancellation returns a value + finalizes the partial.** Copy
   `finalizeInterruptedAssistant`: close the in-flight turn with an `aborted`
   marker + completion time, mark orphaned tool calls interrupted, persist, idle.
   Real CancellationToken threaded into LLM call *and* each subagent, cascading.
4. **Steer at step boundaries via context injection** (#16102 pattern): drain new
   inbox messages at top of each iteration, inject tagged `[STEER]…`. Decide up
   front: mid-run message = "new turn" (implicit re-read) vs "guidance for current
   objective" (tagged injection). Tagging is cleaner for a goal-directed loop.
5. **One SSE stream, delta-based, explicit `idle` event.** `part.delta` for
   tokens, `part.updated`/`message.updated` for structure, distinct `wave.idle`.
   Reconnect = refetch `/conversation` then resume; deltas additive.
6. **Offer sync and async submit** (`prompt` vs `prompt_async`).

**Do NOT copy** the `assertNotBusy`/`BusyError` reject-while-busy path — it's the
source of their queue-loss bugs (#5333). Prefer append-and-coalesce everywhere.

### Sources
`packages/opencode/src/effect/runner.ts`; `session/run-state.ts`; `session/prompt.ts`
(`runLoop`, `finalizeInterruptedAssistant`); `session/processor.ts`; `session/status.ts`;
`session/message-v2.ts`; `server/routes/instance/httpapi/groups/{session,event}.ts`
+ `handlers/session.ts`. Issues #16102, #13304, #12707, #5333.
