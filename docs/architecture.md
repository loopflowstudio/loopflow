---
layout: default
title: Architecture
---

# Architecture

> **Moment of transparency — July 18, 2026**
>
> The durable model is coherent: Wave, Project, and Task are stable Work;
> Epoch/Run/Wait derive lifecycle; Basis orders input; Launch and Turn own
> execution evidence. The former `project_sessions` and `task_sessions` storage
> is gone.
>
> The communication model is smaller too. Steer is authored input. Chat is the
> human Wave presentation. `MEMORY.md` is the only memory. Radio, channel
> identity, live memory updates, ambient recent-chat prompt context, implicit PR
> review state, and producerless evidence Receipts are gone.
>
> Feedback is one derived interactive interval, routed when it opens to User or
> the immediate parent. Presentation cannot advance it; only explicit
> `work continue` can. Task phase plans name a reviewer rather than overloading
> headless execution policy.
>
> Runtime hosting is not yet coherent. The Home resident starts Wave listeners,
> but there is no Project server and no generic Work keeper that wakes Ready
> Project or Task Work. A live Project runner can answer child Feedback; a
> stopped Project has no resident owner for that route. That is the next design,
> not a property the current code should pretend already exists.

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

This is the process topology now:

```text
App / CLI -> shared local SQLite, Linear, GitHub

lf start -> Home resident -> per-Wave listener -> Wave resident
                 /health       HTTP/SSE/journal     cadence/provider loop
                 /start

parent or CLI -> reserve Run -> __work project -> Project runner
                                      |
                                      +-----------> Task Run

parent or CLI -> reserve Run -> __work task -> Task runner -> worktree -> serial PRs

Steer -> Epoch Basis -> provider boundary -> agent Turn
Run -> Launch -> provider process / optional Turn
interactive flow + Launch attention -> Feedback projection
```

`lf` is the machine-wide CLI and JSON interface. `lf start` routes each Wave to
its placed Home and asks one Home-local keeper to serve it; `lf wave <name>`
keeps the foreground development path. Project and Task executors are bounded
local child processes sharing SQLite. They enter through
`lf __work <kind> <id>`, resolve the exact ambient Run lease once, and dispatch
to typed Project or Task domain policy.

### What each server or loop does

| Process | Owns now | Does not own |
| --- | --- | --- |
| Home resident | machine-local `/health` and `/start`; starts and tracks placed Wave listeners | Ready-Work scans, Project/Task dispatch, domain judgment |
| Wave listener | one Wave's HTTP/SSE surface, journal, conversation runtime, and resident supervision | Wave cadence, Project/Task execution |
| Wave resident | one Wave's provider continuity, cadence, chat/control lane, and Project judgment | generic Work hosting |
| Project runner | one bounded Project Run; KRs, Task observations, parent-routed Task Feedback while alive | a stable endpoint or resident server |
| Task runner | one bounded Task Run; authored flow, workspace, PR, CI, and Feedback checkpoints | a stable endpoint or child Work |
| `lfd` | webhook ingress, liveness probes, and maintenance sweeps | Work execution or control authority |

The phrase “Project server” currently names a missing capability. Project Work
is durable and addressable in the store, but its runner exists only after a
caller reserves a Run and launches `__work project`. Task event writes make a
best-effort `wake_project` call, and a live Project runner polls
`child_attention`. Opening or re-arming parent-routed Feedback is not itself a
resident Project inbox or a generic wake signal. If that runner is stopped, no
Home-owned loop continuously notices the useful input and starts it. Ad hoc CLI
operation exposes this gap more often, but the gap is architectural.

Current decentralized truth is deliberately split by substrate:

| Substrate | Owns |
| --- | --- |
| local SQLite | this Home's runtime, credentials, operation results, and Work state |
| append-only journals | Wave conversation and durable run narrative |
| repository files | Wave goals and memory |
| Linear | shared Wave/Project/Task planning truth |
| GitHub | branches, PRs, checks, and merges |
| SSH | reach to another Home without a central Loopflow coordinator |

The normalized boundary is now visible in code:

- Project and Task ids are stable Work identity;
- Epoch/Run/Wait derive lifecycle and status;
- one Run lease owns execution authority;
- Launch owns provider route, continuation, and containment;
- Turn owns observed boundary outcome, Basis, output, and usage;
- Project and Task loops own only their domain policy;
- child attention is consumed before parent background work when that parent is
  running;
- Task action surfaces carry one next legal action and reason, not a mirrored
  matrix of every blocked alternative.

Remaining `session` names belong to provider, tmux, Ghostty, browser, or human
substrate, or to historical migrations. They are not Work, Run, or Launch
identity.

### Foundations already changed

Several prerequisites now match the target:

1. Provider steering is attempted against the exact active Turn. The adapter
   returns `Sent`, `NotSteerable`, `Failed`, or `Unknown`; there is no
   provider-wide `supports_steer` flag. Codex can live-send. The one-shot Claude
   CLI, OpenCode where incorporation is not proven, and opaque TUIs fall back
   to a later seed. Plain Steer never implies interrupt.
2. Observed Turn is the only additive spend grain. `lf usage`, `lf top`,
   `lf runs`, Doctor coverage, JSON, and Mac telemetry read one Turn query.
   Missing usage remains absent rather than becoming zero.
3. Provider accounts are the routing primitive. Access profiles are verified
   login venues owned by an account, not agent identity. Account selection is
   fixed at the outer invocation and inherited through one opaque, fail-closed
   lease handle, including across SSH.
4. Revoked body recovery releases authority only after positive absence
   evidence; an unprovable probe remains fenced.
5. A current actionable CI incident can interrupt a parked Task Feedback once
   and settle before the background lifecycle resumes. This is the narrow
   implementation precursor to the generic control lane.
6. Authored Project/Task direction is only Steer. `ChildDirective`, directive
   versions, follow-up/replacement/resume prose variants, and writable Ack are
   gone. A confirmed live Send cannot consume the Steer; a successful later
   boundary Basis derives application.
7. Project and Task boundaries capture immutable Basis on `agent_turns`, and
   terminal completion rejects a stale or unapplied Basis.
8. Stable Project/Task Work rows contain domain state only. Epoch/Run/Wait
   derive status, and completion closes an Epoch only after a successful
   current boundary and quiescent containment.
9. Stored Feedback and Handoff aggregates are deleted. Interactive flow
   position, live Launch, and `attention: User | Parent(WorkRef)` derive
   Feedback. Presentation has no continuation callback; a Basis-fenced
   `continue_feedback` is the only close.
10. `ChildCommand` is deleted. Direct Run/Work controls and typed CI incident
    claims replace its lifecycle/source/effect/claim state.
11. Global promotion and PM reteam now fence active writers through Run and
    containment evidence rather than Session status.
12. One opaque `LF_RUN_LEASE` resolves exact active Run authority without a
    caller-supplied Work, generation, or Author. Observed root Turn output feeds
    the Project runner's oldest-first child control lane.
13. Project and Task enter through one `run_work(WorkRef)` boundary. Session
    storage, status, body generations, lifecycle mirroring, recovery, and
    executor commands are deleted.
14. Roadmap and Swift surfaces consume `WorkStatus` directly. The action model
    is `recommended + reason`; the exhaustive blocked-action matrix is gone.
15. Project owns `ProjectDefinition`; Task owns `TaskDirective` plus
    `project_id`. Task does not copy parent Project planning truth.
16. Task phase plans use `FeedbackReviewer::{User, Parent}`. The standard route
    is User for clarify, Parent for pursue, and User for mutate; an override
    affects future checkpoints only.
17. Radio, channel identity, live memory events, recent Wave chat prompt
    context, Feedback escalation, implicit PR Review state, and the evidence
    Receipt resolver are deleted.

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

Work is the long-lived logical address, not a promise of one OS process per
Work. It remains addressable when no executor or provider process exists. The
owning Home must have exactly one mechanism that notices useful Ready input,
reserves one Run, and starts the kind-specific executor. Whether that mechanism
is a lightweight in-process actor per Work or a Home-wide Ready scan is the
server-topology decision still to make. It must not create a second lifecycle
model beside Epoch/Run/Wait.

