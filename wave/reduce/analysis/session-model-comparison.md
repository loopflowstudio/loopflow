---
head: 615729570782d730d2ea3b196e34779db9f63555
status: bootstrap
---

# Session Model Comparison

## Question

What should loopflow call and store as an agent session?

The live system now has three nearby concepts (the `Conversation` subsystem was
removed in HEAD `42a663ee`; see the shipped history in the wave and git):

- `Run` - wave/flow execution lineage.
- `Session` - tmux/control session tied to a wave and optional run.
- `ExecutionProcess` - process/container backend state.
- runtime events - `run.*`, `flow.*`, `step.*`, `AgentStarted`, `AgentEnded`,
  `SessionCreated`, `SessionUpdated`, and `OutputLine`.

That split may be correct internally. It is not yet a clear product model.

## Reference systems

### Codex

Codex makes the user-facing object a resumable coding session. A resumed run
keeps the original transcript, plan history, and approvals. The CLI also has a
remote app-server mode, so the same session model can span a local terminal and
remote server.

Codex's approval model is explicit: default local work can read/edit in the
workspace and asks before internet or out-of-workspace actions. Sandbox mode,
approval policy, and reviewer are named as first-class settings.

Codex instruction loading is also session-shaped. It reads `AGENTS.md` once per
run or launched TUI session, layering global and project guidance in a
documented precedence order.

Sources:

- https://developers.openai.com/codex/cli/features
- https://developers.openai.com/codex/concepts/sandboxing
- https://developers.openai.com/codex/guides/agents-md

### OpenCode

OpenCode exposes sessions directly from the CLI:

- `opencode --continue` continues the last session.
- `opencode --session <id>` continues a specific session.
- `opencode --fork` branches from a prior session.
- `opencode session list/delete` manages sessions.
- `opencode export/import` serializes session data.

OpenCode also separates surfaces from the backend. `opencode serve` starts a
headless server; `opencode attach` connects a terminal UI to an already running
backend; `opencode run --attach` runs non-interactive commands against that
server. The session remains the product object across TUI, CLI, web, and server
surfaces.

OpenCode agents carry permission configuration. Agent creation can generate a
custom system prompt plus explicit allowed permissions.

Sources:

- https://opencode.ai/docs/cli/
- https://opencode.ai/docs/agents/
- https://opencode.ai/docs/permissions/

## Loopflow's current model

### Run

`Run` is flow execution lineage. It owns wave linkage, flow, task, repo, areas,
directions, iteration, step index, PR state, queue role, and stack position.

Representative files:

- `rust/loopflow/src/lfd/http/dto.rs`
- `swift/LoopflowCore/Models/Run.swift`
- `python/loopflow/models.py`

### Control Session

`Session` is a control/terminal session. It has `wave_id`, optional `run_id`,
optional `parent_session_id`, `use` (`wave_agent`, `worker`, `palette`), step,
agent, cwd, argv/env, source, tmux name, and lifecycle timestamps.

Statuses:

```text
pending -> attached -> running -> succeeded|failed|canceled
```

Representative files:

- `rust/loopflow/src/lfd/types/session.rs`
- `rust/loopflow/src/lfd/http/dto.rs`
- `swift/LoopflowCore/Models/Session.swift`
- `python/loopflow/models.py`

### Concerto terminal workspace

Concerto has a broader terminal concept than lfd should own. A user can open a
plain tmux/CLI session that is not an lf run, not an agent conversation, and not
part of wave execution. That is native workspace state unless the product
decides all terminals are opened inside the main agent's tmux session.

This matters for naming. lfd should probably only persist **lf sessions**:
agent/wave/palette sessions that loopflow can launch, observe, resume, account
for, and relate to a wave or run. Concerto can still render ordinary terminal
tabs, splits, and workspaces, but those should not be promoted into lfd Session
Records unless they become loopflow-managed work.

### Execution Process

`ExecutionProcess` tracks backend process/container state: pid, container id,
repo, worktree, agent, run mode, and status.

Statuses:

```text
unspecified|waiting|running|completed|failed
```

Representative files:

- `rust/loopflow/src/lfd/types/execution.rs`
- `rust/loopflow/src/lfd/store/`

### Conversation (removed)

