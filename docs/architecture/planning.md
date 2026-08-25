# Planning

Planning preserves purpose across provider processes. A Wave chooses Projects;
a Project pursues measurable KRs; a Task performs one concrete change. Their
controllers own distinct loops, while Skill boundaries reuse the discovery,
prompt, provider, harness, and Run-evidence path described in
[Execution](execution.md).

```bash
lf start product
lf project run <project-id>
lf task run INF-123
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
| Work | durable status, Flow playhead, inputs, failure/progress for one Wave/Project/Task | provider-process liveness |
| Run | one provider-launch record | current Work state or control capability |

Every Project belongs to exactly one Wave. Projects do not contain Projects.
Only a Task owns a delivery worktree.

The shared durable types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs): `WorkRef`, `WorkStatus`,
`FlowPosition`, `Steer`, `Ask`, `Home`, and `Placement`. Wave, Project, and Task
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

Flow YAML is the authored definition.
[`FlowPosition`](../../rust/loopflow/src/durable.rs) is the durable playhead for
Work. A process reads the current position, executes one boundary, and advances
only after that boundary returns the required result.

Direct TTY flows can use the current conversation for a human node. Headless
Task flows create a typed Ask, park the playhead, and advance only after an
explicit result.

## Run a durable Work boundary

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
inputs, Flow position, Task worktree, or provider observations. The next
process resumes from those facts and launches a fresh Run when needed.

Project and Task controller implementations live in
[`project/runner.rs`](../../rust/loopflow/src/project/runner.rs) and
[`task/runner.rs`](../../rust/loopflow/src/task/runner.rs). Wave listener and
resident behavior lives under [`wave/`](../../rust/loopflow/src/wave/).

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

Monotonic phase, iteration, and cursor fields prevent an older process from
rolling progress backward. Domain-specific races use narrower fences: Ask
claim ids, exact PR heads, landing generations, or OS locks.

## Steer

```bash
lf task steer INF-123 "keep the public name"
lf work steer task task_... "show the failing fixture"
```

A Task or Project Steer is ordered authored input addressed to stable Work. A
stopped controller is relaunched; a running controller reads it at its next
boundary. The receipt proves storage, not that a provider read or applied the
correction. Wave Chat has a separate optional live transport.

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

## Resident planning

```text
lfd
  `-- Wave listener
        |-- conversation and event journal
        `-- resident loop
              `-- Project controller
                    `-- Task controller in managed worktree
                          `-- Flow Skill boundary
```

The Wave listener owns its HTTP surface, journal, and the resident child it
directly spawned. The resident, Project controller, Task controller, and Ask
runner are separate launch loops because their recovery and settlement rules
differ. They share execution components rather than one universal runner. The
resident refreshes portfolio evidence and chooses the next useful Project
boundary. Project Work refreshes its definition, KRs, metrics, and Tasks before
deciding. Task Work executes its Flow and delivery steps.

Deterministic controller session names reduce duplicate local launches. They
are supervision policy, not durable Run ownership. If a controller or provider
disappears, the parent records resumable planning failure and returns judgment
to the next boundary.

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
[Homes and processes →](homes.md) explains how resident loops are placed and
supervised.
