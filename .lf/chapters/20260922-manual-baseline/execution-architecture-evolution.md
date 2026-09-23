# How the execution model changed

## Scope

This is a reconstruction, not a formal chapter comparison: no chapter start
snapshot exists. “Before” means the execution model observable around the start
of summer 2026; “after” means the repository state and operational evidence on
22 September 2026. Planning changes are omitted except where durable Work hands
an objective to an executor.

## Narrative

At the beginning of summer, Loopflow's runtime was built around keeping an
execution environment intact. The Mac client could reach `lfd` remotely;
long-lived Wave, Project, and Task controllers had Session-shaped identities;
provider continuity, process containment, interactive attachment, and Work
lifecycle were represented in overlapping types. Environment variables could
help determine what role a process believed it had. Branch and worktree names
carried information that later code tried to recover. The system knew a great
deal about the container surrounding an agent, but those facts did not cleanly
answer four different questions: what work persists, what is running now, what
can be controlled, and what happened historically.

That ambiguity produced practical failures. A renamed or relocated worktree
could damage identity. A controller record could outlive its process and remain
“running.” A provider conversation could be resumable while the Loopflow
controller that created it was gone. Recovery code had several plausible
objects—Session, Run, body, lease, environment, tmux—to consult, and therefore
several ways to make the wrong object authoritative.

The first major response was subtraction. The July 18 LOO-196 change deleted
ProjectSession and TaskSession as controller identities, removed their duplicate
status and runner paths, and converged Wave, Project, and Task execution on a
shared `Work -> Epoch -> Run -> Launch` model. Run became the single executor;
Launch represented one provider/process lifetime; stable Work and Epoch history
survived replacement. Adjacent work let one Run recover across accounts and
providers instead of binding it to the first credential route.

This was the cleanest design available at that moment because it ended a split
brain. It also concentrated too much meaning in Run. During late-July and August
dogfooding, the seams became visible.

One seam was time. Persisted failure and “running” records were repeatedly shown
as present truth after the underlying situation changed. In August, status could
replay a July credential failure even while current runners selected healthy
accounts; a dead provider invocation remained live; a Wave journal could say
failed while work beneath it proceeded. The fix was not a better Run status
enum. The design began separating historical evidence from current observation:
an old event says what was last recorded, while process liveness must be checked
against current OS evidence.

Another seam was instrumentation. Run context was held in thread-local state and
could disappear when execution crossed a Tokio worker thread. Concurrent trace
writes could hit a SQLite lock, mark capture partial, and then terminate healthy
Task work. Later, Task recovery and local promotion could be blocked because
trace, journal, Invocation, or Exec receipts were incomplete even when the
relevant process was plainly absent. These failures forced a stronger rule:
monitoring may describe or degrade an execution, but it may not become the fact
that decides whether the execution exists or can proceed. Execution context was
made explicit across async boundaries; trace contention was made nonfatal; live
process truth displaced telemetry completeness in recovery decisions.

A third seam was physical location. Resident and default agents working directly
on canonical main allowed one long-lived context to contaminate later Task
placement. Task worktrees already showed the better model: durable work and its
delivery state could survive the agent process, while each concurrent writer
belonged in a distinct worktree. Main agents moved into worktrees too. At machine
scale, the same idea became Home: stable machine identity is separate from its
current SSH route, and Work placement is separate from proof that a process is
live.

By the end of August, the design had stopped looking for one durable object that
could stand in for an executing agent. It assigned different meanings to
different things.

- Work preserves purpose, status, input, and a controller playhead across
  processes.
- A controller reconstructs one boundary from current durable facts, executes
  it, records one monotonic transition, and can disappear.
- A Run records one mediated provider launch on the Home that launched it.
- The harness directly owns the child process it spawned. No later observer may
  infer signal authority from a Run id, Work id, PID, tmux name, or parentage.
- The provider owns resumable conversation history.
- A Session once again names that real provider conversation or an unresolved
  human boundary, but no longer names the executor or the Work.
- The kernel supplies current local process facts.
- A Home owns its credentials, processes, Run records, and local services.
- Read surfaces combine these facts into disposable projections; they do not
  inherit the authority of what they display.

