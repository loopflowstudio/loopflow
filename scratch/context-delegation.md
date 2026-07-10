# Context-tiered execution

## Evidence

- Every assembled prompt receives the same `LOOPFLOW.md`. That document tells a
  one-off implementation run to inspect Linear, resolve a wave, find a server,
  and start child loops even when its seed already names one concrete job.
- Generic skill orientation repeats the problem: `implement`, `gate`, `qa`,
  `research`, and dozens of peers say to infer whether a `wave/<name>/`
  directory "matches this work" and then read that wave's live tasks.
- A served wave receives a second instruction saying the orchestrator "never
  grinds inline." A single blocked next move is therefore pushed into another
  worktree and transcript instead of being resolved in the room that found it.
- `wave_pursue` and `project_pursue` currently make `lf loop` the normal route,
  while `task_pursue` does not explicitly close that capability. The result is
  recursive whole-task delegation rather than strict-subset delegation.
- `lf loop --max-passes 1` is accepted. It creates loop state and a worktree but
  removes the only reason to use the primitive: room for another pass after the
  first boundary.
- Detached launch currently returns a tmux session and then dies: the server
  emits `lf loop ... --wave W`, but `--wave` is a top-level option and must
  precede `loop`. Most public examples teach the same invalid order.
- Explicit wave context and evidence attribution resolve independently, and a
  placed loop's registry run id differs from the trace id its passes emit.
  Status, prompt logs, and `lf trace` therefore do not identify the same work.

## Design

Make the shared operating document a capability floor. Tier behavior belongs in
the skill that exercises it; no prompt renderer should infer a tier from a skill
name.

| Context | Default execution | Delegated loops |
| --- | --- | --- |
| One-off / task | Do the seed in the current process and worktree | None |
| Project | Do the sole blocking task inline | Independent task loops only |
| Wave | Do the sole blocking move inline | Independent project or task loops |

Every tier may create child Loopflow processes as part of inline execution.
Operational commands such as `lf commit`, `lf pr land`, and `lf rebase`, plus
direct skill or flow calls, do not create a new delegated lifecycle and remain
available everywhere.

Universal guidance must say:

- Execute here first. Delegate only a strict subset that can finish without
  delegating the parent objective again.
- PM, wave discovery, server startup, and child loops are not prerequisites for
  ordinary work. Use them only when the active skill or human explicitly grants
  that capability.
- Never guess a wave name. A missing exact wave, failed PM reader, or stopped
  server is reported and bypassed with inline work when the seed remains
  computable.
- A one-pass operation is a direct flow run, never a loop.

Tier skills then add only their capability:

- `wave_pursue`: select from exact ambient wave state; use project/task loops
  only for independent work with a real repeated lifecycle. Never dispatch the
  whole wave objective. A failed PM reader does not become the wave's new task.
- `project_pursue`: turn KRs into work; use task loops only when a task needs its
  own PR/recheck lifecycle or useful parallelism. Never start projects or waves.
- `task_pursue`: implement in the current process. Use operational Loopflow
  children freely, but never invoke `lf loop`. Read and complete its own filed
  task, inspect related work when useful, and file discovered follow-up tasks
  without launching them. Do not repair PM/auth as a detour.

`--detach` is not permission to delegate. It changes ownership of an already
justified child loop from the foreground caller to the served wave. Use it only
when the parent has another useful move to make while the child runs. If the
parent needs the result next, keep the loop foreground; if the work is a sole
local blocker, do it inline. Never start a server just to make detachment
available.

The CLI rejects `max_passes < 2` before resolving a wave, checking a server, or
creating a worktree. Its error points to `lf flow <name> "<seed>"` for one-shot
execution.

Detached launch emits parser-valid top-level wave scope and enforces the same
two-pass minimum at its HTTP door. Explicit wave scope must also drive evidence
attribution, and each placed loop's registry run id must be the `LF_RUN_ID` its
passes and operational descendants inherit.

## Acceptance

- A bare assembled `implement` prompt contains no PM lookup or child-loop launch
  recipe.
- Generic execution skills consult wave/PM state only when the prompt or seed
  names an exact wave, task, project, or concrete coordination question.
- A wave pass says to resolve one local blocker inline and grants only
  project/task loop delegation.
- A project pursue prompt grants task loops but forbids project/wave children.
- A task pursue prompt explicitly owns execution, permits operational Loopflow
  children and scoped PM use, and forbids every `lf loop` child.
- `lf loop task "work" --max-passes 1` fails before placement with the direct
  flow correction.
- A detached loop's generated argv parses, and the loop door rejects one-pass
  requests before creating a tmux session.
- Explicit `--wave` overrides ambient attribution, and foreground/detached loop
  passes emit under the registry run id shown by `lf status`.
- Existing multi-pass foreground and detached loops remain unchanged.
