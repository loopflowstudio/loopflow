# Loopflow core architecture

This is the executable design for the core cutover. The implementation plan
owns sequencing and deletion accounting; this document owns the data model,
transactions, and observable behavior.

## Contract

```text
Work -> Epoch -> Run -> Launch -> optional Turn
                    \-> Wait

Steer advances the current Basis.
Run owns execution authority.
Launch owns one provider or TUI process lifetime.
Turn records one observed provider exchange when the harness exposes it.
```

Wave, Project, and Task are stable Work identities. Execution never creates a
new Work id. A terminal restart creates an Epoch. A wake creates a Run. Provider
or account fallback creates a Launch. A provider interaction creates a Turn
only when Loopflow can observe that boundary.

The core has no `Session`, `Body`, `Actor`, writable `Ack`, `Handle`, `Block`,
`Sleep`, `Interaction`, or replacement message. Those names describe current
implementation machinery, not distinct product facts.

## Durable types

```rust
enum WorkRef {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

struct EpochRef {
    work: WorkRef,
    n: u32,
}

struct Basis {
    epoch: EpochRef,
    rev: u64,
}

enum EpochState {
    Open,
    Done,
    Abandoned,
}

enum Author {
    Human,
    Run(RunId),
}

struct Steer {
    id: SteerId,
    basis: Basis,
    author: Author,
    text: String,
    issued_at: Timestamp,
}

enum SendVia {
    Live,
    Seed,
}

enum SendState {
    Sending,
    Sent,
    Failed,
    Unknown,
}

enum RunState {
    Reserved,
    Active,
    Stopping,
    Ended,
}

enum LaunchState {
    Starting,
    Live,
    Stopping,
    Ended,
}

enum TurnState {
    Starting,
    Active,
    Succeeded,
    Failed,
    Interrupted,
    Unknown,
}
```

All ids are newtypes. Public DTOs carry required fields or explicit `Option`;
they have no wire defaults.

`WorkRef` is a typed union over the existing Wave, Project, and Task stores. Do
not add a generic Work table that duplicates their identity or parentage.

## Stored facts

The live schema has one owner for each fact:

| Fact | Owner |
| --- | --- |
| Wave/Project/Task identity, parentage, authored fields, external binding | existing domain row plus immutable truth events |
| current and historical pursuit | `epochs` |
| ordered prompt-relevant change | `epoch_revisions` plus one typed source row |
| authored direction | `steers` |
| delivery attempt | `steer_sends` |
| execution authority | `runs` |
| provider/process lifetime and continuation token | `launches` |
| observed provider boundary, fixed Basis, outcome, and usage | `agent_turns` extended to the product Turn contract |
| durable reason another Run is not useful yet | `waits` |
| machine identity and mutable routes | `homes` and route observations |

`Epoch.current_rev` is the only input cursor. `epoch_revisions` contains exactly
one row for each allocated revision and names its typed source. Steers, decisions,
approvals, selected external evidence, and authored truth events each reference
their revision. There is no second directive version, steer sequence, or
incorporation flag.

An authored truth event is an immutable normalized snapshot or a reference to
an immutable ingestion event. It is not a rendered prompt copy. Rendering reads
canonical domain facts selected by the revision history.

The minimum relational constraints are:

- one nonterminal Epoch per Work;
- unique `(work, epoch_number)`;
- unique `(epoch, rev)` and gap-free revision allocation under the Epoch write
  lock;
- every typed input row owns exactly one `(epoch, rev)`;
- at most one non-ended Run per Epoch;
- at most one non-ended writable root Launch per Run;
- at most one nonterminal root Turn per Launch;
- unique `(steer, turn, via)` Send attempt;
- at most one unresolved Wait for an Open Epoch with no active Run.

SQLite partial unique indexes enforce the active-slot constraints. Foreign keys
enforce Epoch, Run, Launch, Turn, and Home containment. Application checks do
not stand in for constraints that SQLite can express.

## Basis