| Noun | Durable truth | Deliberately absent |
| --- | --- | --- |
| Work | stable, addressable Wave, Project, or Task identity and parentage | Session identity, provider state, and required resident process |
| Epoch | one pursuit of Work: `Open`, `Done`, or `Abandoned` | retry count and provider generation |
| Basis | `(epoch, rev)` for every prompt-relevant durable input | separate truth/directive/response cursors |
| Steer | ordered authored direction from User or active parent Run | replacement, lifecycle, and response variants |
| Run | one wake-to-wait authority period and lease | provider transcript and process generation |
| Wait | exact durable fact required before another useful Run | Blocked and Sleeping lifecycle states |
| Launch | one provider or TUI process lifetime, route, containment, and optional resume token | Work-level provider session |
| Turn | observed provider boundary, immutable Basis, outcome, and usage | required boundary for opaque TUIs |
| Send | one delivery attempt for one Steer and exact Turn | incorporation state |
| Home | stable local execution authority identified by `HomeId` | hostname as identity |

`Exec` remains a low-level process record beneath Launch. It is evidence, not
a public lifecycle target.

There is no first-class `Actor`, writable `Ack`, `Handle`, `Body`, `Session`,
`Block`, `Sleep`, `Interaction`, `InteractionId`, Feedback row, or `FeedbackId` in
the target. Feedback is derived from flow, Launch, and attention.

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

**Decision.** Feedback conversation is not a decision protocol. Questions,
brainstorming, critique, and consequential guidance are ordinary Steers inside
the Feedback. There is no Feedback disposition, approval state, or encoded
decision tree.

When a specific tool genuinely declares a machine-readable choice, its response
stays typed. Persist it, allocate its revision, and release that tool before
optional provider notification. This narrow mechanism does not define Feedback
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

Run authority follows the fixed-grant shape already used for provider accounts.
The executor receives one opaque `LF_RUN_LEASE`; its secret hash identifies
exactly one active Run. The caller does not submit Run, Work, Session,
generation, or Author fields. Store validation returns the Run, Work, Epoch,
and Basis or fails closed. Nested commands in that Run inherit the same grant;
a distinct child Run receives its own lease. Missing or malformed in-Run
authority is never reinterpreted as User. An authenticated external entrypoint
constructs User context explicitly.

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
one. This is how a child Feedback or CI incident wakes useful execution without
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

Turn also retains the optional root assistant text Loopflow observed. This is
the minimum durable output needed to conduct a child Feedback and reconstruct an
operator-facing conversation; it is not a provider transcript or a new Message
aggregate. Partial, failed, and interrupted output remains evidence and does
not apply Basis. Tool calls, native child transcripts, and vendor event detail
remain optional trace artifacts.

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

## Feedback, Wait, and attention

**Decision.** A Feedback is an interactive interval in a Work flow. It may be
critique, questions, brainstorming, or direction. One Feedback can contain many
Steers and provider Turns.

Feedback is derived from facts the system already needs:

```text
current flow step is interactive
+ active Launch
+ Feedback route to User or immediate parent Work
+ latest observed root Turn output when the provider exposes Turns
= open Feedback
```

There is no Feedback row, Feedback id, reviewer generation, disposition, approval,
or copied evidence aggregate. Stable Work identifies the conversation target;
`LaunchId` opens the current provider/TUI surface; Basis fences a stale close.
At most one Feedback may be current for one Work because one flow has one current
step.

Feedback route and pending attention are different facts on that Launch. The
route exists for the whole interactive interval. `attention_at` exists only
while the routed peer owes the next response. A parent Steer clears pending
attention after its durable commit but leaves the Feedback open. The child's next
terminal Turn re-arms attention. `continue_feedback` alone clears the route and
advances flow. This is turn-taking state, not a Message, disposition, or Feedback
aggregate.

```text
steer(work, text, if_basis)   # one message inside the Feedback
continue_feedback(work, if_basis) # end the interval and advance the flow
open_launch(launch)           # attach a User client to its surface
```

