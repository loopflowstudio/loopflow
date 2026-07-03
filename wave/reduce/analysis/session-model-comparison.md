---
head: d370450a81ccdd4b8c00f3052f8271e3b515a575
status: bootstrap
---

# Session Model Comparison

## Question

What should loopflow call and store as an agent session?

The current system has several nearby concepts:

- `Run` - wave/flow execution lineage.
- `Session` - tmux/control session tied to a wave and optional run.
- `ExecutionProcess` - process/container backend state.
- `Conversation` - provider conversation plus persisted event stream.
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

### Conversation

`Conversation` tracks provider-level interaction and persisted events. It owns
harness name, provider session id, run id, config, status, input support,
context snapshots, turn/item events, and token usage.

Statuses:

```text
starting -> active -> ending -> ended|failed
```

Representative files:

- `rust/loopflow/src/lfd/conversations/types.rs`
- `rust/loopflow/src/lfd/conversations/mod.rs`
- `rust/loopflow/src/lfd/conversations/README.md`
- `python/loopflow/models.py`

Current direction: cut this subsystem rather than wrap it. The code was kept
because it may be useful later, but loopflow is not currently doing enough with
provider conversations to justify the concept. If it becomes useful later, bring
it back from git with fresh product pressure.

## Misalignment

Loopflow's product language says "sessions" are what the human watches and
reattaches to. The implementation currently uses:

- `Session` for terminal/control attachment.
- `Conversation` for provider transcript/event stream that is not currently
  earning its place.
- `ExecutionProcess` for backend process lifecycle.
- `Run` for wave/flow lineage.
- `AgentStarted/AgentEnded` and `OutputLine` for older agent-shaped event
  vocabulary.

That creates a naming inversion: the most session-like thing from a coding-agent
product perspective is `Conversation`, but the user-facing API already calls the
terminal/control record `Session`.

The split leaks into clients. Python exposes both `Session` and `Conversation`.
Swift mirrors `Session` and `Run`, while conversation support is documented in
Rust but not equally first-class across the native model layer. lfd events carry
both session events and agent events.

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
`Conversation` is removed for now. Future provider transcript/input work should
return only when it can hang off the Session Record without reintroducing a
parallel user-facing object.

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
  current DTOs.
- Check whether `AgentStarted`, `AgentEnded`, and `OutputLine` are legacy
  vocabulary or still load-bearing.
- Decide whether Concerto should open ordinary terminals as standalone workspace
  panes or only as panes inside a main agent tmux session.
- Map every conversation root before deletion: Rust manager/harness/routes/store,
  usage aggregation, Python models/client/tests, Swift service/session state,
  docs, e2e, migrations, and generated summaries.