Every prompt-relevant durable write allocates the next revision in the same
transaction as its source row:

```text
lock current Open Epoch
validate caller, optional if_basis, and active Run lease where required
next = current_rev + 1
insert epoch_revisions(epoch, next, kind, source)
insert or update the typed source fact with Basis(epoch, next)
set Epoch.current_rev = next
resolve a matching Wait, and reserve a Run if this fact wakes Work
commit
```

No side effect occurs before this commit.

A structured Turn records one immutable starting Basis before provider input
begins. An opaque TUI records the same Basis on its Launch because Loopflow
cannot observe inner Turns. Live delivery never changes an active boundary's
Basis.

The applied Basis is derived from the greatest successful root boundary Basis
in the Epoch. There is no Ack mutation or Ack table. Failed, interrupted, and
unknown boundaries apply nothing.

A Steer at revision 9 is outstanding while the applied revision is less than
9. A live Send to a Turn that began at revision 8 may improve latency, but the
Steer remains outstanding. A later Turn begins at revision 9 or later, receives
the Steer in its seed, and may apply it by succeeding.

## Direction and typed input

`steer(work, text, if_basis)` is the only authored-direction operation. Human
to Wave and active parent Run to child Work call the same function. “Do this
instead” is another append-only Steer. Preemption is `interrupt` followed by a
normal Steer.

Decisions, approvals, CI incidents, external observations, and lifecycle
controls are not Steers. A typed decision or approval persists its semantic
answer, allocates its revision, and resolves its Wait before Loopflow attempts
to notify a provider. Live or seeded prose is optional explanatory delivery;
it never owns resolution.

CI is typed evidence:

```text
observe failed checks
-> ensure one CiIncident for repository, PR head, and failure set
-> allocate an execution-input revision if reconciliation selects it
-> reserve one bounded repair Run in the same transaction
```

Duplicate observations resolve to the same incident and cannot reserve a
second Run.

## Authority

Authorization is request context, never a caller-supplied domain field:

```rust
enum ControlCtx<'a> {
    Human(&'a AuthenticatedLocalRequest),
    Run(&'a RunLease),
}
```

- a trusted Home-local Swift or CLI transport constructs Human context;
- an active Run lease may mutate only immediate child Work;
- the target must still match the caller's observed Epoch/Basis;
- Linear and GitHub ingestion credentials append only their typed events;
- the Home keeper performs only recovery transitions;
- an environment variable may transport an opaque lease but absence never
  selects Human authority.

`Author` is Steer provenance after authorization succeeds. It is not a
privilege model and cannot be submitted on the public wire.

## Epoch transitions

Epoch state has three values. Waiting and execution health do not add states.

| From | Operation | Preconditions and transaction | To |
| --- | --- | --- | --- |
| none | start | Work has no Epoch; capture current truth as revision 0 | Open |
| Done/Abandoned | restart Fresh | prior Epoch terminal; no active/unreaped Run; capture current truth as revision 0 | new Open Epoch |
| Abandoned Task | recover from Basis | same physical-workspace safety checks; prior Basis exists; capture current truth and recovery reference | new Open Epoch |
| Open | commit done | current Run proposed done; successful boundary Basis equals current Basis; domain closure passes; containment absent; no newer input wins transaction | Done |
| Open | abandon | authenticated request; current Run stopped and containment absent | Abandoned |

Sleep, wake, retry, provider handoff, phase changes, PR rotation, and temporary
failure stay inside the same Open Epoch.

Fresh restart carries stable Work identity, parentage, external binding, and
history. It does not carry Steers, Runs, Waits, continuation tokens, lifecycle
cursors, or applied Basis.

Task recovery may retain its checked workspace and PR lineage and may reference
a selected prior Basis as reconstruction evidence. The new Epoch still starts
at revision 0; old revisions never grant current authority.

## Run transitions

A Run is one wake-to-wait authority period. It is not a provider process or a
single model invocation.