Closing records no judgment. Consequential feedback is already durable as
Steer. The routed peer—User or parent—closes the Feedback; the child cannot
silently release itself from requested attention.

While its Launch is usable, a Work in Feedback remains Running. It is not asleep
or blocked merely because the next participant has not replied.

### Parent responsiveness

**Decision.** Wave and Project loops have one control lane above their own
background pursuit:

1. explicit interrupt and direct User interaction;
2. open child Feedbacks awaiting this parent, oldest first;
3. other child evidence that can unblock progress;
4. the parent's own pursue, mutate, selection, and cadence work.

The control lane is an ordered projection over durable inputs and pending child
attention, not another stored inbox or priority table. The same parent agent
that is running clarify, pursue, mutate, cadence, or another flow services it.
There is no reviewer Launch and no second parent agent.

**Current gap.** This ordering exists inside a live Wave resident or Project
runner. It does not yet guarantee that a stopped Project starts when Task
Feedback is routed or re-armed to it. The server-topology follow-up is complete
only when the owning Home can derive that useful input from durable state and
wake exactly one Project Run without relying on an in-process Task callback.

The projected parent seed contains the child's latest durable root Turn output
plus current Work, flow, workspace, PR/CI, and other relevant domain facts.
This is what lets the same protocol carry critique, questions, or brainstorming
without restoring a Feedback prompt row. Attention without understandable child
content is not considered serviced.

When a child terminal Turn re-arms parent attention, the same transaction
allocates one idempotent `evidence` revision on the parent's current Epoch using
that child Turn as source. The next parent boundary therefore starts from a
Basis that includes the child reply, and an older completion proposal loses.
The source remains the child Turn/Launch; there is no copied inbox row.

The active Run listens for control input concurrently with provider events. At
every parent Turn boundary it drains control before starting more background
work. If child attention arrives during a background Turn, it first tries live
delivery to that exact Turn. If the Turn cannot be steered, the controller
interrupts it and starts the next Turn from the already-durable child input.
The durable playhead does not advance on interruption, so background work
resumes after the interaction. Plain User Steer still never implies interrupt;
this is explicit parent scheduler policy for higher-priority child attention.

The same rule handles other actionable control evidence. Current code has
proved the narrow form for CI: a claimed incident can interrupt a parked Feedback
once, settle its bounded repair before the parent lifecycle loop, and leave the
background playhead untouched. The target removes that CI-only ordering hook by
making control input and background flow position separate facts. Completing a
control item cannot rewrite, advance, or validate the background playhead.

Transport delivery to the parent does not discharge attention. The item stays
first until the parent actually sends an ordinary Steer to the child or closes
the Feedback. Steer clears only the pending turn; the Feedback remains open. A
later terminal child Turn re-arms pending attention and creates the next
control-lane item. After all child attention drains, the same parent agent
resumes its own flow from durable position and Basis.

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

Swift is one User client. It derives `AwaitingUser` from a live Feedback routed to
User and opens the Launch by `LaunchId`, including after Swift was closed. An
external agent using the User API can conduct the same Feedback without becoming
a Loopflow parent Run. A parent-routed Feedback appears only in that parent's
control lane. Interaction and Handoff ids disappear.

## Wave memory

**Current.** `wave/<name>/MEMORY.md` is the one Wave memory truth. Prompt
assembly reads applicable ancestor files directly, oldest ancestor first.
There is no memory event stream, write endpoint, replay buffer, or export
workflow. Agents edit the reviewed file through ordinary repository changes;
`lf memory show` is read-only convenience and works while the Wave is stopped.

## Decentralized Home

**Current decision.** `HomeId` is durable execution authority. Hostname, socket, SSH
route, and reachability are mutable observations. Only the owner mutates Work,
Runs, Launches, and Turns. Remote Homes may observe but cannot seize authority
because a probe timed out. A remote route may be re-observed, but the owning
machine's `local` marker cannot be rewritten as a remote route.

