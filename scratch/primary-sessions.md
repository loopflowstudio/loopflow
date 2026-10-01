# Primary Sessions (LOO-364)

Status: draft, restated 2026-10-01 from Unit 2 of
`loopflow.growth-thoughts/scratch/growth-thoughts.md`. That plan was written
against Runs and a `PrimarySession { scope, run_id, replacement_run_id }`
pointer record. Jack Heart moved the unit out of LOO-353 on 2026-10-01; Desktop
presentation stays there. LOO-332's cron floor landed as #1382.

## Outcome

Jack talks to one ongoing conversation per repository and per Wave, and one per
launched Task. He does not track Sessions, remind agents to continue, or chase
whether Tasks landed: the conversations find themselves, pick up operational
input automatically, and call the same reconciliation the minute schedule runs.

## The model, on Exec / AgentSession / FlowSession

| Unit 2 said | Landed owner |
| --- | --- |
| Primary Run + pointer record | An `AgentSession` whose `primary_scope` names what it is primary *for*. No sidecar table. |
| `run_id` / `replacement_run_id` | The Session id. Replacement is one transaction: complete the predecessor, admit the successor. No intermediate pointer. |
| Prepared Run, owner token | The Session's prepared captured input (`input_published`), launched by the existing durable-terminal path with a `Primary` token. |
| Run records as history/provider authority | `session_events`; provider identity and usage stay on the AgentSession. |
| Task Flow cursor (`FlowPosition`) | The Task's managed `FlowSession`. It is the only cursor. |
| Worker Runs, checkpoint, stop | `Exec` rows and the FlowSession claim. |
| Flow/independent Session membership | LOO-358's checkout association; no primary-specific membership. |

**A primary Session is an ordinary interactive `conversation` AgentSession.**
Primary is a nullable attribute of the row (`primary_scope`), with the scope's
identity in the columns the row already has (`wave_id`, `repo`, `task_id`). The
current primary of a scope is its one uncompleted row; a partial unique index
states that. Completed predecessors remain ordinary history. Membership,
attribution, title, rename, connect and complete are the existing Session
operations and grant no Flow or process authority.

- **Wave Session** — `primary_scope='wave'`, `wave_id`. Launched with the Wave
  selector, so goal and memory arrive through the Session's ordinary Wave
  context (Jack, 2026-09-30); there is no separate memory channel.
- **Repository Session** — `primary_scope='repository'`, `repo`. Works with zero
  Waves, zero Tasks and unavailable PM.
- **TaskSession** — `primary_scope='task'`, `task_id`, in the Task's checkout.
  Ensured by Task launch, never by filing or viewing. It is a member of the
  Task through the checkout association like any other Session there.

### TaskSession and the Flow

The TaskSession operates the existing Flow runtime through `lf task` commands.
It holds no claim and no cursor. A switch is data on the managed FlowSession:

- **Switch now** — record the captured successor, stop the exact driver Exec,
  confirm it stopped, checkpoint locally, then atomically mark the FlowSession
  `Replaced` and install the successor. Extends `task restart`.
- **Finish, then switch** — record the captured successor and the exact loop
  occurrence (innermost repeat interval containing the cursor, by structural
  occurrence within the current activation). Claim, reclaim and review
  preparation observe the pending switch in the same store transaction and
  consume it instead of launching the decider. Crossing intervals, no active
  loop, and an already-claimed decider are reported, never guessed.

The pending switch lives beside the cursor, not in it, so a worker settling an
older in-memory position cannot erase it. Target definition and account
selection are captured at acceptance.

### Input delivery and wakes

The observation outbox survives without a consumer since #1360. The primary
Session of the recipient scope becomes its consumer:

- Claim pending observations for the scope under the Session's driver fence,
  deliver them as one structured turn, and record the receiving Session event.
  `delivered_at` means "entered a turn", never "recovered".
- Delivery serializes with Jack's typing through the Session's one driver
  (the Codex live-connection path already crosses that fence). No terminal
  keystroke injection, no second Harness on the same history, no resident.
- Wake producer: the minute schedule. When a scope has pending observations
  and its primary Session is idle, the scheduled check admits one delivery
  turn. The TaskSession is the fast path for its own Task; the Wave Session
  handles broader recovery. Both, and the cron, call `lf task reconcile`.

Native turn delivery into a live interactive client is unproved for Claude and
OpenCode. That spike gates the delivery slice, not the slices before it.

## Delivery

One Task, coherent slices. Each stands on its own.

1. **Wave primary Session runtime** — this slice, below.
2. Repository primary Session: `ensure --repo`, `repo/session` skill.
3. TaskSession: ensured by `task run`, `task/session` skill, in the Task
   checkout.
4. Flow switching on the FlowSession (both timings), via `task restart`.
5. Outbox delivery and scheduled wakes, after the native turn-delivery spike.
   Repository-scope attention requests extend the outbox recipient here.
6. Scope checkout for design writing (`ensure_agent_worktree` with a local
   base) so primary conversations stop sharing the main checkout.

### Slice 1 — Wave primary Session (built)

- Migration draft `primary_session_scope`: `agent_sessions.primary_scope` and a
  partial unique index on the current Wave primary.
- Store: one `ensure_primary_session(scope, replacing, session)` transaction.
  It returns the scope's current primary, or admits the given Session; when the
  current primary is the one being replaced it completes it and admits the
  successor together. Concurrent callers converge on one row.
- `lf session ensure --wave NAME [--json]` — find or admit the Wave's primary,
  publish its prepared input, and start its durable terminal once. Repeats
  return the same Session and never start a second launcher. A failed start
  keeps the Session; the next ensure retries it. The command is read-only for
  every other Session.
- `lf session replace ID [--json]` — stop the predecessor's provider client,
  then complete it and admit a fresh primary in one transaction. A repeat with
  the same ID returns the successor already admitted. If the provider cannot be
  stopped, nothing changes.
- `lf session connect ID` opens a primary whose first launch has no provider
  history yet through the prepared-input path instead of refusing.
- Builtin `wave/session` skill: reconcile first (`lf task reconcile`, then
  status), act within existing Task controls, leave interactive work ready,
  capture ideas in scratch before creating Tasks. It never becomes a second
  driver of a Task's Flow.

Not in this slice: automatic wakes, Ctrl-C interception, Desktop discovery
calling ensure (LOO-353), a dedicated checkout (slice 6), and exposing
`primary_scope` on the Session DTO (added with its first Desktop consumer).

### Delete — do not maintain

Nothing is removed in slice 1. The delivery slice deletes the consumerless
`pending_observations` / `mark_observation_delivered` callers' absence by
giving them their one caller; it does not restore the listener.

## Checks

`cargo test -p loopflow --lib primary::tests` — 4 passed (2026-10-01). Gate owns
affected suites; a real provider launch belongs to demo.
