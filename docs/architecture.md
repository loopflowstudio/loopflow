---
layout: default
title: Architecture
---

# Architecture

> **Moment of transparency — July 17, 2026**
>
> Loopflow now runs its durable input through Work, Epoch, Basis, Steer, Send,
> and the existing agent Turn ledger. Project/Task Sessions, body leases,
> lifecycle `ChildCommand`, duplicate runners, and separate Review/Handoff
> aggregates still own execution and attention. Run is therefore a normalized
> authority bridge, not yet the sole controller.
>
> This page is deliberately a working architecture ledger during that cutover.
> It keeps the current reality, target decisions, research evidence, and open
> questions together. Sections say **Current**, **Decision**, **Target**, or
> **Open** so aspiration cannot masquerade as shipped behavior.

## Product direction

Loopflow is a local API for agents to launch, observe, and steer other agents.
Its first customer is its creator's own work. The product earns generality by
surviving that dogfood, not by designing an enterprise distribution layer
before the core is legible.

Large-company deployment does not imply a central Loopflow server. Each Wave
has a Home. The Home owns its local state and execution authority. Other Homes
may discover and observe it, while mutations route to the owner. Monitoring and
steering many agents are projections over these decentralized authorities.

Three planning nouns remain distinct:

- **Wave:** durable operating context; owns memory, cadence, budget, chat, and
  judgment about which Projects matter;
- **Project:** measured bet inside exactly one Wave; owns definition, KRs, and
  closure criteria;
- **Task:** concrete implementation, investigation, document, or shipped change
  inside exactly one Project.

## Current implementation

**Current.** This is the system the code still runs:

```text
User -> Wave Chat -> Wave resident
                       |
                       v
                 Project Work / Session bridge
                       |
                       v
                   Task Work / Session bridge -> worktree -> serial PRs

Steer -> Epoch Basis -> provider boundary -> agent Turn
ChildCommand lifecycle -> body generation / Run bridge -> provider process
InteractionReview + InteractiveHandoff -> Swift/terminal attention
```

`lf` is the machine-wide CLI and JSON interface. `lf wave <name>` runs one
resident process with that Wave's listener, journal, cadence, memory, and
Project selection. Project and Task Sessions are local child processes sharing
SQLite. There is no global service.

Current decentralized truth is deliberately split by substrate:

| Substrate | Owns |
| --- | --- |
| local SQLite | this Home's runtime, credentials, receipts, and agent bus |
| append-only journals | Wave conversation and durable run narrative |
| Linear | shared Wave/Project/Task planning truth |
| GitHub | branches, PRs, checks, and merges |
| SSH | reach to another Home without a central Loopflow coordinator |

`lfd` is currently a small machine daemon for durable webhook ingress and
liveness probes. It hosts no Sessions and is not a control API. The target Home
runtime may consolidate local keeping and Work wakeups, but it does not turn
`lfd` or any other process into a company-wide authority.

The current runtime contains several overlapping representations:

- Project/Task Session identity still sits beside stable Work identity;
- body generation mixes execution ownership with provider-process lifetime;
- `ChildCommand` still mixes lifecycle controls and CI trigger delivery;
- Sleeping, Blocked, review, handoff, lease, and health states overlap;
- provider session ids sit on Work-shaped records;
- Wave, Project, and Task runners repeat reservation, settlement, and recovery;
- ambient environment can participate in inferring caller authority.

These are the implementation being replaced, not compatibility contracts.

### Foundations already changed

**Current.** Several prerequisites now match the target:

1. Provider steering is attempted against the exact active Turn. The adapter
   returns `Sent`, `NotSteerable`, `Failed`, or `Unknown`; there is no
   provider-wide `supports_steer` flag. Codex can live-send. The one-shot Claude
   CLI, OpenCode where incorporation is not proven, and opaque TUIs fall back
   to a later seed. Plain Steer never implies interrupt.
2. Observed Turn is the only additive spend grain. `lf usage`, `lf top`,
   `lf runs`, Doctor coverage, JSON, and Mac telemetry read one Turn query.
   Missing usage remains absent rather than becoming zero.
3. Provider accounts are the routing primitive. Access profiles are verified
   login venues owned by an account, not agent identity.
4. Revoked body recovery releases authority only after positive absence
   evidence; an unprovable probe remains fenced.
5. A current actionable CI incident can interrupt a parked Task Review once
   and settle before the background lifecycle resumes. This is the narrow
   implementation precursor to the generic control lane.