`Conversation` used to track provider-level interaction and persisted events —
harness name, provider session id, run id, config, status, input support,
context snapshots, turn/item events, and token usage. Its Rust manager,
harnesses, runtime, types, usage helpers, routes, store traits, migrations,
Python models/client, Swift service/session-state hooks, docs, and e2e tests
were all pulled in HEAD `42a663ee`.

It was cut rather than wrapped: loopflow was not doing enough with provider
conversations to justify the concept, and keeping it created a parallel model
beside lfd sessions that made this Session Record design harder to see. Git is
the archive. If provider transcript/input work returns, it should hang off the
Session Record rather than reintroduce a parallel user-facing object.

## Misalignment

Loopflow's product language says "sessions" are what the human watches and
reattaches to. Removing `Conversation` closed the worst gap — the naming
inversion where the most session-like object was not the one called `Session`.
What remains is a smaller, healthier split:

- `Session` for the loopflow-managed agent/control interaction.
- `ExecutionProcess` for backend process lifecycle.
- `Run` for wave/flow lineage.
- `AgentStarted/AgentEnded` and `OutputLine` for agent-shaped event vocabulary
  whose load-bearing status is not yet settled (see "Next evidence to gather").

The open question is no longer "which of these is really the session" but
whether a user should reason from one **Session Record** read model or keep
stitching `Session` + `Run` + `ExecutionProcess` by hand across CLI, lfd, lfq,
and Concerto.

## What the references suggest

The stable product object should be the thing a user can:

- start
- watch
- feed input to
- attach from another surface
- resume
- fork
- export/import or audit
- review for tools, files, approvals, usage, and transcript

Codex and OpenCode both make that object legible as a session across surfaces.
They do not expose separate user concepts for terminal attachment, provider
conversation, backend process, and task lineage unless the user needs that
layer.

## Realignment candidate

Make one **Session Record** the user-facing spine.

Internal components can stay separate:

- run lineage
- control attachment
- provider transcript
- backend process
- output stream
- approval stream
- usage accounting

But the public API, UI, and docs should let users reason from one object:

```text
session
  id
  wave_id
  run_id
  role: wave_agent | worker | palette
  harness/provider
  command/cwd/worktree
  status
  attach/connect info
  transcript/events
  context snapshot
  approvals
  usage
  process/backend details
```

`Run` remains the flow execution unit. `Session` becomes the loopflow-managed
agent interaction unit. `ExecutionProcess` becomes a backend detail.
`Conversation` is already gone. The `transcript/events` and `approvals` facets
above are therefore aspirational, not backed by current storage — future
provider transcript/input work should return only when it can hang off the
Session Record without reintroducing a parallel user-facing object.

Do not make every Concerto terminal a Session Record. A normal CLI terminal is a
workspace pane. It becomes a Session Record only when it is launched or adopted
as loopflow-managed work with an lf command/run/session identity.

Deletion rule: pull all the roots. Do not leave compatibility endpoints, client
models, store traits, migrations, tests, docs, UI hooks, or summary references
for a subsystem whose product use is gone.

## Cascading simplification

- One lifecycle vocabulary for CLI, lfd, lfq, Concerto, docs, and tests.
- One "current session" lookup instead of separate current agent/control paths.
- One event stream shape for UI: session status, output, usage, and attention
  can hang off the same id when those features earn their way back.
- Less DTO mirror surface: the public DTO can expose one session aggregate while
  internal store tables stay normalized.
- Easier continuation semantics: resume/fork/export attach to session id, not a
  choice between run id, session id, provider session id, and process id.

## Preserve

Do not collapse real layers prematurely:

- `Run` is still needed for wave/flow/queue lineage.
- `ExecutionProcess` is still needed for local/Docker process supervision.
- Tmux/control attachment is a real transport facet.
- Concerto terminal workspace state is broader than lfd session state.

The simplification is naming and API shape first. Storage can stay normalized
until a prototype proves a smaller implementation is safe.

## Next evidence to gather

- Trace `lfq sessions`, `lfq attach`, and Concerto session rendering against
  current DTOs — this is the continuity audit that feeds the proposal spine.
- Check whether `AgentStarted`, `AgentEnded`, and `OutputLine` are legacy
  vocabulary or still load-bearing.
- Decide whether Concerto should open ordinary terminals as standalone workspace
  panes or only as panes inside a main agent tmux session.

The conversation-root mapping this section once called for is done: every root
was pulled in HEAD `42a663ee`.