| From | Operation | Durable step before side effects | Terminal condition |
| --- | --- | --- | --- |
| none | `reserve` | insert Reserved Run with trigger and lease hash; set Epoch active slot | one Run owns the slot |
| Reserved | launch receipt | record executor/containment intent, then start outside transaction | Active after owner proves it holds the lease |
| Reserved/Active | `stop` | set Stopping and revoke lease | Ended only after every owned containment unit is absent |
| Active | `advance` to Wait | record boundary outcome; insert Wait; clear active slot and end Run in one transaction | no newer waking input exists |
| Active | `advance` to another boundary | record outcome and next fixed Basis before provider side effect | same Run continues |
| Active | commit done | stop containment, then run the completion transaction | Epoch Done and Run Ended atomically |
| Stopping | reap observation | record containment absence | clear active slot and mark Ended |

`reserve`, `advance`, and `stop` are the internal domain operations. Boot,
activation, progress, interruption, and reap are receipts or phases inside
them—not additional public lifecycle APIs.

Recovery never reactivates or mutates a failed Run. After the old Run is Ended,
reconciliation reserves a new Run with `retry_of`. Automatic recovery is legal
only when durable effect evidence says replay is safe. Unsafe or unknown effects
produce a typed Wait.

### Input versus Run end

Appending a waking input and ending a Run serialize on the Epoch row:

- if input commits first, `advance` sees a newer revision and continues or
  leaves it visible for the next Run;
- if `advance` commits first, it records its Wait and clears the slot; the input
  transaction resolves that Wait and reserves the next Run;
- there is no state where input is durable but no Run or Wait can discover it.

### Stop versus recovery

Revoking a lease fences logical writes immediately but does not free the active
slot. The slot remains occupied in Stopping until process group or tmux absence
is observed. A new Run cannot overlap a stale writer.

## Launch and Turn transitions

A Launch owns provider, model, account, Home, containment identity, and an
optional opaque continuation token. Fallback changes Launch, not Run, Epoch, or
Work.

| Launch transition | Ordering |
| --- | --- |
| create Starting | persist route, Basis for an opaque TUI, and containment intent before spawn |
| Starting -> Live | record actual containment and provider readiness |
| Starting/Live -> Stopping | fence new input before requesting provider/process shutdown |
| Stopping -> Ended | require containment absence; record outcome and optional continuation token |

A persistent provider process may be live but quiescent between Turns. It may
remain within an active Run. Work completion first stops it and proves
containment absence.

For a structured provider, create the Loopflow Turn with its Basis before
sending the initial prompt. Correlate the vendor Turn id when observed; never
use it as Loopflow identity.

| Turn transition | Meaning |
| --- | --- |
| Starting -> Active | provider accepted the boundary and correlation is known where available |
| Starting/Active -> Succeeded | normal root boundary observed; its fixed Basis becomes applied evidence |
| Starting/Active -> Failed | provider reported terminal failure |
| Starting/Active -> Interrupted | interrupt fence observed |
| Starting/Active -> Unknown | connection or process was lost after input may have begun |

Only Succeeded can support completion. Partial usage survives every terminal
outcome when the provider reported it.

Opaque TUIs have no synthetic Turns. Their Launch Basis is the execution
boundary. Process exit alone is not success; explicit `done`, handback, or
failure supplies the Loopflow outcome.

## Steer delivery

`steer` returns after durable creation. The controller may then optimize
delivery:

```text
persist Steer and revision
-> if an exact structured Turn is active, insert Sending Send
-> call send_current(turn, steer)
-> record Sent | Failed | Unknown, or Failed(NotSteerable) as a typed outcome
-> regardless of result, keep Steer outstanding until a later successful Basis
```

The adapter boundary remains:

```text
send_current(turn, steer) -> Sent | NotSteerable | Failed | Unknown
interrupt(boundary)       -> Ended | Fenced | Unknown
launch(seed, route)       -> Launch
observe(boundary)         -> Progress | Succeeded | Failed | Unknown
```

