# Superseded first pass — execution decentralization

This draft was written before the scope was narrowed to execution and before the
two independent narratives were compared. The current synthesis is
`scratch/execution-architecture-synthesis.md`; retain this file only as the
research trail required by the originating Run.

## Scope and confidence

There is no formal chapter start record. This report therefore uses the first
recoverable execution-design boundary as its opening: the July 18 merge that
deleted Project and Task Session controllers in favor of one Run execution path.
It closes on September 22, 2026. Earlier implementation history is used only
where a dated Task or Wave memory describes the state being removed.

This is an execution report. The Wave/Project/Task planning redesign is outside
scope except where stable Work hands purpose to the runtime.

Coverage is incomplete. The repository retains strong dated design statements,
Task contracts, merged-PR receipts, current architecture documentation, and
chapter-review observations. It does not retain a single beginning-of-chapter
runtime snapshot or a complete mapping from every intermediate design to its
deployed duration.

## Finding

The execution redesign made two moves, not one.

First, it **centralized execution**. Project and Task Session controllers,
duplicate runners, body generations, mirrored transitions, and ambient authority
were collapsed into a shared `Work -> Epoch -> Run -> Launch` path. The July 18
contract called Run “the only execution authority” and Launch “the only
provider/process lifetime.” This was a necessary deletion: the old runtime had
several objects claiming to be the same executing thing.

Then the design **decentralized authority away from Run itself**. By the end of
the interval, a Run is an immutable, Home-local evidence record. It does not own
Work, prove liveness, authorize a mutation, confer a signal capability, or become
a universal index entry. Stable Work preserves purpose; a controller owns one
durable playhead transition; the direct spawner owns its child handle; the
provider owns conversation history; the kernel proves current process facts; a
Home owns local records and credentials; narrow locks and exact-head receipts own
specific races.

The result is not a peer-to-peer runtime and not “no center.” It is a runtime
with **several local centers, each authoritative for one kind of fact**, joined by
stable identifiers and explicit projections. Decentralization here means that
no record is allowed to borrow authority from another domain merely because the
two refer to the same execution.

## The beginning: one problem represented several times

The July 18 Task contract reconstructs the opening state unusually clearly.
Project and Task execution still had Session-controller identities, separate
status types, child write leases, body generations, authority-bearing environment
variables, mirrored Run transitions, and duplicate runners and stores. A
provider's resumable conversation, a long-lived controller, and an executing
process were all liable to be called a Session. Recovery could reconstruct
control from a surviving handle or ambient environment.

This was distributed in the unhelpful sense: the same lifecycle was mirrored in
several places, with no crisp answer to which one could advance, stop, recover,
or complete work.

The first redesign answered by collapsing these paths. On July 18:

