# 5 Whys: Project Runs could not reliably resume child Tasks

## The Problem

A live Project controller followed `project/pursue` and called the ordinary
`lf task resume` path, but the mutation failed because its Run had no durable
Turn Basis; the child stayed parked even though the Project was otherwise able
to continue provider work.

## Chain

Missing Turn row → live Project did not require trace capture → control reused
best-effort evidence → tests constructed the authority they meant to prove →
parent-to-child mutation authority had no explicit owner

**Problem**: On 2026-08-19, Stability & Security iteration 36 recommended
resuming LOO-224, but `lf task resume LOO-224 --json` failed with `Run ... has
no durable Turn Basis for child control`. The refusal preserved the Task, PR,
worktree, and unresolved wait, but stranded the recommended action.

**Why 1**: The child-control validator queried `agent_turns` for the calling
Project Run and found no row whose Basis could authorize the Task mutation.

↳ *Could we have caught this earlier?* A fixture that launched the real
Project runner and invoked the public resume path would have failed. The tests
instead proved the validator against manually assembled Run, Invocation, and
Turn rows.

**Why 2**: A Project provider process was allowed to be healthy without a
durable Turn row. `CaptureHandle::begin` was the operation that wrote the
initial Turn, while Project capture was explicitly best-effort: missing runtime
context returned no capture (silently until the 2026-08-21 repair), while
capture-publication errors logged a warning and let the harness continue.
Later phases called `begin_turn_at` only when that optional capture existed.
Process replacement therefore had no invariant requiring child-control Basis
publication before provider work resumed.

↳ *What process allowed this?* Project liveness and trace availability were
reviewed as separate concerns, but the authority query quietly coupled them.
The 2026-08-21 runtime-context repair explicitly preserved Project-best-effort
capture without an end-to-end child-control proof.

**Why 3**: The 2026-07-22 child-fencing change reused the trace Turn's Basis to
fence stale Project direction. That Basis contained the right data, but the row
carrying it belonged to execution evidence, not to a control capability whose
issuance and refresh the Project controller owned.

↳ *What assumption was wrong?* “A live Project Run has a current durable Turn”
was treated as an invariant. The runner's error policy made the opposite true:
trace capture could be absent without making the Project fail.

**Why 4**: Coverage concentrated on mutation-sink safety. It proved that no
Turn was denied, a current Turn was accepted, stale direction was denied, and
unrelated parents were denied. It did not cross the issuer and consumer
boundary: initial Project launch, phase transition, controller-process
replacement, and ordinary `lf task resume` in one deterministic behavior
fixture. The validator was correct for states that the real runner was not
required to create.

↳ *Why was that assumption encoded?* Reusing an existing Basis avoided a new
authority concept and made the storage-level fence small. The simplification
was local: it hid the lifecycle contract between the Project runner, trace
capture, and Task command.

**Why 5 (Root)**: Parent-to-child mutation authority was inferred from generic
execution evidence instead of being modeled as explicit durable control state
owned by the immediate parent controller. With no first-class issuance,
refresh, recovery, and revocation contract, Project liveness and Task-control
authority could drift independently. The 2026-08-25 SQL-lifecycle deletion
then removed the accidental carrier entirely; its launch/replay proofs did not
exercise Project-to-existing-Task resume, so no replacement contract was
forced before the architecture changed.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 2 | Which exact 2026-08-19 condition omitted capture: missing runtime context, capture publication failure, or controller recovery? The contract allowed all three, so the answer does not change the root cause. | Medium |
| Why 4 | Which other parent-child mutation entry points still infer permission from optional execution evidence or caller attribution? | High |
| Why 5 | How should architecture-reduction reviews inventory authorization consumers before deleting their storage lifecycle? | High |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Issue an opaque Project child-control capability before provider launch, store only its hash, and require it on the existing Task-resume path before any mutation. | A healthy Project reaching pursuit without usable resume authority. |
| Structural | Bind the capability to exact Project Work, controller Run, flow position, and Steer frontier; refresh it at phase boundaries, replace it on process recovery, and scrub it before the child process starts. | Cross-Project control, stale-direction writes, superseded controllers, and authority leakage into the child. |
| Systemic | Treat execution records as evidence only. Require each parent-child mutation edge to name an explicit authority issuer and prove launch, phase transition, process replacement, stale direction, unrelated parent, retry, and lifecycle deletion through its public command path. | Future control paths whose validators are safe in isolation but whose controllers cannot reliably produce the required authority. |

## LOO-227 Implementation

- [x] Add durable Project child-control capability issuance, refresh, recovery
  replacement, authorization, and revocation in planning SQLite.
- [x] Check authority at `resume_task_async` entry before PR reconciliation can
  mutate state, then check again at the launch boundary.
- [x] Keep local User control intact while denying an in-Run caller with a
  missing capability, an unrelated Project, a stale Steer frontier, or a
  superseded controller Run.
- [x] Prove the ordinary resume path is idempotent and add a release-materialized
  fixture spanning phase transition plus controller-process replacement.

## Follow-up Designs

The incident exposed two broader control-model questions outside LOO-227's
parked-Task resume contract:

- Design Wave-owned authority across Wave→Project and Wave→Task before
  tightening shared controls such as `task steer` or `task run`. The mutation
  inventory and counterexample are recorded in `scratch/questions.md`.
- Design an architecture-reduction gate that requires a public behavior proof
  for every authorization consumer of a lifecycle or schema being deleted.

## Evidence

- Commit `026dc10c7` introduced `validate_control_caller`, which selected the
  latest `agent_turns` Basis for the supervising Project Run.
- In that implementation, `project/runner.rs` created the row through optional
  `CaptureHandle::begin`; capture failure warned and returned `None`, and later
  phase turns were recorded only under `if let Some(capture)`.
- Commit `9151b1596` repaired missing runtime context for User-launched Work
  bodies while explicitly preserving Project-best-effort capture. Its proof did
  not exercise Project child resume.
- Commit `5f7f66833` removed the SQL Invocation/Turn lifecycle and declared
  generic Run identity to be evidence and causality only. Its focused proofs
  covered launch, capture, replay, and accounts, not Project-to-existing-Task
  resume.
- The restoration command fixture now enters through the Project runner's
  production publication boundary, which creates the generic Run and exact
  control capability together before provider work. It crosses phase and
  process replacement through `resume_task_async`, returns the same Task Work
  on retry and recovery, and denies stale and superseded authority. The store
  fixture separately keeps unrelated and missing authority denied.
