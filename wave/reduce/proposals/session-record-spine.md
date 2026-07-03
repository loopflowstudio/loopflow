---
status: draft
source_analysis: wave/reduce/analysis/session-model-comparison.md
---

# Session Record Spine

## Design decision

Make one user-facing **Session Record** the spine for agent interaction across
CLI, lfd, lfq, and Concerto.

The Session Record is the object a user starts, watches, resumes, attaches to,
exports, audits, and reviews. Run lineage, backend process, tmux attachment,
usage, and context snapshots become facets of that object rather than competing
public nouns.

Scope: this is an **lf session** spine. lfd should own loopflow-managed agent,
wave, worker, and palette sessions. Concerto can also show ordinary terminal
sessions that are not loopflow work; those are workspace/terminal state unless
the product deliberately decides to open all terminals inside the main agent's
tmux session.

## Why

After the conversation subsystem is removed, the remaining split is smaller and
healthier:

- `Run`
- `Session`
- `ExecutionProcess`
- agent/output/session events

Those are real layers. The proposal is not to collapse them into one table. The
proposal is to make the **public read model** match the product question:
"what is my agent doing, and how do I get back to it?"

Codex and OpenCode both make resumable sessions central across terminal, server,
and app surfaces. Loopflow should not make users choose between run id, session
id, process id, and UI terminal pane id when they are trying to answer one
session-shaped question.

`Conversation` was the false pressure, and it has now been removed (reduce's
first reduction). The Session Record can be a small aggregate over live concepts
instead of a compatibility wrapper around a dormant subsystem.

## Shape

Public API shape:

```text
session
  id
  wave_id
  run_id
  role
  status
  agent
  command/cwd/worktree
  attach/connect info
  context snapshot
  usage
  backend/process details
```

Internal storage can remain normalized. The design change is the public spine:
clients ask for a session and receive enough facets to render, resume, attach,
audit, or continue.

Concerto may still need a separate `TerminalWorkspace` or `TerminalPane`
concept. A shell tab running `git status` is not automatically a Session Record.
It becomes one only when loopflow launches or adopts it as managed work.

## Live-concept shape

With conversations gone, the proposal is now a read model over the live layers:

- Keep `Run` as flow execution lineage: wave, flow, task, queue, PR, stack,
  iteration, and current step.
- Keep `ExecutionProcess` as backend supervision: pid/container, worktree,
  run mode, started/ended timestamps, and stuck-process recovery.
- Keep `Session` as the loopflow-managed interaction record: wave/worker/palette
  role, agent, command, cwd, tmux/connect info, lifecycle, and optional run.
- Expose `SessionRecord` as a read model, not necessarily a stored DTO:
  `Session + optional Run + optional ExecutionProcess + recent output/attention
  + usage if available`.
- Keep Concerto unmanaged terminals out of lfd. They live in
  `TerminalWorkspace`/`TerminalPane` unless explicitly adopted as lf work.

This makes the first implementation slice small: build the read model and update
clients/UI to consume it where they currently stitch sessions and runs by hand.

Removing conversations also removed the usage/cost dashboard it fed. Any future
metering must derive from the live session/run model — usage as a facet of the
Session Record — not from resurrected conversation events. The dashboard is not a
thing to rebuild; it is a thing to re-derive from live state if the product wants
it back.

## Prototype plan

Build a read-model aggregator before changing storage:

1. Add an internal `SessionRecord` view assembled from existing store tables.
2. Populate it for one active run with control session + process + latest
   events.
3. Render it in a narrow CLI/debug endpoint or script.
4. Compare against current `lfq sessions` and Concerto session view.

Prototype success means the aggregate makes existing behavior clearer without
forcing a risky database migration.

## Open questions

- Should `run_id` remain required for worker/wave sessions and optional only for
  palette sessions?
- Should Concerto support unmanaged terminal panes beside lf sessions, or should
  every terminal live inside a wave/agent tmux topology?
- If unmanaged terminal panes exist, what is the adoption path that turns one
  into a loopflow-managed session?
- Do `AgentStarted`, `AgentEnded`, and `OutputLine` survive as compatibility
  events or become session event variants?
- Is resume/fork a Session Record operation in loopflow, or delegated entirely
  to the underlying harness until all harnesses support it?

## Gate

This proposal changes public vocabulary and DTO/API shape. It needs human design
agreement before implementation.
