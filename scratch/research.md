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