The stored Send maps `NotSteerable` to a terminal failed attempt with a typed
reason; it is not an extra durable state. `Unknown` is immutable and is never
retried against the same Turn. Failed or Unknown live delivery does not prevent
the Steer from appearing in a later seed.

At the next boundary, the controller reads all Steers above the applied
revision through the boundary Basis, renders one ordered seed projection, and
creates individual Seed Send receipts in the same transaction that fixes the
boundary Basis. A crash cannot leave an accepted Steer only in memory.

Provider acceptance proves transport, not incorporation. Codex may deliver to
the current Turn. One-shot Claude seeds another Turn. OpenCode may use live
delivery only where observed behavior meets the contract. An opaque TUI has no
current Turn to send to. These paths differ in latency, not Work semantics.

## Completion

`done(run, basis)` records a proposal tied to the current root boundary. It does
not directly mutate Epoch state.

The controller may commit Done only after:

1. the boundary succeeded normally;
2. proposal Basis equals that boundary's immutable Basis;
3. proposal Basis equals the Epoch's current Basis;
4. domain closure checks pass;
5. every Run-owned process group, tmux unit, background task, and observable
   native descendant is absent or fenced from writes;
6. the final transaction still sees the same current revision and active Run.

New truth, Steer, decision, approval, or selected evidence advances the Basis
and makes the proposal stale. A live Steer therefore cannot be blessed by the
older Turn that happened to receive it. The next successful boundary must start
from the newer Basis.

There is no explicit Ack call. There is no `try_complete` state. The proposal
is evidence; the fenced transaction is completion.

## Wait and status

An Open Epoch with no useful immediate execution records one typed Wait:

```rust
enum WaitOn {
    Input { after: Basis },
    Time { not_before: Timestamp },
    Event(EventRef),
    Child(WorkRef),
    Capability(CapabilityRef),
    Effect(EffectRef),
}
```

The fact that satisfies or invalidates a Wait records the resolution and, when
execution is useful, reserves a Run in the same transaction. There is no
generic unblock or deletion of Wait history.

Status is a projection:

| Durable/evidence facts | Projected status |
| --- | --- |
| Epoch Done | Done |
| Epoch Abandoned | Abandoned |
| Open plus active Run | Running, with independent health |
| Open plus unresolved Wait | Waiting on its typed fact |
| Open plus neither | Ready; keeper reconciliation should reserve or explain |

Run health (`Starting`, `Working`, `Stalled`, `Recovering`, `Dead`, or
`Unobservable`) is fresh evidence, not stored Work lifecycle. An unreachable
Home is not proof that its Run is dead.

An interactive human step is an opaque TUI Launch behind tmux. Swift derives
`AwaitingHuman` only from a live human-routed TUI Launch and opens it by
`LaunchId`, even if Swift was closed when it started. A parent-routed
non-blocking request is ordinary child Steer and never appears in human
attention. There is no Interaction or Handoff identity.

## Home and keeper

`HomeId` is durable machine authority. Hostname, socket, SSH route, and
reachability are mutable observations. Only the owning Home mutates its Work,
Runs, Launches, and Turns. Remote Homes may read replicated/probed status but
cannot time out and seize authority.

The Home keeper:

- finds Reserved Runs that missed their boot deadline;
- probes only Runs owned by this Home;
- invokes the controller's stop path;
- verifies containment absence;
- reserves safe recovery or records a typed Wait;
- publishes status evidence.

Moving Work to another Home is an explicit migration requiring no active Run.

## Provider-native subagents

Native subagents are not Work. They inherit the root Run's authority, workspace,
and completion obligation. Provider child ids are trace evidence when exposed,
not public control targets.

Run stop must fence every owned writer. Process-group or tmux containment is
the portable proof. A provider mode whose native child can continue mutating
after root containment dies is unsupported until it exposes a reliable stop
and status fence. Dormant provider conversations do not block completion.

## Reconstruction floor

