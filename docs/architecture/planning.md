# Planning

Tracked Work preserves purpose across provider processes. A Wave contains
Projects; a Project defines a measured bet; a Task carries one concrete change
and its delivery identity. This layer is useful without a long-lived agent.
Finite Project operations and claimed Task workers act on it when invoked.

```bash
lf start product
lf project run <project-id>
lf task run INF-123
lf task prepare INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf project prepare <project-id>
```

## The planning model

```text
Wave
  |-- goal, memory, cadence, chat
  `-- Project
        |-- definition, KRs, closure criteria
        `-- Task
              `-- concrete work and delivery
```

| Model | Owns | Does not own |
| --- | --- | --- |
| Wave | durable context, memory, cadence, conversation, project selection | project KRs or Task worktrees |
| Project | one measured bet, definition, KRs, closure judgment | memory, cadence, nested Projects |
| Task | one implementation, investigation, document, or shipped change; one active remote branch, worktree, and PR | another simultaneously active delivery branch |
| Work | durable status and inputs for one Wave/Project/Task; Task-only Flow position and claim | provider-process liveness |
| Run | one provider-launch record | current Work state or control capability |

Every Project belongs to exactly one Wave. Projects do not contain Projects.
Only a Task owns a delivery worktree.

The shared durable types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs): `WorkRef`, `WorkStatus`,
`FlowPosition`, `Steer`, `Home`, and `Placement`. Wave, Project, and Task
domain models add their own facts under [`wave/`](../../rust/loopflow/src/wave/),
[`project/`](../../rust/loopflow/src/project/), and
[`task/`](../../rust/loopflow/src/task/).

## Compose Skills with a Flow

```yaml
# .lf/flows/build.yaml
- skill: implement
- skill: compress
- skill: gate
```

```bash
lf flow build
```

A Flow is an ordered graph of:

- Skill nodes, which launch a provider;
- Op nodes, which perform a bounded mechanical action;
- Xor nodes, which route from recorded results;
- human nodes, which stop at an explicit interaction boundary.

Flow YAML is the authored definition. An active Task Flow persists its expanded
invocation and exact cursor in `FlowPosition`. A Task worker executes that saved
position directly and settles through its versioned claim. Completion removes
the invocation; interruption retains the cursor. Wave's in-memory Playhead,
continuation queue, and body events do not participate in Task execution.

Direct TTY flows can use the current conversation for a human node. Headless
Task flows persist the human position and start its saved Skill as a provider
Run. Approve advances; Iterate returns to the preceding autonomous
step with new direction; provider exit leaves the playhead parked and
provider-native history resumable.

## Run one Work boundary

```text
load current Work facts
          |
          v
refresh provider truth and authored input
          |
          v
choose next Flow boundary
          |
          v
invoke Skill / Op / human session
          |
          v
record one domain transition
          |
          `---- next boundary or terminal Work
```

Task execution is deliberately boundary-based:

1. Resolve stable Work identity.
2. Load its current status, selected Flow, inputs, and domain evidence.
3. Refresh Linear, GitHub, metrics, or repository facts required by this
   boundary.
4. Build one complete prompt or mechanical operation from those facts.
5. Execute it.
6. Advance the exact Flow version or release the worker claim.
7. Rebuild from durable facts before the next boundary.

A crash loses in-memory judgment. It does not lose Work identity, accepted
inputs, Task Flow position, worktree, or provider observations. The next Task
worker resumes from those facts and launches a fresh Run when needed. Project
operations instead reread current Project facts on every invocation.

## Workers and operations

The Task worker lives under
[`controller/task/`](../../rust/loopflow/src/controller/task/) and exits after
one claimed boundary. Finite Project launch and bookkeeping lives in
[`ops/project.rs`](../../rust/loopflow/src/ops/project.rs). Wave listener,
runtime, and optional service behavior lives under
[`controller/wave/`](../../rust/loopflow/src/controller/wave/).

## Work state

`WorkStatus` has three durable values:

| Status | Meaning |
| --- | --- |
| `Ready` | the Work may take another planning boundary |
| `Done` | its current objective has converged |
| `Abandoned` | work stopped without convergence |

Runtime activity is a separate projection. A ready Work may have no live
process; one Work may launch many Runs over time; an unterminated Run does not
make Work “running.” Reopen returns the same stable Work to `Ready` after
clearing transient input defined by that domain.

The Task Flow version prevents an older worker from rolling progress backward.
Domain-specific races use narrower fences: exact human FlowPosition tokens, PR
heads, landing generations, or OS locks.

## Steer

```bash
lf task steer INF-123 "keep the public name"
lf work steer task task_... "show the failing fixture"
```

A Task or Project Steer is ordered authored input addressed to stable Work.
Task convenience commands may start a worker; an arbitrary Task-attributed Run
can read the Steer but cannot move the Flow position. Project Steer launches a
fresh finite operation. The receipt proves storage, not that a provider read or
applied the correction. Wave Chat has a separate live transport.

`Author::Run` may store an opaque Run id as provenance. The store does not need
to resolve that Run record, and resolution would not grant mutation authority.

## Questions and human sessions

```bash
lf --as wave:product : "which Project owns this?"
lf ask "review which migration should survive"
lf session list --json
lf session open <session-id> --json
lf session complete <session-id>
```

Another agent perspective is an ordinary `lf --as` Run. `lf ask` is reserved for
human judgment: it blocks the originating Run while a durable TUI agent shares
its checkout. Agent readiness leaves the session visible. Complete closes that
conversation and resumes the originating Run with the ready summary.

A human FlowStep is durable because the Task playhead is durable. The Task runs
`lf --tui --as task:<id> <skill>` and stores that ordinary Run's id beside the
exact playhead. An ad-hoc Ask persists a small Home-local session record and its
ordinary Run id while its caller waits. Both project through one `SessionRecord`
DTO with distinct `ask` and `flow` kinds. The Mac app resumes provider-native
history and authors the kind's one valid action; it owns no second Session
state. A thin detached PTY cradle only keeps the initial provider client alive
before a UI arrives.

## Execution topology

```text
Task CLI
  `-- exact Task Flow-position claim
        `-- one Task worker boundary Run

Project CLI
  `-- one finite project/operate Run

lfd
  `-- Wave listener / resident
```

The Wave listener and resident are not prerequisites for Project or Task
motion. The exact Task-position claim admits one worker. Other agent
perspectives remain ordinary attributed Runs, and human sessions reuse either
their originating Run or the Task's persisted playhead. Each Project operation
refreshes its definition, KRs, metrics, and Tasks before deciding. Each Task
worker executes one saved boundary, settles its claim, and launches the next
worker when another autonomous boundary remains. A human boundary parks the
Flow; completion removes its position without selecting another Flow.

## Boundary contracts

- Stable Work identity is the join point for planning input and progress.
- Provider processes are replaceable; Work survives them.
- Every Task boundary and Project operation rebuilds from current durable facts.
- A Flow playhead advances only from the required boundary result.
- Steer is durable correction; another agent perspective is an ordinary Run.
- An unresolved Session is either an interactive Run, a Task's persisted human
  FlowPosition, or a Run-owned `lf ask` boundary.
- Run ids remain evidence and provenance, never planning capabilities.
- Linear owns shared Project and Task planning truth. Local projections support
  bounded reads and resumable transitions; they do not author provider truth.

## Next

[Delivery →](delivery.md) follows Task Work through Git and GitHub.
[Homes and processes →](homes.md) explains how services and boundary Runs are
placed and supervised.
