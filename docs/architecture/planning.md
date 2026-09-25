# Planning

```bash
lf start product
lf task prepare INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf task run INF-124
lf wave new-chapter --wave product --chapter 2026-09 --plan plan.json --dry-run --json
```

## The planning model

Wave → Task is the public hierarchy. The Wave owns durable purpose, memory,
conversation, cadence, placement, and metric instruments. One internal Project
holds its current chapter KRs, metric targets, Flow recommendation, and Task
membership. The Wave owns the sole objective.
A Task owns its concrete change, worktree, active remote branch, and serial PRs.
Only Tasks persist a selected Flow position and worker claim.

`wave_chapters` owns the current binding and transition receipts. A unique
index permits one current chapter and one incomplete transition per Wave.
Rotation locks against Task filing, prepares a provider UUID before creation,
and recovers ambiguous replies by reading that identity. Task retirement and
worker claims share SQLite transactions; first execution survives Flow resets
as a Task event. Local transfer updates only the parent; stale worker saves
cannot restore it.

Started unfinished Tasks move, untouched backlog is canceled, and terminal work
stays historical. Uncertain evidence remains unresolved. Archived content and
membership are read from the frozen boundary receipt, not today's moved Tasks.
Task observations go directly to the Wave; its single operation judges KRs and
selects work. Status and roadmap join current chapter planning directly to Tasks.
Both retain stranded Tasks when the chapter plan is unavailable; historical
Project operator state remains in diagnostics.

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
- review nodes, which stop at an explicit interaction boundary.

Flow YAML is the authored definition. An active Task Flow persists its expanded
invocation and exact cursor in `FlowPosition`. A Task worker executes that saved
position directly and settles through its versioned claim. Completion removes
the invocation; interruption retains the cursor. Wave's in-memory Playhead,
continuation queue, and body events do not participate in Task execution.

Direct TTY flows can use the current conversation for a review step. Headless
Task flows persist the review position and start its saved Skill as a provider
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
invoke Skill / Op / session
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
worker resumes from those facts and launches a fresh Run when needed. Wave operations reread current chapter facts on every invocation.

## Workers and operations

The Task worker lives under
[`controller/task/`](../../rust/loopflow/src/controller/task/) and exits after
one claimed boundary. Deterministic chapter rotation lives in
[`ops/chapter.rs`](../../rust/loopflow/src/ops/chapter.rs). Wave listener,
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
Domain-specific races use narrower fences: exact review FlowPosition tokens, PR
heads, landing generations, or OS locks.

## Steer

```bash
lf task steer INF-123 "keep the public name"
```

Task steering posts a Linear issue comment. Direct Linear comments enter the
same path. Only the claimed Task advancer attempts live delivery; independent
Work-bound Runs receive their ordinary context. The next advancing worker
refreshes Linear and includes saved comments. Idle steering starts no worker.
Local event rows and comment-id deduplication are a delivery cache; Linear owns
the authored direction. Publication, seed inclusion, and provider transport
acceptance are separate evidence, none proving application by the model.

Wave guidance is extra input to `wave/operate`.
Wave chat sends ordinary channel messages.

## Questions and sessions

```bash
lf --as wave:product : "which Task should start?"
lf ask "review which migration should survive"
lf session list --json
lf session open <session-id> --json
lf session complete <session-id>
```

Another agent perspective is an ordinary `lf --as` Run. `lf ask` is reserved for
a decision from the user: it blocks the originating Run while a durable TUI agent shares
its checkout. Agent readiness leaves the session visible. Complete closes that
conversation and resumes the originating Run with the ready summary.

A review FlowStep is durable because the Task playhead is durable. The Task runs
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

Wave operation
  `-- one finite wave/operate Run

lfd
  `-- Wave listener / resident
```

The Wave listener and resident are not prerequisites for Task
motion. The exact Task-position claim admits one worker. Other agent
perspectives remain ordinary attributed Runs, and sessions reuse either
their originating Run or the Task's persisted playhead. Each Wave operation
refreshes its definition, KRs, metrics, and Tasks before deciding. Each Task
worker executes one saved boundary, settles its claim, and launches the next
worker when another autonomous boundary remains. A review boundary parks the
Flow; completion removes its position without selecting another Flow.

## Boundary contracts

- Stable Work identity is the join point for planning input and progress.
- Provider processes are replaceable; Work survives them.
- Every Task boundary and Wave operation rebuilds from current durable facts.
- A Flow playhead advances only from the required boundary result.
- Steer is durable correction; another agent perspective is an ordinary Run.
- An unresolved Session is either an interactive Run, a Task's persisted review
  FlowPosition, or a Run-owned `lf ask` boundary.
- Run ids remain evidence and provenance, never planning capabilities.
- Linear owns shared Project and Task planning truth. Local projections support
  bounded reads and resumable transitions; they do not author provider truth.

## Next

[Delivery →](delivery.md) follows Task Work through Git and GitHub.
[Homes and processes →](homes.md) explains how services and boundary Runs are
placed and supervised.
