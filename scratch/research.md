# Research: steerable task sessions

## System understanding

Loopflow currently has two execution loops with different capabilities.

The served wave path is a listener/resident pair. The listener owns the durable
thread, inbox, and lifecycle fold; the resident owns the provider harness. The
resident's loop can receive messages while a turn is running, call
`Harness::send_input`, interrupt the provider, queue unsupported live steering,
and publish structured turn deltas.

The generic `lf loop` path creates a placed worktree, then repeatedly spawns a
blocking `lf --yolo -b flow …` process. Detached loops run inside tmux, but the
outer driver captures output and does not consume terminal input. It reports
completion through run state and the bus, not through the wave's steerable
harness loop.

### Architecture

- `wave/server.rs`, `wave/resident.rs`, and `flowloop/wave.rs` implement the
  durable, steerable Wave conversation.
- `flowloop/driver.rs`, `flowloop/pass.rs`, and `flowloop/run.rs` implement the
  generic placed-loop process.
- `harness/{codex,claude,opencode}.rs` normalize provider sessions behind
  `start`, `send_input`, `interrupt`, `stop`, and provider-session persistence.
- `wave/bus.rs` gives waves a durable cursor over a short-lived sqlite bus.
  Work-line channels already form a family under the wave channel.
- `engine/worktrees.rs`, `lfd/executor`, queue operations, and `Run` stack
  fields still carry several overlapping placement and delivery models.

### Data flow

Today a detached task loop is launched through the wave server's `/loops`
door, enters a fresh worktree, and runs independent headless passes. Its tmux
session is deliberately read-only for inspection. There is no message path
from the Wave LM into the active harness because the task path never owns an
inbox-aware harness.

By contrast, a wave message is journaled, delivered through the resident SSE
subscription, and either injected into the live harness or retained for the
next turn. Codex supports true mid-turn injection. Loopflow's Claude and
OpenCode adapters currently report that they cannot, so the wave loop queues
the message honestly.

### Key abstractions

- Wave: durable conversation, memory, project selection, and steerable parent.
- Linear project/task: formal roadmap identity before execution; the incoming
  product PM API projects it into one atomic per-Wave SQLite `PmShowResult`
  snapshot used by CLI, agents, and Swift.
- Task session: missing first-class concept joining a Linear issue, immutable
  worktree, provider thread, controls, status, transcript, and PR lifecycle.
- Harness: already the correct provider-neutral control boundary.
- Tmux: process lifetime and human attachment, not provider control.

## External patterns

### Codex