- [PR #1099](https://github.com/loopflowstudio/loopflow/pull/1099) deleted the
  Project/Task Session-controller model and made Run the shared execution path.
- [PRs #1098](https://github.com/loopflowstudio/loopflow/pull/1098) and
  [#1100](https://github.com/loopflowstudio/loopflow/pull/1100) made a Run
  recoverable across provider accounts rather than binding continuity to one
  process or credential route.
- The product model settled on separate nouns: Work as stable identity, Run as a
  bounded execution period, AgentInvocation as one provider/process attempt, and
  Turn as one observed provider boundary.

That convergence removed duplicate authority. It did not yet settle where
authority should live.

## The middle: dogfood exposed Run as another overloaded center

The next failures were not random defects. Each showed one execution concern
being used as proof for another.

### Recorded state was presented as current process truth

LOO-229 documented terminal failure events replayed as present-tense failures,
Wave journals reporting failed while child work ran, and a provider invocation
remaining “running” hours after its process died. The runtime had durable history
but no disciplined boundary between history and liveness.

[PR #1225](https://github.com/loopflowstudio/loopflow/pull/1225) moved status
toward the rule that a past event is history unless refreshed against current
evidence. The end-state rule is stricter: an unterminated Run means only that no
terminal receipt was written. It does not prove that anything is alive.

### Telemetry could terminate or strand the work it observed

LOO-237 found that Run context lived in thread-local state and disappeared when
execution crossed an async thread. LOO-238 found concurrent trace writes hitting
a SQLite lock, marking capture partial, and then killing an otherwise healthy
Task Run. LOO-265 found recovery and local promotion blocked by missing trace,
journal, Invocation, or Exec receipts even when the Task process itself was
absent.

[PR #1214](https://github.com/loopflowstudio/loopflow/pull/1214) made execution
context explicit across User, Wave, Project, and Task runners.
[PR #1226](https://github.com/loopflowstudio/loopflow/pull/1226) prevented normal
trace contention from killing Task execution.
[PR #1237](https://github.com/loopflowstudio/loopflow/pull/1237) made Task
recovery depend on direct process truth and demoted incomplete telemetry to a
diagnostic condition.

This is the decisive decentralizing move: observation stopped being a control
plane.

### Process location was confused with durable identity

LOO-253 showed resident and default agents mutating canonical main, so one
long-lived execution context could jam later Task placement. PR #1230 moved main
agents into dedicated worktrees. Earlier Task isolation work had already made a
Task's worktree and PR survive provider interruption instead of belonging to the
current process.

The same principle later governed Homes. Placement selects where Work belongs;
it does not assert that a process exists. A Home is stable machine authority,
while its SSH route is replaceable. Remote access executes the same reader on
that Home; there is no implicit fleet fan-out and no central Run database.

### Session returned, but with a smaller meaning

Deleting Session as an executor did not erase the real thing providers call a
session. By August 30, the term had returned with a narrower contract: one
provider-native resumable conversation or one unresolved human boundary. The
provider owns history; a Task FlowPosition or blocked Ask owns the human
decision; tmux is only a detached PTY cradle; the Mac app only presents the
projection.

This is a useful measure of the redesign's maturity. The architecture stopped
deleting a real external concept merely because the old internal aggregate was
wrong. It retained Session without giving it Work identity, process authority,
or lifecycle ownership.

## The end state: execution authority is deliberately plural

| Concern | Opening model | End-of-interval owner |
| --- | --- | --- |
| Purpose across attempts | Session/controller identity and mirrored state | Stable Work and its durable Flow position |
| One provider launch | Run mixed execution identity and control | Immutable Home-local Run record |
| Provider/process attempt | Launch/body/controller variants | Harness child plus provider-specific AgentInvocation evidence |
| Conversation continuity | Loopflow Session aggregate or resume handle | Provider-native Session id/history |
| Human checkpoint | Session/controller path | Task FlowPosition or Run-owned Ask boundary |
| Current liveness | Persisted “running,” containment, tmux, or journal state | Current OS facts joined to exact local receipts |
| Stop/cancel capability | Reconstructed from Run, Session, PID, or environment | The process that directly spawned the child; otherwise no capability |
| Machine placement | Caller context or service topology | Stable Home identity plus explicit Work placement |
| Execution history | Shared ledger and service-backed reads | Append-only Run bundle on the launching Home |
| Read model | Long-lived service or database treated as lifecycle truth | Disposable projections rebuilt from authoritative records |
| Coordination | Database bus, channels, and process nudges | Durable Work input and Home-owned Ready scanning; nudges only reduce latency |
| Delivery race | Broad writer/controller ownership | Task worktree plus narrow Git lock, exact PR head, or landing generation |

The current execution path now has no durable planning prerequisite. A direct
Skill can launch with only argv, repository context, provider routing, and a
Home-local Run manifest. Work attribution can enrich the prompt but does not
reserve the Work or authorize a mutation. Execution accepts preassembled
context; it does not reach back into controller state to discover what it is.

Before spawning a provider, the harness atomically publishes a Run manifest.
Optional context and event streams may follow, and the first terminal receipt
wins. Telemetry uses a bounded queue and may degrade without holding settlement
open. A reader scans the bundles into a disposable `RunSnapshot`; there is no
authoritative Run index to repair.

Process control is intentionally even more local. Run records contain no
`owner.json`. A Run id, Work id, PID, parent Run, tmux name, or telemetry row is
insufficient to signal a process. The direct spawner may cancel the child handle
it owns. Durable cross-process control would require a birth-validated process
scope with PID, boot/Home identity, and exact process group, revalidated at every
signal. Rather than pretend this capability exists, the current architecture
marks it absent.

The current architecture checker passes all discovered surfaces: 46 public API
families, 9 process boundaries, 32 SQLite owners or mirrors, 6 read projections,
19 HTTP routes, 6 provider edges, 26 subprocess edges, and 5 compatibility
seams. That proves the present ownership map is internally enumerated. It does
not prove that the runtime behaves correctly under every failure.

## What decentralization improved

**Recovery no longer requires preserving the process that started the work.**
Work, its playhead, Task worktree, PR chain, and authored input survive provider
replacement. A fresh controller can rebuild one boundary from durable facts.

**A local failure has a smaller blast radius.** Optional telemetry cannot turn a
successful provider result into failure. One failed Wave start does not kill
successful siblings. A process on one Home does not become globally controllable
because another machine can see its Run id.

**The runtime can state what it does not know.** Unterminated is not live;
placement is not execution; parentage is not control authority; provider finality
is not inferred; a missing receipt is not zero. These are negative guarantees,
but they remove several classes of unsafe recovery.

**Interfaces can consume execution without owning it.** CLI, Mac, status, and
usage views project the same local facts. Closing a pane does not settle a
Session. A UI cannot make a stale process current merely by retaining it.

## What remains unresolved

The chapter succeeded more at **separating authority** than at **reconstructing a
coherent story across those authorities**.

The chapter review found six settled records without context, 35 without a
replayable launch contract, 21 without token evidence, 76 without cost, and no
recorded unattended replay cohort. An earlier ledger outage silently dropped
29.2 hours of writes, and fresh-database tests did not detect the long-lived
schema drift. The Product evidence also found cases where PM, Work, Run, Git,
and presentation state disagreed, including a terminated provider shown as live.

Those are not arguments for putting execution back into one database. They show
that the observation layer still assumes a more centralized runtime than the
runtime now is. A trace that records only the provider stream cannot explain the
Home placement decision, the controller boundary that launched it, the exact
process ownership available at failure, or the delivery fact that followed.
Conversely, a fleet status assembled from old controller and telemetry rows can
misstate current OS truth.

There is also a deliberate capability gap. Cross-process stop and interrupt are
not generally available without exact ownership evidence. That is safer than
PID inference, but the user-facing result may be “unknown; cannot safely signal.”
The architecture needs to decide where a real process-owner receipt is worth
introducing rather than letting monitoring imply the capability.

## Consequence for the next Intelligence chapter

Monitoring and tracing should be rebuilt as a **join over execution authorities,
not a second execution authority**.

The useful unit is an execution chain:

```text
durable Work boundary
  -> Home placement and runtime artifact
  -> immutable Run launch contract
  -> exact provider account/session/attempt
  -> owned or merely observed OS process
  -> provider events, usage, and terminal proof
  -> resulting Work transition and delivery evidence
```

Each edge needs a stable identifier, timestamp, source owner, freshness, and
explicit missingness. The read model may aggregate across Homes, but it must be
rebuildable and non-authoritative. When two sources disagree, the product should
show the disagreement rather than collapse it into `running`, `failed`, or
`healthy`.

The first proof should not be a dashboard. It should be a small set of real
failure reconstructions on the long-lived store:

1. A provider process dies after publishing a manifest but before terminal
   settlement.
2. Trace capture degrades while the Task still lands.
3. A controller dies and another resumes the same Work boundary.
4. A Run moves across provider accounts while preserving one causal record.
5. A remote Home is unavailable, so fleet status distinguishes unavailable
   evidence from stopped work.
6. A human Session survives its first client and resumes from provider history
   without becoming Work or process authority.

For each case, the trace should answer: what was intended, what actually ran,
where it ran, what could legally control it, what evidence is missing, what
transition followed, and which source supports each claim.

That would complete the chapter's architectural move. Execution is already
decentralized by authority. Intelligence now has to make that distribution
legible without flattening it back into the false single lifecycle the redesign
removed.

## Observations, interpretations, and open hypotheses

### Observed

- LOO-196 and PR #1099 explicitly made Run the sole executor and removed
  Project/Task Session-controller authority.
- Current architecture explicitly defines Run as Home-local launch evidence and
  denies it Work-mutation or signal authority.
- Current controller documentation says controllers share execution components
  rather than use one universal runner.
- Current Run reads are Home-local; remote reads execute on the named Home; no
  central Run database exists.
- August incidents show stale persisted state, ambient Run context, trace
  contention, and telemetry completeness affecting execution control.
- The September baseline review found material holes in settled record context,
  replay contracts, usage, cost, and replay evidence.

### Interpretation

- The chapter's main execution story is a transition from duplicated lifecycle
  aggregates, through a single Run executor, to capability-shaped authority.
- The later model is decentralized because authority follows the actor that can
  actually prove the fact, not because every component is autonomous.
- Monitoring lagged the runtime redesign and is now the highest-leverage place
  to finish it.

### Open hypotheses

- A non-authoritative cross-Home execution index may be necessary for a useful
  fleet view, but it should be disposable and expose source freshness.
- Some stop/recovery paths may justify a durable birth-validated `ProcessOwner`
  receipt; the current evidence does not show that every provider or controller
  needs one.
- The remaining runtime contradictions may be mostly missing joins rather than
  incorrect domain models. Real-store failure reconstruction should test that
  before another schema redesign.

## Evidence sources

- `wave/product/MEMORY.md` — Work/Run/AgentInvocation/Turn model, runtime
  failures, Session revision, daemon-less reads.
- `wave/infrastructure/MEMORY.md` — durable-state-not-message-bus rule,
  Home-owned Ready scanning, process/control boundaries, Task recovery lessons.
- `wave/intelligence/MEMORY.md` — ledger failures, trace/span contract, complete
  local record direction, real-store testing constraints.
- `docs/architecture/execution.md` — current launch, record, settlement, replay,
  and signal-authority contracts.
- `docs/architecture/planning.md` — stable Work and boundary-based controller
  execution.
- `docs/architecture/homes.md` — Home locality, placement, process observation,
  and remote execution.
- `docs/architecture/data.md` — distributed truth map and non-authoritative
  projections.
- `docs/architecture-reference.md` — checked current ownership inventory.
- `lf activity --since 100d --wave product|infrastructure --json` — dated PR
  merge receipts.
- `lf status product|infrastructure --json` — retained Task contracts.
- `scratch/chapter-review.html` — baseline evidence gaps through September 22.