The Run record itself changed accordingly. Before the provider starts, Loopflow
publishes an immutable manifest containing the launch contract and attribution.
Context and events are append-only evidence; the first terminal receipt wins.
Optional telemetry uses a bounded side channel and cannot prevent settlement.
`lf runs` and `lf usage` scan Home-local bundles directly. There is no central
Run database and no authoritative index that must be repaired before execution
can continue.

The same reduction happened around the daemon. The Mac's former HTTP-to-`lfd`
data path was deleted; local reads now execute the same `lf --json` projections
as the CLI. `lfd` remains a Home service keeper and webhook receiver, not the
universal source of runtime truth. Remote reads explicitly run on the target
Home rather than pretending that one local view has global knowledge.

The resulting architecture is less convenient in one honest way: sometimes the
system cannot safely control what it can see. An unterminated Run is not proof
of a live process. A visible PID is not sufficient authority to signal it. A
remote Home may be unavailable rather than stopped. This produces more
`unknown` states, but it avoids making destructive recovery decisions from stale
or incidental evidence.

The redesign is not finished. It is now easier to state who owns each fact, but
harder to reconstruct one complete account of an execution. The chapter review
found settled records without context, replay contracts, token counts, or cost,
and no unattended replay cohort. Product evidence still found disagreement
between Work, Run, Git, process, and presentation state. The runtime's conceptual
boundaries moved faster than the observation layer that should join them.

The next technical step is therefore not another executor. It is a trace model
that follows one execution across those boundaries without gaining their
authority: durable Work boundary, Home and artifact, Run manifest, provider
attempt and native Session, exact process evidence, terminal receipt, resulting
Work transition, and delivery outcome. Each edge needs its source, time,
freshness, and missingness. A fleet index may be useful, but it must remain a
rebuildable projection rather than the object that decides what is alive.

## Is decentralization the right description?

Partly. It accurately describes the end-state placement of execution evidence
and authority: Runs are Home-local; there is no central execution database;
providers retain their own Sessions; direct process control stays with the
spawner; remote Homes answer for themselves. But it does not explain why the
architecture changed.

The stronger through-line is **separating identity, continuity, control, and
observation after repeated failures caused one to impersonate another**. Some
parts were centralized along the way: duplicate Session controllers collapsed
into one execution path; one Run-record format replaced competing histories;
one CLI projection replaced Mac-owned lifecycle logic. Other parts were pushed
outward to the actor capable of proving them.

Decentralization is therefore a consequence and a constraint, not the original
goal. The design became more distributed because truthful execution authority is
inherently local. The next chapter will show whether that distribution is a
coherent architecture or merely fragmentation: monitoring and tracing must make
the joins legible without recreating the false single lifecycle that the summer
redesign removed.

## Dated anchors

- **30 June:** remote Mac execution still centered on HTTPS access to `lfd`;
  client/service boundaries reflected the earlier service-shaped runtime.
- **18 July:** PRs
  [#1098](https://github.com/loopflowstudio/loopflow/pull/1098),
  [#1099](https://github.com/loopflowstudio/loopflow/pull/1099), and
  [#1100](https://github.com/loopflowstudio/loopflow/pull/1100) converged on Run
  execution and cross-provider recovery.
- **21–23 July:** startup, Work recovery, fleet trace, worktree isolation, and
  Home/release changes began testing the shared runtime against real failures.
- **20–28 August:** explicit Run context, nonfatal trace contention, truthful
  status, worktree-isolated main agents, watched landing, and process-truth-based
  recovery corrected the first centralized Run model.
- **30 August:** native Sessions were retained as provider/human continuity, not
  restored as executor identity.
- **22 September:** the checked architecture enumerates every discovered owner,
  projection, process boundary, provider edge, and subprocess edge, while the
  chapter evidence still reports incomplete trace and replay coverage.

## Evidence boundary

Observed facts come from dated Wave memory, current Task contracts, `lf activity`
merge receipts, current architecture documentation, the checked architecture
map, and the baseline chapter review. The causal narrative—especially that Run
became temporarily overburdened—is interpretation. No formal summer snapshot
survives to prove that every deployed machine occupied exactly the reconstructed
state at the same time.