`Placement { work: WorkRef, home_id: HomeId, placed_at }` is the sole execution
location relation. New Wave Work starts on the local Home; Project and Task
Work inherit their parent's current placement once. A move is explicit and is
refused while the Work has a live Run. Run reservation resolves placement in
the same transaction and refuses unless it names the local Home, so neither a
caller nor a process on the wrong machine can select a different authority.
The current mutation surface moves Wave Work only. Project and Task movement
is not yet a public operation; their inherited placement is enforced by the
same Run reservation path rather than by a second executor lifecycle.

One machine-local Home resident hosts the existing per-Wave listener tasks.
`lf start` groups Waves by Home, ensures the local keeper once, and routes
remote groups through `lf ssh <HomeId> --remote-native`. That transport forwards
no origin authority and makes the target verify its own HomeId before mutating
lifecycle state. The internal start hop carries the exact WaveId and refuses a
name/id collision, so a remote Home cannot mint a second identity for the same
Work. `lf stop` stops only the selected Wave and leaves the keeper
and sibling Waves running.

The target keeper additionally:

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
- known Loopflow-mediated effects and unknown-effect records.

Losing a token, provider, account, or transcript starts another Launch in the
same Run when replay is safe. The renderer either produces a sufficient seed or
names an exact Wait. It never silently starts from an empty prompt.

## Workspace, PR, and CI boundaries

**Current.** Only Task Work owns a writable worktree. Wave and Project control
Runs operate from the canonical main checkout and currently refuse to start
when it is dirty. Provider launch fails before execution if it receives another
writable repository root.

**Target.** Wave and Project control Launches receive repository context
read-only. A dirty canonical checkout remains visible evidence but cannot block
parent control or child Feedback. Writable repository changes still belong
only to Task Workspaces.

Task PR rows store evidence rather than a mutable phase label: publication
request, nested GitHub PR record, merge, abandonment, and `after_merge`. Serial
PRs remain inside one Task; concurrent dependency nodes are separate Tasks.

**Current.** Workspace identity belongs to stable Task Work. Pursuit-specific
authority belongs to Epoch. Historical PRs remain attributed; recovering an
abandoned Task grants the new Epoch checked authority over the same workspace
instead of re-keying every PR to a replacement executor.

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

Migration `0.11.036_delete_sessions.sql` is one-way and has
no dual-write mode. It:

1. copies surviving Project and Task domain facts into stable `projects` and
   `tasks` rows;
2. rewrites PR, CI, observation, control, and Run ownership references to
   stable Work ids before deleting the old identity;
3. preserves provider continuation on Launch and stable external bindings on
   Work;
4. drops the obsolete dependent tables, then `task_sessions` and
   `project_sessions`;
5. leaves no Task/Project status column on the replacement records.

Fresh-database proof asserts that `tasks` exists, both Session tables are
absent, and neither product table carries `status`, `status_reason`, or
`status_at`. Historical migration files remain immutable and necessarily name
the old tables: an older database must apply that history before it can apply
the deletion.

## Implementation frontier

The core reduction is implemented:

- Session tables, product status enums, body generations, CRUD, authority,
  recovery, and Run mirrors are gone;
- `run_work(WorkRef)` is the single Project/Task authority entrypoint;
- Project and Task policy loops receive an already-resolved store and lease;
- completion flows only through the successful-boundary Basis fence;
- roadmap/Swift use `WorkStatus` and Work-shaped fields;
- Task attention projects one next legal action plus its reason;
- Project/Task planning state is `ProjectDefinition`, `TaskDirective`, and a
  live `project_id` relation rather than copied parent snapshots;
- Feedback presentation, continuation, and reviewer choice are separate APIs;
- Radio, channel identity, live memory, recent-chat prompt context, implicit PR
  Review state, policy-based reviewer routing, and evidence Receipts are gone.

Separate Project and Task domain policy remains correct: KRs versus workspace,
PR, CI, and closure are real differences. Their process supervision is not yet
correctly shared. Both runners still implement provider control, recovery,
playhead progression, input draining, failure settlement, and process lifetime
in parallel, while Wave has a third listener/resident stack.

### Server-topology follow-up

This is the unaddressed slice. It is done when:

1. Home, Wave, Project, Task, Run, and Launch process ownership fits on one
   screen, including which pieces are long-lived and which are replaceable;
2. one Home-owned mechanism derives useful Ready Work from durable state and
   reserves exactly one Run—no Task callback, app process, or CLI shell is
   required to keep progress alive;
3. a stopped Project answers immediate-child Feedback through the same path as
   a live Project, while direct User review remains an explicit Task option;
4. Wave-to-Project and Project-to-Task control are the same one-hop protocol;
   Wave-to-human remains the human chat/Steer surface, not Feedback escalation;
5. CLI, Mac, and unattended parent review invoke the same controls and differ
   only in who supplies User or parent authority;
6. process exit, app exit, listener restart, a dirty canonical checkout, and a
   failed best-effort nudge cannot lose input or strand Ready Work; status names
   the exact durable fact still needed;
7. generic Run reservation, Launch supervision, provider recovery, Steer
   delivery, interrupt, and Wait settlement have one implementation; each Work
   kind contributes only domain prompt, flow, evidence, and closure policy;
8. the design decides whether an open Feedback retains a Run or records a typed
   Wait without inventing a generic Blocked state;
9. a deterministic test starts a Task from an ad hoc CLI, stops its parent,
   opens parent-routed Feedback, and proves the owning Home wakes one Project
   that can Steer or continue it;
10. the old Wave listener/resident and Project/Task launch paths that the design
    replaces are deleted rather than kept as compatibility routes.

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
- child Feedback versus parent background Turn on live and seed-only providers;
- repeated child Feedback messages versus parent flow resumption;
- actionable CI incident versus a parked Feedback and active Run;
- non-actionable CI evidence versus a parked Feedback;
- duplicate CI observation versus crash after reserve or active-Run claim;
- fifty SQLite writers versus revision and operation-result allocation.

Every harness runs the same durable-outcome scenarios: live accepted, live
rejected, ambiguous response, seed-only, persistent session, and opaque TUI.
Credentialed smoke tests validate vendor drift; fake protocols remain normative.

## Open questions and active research

These may change implementation details but must not add another core noun or
source of truth. The server-topology follow-up above is the primary design;
these are narrower research items:

1. Capture one successful Codex `turn/steer` response against the dogfood
   app-server and verify the assumed result shape.
2. Normalize OpenCode usage end to end. Task capture now records normalized
   conversation events, but `StreamEvent::Usage` and
   `ConversationEvent::TurnUsage` still enter through different surfaces and
   disagree on accumulation versus replacement. Re-measure before deleting
   either parser or declaring the old missing-usage observation fixed.
3. Prove route reconciliation when two operators observe the same Home through
   different SSH aliases. `HomeId` remains identity; route remains observation.
4. Prove containment for each provider's native subagents and background tasks.
   Unsupported unobservable writers must fail closed.
5. Define the smallest explicit success/handback mechanism for opaque TUIs;
   process exit cannot imply success.
6. Exercise migration 36 against a copied long-lived dogfood database whose
   old successor history includes ambiguous terminal boundaries. Preserve
   unknown lineage; never invent an Epoch boundary.
7. Decide whether historical Epoch appears in public diagnostic results. Work
   remains the control target either way.

Questions about central orchestration, generic workflow engines, recursive
Projects, provider-wide steer capability, writable Ack, replacement messages,
or separate Interaction/Feedback decision aggregates are closed by the decisions
above.

## Important paths

**Current implementation:**

- `rust/loopflow/src/work_runner.rs`
- `rust/loopflow/src/project/runner.rs`
- `rust/loopflow/src/task/`
- `rust/loopflow/src/flowloop/wave.rs`
- `rust/loopflow/src/ops/child.rs`
- `rust/loopflow/src/store/`

**Durable and provider seams:**

- `rust/loopflow/src/harness/`
- `rust/loopflow/src/durable.rs`
- `rust/loopflow/src/store/durable.rs`
- `rust/loopflow/src/trace.rs`
- `rust/loopflow/src/lf/commands/usage.rs`
- `tests/fixtures/dto/`
- `swift/Loopflow/Models/`