6. Authored Project/Task direction is only Steer. `ChildDirective`, directive
   versions, follow-up/replacement/resume prose variants, and writable Ack are
   gone. A confirmed live Send cannot consume the Steer; a successful later
   boundary Basis derives application.
7. Project and Task boundaries capture immutable Basis on `agent_turns`, and
   terminal completion rejects a stale or unapplied Basis.
8. Stable Project/Task Work rows and Epochs survive Session succession. The
   bridge imports exact legacy generations into Runs and closes an Epoch only
   after its executor is quiescent.

The structural gap is now execution and attention. Session/body stores still
reserve, activate, revoke, and reap through duplicated Project/Task paths, then
mirror those transitions into Run. Review and Handoff still encode an
interactive flow as separate state machines and can starve behind parent
background work. The next cut deletes those aggregates and makes shared Run
control plus the parent control lane authoritative.

## Target contract

**Decision.** One model conducts Wave, Project, and Task Work:

```text
Work -> Epoch -> Run -> Launch -> optional Turn
                    \-> Wait

Steer advances the Work Basis.
Run owns execution authority.
Launch owns provider/process continuity.
Turn records an observed provider exchange when one exists.
```

The provider may change how input arrives. It may not change whether input is
durable, whether stale execution may complete Work, or whether a dead executor
retains write authority.

Work is the long-lived logical server. It remains addressable when no executor
or provider process exists. A generic Work runtime hosted by the owning Home
serves every Wave, Project, and Task; the Work kind supplies domain truth,
flow, closure checks, and allowed effects rather than another lifecycle stack.
One Home resident may host many Work instances. An OS process is only a Run or
Launch implementation detail and may disappear without changing Work identity.

| Noun | Durable truth | Deliberately absent |
| --- | --- | --- |
| Work | stable, addressable Wave, Project, or Task logical server and parentage | Session identity, provider state, and required resident process |
| Epoch | one pursuit of Work: `Open`, `Done`, or `Abandoned` | retry count and provider generation |
| Basis | `(epoch, rev)` for every prompt-relevant durable input | separate truth/directive/response cursors |
| Steer | ordered authored direction from User or active parent Run | replacement, lifecycle, and response variants |
| Run | one wake-to-wait authority period and lease | provider transcript and process generation |
| Wait | exact durable fact required before another useful Run | Blocked and Sleeping lifecycle states |
| Launch | one provider or TUI process lifetime, route, containment, and optional resume token | Work-level provider session |
| Turn | observed provider boundary, immutable Basis, outcome, and usage | required boundary for opaque TUIs |
| Send | one delivery attempt for one Steer and exact Turn | incorporation state |
| Home | stable local execution authority identified by `HomeId` | hostname as identity |

`Exec` remains a low-level process receipt beneath Launch. It is evidence, not
a public lifecycle target.

There is no first-class `Actor`, writable `Ack`, `Handle`, `Body`, `Session`,
`Block`, `Sleep`, `Interaction`, `InteractionId`, Review row, or `ReviewId` in
the target. Review is derived from flow, Launch, and attention.

## Identity and containment

**Decision.** Wave, Project, and Task are the only durable Work identities.
Their ids do not change when execution stops, an external title changes, Work
recovers, or a provider changes.

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
```

`WorkRef` is a typed union over the domain stores, not a generic Work table that
duplicates identity or parentage. Project and Task ids are local Loopflow ids;
Linear ids are external bindings. A Project advancing Epoch does not reparent
its Tasks. An explicit quiescent move changes parentage and records history.

The runtime dispatches on `WorkRef` with an explicit typed match. It does not
use a provider registry, factory trait, or generic Work row to erase the domain
differences. All three kinds share input ordering, Run authority, provider
delivery, containment, recovery, and status projection. Wave policy supplies
chat/cadence/project selection; Project policy supplies KRs and child Task
judgment; Task policy supplies workspace, PR, CI, and implementation flow.

Use newtypes for every id. Public DTO fields are required or explicitly
optional and have no wire defaults.

## One input revision

**Decision.** Every Epoch owns one monotonically increasing `rev`. Every durable
fact that changes what the executor must honor allocates the next revision in
the same transaction:

- authored Work truth;
- Steer;
- a typed tool response when a tool genuinely requires machine-readable input;
- selected external evidence.

```text
lock current Open Epoch
validate caller, optional if_basis, and active lease where required
next = current_rev + 1
insert epoch_revisions(epoch, next, kind, source)
write the typed source fact at Basis(epoch, next)
set Epoch.current_rev = next
resolve a matching Wait and reserve a Run if the fact wakes Work
commit
only then attempt provider or process side effects
```

`epoch_revisions` contains exactly one row for every allocated revision and
names its typed source. Each source row owns exactly one revision. There is no
second directive version, steer sequence, or incorporation flag.

An authored truth event is an immutable normalized snapshot or a reference to
an immutable ingestion event. It is not a copied rendered prompt.

### Fixed boundary Basis

**Decision.** A structured Turn records one immutable starting Basis before
provider input begins. An opaque TUI records the Basis on its Launch because
Loopflow cannot see inner Turns. Live delivery never advances that boundary.

The applied Basis is derived from the greatest successful root boundary Basis
in the Epoch. Failed, interrupted, and unknown boundaries apply nothing. There
is no Ack call or Ack table.

If Turn 8 begins at revision 12 and receives revision 13 live, it may react
immediately but can apply only revision 12. Revision 13 remains outstanding. A
later boundary begins at 13 or later, receives it in the seed, and may apply it
by succeeding.

## Steer

**Decision.** `steer(work, text, if_basis)` is the only authored-direction
operation. User-to-Wave and parent-Run-to-child call the same API. “Do this
instead” appends another Steer. Preemptive redirect composes `interrupt` and
`steer`; replacement is not a message kind.

```rust
enum Author {
    User,
    Run(RunId),
}