Continuation tokens and provider transcripts are optional optimizations. A new
Launch renders from:

- current authored Work truth selected by Epoch revisions;
- outstanding Steers in revision order;
- typed decisions and approvals;
- selected external evidence and unresolved Wait;
- flow position and domain closure state;
- workspace path, git HEAD, PR/CI/review lineage;
- known Loopflow-mediated effects and unknown-effect receipts.

Losing a token, provider, account, or transcript starts another Launch in the
same Run when replay is safe. The renderer must either produce a sufficient
seed or name an exact typed Wait. It never silently emits an empty prompt.

## Migration

The cutover is one-way and has no dual-write compatibility mode.

1. Stop Wave residents and quiesce/reap every Project and Task executor. Refuse
   migration with an actionable list if any writer remains.
2. Mint stable local Project and Task ids and preserve their external bindings.
3. Group Session successor chains into Epochs only at terminal restart
   boundaries. Process generations, retries, and provider handoffs become Runs
   and Launches inside an Epoch.
4. Convert copied initial/revised directives into canonical truth events.
   Convert follow-up, replacement, and resume prose into ordered Steers.
   Convert decision, CI, review, and lifecycle variants into their typed facts.
5. Do not guess old incorporation. Seed current Open Epochs with all current
   required input outstanding, then restart them under the new controller.
6. Move provider continuation data to Launch. Preserve absent historical links
   as absent rather than inferring them from timestamps.
7. Verify foreign keys, unique active slots, current Work lookup, and copied
   dogfood reconstruction before dropping live Session/body/command tables.
8. Restart residents and let the Home keeper reserve current work.

Shipped migration files remain historical source. Old live tables, readers,
writers, DTOs, and compatibility parsing are deleted in the same branch.

## Normative race tests

Use deterministic barriers around transactions and side-effect boundaries:

- Steer commit versus Turn success;
- confirmed live Send versus controller crash before next seed;
- input revision versus done commit;
- input revision versus Run advance to Wait;
- reserve versus reserve;
- stop versus recovery reserve;
- provider send begins versus disconnect;
- decision persistence versus a seed-only provider blocked in its waiting tool;
- duplicate CI observation versus crash after Run reservation;
- fifty local SQLite writers versus receipt allocation.

No sleep-based race test is evidence for these guarantees.

## Next implementation slice: durable input spine

The next `lf code` pass owns one vertical invariant: **no authored or typed
input can be accepted without advancing a durable Basis, and no older boundary
can complete Work after that advance.**

It must:

1. add the stable Work/Epoch/revision persistence needed by this invariant;
2. add Steer and Send rows with durable-first delivery;
3. record fixed Basis on every structured root Turn and opaque TUI boundary;
4. derive applied Basis only from successful boundaries;
5. persist typed decision/approval before optional provider notification;
6. replace current Task and Project completion/directive checks with the Basis
   fence;
7. route Human and parent Run direction through the same Steer operation;
8. delete the ChildCommand/ChildDirective branches made obsolete by the cut.

This slice is not done with dormant tables, dual writes, or a ChildCommand
compatibility adapter. It is done when behavior traverses the new rows and:

- killing the controller after a confirmed live Send still seeds the Steer;
- a live Steer racing successful completion makes that completion stale;
- several Steers render once in order and apply together at the later Basis;
- an Unknown live Send is not repeated to that Turn and still seeds later;
- a seed-only blocked decision resolves from the typed write before prose;
- stale parent lease and stale Epoch/Basis mutations are rejected;
- current Task and Project control has no production reference to
  `ChildDirective`, replacement, follow-up, resume-message, or command decision
  variants;
- the migration succeeds on a copied dogfood database with no active writer;
- format, clippy, focused migration/race/controller tests, and affected DTO
  round trips pass.

Run/Launch keeper unification, Wait/interaction collapse, reconstruction drills,
OpenCode usage normalization, and final Session/body purge remain later slices
unless this cut makes a specific old branch deletable.