Codex keeps a session-scoped `AgentControl` shared across the root and its
children. Its public shape is the useful pattern: address a child by persistent
thread id, send structured input, interrupt it, subscribe to status, and inject
completion into the parent. The control layer also reserves capacity before
spawning and can resume persisted child history. See
[`AgentControl`](https://github.com/openai/codex/blob/main/codex-rs/core/src/agent/control.rs),
[spawn/resume](https://github.com/openai/codex/blob/main/codex-rs/core/src/agent/control/spawn.rs),
and the [`send_input` handler](https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/handlers/multi_agents/send_input.rs).

Codex derives child runtime policy from the active parent turn, including cwd,
approval, permission profile, model, and provider. Loopflow should copy the
explicit snapshot principle but replace cwd inheritance with the task's
immutable worktree. See
[`apply_spawn_agent_runtime_overrides`](https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/handlers/multi_agents_common.rs).

### OpenCode

OpenCode's Task tool creates a child Session with `parentID`; passing `task_id`
continues that session. Background work uses one id for start, serialized
follow-up (`extend`), wait, promotion, and cancel, then injects a synthetic
result into the parent. See the
[`TaskTool`](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/tool/task.ts)
and [`BackgroundJob`](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/background-job.ts).

The useful behavior is follow-up serialization: a new instruction sent while
the child is busy becomes the next child turn rather than racing the current
one. The unsuitable part is durability: the experimental background registry
is process-local, while Loopflow task sessions must survive process death.

### Claude Code

Claude subagents have stable ids, separately persisted transcripts, structured
`SendMessage` steering, automatic resume on a new message, and optional
worktree isolation. Agent Teams add a lead, independent sessions, shared task
state, and a mailbox. Tmux/iTerm panes are one display mode; the same agent
communication exists in the default in-process mode. See
[subagents](https://code.claude.com/docs/en/sub-agents) and
[agent-team architecture](https://code.claude.com/docs/en/agent-teams).

The reusable distinction is that transcript identity and process lifetime are
separate. A stopped child can resume under the same id and history. The caveat
is that the orchestration implementation is documented behavior rather than an
equally inspectable open-source control layer.

## Wave-to-Task steering benchmark (2026-07-10)

This benchmark is deliberately narrower than “multi-agent features.” Loopflow
does not need peer-to-peer Task chat, nested teams, or arbitrary agent roles to
match products whose workers are less durable delivery units. The comparison
asks one question: **can the parent Wave direct, observe, correct, stop, resume,
and make decisions for a named Task at least as reliably as a parent agent can
control its child?**

Primary references:

- Codex documents parent orchestration, follow-up routing, waiting, steering,
  stopping, and thread inspection in its
  [Subagents guide](https://learn.chatgpt.com/docs/agent-configuration/subagents).
  Its open-source `send_input` accepts an `interrupt` flag, returns a
  `submission_id`, reloads a known stopped thread, and emits a structured
  collaboration item containing the target and resulting agent status:
  [`send_input.rs`](https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/handlers/multi_agents/send_input.rs).
  `AgentControl` also exposes status subscriptions, interrupt, and automatic
  completion injection into the parent:
  [`control.rs`](https://github.com/openai/codex/blob/main/codex-rs/core/src/agent/control.rs).
- Claude Code agent teams have a lead, named teammates, direct `SendMessage`,
  automatic delivery, idle notifications, a shared dependency-aware task
  list, plan approval/rejection with feedback, graceful shutdown requests,
  and direct teammate inspection:
  [Agent teams](https://code.claude.com/docs/en/agent-teams). Claude subagents
  retain independent transcripts and stable agent ids; `SendMessage` resumes a
  stopped subagent:
  [Subagents](https://code.claude.com/docs/en/sub-agents) and
  [Tools reference](https://code.claude.com/docs/en/tools-reference).
- OpenCode’s Task tool accepts a stable `task_id`, runs children in the
  background, injects completion into the parent, and extends an already
  running job:
  [`task.ts`](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/tool/task.ts).
  Its background-job engine serializes extensions behind the previous tail,
  supports status/wait/cancel, and explicitly documents that the registry is
  process-local rather than durable:
  [`background-job.ts`](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/background-job.ts).
  The public server separately exposes asynchronous prompt and abort endpoints:
  [OpenCode server](https://opencode.ai/docs/server/).

### Capability matrix

| Parent-control capability | Codex | Claude Code | OpenCode | Loopflow now | Required Loopflow floor |
|---|---|---|---|---|---|
| Stable child address | Thread/agent path and id | Named teammate or stable subagent id | `task_id` / session id | Linear issue or Task Session id | Keep; reject cross-Wave control rather than relabeling it human |
| Follow-up while busy | Addressed `send_input` | Mailbox / `SendMessage` | Serialized `extend` | Durable FIFO command | Keep, but name it follow-up and guarantee it survives the turn boundary |
| Immediate redirect | `send_input(interrupt: true)` atomically interrupts then submits | Direct message plus interactive interrupt; not documented as one atomic operation | Separate abort and async prompt | `send` is live only for Codex; other providers queue | Add provider-neutral `steer`: live injection or interrupt-and-resume |
| Replacement semantics | Interrupt flag makes intent explicit | Lead can reject a plan with replacement feedback | Cancel then prompt can be composed | Interrupt replacement sits behind older queued inputs | Supersede all unaccepted input when replacing current work |
| Submission receipt | `submission_id`, target status, structured collaboration item | Task/message state and automatic delivery | Job id, boolean extend result, wait result | Command id plus inferred `live/queued` string | Return persisted → claimed → accepted/failed and the actual effect |
| Status and wait | Status subscription and bounded wait across targets | Shared task list, idle notifications | list/get/wait with timeout | Task status/wait | Keep; make command acceptance waitable, not only Task completion |
| Completion reaches parent | Automatic structured completion notification | Automatic report/idle notification | Synthetic result injected into parent | Prose Wave message | Fold typed Task events into the Wave inbox with a durable cursor |
| Stop and lifecycle end | Interrupt and close agent | Interrupt, graceful shutdown request, cleanup | Abort/cancel | Interrupt and abandon | Keep both cancel-current-turn and end-Task semantics distinct |
| Resume same history | Ensures known thread is loaded before send | Stable subagent resume; teams have documented resume limits | Same `task_id` continues session | Same provider session/worktree | Keep; a command to a stopped nonterminal Task must resume it automatically |
| Parent decision gate | Parent can steer and inspect; child approvals share policy | Lead approves/rejects teammate plans; permission requests reach lead | External client can answer permissions | Harnesses auto-approve; no Task→Wave question | Add one durable Task decision request/answer protocol |
| Crash durability | Persisted thread/agent graph | Persisted subagent transcript; team runtime has limits | Session persists, background registry does not | SQLite command/event/session + git worktree | Loopflow should remain stronger here |
| Work isolation | Configurable; concurrent writers require discipline | Optional/enforced worktrees on some surfaces | Child session, normally shared project worktree | Immutable Task worktree | Loopflow remains stronger here |

### Where the current implementation is below the floor

1. `lf task send` has capability-dependent meaning. During a Codex turn it is
   live steering; during Claude/OpenCode it is a later follow-up. The Wave
   cannot express “change direction now” independently of provider.
2. `delivery: live` is inferred from Task status and provider before the runner
   claims the command. It is not an acceptance receipt.
3. `interrupt --message` appends the replacement behind previously queued
   messages. Obsolete guidance can run before the correction.
4. A command can arrive after the runner’s last poll but before turn teardown.
   The Task becomes waiting while the durable command remains unclaimed until
   some later command happens to restart it.
5. `CommandAccepted` stays only in Task history. Command and completion notices
   are copied into the Wave journal as unattributed prose `UserMessage` rows,
   which can wake an unnecessary Wave turn and cannot be correlated safely.
6. A foreign Wave process can name another Wave’s issue. Because a mismatched
   `LFD_WAVE_ID` falls back to `TaskCommandSource::Human`, the command is
   accepted rather than refused.
7. Every provider harness uses `ApprovalPolicy::AutoApprove`. A Task cannot
   pause on a plan, ambiguity, or consequential choice and ask its owning Wave
   to decide.

### Benchmark API semantics

The smallest API that clears the benchmark is three operations, not one
provider-dependent “send”:

```text
follow_up(task, text)
  Preserve current work. Deliver text exactly once as the next Task turn.

steer(task, text)
  Change direction now. Inject into the live turn when supported; otherwise
  interrupt the turn and resume the same provider session with text next.

interrupt(task, replacement?)
  Stop current work. With replacement, supersede every unaccepted input and
  make replacement the next turn. Without replacement, leave the Task waiting.
```

All return the same durable receipt:

```rust
struct TaskCommandReceipt {
    command_id: TaskCommandId,
    task_session_id: TaskSessionId,
    state: TaskCommandState,       // persisted | claimed | accepted | failed | superseded
    applied_as: Option<TaskCommandEffect>, // live_steer | next_turn | replacement
    generation: Option<u32>,
    accepted_at: Option<OffsetDateTime>,
    error: Option<String>,
}
```

`steer` and `interrupt(replacement)` wait for an accepted/failed receipt by
default because the Wave is making a control decision, not dropping mail.
`follow_up` may return after durable persistence, but its later acceptance,
failure, or supersession still enters the Wave’s Task-event inbox.

Task→Wave judgment uses one additional protocol rather than provider-specific
approval plumbing:

```rust
TaskEventKind::DecisionRequested {
    decision_id: TaskDecisionId,
    prompt: String,
    options: Vec<String>,
}

TaskCommandKind::Decide {
    decision_id: TaskDecisionId,
    choice: String,
    message: Option<String>,
}
```

The Task waits durably; the Wave receives the request as a typed inbox event,
answers it, and the same Task/provider session continues. Plan approval is the
first use. This matches Claude’s lead/teammate control without importing its
general shared-team machinery.

### Benchmark scenarios

The redesign is not at parity until the same black-box suite passes for Codex,
Claude, and OpenCode adapters:

1. **Live redirect:** steer an active Task. Codex changes within the current
   turn; Claude/OpenCode interrupt and resume. The receipt names the actual
   effect.
2. **Gentle follow-up:** a follow-up never interrupts and becomes exactly the
   next turn on every provider.
3. **Replacement:** queue A and B, then interrupt with C. A and B become
   `superseded`; C is the next input.
4. **Boundary race:** persist a command concurrently with turn completion. It
   is accepted exactly once before exit or triggers automatic same-session
   resume; it is never stranded.
5. **Crash recovery:** crash after persistence, claim, and provider acceptance
   in separate runs. Recovery never loses or duplicates an instruction.
6. **Ownership:** the owning Wave can control the Task; a different Wave is
   refused; an explicit human escape hatch remains distinguishable.
7. **Decision round trip:** Task requests plan approval, Wave rejects with
   feedback, Task revises, Wave approves, and the same transcript continues.
8. **Automatic observation:** completion, failure, command acceptance, and
   decision requests wake or queue behind the Wave exactly once through typed
   events, including after the Wave was stopped.
9. **Parallel targeting:** one Wave steers Task A while Task B runs; ids,
   receipts, transcripts, and worktrees never cross.
10. **Lifecycle:** interrupt stops one turn, abandon ends the Task, and neither
    operation is confused with deleting history or worktree state.

## Tensions

- The steerable implementation is wave-only, while the isolated implementation
  is task-like but unsteerable.
- Tmux looks interactive but cannot control providers consistently: Codex owns
  a JSON-RPC pipe, OpenCode uses HTTP, and Claude runs a process per turn with
  null stdin in Loopflow's adapter.
- `Run` mixes execution identity, git stack state, queue state, delivery, and
  ancestry. A Task Session needs fewer, firmer fields.
- The wave journal is durable conversation; the bus is intentionally not a
  log. Task commands cannot depend on terminal keystrokes or an expiring row
  without an acknowledgement contract.
- The current design doc still describes foreground Wave epochs even though
  the simpler product is now a stable Wave managing durable child sessions.

## Observations

### Complexity

The largest avoidable duplication is between `flowloop/wave.rs` and the generic
driver/pass loop. Placement and stacked delivery then amplify it through
`engine/worktrees.rs`, `lfd/executor`, queue operations, `combine_prs`, CLI
flags, DTOs, and sqlite fields.

### Quality

Provider control is already honest and explicit. The Harness capability bit
prevents pretending that queued next-turn input is live steering. Wave inbox
claim/requeue behavior is also stronger than the detached-loop path.

The generic loop's tmux ownership is operationally useful, but its read-only
inspection surface and blocking child process are a poor foundation for task
management.

### Potential

A first-class Task Session can reuse the harness control semantics, existing
work-line channels, run registry, tmux process lifetime, and provider session
ids. This removes the need to move the Wave LM, create task servers, or keep a
second generic loop implementation.

## Open questions resolved by the design

- Task conversation is separate, but every Wave→Task instruction and terminal
  result is mirrored into the Wave journal as a linked event.
- Linear identity is required before worktree creation. New-record mutation
  fails closed during a Linear outage, while an already-identified task may
  launch from an acceptable cached PM snapshot; no offline authoring or
  reconciliation state machine is added.
- Task worktrees and sessions remain resumable through review and dependency
  waits; completion means merged or explicitly abandoned, not PR opened.
- Dependent delivery and multi-contributor integration are deferred until the
  independent main-targeting task lifecycle is proven.

## Recommendation

### Replace foreground epochs and detached loops with durable task sessions

**Observation**: A stable Wave can already receive, reason about, and publish
controls. Providers already expose structured session operations. The missing
piece is a task-scoped durable control loop.

**Cost**: A new task-session state model, CLI verbs, persistent command/event
handling, a shared steerable runner, schema migration, and deletion of old
placement/stack/detached-loop surfaces.

**Benefit**: One answer for starting work, immutable task worktrees, direct
Wave supervision, resumable review, provider-neutral steering, and large code
deletion.

**Verdict**: Build as the large MVP. Defer dependency and integration delivery,
remote control, and rich task UI until this lifecycle has real dogfood evidence.