struct Steer {
    id: SteerId,
    basis: Basis,
    author: Author,
    text: String,
    issued_at: Timestamp,
}

enum SendVia { Live, Seed }
enum SendState { Sending, Sent, Failed, Unknown }
```

`Author` is provenance after authorization succeeds. It is not a privilege
model and is not caller-authored wire input.

### Durable-first delivery

**Decision.** `steer` returns after the Steer and revision commit. Delivery is
an optimization recorded afterward:

```text
persist Steer
-> if an exact structured Turn is active, insert Sending Send
-> call send_current(turn, steer)
-> record Sent | Failed | Unknown or typed NotSteerable reason
-> keep Steer outstanding until a later successful Basis covers it
```

The adapter boundary is:

```text
send_current(turn, steer) -> Sent | NotSteerable | Failed | Unknown
interrupt(boundary)       -> Ended | Fenced | Unknown
launch(seed, route)       -> Launch
observe(boundary)         -> Progress | Succeeded | Failed | Unknown
```

`NotSteerable` is a terminal failed Send with a typed reason, not another stored
state. `Unknown` is immutable and never retries against the same Turn.

At the next boundary, the controller reads outstanding Steers through the new
boundary Basis, renders one ordered seed, and writes individual Seed Send
receipts in the transaction that fixes the Basis. A confirmed live Send can
never leave its only future copy in memory.

### Typed tool responses and CI

**Decision.** Review conversation is not a decision protocol. Questions,
brainstorming, critique, and consequential guidance are ordinary Steers inside
the Review. There is no review disposition, approval state, or encoded
decision tree.

When a specific tool genuinely declares a machine-readable choice, its response
stays typed. Persist it, allocate its revision, and release that tool before
optional provider notification. This narrow mechanism does not define Review
or parent scheduling.

A CI failure is evidence, not direction:

```text
observe failed checks
-> ensure one CiIncident for repository, PR head, and failure set
-> select it as execution input
-> allocate its revision
-> if a Run is active, put it first in that Run's control lane
-> otherwise reserve one bounded repair Run atomically
```

An actionable incident that arrives during a parked Turn interrupts that Turn
once and becomes the next boundary. It does not create a second Run beside the
current writer. A failure that only `lf pr land` can resolve remains honest CI
evidence but never enters the control lane. Duplicate observations can neither
reserve a second Run nor claim the same incident twice. A User saying “fix CI
now” is a separate Steer.

Incident settlement records both the failed head and the first authoritative
repaired head. The repaired head comes from a fresh external observation after
the repair boundary, not from the warm PR snapshot that triggered it. A cached
head may select work; it cannot prove what that work shipped.

## Authority

**Decision.** Authorization is non-serializable request context:

```rust
enum ControlCtx<'a> {
    User(&'a AuthenticatedRequest),
    Run(&'a RunLease),
}
```

- an authenticated external client constructs User context; it may be a person
  in Swift/CLI or another system's agent;
- an active Run lease may mutate only immediate child Work;
- a target still must match the caller's observed Epoch/Basis;
- Linear and GitHub ingestion credentials append only their typed events;
- the Home keeper performs only recovery transitions;
- environment variables may transport opaque credentials but never choose
  caller identity.

A Wave Run can control its Projects; a Project Run can control its Tasks. A
Task Run has no durable child Work. The same structural check applies to every
legal child control rather than a second agent-specific operation matrix.

## Epoch

**Decision.** Epoch state is only `Open | Done | Abandoned`. Wait and execution
health do not add lifecycle states.

| From | Operation | Preconditions and transaction | To |
| --- | --- | --- | --- |
| none | start | Work has no Epoch; capture current truth at revision 0 | Open |
| Done/Abandoned | restart Fresh | prior Epoch terminal; no active/unreaped Run | new Open Epoch |
| Abandoned Task | recover from Basis | workspace/PR safety checks; prior Basis exists | new Open Epoch |
| Open | commit done | successful boundary Basis current; closure passes; containment absent; no newer input wins | Done |
| Open | abandon | authenticated request; Run stopped; containment absent | Abandoned |

Sleep, wake, retry, provider handoff, phase change, and PR rotation stay inside
the same Open Epoch.

Fresh restart carries Work identity, parentage, external binding, and history.
It does not carry Steers, Runs, Waits, continuation tokens, cursors, or applied
Basis. Task recovery may retain a checked workspace and PR lineage and refer to
selected prior evidence, but its new Epoch still begins at revision 0. Old
revisions never grant current authority.

## Run

**Decision.** A Run is one wake-to-wait execution authority. It may contain
several provider Turns and several sequential Launches.

```rust
enum RunState { Reserved, Active, Stopping, Ended }
```

| From | Operation | Durable step before side effects | Result |
| --- | --- | --- | --- |
| none | `reserve` | insert Run with trigger and lease hash; take Epoch active slot | Reserved |
| Reserved | launch receipt | record executor/containment intent, then start outside transaction | Active after lease proof |
| Reserved/Active | `stop` | set Stopping and revoke lease | Ended only after containment absence |
| Active | `advance` to Wait | record boundary outcome and Wait; clear slot atomically | Ended |
| Active | `advance` to boundary | record next fixed Basis before provider side effect | Active |
| Active | commit done | stop containment, then completion transaction | Run Ended + Epoch Done |
| Stopping | reap observation | record containment absence | Ended and slot clear |

`reserve`, `advance`, and `stop` are the internal domain operations. Boot,
activate, continue, settle, finish, revoke, reap, and retry are not separate
public lifecycle APIs. Started, progress, interruption, and reap are receipts
or phases.

Recovery reserves a new Run linked by `retry_of`; it never mutates a failed Run
or impersonates its lease. Automatic recovery requires durable evidence that
replay is safe. Unsafe or unknown effects produce a typed Wait.

New input does not imply a new Run. While the Epoch already has an active Run,
the input becomes durable control work for that Run and is selected at its next
boundary. Only input that finds no active Run may resolve a Wait and reserve
one. This is how a child Review or CI incident wakes useful execution without
creating overlapping authority.

### Races

**Decision.** Waking input and Run end serialize on the Epoch row:

- if input commits first, `advance` sees the newer revision;
- if `advance` commits first, it records Wait and clears the slot; input resolves
  that Wait and reserves the next Run;
- input is never durable while invisible to both a Run and a Wait.

Revoking a Run fences writes immediately but retains the active slot until
containment absence is proved. Recovery cannot overlap a stale writer.

Absence is a positive observation, not the lack of a successful probe. The
portable verdict is `Absent | Present | Unprovable`; only `Absent` releases the
slot. A live tmux unit vetoes stale process evidence, and a recorded process
group is preferred to a recyclable pid. Stop/recovery must preserve this
fail-closed behavior when the old child-body probe moves into the generic Run
controller.

## Launch and Turn

**Decision.** Launch owns provider, model, account, Home, containment identity,
and optional opaque continuation token. Account is the routing primitive;
browser profiles are verified login venues belonging to accounts, not Launch
identity. Provider/account/model fallback creates another Launch in the same
Run.

```rust
enum LaunchState { Starting, Live, Stopping, Ended }
enum TurnState { Starting, Active, Succeeded, Failed, Interrupted, Unknown }
```

Persist Launch route, boundary Basis for a TUI, and containment intent before
spawn. Record Live only after containment and provider readiness exist. Fence
new input before shutdown. Record Ended only after containment absence.

For structured providers, create the Loopflow Turn with its Basis before
sending the initial prompt. Correlate a vendor Turn id when observed; never use
it as Loopflow identity. Only Succeeded applies Basis or supports completion.
Partial usage survives every terminal outcome when reported.

Opaque TUIs have no synthetic Turns. Their Launch is the boundary. Process exit
alone is not success; explicit `done`, handback, or failure supplies the
Loopflow outcome.

The live schema enforces:

- one nonterminal Epoch per Work;
- unique `(work, epoch_number)` and `(epoch, rev)`;
- every typed input owns exactly one revision;
- one non-ended Run per Epoch;
- one non-ended writable root Launch per Run;
- one nonterminal root Turn per Launch;
- unique `(steer, turn, via)` Send attempt;
- one unresolved Wait for an Open Epoch with no active Run.

Use SQLite constraints where expressible. Do not substitute application checks
for partial unique indexes and foreign keys.

## Completion

**Decision.** `done(run, basis)` records a proposal tied to the current root
boundary. It does not directly mutate Epoch.

Commit Done only after:

1. the boundary succeeded normally;
2. proposal Basis equals that boundary's immutable Basis;
3. proposal Basis equals current Epoch Basis;
4. domain closure checks pass;
5. every Run-owned process group, tmux unit, background task, and observable
   native descendant is absent or fenced from writes;
6. the final transaction still sees the same revision and active Run.

Any newer truth, Steer, typed tool response, or selected evidence makes the
proposal stale. A live Steer cannot be blessed by the older Turn that received
it. There is no `try_complete` state and no explicit Ack.

## Review, Wait, and attention

**Decision.** A Review is an interactive interval in a Work flow. It may be
critique, questions, brainstorming, or direction. One Review can contain many
Steers and provider Turns.

Review is derived from facts the system already needs:

```text
current flow step is interactive
+ active Launch
+ attention owed by User or immediate parent Work
= open Review
```

There is no Review row, Review id, reviewer generation, disposition, approval,
or copied evidence aggregate. Stable Work identifies the conversation target;
`LaunchId` opens the current provider/TUI surface; Basis fences a stale close.
At most one Review may be current for one Work because one flow has one current
step.

```text
steer(work, text, if_basis)   # one message inside the Review
close_review(work, if_basis) # end the interval and advance the flow
open_launch(launch)           # attach a User client to its surface
```

Closing records no judgment. Consequential feedback is already durable as
Steer. The routed peer—User or parent—closes the Review; the child cannot
silently release itself from requested attention.

While its Launch is usable, a Work in Review remains Running. It is not asleep
or blocked merely because the next participant has not replied.

### Parent responsiveness

**Decision.** Wave and Project loops have one control lane above their own
background pursuit:

1. explicit interrupt and direct User interaction;
2. open child Reviews awaiting this parent, oldest first;
3. other child evidence that can unblock progress;
4. the parent's own pursue, mutate, selection, and cadence work.

The control lane is an ordered projection over durable inputs and child
attention, not another stored inbox or priority table. The same parent agent
that is running clarify, pursue, mutate, cadence, or another flow services it.
There is no reviewer Launch and no second parent agent.

The active Run listens for control input concurrently with provider events. At
every parent Turn boundary it drains control before starting more background
work. If child attention arrives during a background Turn, it first tries live
delivery to that exact Turn. If the Turn cannot be steered, the controller
interrupts it and starts the next Turn from the already-durable child input.
The durable playhead does not advance on interruption, so background work
resumes after the interaction. Plain User Steer still never implies interrupt;
this is explicit parent scheduler policy for higher-priority child attention.

The same rule handles other actionable control evidence. Current code has
proved the narrow form for CI: a claimed incident can interrupt a parked Review
once, settle its bounded repair before the parent lifecycle loop, and leave the
background playhead untouched. The target removes that CI-only ordering hook by
making control input and background flow position separate facts. Completing a
control item cannot rewrite, advance, or validate the background playhead.

Transport delivery to the parent does not discharge attention. The item stays
first until the parent actually sends an ordinary Steer to the child or closes
the Review. A child reply creates another control-lane item. After all child
attention drains, the same parent agent resumes its own flow from durable
position and Basis.

Serving child attention must not require a clean writable canonical checkout.
Wave and Project control Launches get read-only repository context; writable
repository work belongs to Task Workspaces. A dirty main checkout may be
reported as evidence but cannot prevent a parent from steering a child.

### Wait

**Decision.** An Open Epoch with no useful immediate execution records one
typed Wait:

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

The satisfying or invalidating fact resolves the Wait and reserves a Run in
one transaction when execution is useful. There is no generic unblock and Wait
history is not deleted.

Status is derived:

| Facts | Status |
| --- | --- |
| Epoch Done | Done |
| Epoch Abandoned | Abandoned |
| Open plus active Run | Running, with independent health |
| Open plus unresolved Wait | Waiting on its exact fact |
| Open plus neither | Ready; keeper must reserve or explain |

Run health (`Starting`, `Working`, `Stalled`, `Recovering`, `Dead`, or
`Unobservable`) is fresh evidence, not Work lifecycle. An unreachable Home is
not proof that its Run is dead.

Swift is one User client. It derives `AwaitingUser` from a live Review routed to
User and opens the Launch by `LaunchId`, including after Swift was closed. An
external agent using the User API can conduct the same Review without becoming
a Loopflow parent Run. A parent-routed Review appears only in that parent's
control lane. Interaction and Handoff ids disappear.

## Decentralized Home

**Decision.** `HomeId` is durable execution authority. Hostname, socket, SSH
route, and reachability are mutable observations. Only the owner mutates Work,
Runs, Launches, and Turns. Remote Homes may observe but cannot seize authority
because a probe timed out.

One Home-local keeper:

- detects Reserved Runs that missed boot;
- probes locally owned Runs;
- invokes the shared stop path;
- proves containment absence;
- reserves safe recovery or writes a typed Wait;
- publishes status evidence.

Moving Work to another Home is explicit and requires no active Run. The keeper
owns recovery mechanics, not Wave/Product judgment.

## Provider portability

**Research.** The harnesses expose different mechanisms:

| Surface | Input while active | Interrupt | Continuation |
| --- | --- | --- | --- |
| Codex app-server | `turn/steer` against exact regular Turn; review/compaction reject it | `turn/interrupt` | persisted thread resume |
| Claude one-shot CLI used with Max | no second input to `claude -p`; seed later process | kill/fence process | `--resume` captured session id |
| Claude persistent `stream-json` | informative future route; may accept or queue input | streaming interrupt | session resume/fork |
| OpenCode server | async prompt exists but incorporation into already-running model request is not promised | session abort | server-scoped session |
| opaque tmux TUI | no observable inner Turn | process/tmux fence | Launch remains attachable |

Claude's Agent SDK still launches a Claude executable, but supported third-party
use normally requires API-key or cloud-provider authentication. Loopflow's Max
dogfood route therefore treats the sanctioned one-shot CLI as its portability
floor. `stream-json` may improve latency later without changing the contract.

Codex 0.144.5 returns JSON-RPC `-32600` for an idle Turn, stale expected Turn,
missing thread, and malformed request. The adapter recognizes observed policy
messages and treats unknown wording as loud `Failed`. A successful live steer
response shape remains assumed from official app-server documentation rather
than captured in dogfood evidence.

The portability promise is durable outcome, not identical wire behavior:

1. sleeping Work persists Steer, reserves Run, and seeds its first boundary;
2. active direction may arrive live or later, but cannot complete until a later
   successful boundary covers its Basis;
3. `interrupt` + `steer` provides portable preemption;
4. a Turn-boundary race preserves exactly one durable Steer;
5. ambiguous acceptance records Unknown and never blindly repeats to that Turn;
6. ordered bursts preserve sequence and may render as one seed;
7. losing a continuation token reconstructs rather than losing Work.

## Provider-native subagents

**Decision.** Native subagents are not Loopflow Work. They inherit the root
Run's lease, workspace, tools, and completion obligation. Provider child ids
are trace evidence when available, not stable public control targets.

Run stop must fence every owned writer. Process-group or tmux containment is
the portable proof. A provider mode whose native child can mutate after root
containment dies is unsupported until it exposes a reliable fence. Dormant
provider conversations do not block completion.

Use Loopflow child Work when delegation needs durable direction, monitoring,
sleep/wake, recovery, or User steering. Use native subagents for temporary
context isolation and parallel reasoning inside one Run.

## Reconstruction

**Decision.** Provider transcript and continuation tokens are optional hints.
A new Launch renders from:

- current authored Work truth selected by Epoch revisions;
- outstanding Steers in revision order;
- typed tool responses where present;
- selected external evidence and unresolved Wait;
- flow position and domain closure state;
- workspace, git HEAD, PR/CI/review lineage;
- known Loopflow-mediated effects and unknown-effect receipts.

Losing a token, provider, account, or transcript starts another Launch in the
same Run when replay is safe. The renderer either produces a sufficient seed or
names an exact Wait. It never silently starts from an empty prompt.

## Workspace, PR, and CI boundaries

**Current and retained.** Only Task Work owns a writable worktree. Wave and
Project control runs operate from a clean canonical main checkout. Provider
launch fails before execution if it receives another writable repository root.

Task PR rows store evidence rather than a mutable phase label: publication
request, nested GitHub receipt, merge, abandonment, and `after_merge`. Serial
PRs remain inside one Task; concurrent dependency nodes are separate Tasks.

**Target.** Workspace identity belongs to stable Task Work. Pursuit-specific
authority belongs to Epoch. Historical PRs remain attributed; recovering an
abandoned Task grants the new Epoch checked authority over the same workspace
instead of re-keying every PR to a successor Session.

## Usage and trace

**Current and retained.** Additive usage lives only on observed Turns. Exec and
trace lineage never form another spend total. Missing, zero, partial, failed,
interrupted, and cache-only reports remain distinct.

The dogfood ledger proved why: the old exec ledger was a strict subset of Turn
capture and could attribute tokens to whichever provider launched last in the
process. Moving totals to Turn raised measured output because it removed loss,
not because it double-counted.

Trace `TraceId` and `ExecId` are diagnostic lineage. Product `RunId`,
`LaunchId`, and `TurnId` name the target execution spine.

## Migration

**Target.** The cutover is one-way with no dual-write compatibility mode:

1. stop Wave residents and quiesce/reap every Project and Task writer; refuse
   with an actionable list if any remains;
2. mint stable local Project and Task ids and preserve external bindings;
3. group Session successor chains into Epochs only at terminal restart
   boundaries; map process generations, retries, and provider handoffs to Runs
   and Launches inside the Epoch;
4. convert directives to canonical truth events; convert follow-up,
   replacement, resume, and Review conversation prose to ordered Steers;
   convert narrow tool responses, CI, and lifecycle variants to typed facts;
5. do not guess old incorporation—restart current Open Epochs with required
   current input outstanding;
6. move continuation data to Launch and preserve unknown historical links as
   absent;
7. preserve account-first routes and session pins as Launch route identity;
   access profiles remain account-owned login venues;
8. verify foreign keys, active-slot constraints, historical Work lookup, and
   reconstruction on a copied dogfood database;
9. drop live Session/body/command tables, old readers/writers, DTOs, and parsing;
10. restart residents and let the keeper reserve current Work.

Shipped migration files remain history. The live schema retains one
implementation.

## Implementation frontier

**Current status.** The durable input spine is authoritative. Stable Work and
Epoch rows survive Project and Task Session succession; exact process
generations import into the matching Run. `Steer` is the only authored
direction, allocates one ordered Epoch revision, and remains outstanding until
a later successful boundary starts from it. `agent_turns` is the sole Turn and
usage store, with immutable starting Basis. Completion rejects stale or
unapplied Basis. Typed ToolResponse and actionable CI remain separate input.

Execution and attention are not cut over. Session/body stores, lifecycle
`ChildCommand`, `InteractionReview`, Handoff, and duplicate Project/Task
controllers still bridge the new spine to the old runtime. They are the next
deletion boundary, not supported parallel architecture.

The next implementation pass is one **large core-control cutover**, not a set of
small additive phases:

1. collapse interactive Task flow into Review derived from flow + Launch +
   `attention: User | Parent(WorkRef)`;
2. make the same Wave or Project agent concurrently accept control input and
   preempt its durable background playhead to service child Reviews across
   live-send and interrupt/restart providers;
3. make parent control runnable without a writable clean-main checkout;
4. cut User and parent direction through the same Steer API;
5. replace Project/Task reservation, activation, revocation, reaping, and
   settlement with shared Run reserve/advance/stop operations bound to exact
   Run ids and leases;
6. extend existing `agent_launches` for provider and opaque TUI continuity;
7. delete Session/body/ChildCommand/InteractionReview/Handoff authority and
   DTOs rather than bridging them;
8. finish the Run/Launch controller and migrations needed to leave one
   executable architecture.

The deletion is part of correctness. The input-spine checkpoint is 128,049
normalized Rust code lines, 3,078 below the 131,127 architecture checkpoint.
Current main then added 1,093 orthogonal lines, so the physical branch is
129,142. The core cut still owes 8,922 lines to reach at most 120,220 physical /
119,127 normalized. Remove duplicate truth; do not earn the measure by
compressing tests or charging unrelated upstream code to this design.

It is done when:

- killing the controller after confirmed live Send still seeds the Steer;
- live Steer racing successful completion makes completion stale;
- ordered Steers render once and apply together at the later Basis;
- Unknown live Send is not repeated to that Turn and still seeds later;
- one Review contains several Steers and Turns without creating Review rows or
  dispositions;
- Project and Wave service an awaiting child Review before beginning another
  background flow step;
- the agent already running the parent flow receives the Review; no reviewer
  Run, secondary parent agent, or stored priority inbox is created;
- a child Review arriving during a non-steerable parent Turn interrupts and
  becomes the next seeded input;
- responding to the child resumes the interrupted parent playhead without
  replaying completed flow steps;
- User and parent conduct the same Review protocol; only routing differs;
- closing an old Review after Basis or flow position advances is rejected;
- dirty canonical main cannot prevent a read-only parent control response;
- an actionable CI incident arriving during an active Run becomes that Run's
  next control boundary; it never reserves an overlapping repair Run;
- a land-time-only or stale CI incident neither interrupts a Review nor enters
  the control lane;
- CI settlement cannot attribute a cached pre-repair head as the repaired head;
  the first fresh repaired-head receipt is immutable;
- Run authority is released only after positive containment absence; an
  unprovable probe remains fenced;
- global binary promotion and external identity migration fence on active Run
  authority and containment, never Session status or a body-generation guess;
- a narrow typed tool response, where one exists, resolves before optional
  provider prose;
- stale parent lease and stale Epoch/Basis writes are rejected;
- current Wave/Project/Task control has no production directive, replacement,
  follow-up, resume-message, `InteractionReview`, Handoff, or command-decision
  path;
- copied dogfood migration succeeds only after every writer is quiescent;
- format, clippy, migration/race/controller tests, and DTO round trips pass.

The detailed sequence and deletion ledger live in branch-local
`scratch/implementation-plan.md` while the cutover is active.

## Normative race and portability tests

Use deterministic barriers, never sleeps:

- Steer commit versus Turn success;
- confirmed live Send versus crash before seed;
- input revision versus done commit;
- input revision versus Run advance to Wait;
- reserve versus reserve;
- stop versus recovery;
- provider send begins versus disconnect;
- typed tool response versus seed-only blocked tool;
- child Review versus parent background Turn on live and seed-only providers;
- repeated child Review messages versus parent flow resumption;
- actionable CI incident versus a parked Review and active Run;
- non-actionable CI evidence versus a parked Review;
- duplicate CI observation versus crash after reserve or active-Run claim;
- fifty SQLite writers versus receipt allocation.

Every harness runs the same durable-outcome scenarios: live accepted, live
rejected, ambiguous response, seed-only, persistent session, and opaque TUI.
Credentialed smoke tests validate vendor drift; fake protocols remain normative.

## Open questions and active research

**Open.** These may change implementation details but must not add another core
noun or source of truth:

1. Capture one successful Codex `turn/steer` response against the dogfood
   app-server and verify the assumed result shape.
2. Normalize OpenCode usage end to end. Task capture now records normalized
   conversation events, but `StreamEvent::Usage` and
   `ConversationEvent::TurnUsage` still enter through different surfaces and
   disagree on accumulation versus replacement. Re-measure before deleting
   either parser or declaring the old missing-usage observation fixed.
3. Choose the exact stable `HomeId` migration source and route-observation
   format. Hostname remains disqualified as identity.
4. Prove containment for each provider's native subagents and background tasks.
   Unsupported unobservable writers must fail closed.
5. Define the smallest explicit success/handback mechanism for opaque TUIs;
   process exit cannot imply success.
6. Audit Session successor history whose terminal boundary is ambiguous. The
   migration may preserve unknown lineage but must not invent Epochs.
7. Decide whether historical Epoch appears in public diagnostic receipts. Work
   remains the control target either way.

Questions about central orchestration, generic workflow engines, recursive
Projects, provider-wide steer capability, writable Ack, replacement messages,
or separate Interaction/Review decision aggregates are closed by the decisions
above.

## Important paths

**Current implementation:**

- `rust/loopflow/src/child_session.rs`
- `rust/loopflow/src/child_control.rs`
- `rust/loopflow/src/project_session/`
- `rust/loopflow/src/task/`
- `rust/loopflow/src/flowloop/wave.rs`
- `rust/loopflow/src/store/`

**Foundations and target seams:**

- `rust/loopflow/src/harness/`
- `rust/loopflow/src/trace.rs`
- `rust/loopflow/src/lf/commands/usage.rs`
- `tests/fixtures/dto/`
- `swift/Loopflow/Models/`
