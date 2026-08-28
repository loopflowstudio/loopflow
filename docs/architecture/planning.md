# Planning

Tracked Work preserves purpose across provider processes. A Wave contains
Projects; a Project defines a measured bet; a Task carries one concrete change
and its delivery identity. This layer is useful without a long-lived agent.
Controllers form a separate layer above it and may pursue that Work end to end.

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
| Task | one implementation, investigation, document, or shipped change | a second concurrent PR in the same serial chain |
| Work | durable identity, status, authored inputs, and domain facts for one Wave/Project/Task | Flow playhead, provider continuation, or process liveness |
| Run | one provider-launch record | current Work state or control capability |

Every Project belongs to exactly one Wave. Projects do not contain Projects.
Only a Task owns a delivery worktree.

The shared durable types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs): `WorkRef`, `WorkStatus`,
`Steer`, `Ask`, `Home`, and `Placement`. Wave, Project, and Task domain models
add their facts under [`work/`](../../rust/loopflow/src/work/). Controller flow
positions and flow-step interactions live under
[`controller/`](../../rust/loopflow/src/controller/).

## Use tracked Work directly

```bash
lf task prepare INF-123
lf --task INF-123 research "write scratch/runtime.md"
lf --task INF-123 research "write scratch/prompts.md"
lf task steer INF-123 "reconcile both reports"
lf commit -m "Reconcile Task research"
lf pr publish
```

`lf task prepare` ensures Task Work, its single worktree, and the first serial
PR record. It does not install or start controller state. `--task` resolves that
worktree, preloads all recursive scratch Markdown, launches one ordinary Run,
and leaves edits uncommitted. Two such Runs may overlap; neither owns the Task,
advances a playhead, or reserves the worktree. Give concurrent writers distinct
paths and compose a coherent checkpoint through the ordinary delivery verbs.

`lf project prepare` provides the same controller-free boundary for Project
Work. Project-bound and Wave-bound Runs use the owning Wave repository because
those Work kinds have no private worktree. A human, parent agent, cron job, or
another automation system can build its own workflow from these same commands.

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

Flow YAML is the authored definition.
The owning controller persists its playhead. A process reads the current
controller position, executes one boundary through ordinary execution APIs,
and advances only after that boundary returns the required result.

Direct TTY flows can use the current conversation for a human node. Headless
Task flows create a typed Ask, park the playhead, and advance only after an
explicit result.

## Run one controller boundary

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
invoke Skill / Op / Ask
          |
          v
record one domain transition
          |
          `---- next boundary or terminal Work
```

The planning algorithm is deliberately boundary-based:

1. Resolve stable Work identity.
2. Load its current status, playhead, inputs, and domain evidence.
3. Refresh Linear, GitHub, metrics, or repository facts required by this
   boundary.
4. Build one complete prompt or mechanical operation from those facts.
5. Execute it.
6. Record one monotonic domain transition.
7. Rebuild from durable facts before the next boundary.

A crash loses in-memory judgment. It does not lose Work identity, accepted
inputs, controller cursor, Task worktree, or provider observations. The next
controller process resumes from those facts and launches a fresh Run when
needed.

## End-to-end controllers

Project and Task controller implementations live under
[`controller/project/`](../../rust/loopflow/src/controller/project/) and
[`controller/task/`](../../rust/loopflow/src/controller/task/). Wave listener,
runtime, and resident behavior lives under
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

Project and Task controller playheads carry phase, iteration, and cursor state
for their own end-to-end loops. They live in controller-owned rows keyed by
Work id; they are not Project or Task fields and grant no Work mutation
authority. There is no phase epoch, writer token, active-Run slot, or Task
ownership lease.
Domain-specific races use narrow boundaries: Ask claim ids, exact PR heads,
landing generations, or OS locks.

## Steer

```bash
lf task steer INF-123 "keep the public name"
lf work steer task task_... "show the failing fixture"
```

A Task or Project Steer is ordered authored input addressed to stable Work.
Controller-aware convenience commands may wake the built-in controller; an
arbitrary caller can simply read the Steer on its next Run. The receipt proves
storage, not that a provider read or applied the correction. Wave Chat has a
separate live transport.

`Author::Run` may store an opaque Run id as provenance. The store does not need
to resolve that Run record, and resolution would not grant mutation authority.

## Ask

```bash
lf ask "which migration should survive?"
lf ask list --json
lf ask open ask_...
```

Ask is a separate blocking protocol:

1. Creation records origin Work, target perspective, prompt, and optional Run
   provenance.
2. A claim mints the exact generic Run id allowed to answer this attempt.
3. Release requeues the Ask.
4. The first authorized terminal result wins.

Ask results are typed: answer, decline, or a Flow-node resolution. Ask does not
enter the Steer queue, and Steer never impersonates a blocking answer.

## Controller topology

```text
lfd
  `-- Wave controller
        |-- conversation and event journal
        `-- resident loop
              `-- Project controller
                    `-- Task controller in managed worktree
                          `-- Flow Skill boundary
```

The Wave listener owns its HTTP surface, journal, and the resident child it
directly spawned. The Wave, Project, and Task controllers and Ask runner are
separate launch loops because their recovery and settlement rules differ. They
share execution components rather than one universal runner.
The Wave resident refreshes portfolio evidence and chooses the next useful
Project boundary. Project Work refreshes its definition, KRs, metrics, and Tasks
before deciding. A Task controller may execute its end-to-end Flow and delivery
steps; independent Task-bound Runs may do bounded work in the same worktree
without loading or advancing controller state.

Deterministic controller session names reduce accidental duplicate built-in
launches. They are routing policy for that automation implementation, not
durable Task or Run ownership. Other Task-bound Runs remain valid. If a
controller or provider disappears, a later command may start a fresh process
from durable Task and worktree facts.

## Boundary contracts

- Stable Work identity is the join point for planning input and progress.
- Provider processes are replaceable; Work survives them.
- Every boundary rebuilds from current durable facts.
- A Flow playhead advances only from the required boundary result.
- Steer is durable correction; Ask is durable blocking input.
- Run ids remain evidence and provenance, never planning capabilities.
- Linear owns shared Project and Task planning truth. Local projections support
  bounded reads and resumable transitions; they do not author provider truth.

## Next

[Delivery →](delivery.md) follows Task Work through Git and GitHub.
[Homes and processes →](homes.md) explains how controller processes are placed and
supervised.
