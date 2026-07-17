# Core architecture implementation plan

## Outcome

One local control model conducts Wave, Project, and Task Work across Codex, Claude, OpenCode, and opaque tmux TUIs:

```text
Work → Epoch → Run → Launch → optional Turn
                  ↘ Wait

Steer advances the Work Basis.
Run owns execution authority.
Launch owns provider/process continuity.
Turn records an observed provider exchange when one exists.
```

The provider may change how a Steer arrives. It may not change whether the Steer is durable, whether stale execution can complete Work, or whether a dead executor can keep authority.

The end-to-end demonstration is one Task exercised four ways:

1. Start it with Codex, Steer it during an active Turn, and observe a live `Send` receipt.
2. Start it with the one-shot Claude CLI, Steer it during execution, and observe the same Steer seed the next execution boundary.
3. Start it with OpenCode, race a Steer against an ending Turn, and observe exactly one durable Steer regardless of which delivery path wins.
4. Start an opaque TUI behind tmux, leave the Swift app closed, reopen the Launch from Swift, and complete or hand it back without inventing inner Turns.
5. Kill each provider process and restart with no provider transcript or resume token. The new Launch reconstructs from current Work, Steers, flow position, workspace, and external evidence.
6. Change provider or account during recovery. It creates a new Launch in the same Run, not a new Work identity or Epoch.

Every path produces the same Work status, Basis fencing, recovery decision, and completion result.

## Scope boundary

This push owns the central execution model, its persistence, the local agent API, provider adapters, recovery, monitoring projections, Swift attention/open behavior, and usage authority.

It does not redesign:

- Wave/project planning semantics or KRs;
- account/profile ownership beyond moving a provider resume token onto Launch;
- repository-scoped Wave naming;
- replication between Homes;
- provider-native subagent UX or a portable child-session graph;
- the website and user docs, except where their implementation must stop naming removed APIs.

Do not add compatibility shims. Migrate stored data once, cut every caller to the new model, and delete the old readers and writers in the same branch. The implementation checkpoints below are review boundaries, not independently supported dual architectures.

## Target contract

### Durable nouns

| Noun | Stored truth | Deliberately absent |
| --- | --- | --- |
| Work | Stable Wave, Project, or Task identity and parentage | Session identity, provider state |
| Epoch | One terminal pursuit of Work: `Open`, `Done`, or `Abandoned` | Retry count, provider generation |
| Basis | `(epoch, rev)` for all prompt-relevant durable input | Separate truth/directive/decision cursors |
| Steer | Ordered authored direction from `Human` or an active parent `Run` | Replace, resume message, lifecycle command |
| Run | One wake-to-wait execution authority and lease | Provider transcript, process generation |
| Wait | The typed fact required before another useful Run | Blocked/Sleeping lifecycle states |
| Launch | One provider or TUI process lifetime, route, containment, and optional resume token | Work-level provider session |
| Turn | An observed provider exchange, its immutable starting Basis, outcome, and usage | Required recovery floor |
| Send | Immutable attempt to deliver one Steer to one observed Turn | Mutable delivery state on Steer |
| Home | Stable local execution authority identified by `HomeId` | Hostname as identity |

`Exec` remains a low-level process receipt beneath Launch. It is useful evidence, not a public control target.

There is no first-class `Actor`, `Ack`, `Handle`, `Body`, `Session`, `Block`, `Sleep`, `Interaction`, or `InteractionId`.

### One input revision

Each Epoch owns a monotonically increasing `rev`. Any durable fact that changes the context an executor must honor allocates the next revision in the same transaction:

- authored Work truth;
- Steer;
- typed decision or approval;
- typed external evidence selected as execution input.

An execution boundary starts from one immutable `Basis { epoch, rev }`. A live Steer advances current Work Basis but never rewrites the active boundary's Basis. This one cursor replaces copied directives, directive versions, separate incorporation flags, and cross-table completion guesses.

A successful boundary records the Basis it ran from. The latest successfully observed boundary therefore derives the applied Basis; there is no writable Ack API or Ack table. A Steer remains outstanding until a later successful boundary starts at or after its revision.

For structured providers, a Turn is the boundary. For an opaque TUI, the Launch is the boundary because Loopflow cannot see inner Turns. A TUI process exit is not success. `done` or handback must record an explicit Loopflow outcome before the controller treats the boundary as successful.

### Public controls

```rust
steer(work, text, if_basis) -> SteerReceipt
interrupt(work, if_run) -> InterruptReceipt
done(run, basis) -> DoneProposal
abandon(work, reason, if_basis) -> EpochReceipt
status(work) -> WorkStatus
```

`steer` never accepts `live`, `replace`, or delivery-policy flags. The receipt may later show `Live` or `Seed` as what happened.

`interrupt` ends the current Turn or opaque Launch boundary. It authors no direction and does not itself end the Run.

`done` proposes terminal Work completion from the current Run. The controller commits `EpochState::Done` only when the proposal Basis is current, the boundary succeeds, domain closure checks pass, no newer input wins the completion transaction, and Run-owned containment is empty.

Human authority comes from the authenticated local request. Parent authority comes from the active Run lease and the invariant `target.parent == source_run.work`. `Author` exists only as Steer provenance:

```rust
enum Author {
    Human,
    Run(RunId),
}
```

Linear, GitHub, timers, attachments, native subagents, and the keeper are not Authors. They append typed evidence or perform their narrowly owned recovery operation.

The core receives a non-serializable `ControlCtx`, not a caller-authored identity field. Home-local Swift/CLI entrypoints construct Human context from their trusted local transport. Agent entrypoints require the opaque current Run lease. An environment variable may transport that lease but its absence never selects Human authority.

### Internal Run operations

```rust
reserve(work, trigger) -> Run
advance(run, boundary) -> Next | End
stop(run, cause) -> StopReceipt
```

- `reserve` atomically takes the Epoch's one active Run slot.
- `advance` atomically consumes one execution boundary and chooses another Launch/Turn or ends the Run in a typed Wait, failure, or terminal proposal.
- `stop` fences authority immediately, owns physical cleanup, and reaches terminal Run state only after executor absence is established.

`boot`, `activate`, `continue`, `settle`, `finish`, `revoke`, `reap`, and `retry` are not public domain operations. Started/progress/reaped are receipts. Revoke and reap are stop phases. Recovery reserves a new Run after the prior containment is absent; it never mutates or blindly replays the failed Run.

### Waiting and attention

Epoch state is only `Open | Done | Abandoned`.

- Open Epoch plus an active Run projects as `Running`.
- Open Epoch plus no active Run and an unresolved Wait projects as `Waiting`.
- Runtime failure, Home reachability, and stale observations remain evidence until reconciliation starts another Run or records a specific Wait.

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

An input wait may be indefinite. There is no generic `unblock`; the named fact resolves or invalidates the Wait.

An attended interactive step is a live TUI Launch. Swift derives `AwaitingHuman` and opens that Launch whether or not anyone is currently attached. In non-blocking mode the flow routes its request to the parent Run; the response is a Steer on the child. It is not human attention and it creates no special Review, Handoff, or Interaction lifecycle.

### Provider and subagent boundary

A Launch stores provider, model, account, Home, containment identity, and an optional opaque resume token. Compatible continuation may use the token. Incompatible or missing continuation renders one reconstruction seed and starts clean. Provider, model, or account fallback creates another Launch in the same active Run.

Every Launch runs in a Run-owned process group or tmux containment unit. Run end requires all owned containment to be absent. Provider-native child ids are diagnostics only. Native children use the root Run's effective workspace, tools, and Loopflow authority and are attributed to that Run. A provider mode capable of continuing to mutate after root containment dies is unsupported until it exposes a reliable stop/status fence.

## Behavior changes

These are contract changes, not migration accidents.

| Area | Current behavior | New behavior |
| --- | --- | --- |
| Project/Task identity | Successor Sessions can become the new control target | Stable Work id remains the target; terminal restart creates an Epoch |
| Direction | FollowUp, Steer, Interrupt replacement, Resume message, Decide, CI-fix, and directives share a command ledger | Only authored direction is Steer; decisions and evidence stay typed; lifecycle uses controls |
| Replacement | Interrupt may carry replacement text and directives can supersede | `interrupt` plus a normal Steer; history is append-only |
| Delivery choice | Callers and provider capability flags influence live/next-turn behavior | Controller attempts live delivery when the active Turn accepts it; otherwise later seed; caller gets no promise |
| Delivery result | Accepted can be read as incorporated; Uncertain mixes interrupt and send gaps | Send says only what transport observed; applied Basis comes from a later successful boundary |
| Failed/unknown Send | Delivery rows may be retried or superseded through command state | Attempts are immutable; Failed may permit a new safe attempt, Unknown never repeats to the same Turn, and both leave Steer available to seed |
| Ack | Directive incorporation and completion need separately writable/derived acknowledgment concepts | Successful boundary Basis derives what was applied; there is no Ack mutation or DTO |
| Completion | Session/body settlement and directive incorporation jointly infer completion | Current Run proposes `done`; one Basis/quiescence transaction commits Epoch Done |
| TUI completion | Process/handoff lifecycle can stand in for inner agent progress | TUI has no Turns; explicit Loopflow outcome is required; process exit alone is failure/unknown |
| Retry | Successor/generation logic can look like replaying an attempt | Failed Run is immutable; reconciliation starts from current reality in a new Run |
| Provider fallback | Provider session fields live on Project/Task Session and can imply a new attempt | New Launch in the same Run; Work and Epoch do not move |
| Interrupt | Can park dead work, carry replacement direction, or blur Run termination | Ends the current execution boundary only; controller then advances from durable state |
| Status | Waiting, Blocked, interaction waiting, lease, and liveness are partly stored together | Work status is one projection over Epoch, active Run, Wait, attention, and fresh topology evidence |
| Dormancy | Sleeping and Blocked need separate legal-action paths | One typed Wait; indefinite input waits are valid |
| Human interaction | InteractiveHandoff and InteractionReview create ids, policies, and terminal states | Live TUI Launch is openable by `LaunchId`; parent routing becomes Steer |
| Human attention | Parent review and human review can appear in the same interaction machinery | Only a live human-routed TUI Launch derives `AwaitingHuman` |
| Decisions/approvals | Decision commands may also carry message text | Machine-semantic response is one typed input; extra direction is a separate Steer |
| Outstanding direction | Individual directives/commands are rendered through delivery state | Records remain individual; renderer emits one ordered seed projection |
| Native subagents | Completion may seek provider-specific descendant evidence | Completion fences the owned executor containment; dormant provider conversations do not block |
| Authority | Ambient environment can determine Human versus parent caller | Request authentication or Run lease determines authority; env only transports opaque credentials |
| Home | Address/hostname can become machine identity | Stable `HomeId` is identity; address and reachability are mutable observations |
| Usage | `run_events` and `agent_turns` can both report spend; missing may become zero | Observed Turn is the only additive usage grain; missing remains `None` |
| JSON/API | Session, handoff, review, and boolean reroute shapes leak mechanics | Work, Run, Launch, Wait, Steer, typed route, and receipt DTOs encode domain meaning |
| Historical status | Current PM snapshot may be required to explain old execution | Historical Work/Epoch lookup is durable; current plan projection is a separate join |

Expected compatibility breaks:

- old Session ids no longer control Projects or Tasks;
- `lf handoff` and `lf reviews` disappear;
- old status enum cases disappear from Rust, JSON, and Swift;
- `provider_session_id` disappears from Work DTOs and moves to private Launch continuation data;
- `lf runs` changes from trace/process aggregation to product Run → Launch → optional Turn;
- usage totals may change where old paths double-counted or converted missing reports to zero;
- native provider child sessions no longer have a Loopflow lifecycle/control surface;
- old command JSON containing replace/resume/decision/CI variants is rejected after migration.

## Invalid states removed

The goal is not only fewer files. Each normalization must remove a class of representable bugs.

| Normalization | State that becomes unrepresentable or transactionally rejected | Existing warning it closes | Proof |
| --- | --- | --- | --- |
| Stable Work id plus Epoch FK on every execution/input row | Old Steers, Runs, or provider tokens silently attach to a restarted pursuit | PRD-9; successor ambiguity | Restart a Done Task; old-epoch input cannot be selected or controlled through the new Basis |
| Exactly one open Epoch per Work | Two current pursuits disagree about status and direction | PRD-9/10 | Concurrent restart/open transactions: one succeeds |
| One monotonic Epoch revision | Truth, directive, decision, and completion cursors claim mutually inconsistent freshness | W2-288; directive drift | Race every input kind with `done`; stale proposal always loses |
| Reconstruction renders canonical typed facts, never a copied prompt mirror | PM JSON contains five KRs while the Project seed silently contains four | W2-288 | Change each Work field and prove the next seed reads it directly from the authoritative row/history |
| Immutable Basis on every boundary | Live input retroactively blesses an old tool call or completion | completion-fence risk | Live Steer during Turn cannot advance that Turn's Basis |
| One partial-unique active Run slot plus lease token | Two executors both hold current write authority | PRD-10; duplicate dispatch | Concurrent reserve across processes yields one Run; stale lease mutations fail |
| Run cannot terminate until all owned containment is absent | Work becomes Done while a known writer is still alive | native-subagent/cleanup risk | Spawn background child; `done` remains proposed until containment is empty |
| `stop` owns fence and cleanup in one state machine | A nonexistent process remains permanently revoked, or a dead interrupt parks Human-owned work | ENG-3, ENG-4 | Kill before interrupt and during reap; reconciliation reaches one legal Run state and can reserve again |
| Failed Run immutable; new Run requires prior absence | Recovery replays unknown effects or starts beside the old writer | retry side-effect risk | Unknown external effect yields `WaitOn::Effect`; retry before reap is rejected |
| Steer is the only authored direction | CI evidence, lifecycle, decisions, and prose compete in one command enum | ENG-19, ENG-20; PRD-8 | Type-level API has no path to encode CI incident as Steer or replacement as Interrupt |
| Unique Steer revision and immutable Send attempts | Retrying an unknown delivery mutates history or live-sends twice to one Turn | child delivery ambiguity | Unique `(steer, turn, via=live)`; Unknown remains immutable and next seed still contains Steer |
| Typed `RunTrigger::CiIncident` plus incident/head uniqueness | One CI failure falls through into ordinary iterate/gate or launches two repair bodies | ENG-19, ENG-20; PRD-8 | Duplicate webhook and crash-after-reserve produce one incident and one active repair Run |
| One durable flow position advanced in the boundary transaction | A completed review/gate is re-entered, or duplicate reconciliation emits empty serial work | W2-296, W2-297, W2-300 | Reconcile the same evidence repeatedly; flow position advances at most once and then waits/runs from the resulting state |
| Typed Wait, derived Running/Waiting | Stored Blocked/Sleeping/status owner contradicts active execution or missing evidence | ENG-3; PRD-6/10 | Property test every Epoch/Run/Wait combination; only legal projections construct |
| One status/attention projection over normalized facts | CLI, Swift, keeper, and roadmap disagree about counts, owner, or legal action | PRD-6 | All surfaces serialize the same projection fixture rather than recalculate it |
| TUI attention derived from live human Launch | A terminal handoff says Work waits on a human while no openable executor exists | interaction debt | Kill tmux: attention disappears and recovery evidence appears; no stored attention flag can remain stale |
| Provider resume token belongs only to Launch route | Account/provider handoff changes Work identity or poisons later incompatible execution | provider portability | Switch provider/account: same Run, new Launch, incompatible token never selected |
| Stable HomeId, mutable routes | Hostname rename creates a new authority or controls the wrong machine | W2-292 | Change hostname/address; Home identity and ownership remain stable |
| Typed route enum | Healthy successor routing serializes as `project_route_succeeded: false` | W2-295 | JSON cannot encode success as a misleading boolean |
| Typed lineage boundary | A pruned or external parent is reported as corrupt local lineage | ENG-18 | Fixture distinguishes `Local`, `Pruned`, and `External` parent edges |
| One Turn usage producer/parser/store | One run is double-counted or OpenCode real usage is overwritten by zero | W2-280, W2-289 | Parser fixture traverses provider event → Turn row → all rollups exactly once; absent remains null |
| Work-owned external binding passed to mutations | Wave's current Linear team selects state for an issue owned elsewhere | W2-278 | Mutation API cannot be called without the target issue binding; mixed-team fixture selects issue owner |
| Durable historical Work lookup separate from current plan join | Removing a Project from current PM snapshot makes old status fail | ENG-15 | Historical Project remains queryable with no current-plan row |
| One Home keeper reconciles every open Work through the Run controller | A Task with a dead Project parent has no recovery observer, or a second dispatcher races the parent | ENG-5 | Kill the parent and Task Run; the keeper performs the same reserve transition and uniqueness admits one recovery Run |
| Task-scoped writable workspace capability | Provider mutates canonical main through an extra writable directory | ENG-14 | Provider launch rejects writable roots outside the Task workspace |
| One SQLite transaction/retry policy for control writes | `SQLITE_BUSY` drops the receipt while the side effect continues | ENG-7 | Fifty-process contention test records every accepted transition exactly once or returns a typed pre-effect failure |
| Append-only input/receipt rows plus minimal required transition updates | A handwritten partial SQL update silently drops an in-memory field such as interaction policy | W2-298 | Required-column round trips and transition tests fail at write/parse time; no wide Session aggregate is partially rewritten |

Some failures cannot be made impossible by data shape: a provider can lie, a machine can disappear, tmux can be killed, SQLite can remain unavailable, and an external side effect can have an unknown outcome. The new model must make those cases explicit and recoverable:

- missing provider usage stays unknown;
- unknown Send stays immutable and falls back to a later seed;
- unknown external effect creates `WaitOn::Effect`;
- provider disconnect becomes a failed/unknown Launch observation handled by the shared recovery path (PRD-5);
- unreachable Home is fresh topology evidence, not copied Work status;
- executor absence is proved by containment observation, not inferred from a provider response;
- unobservable provider-native writers make that provider mode unsupported rather than optimistically complete.

## Implementation sequence

### 0. Ratify the executable spec

Rewrite `scratch/architecture.md` around the target contract above. Remove the earlier required-Turn Ack, Handle graph, Block/Sleep split, Interaction entity, `settle`, and Session/body terminology. Add transition tables for Epoch, Run, Launch, boundary outcome, Wait resolution, and completion.

Create fixtures for the four provider shapes before changing persistence:

- live steer accepted;
- live steer rejected and later seeded;
- provider response unknown after send began;
- opaque TUI with no Turn events.

Done when:

- every stored type and public mutation fits on the target-contract screen;
- each state transition names its transaction, unique constraint, lease check, and side effect order;
- architecture and plan disagree on no noun or lifecycle boundary;
- unresolved questions are limited to implementation details that cannot change wire or persistence shape.

### 1. Cut persistence to Work, Epoch, Basis, and Home

Add stable ids and foreign keys first:

- retain Wave, Project, and Task as stable Work rows;
- create Epoch rows and map current/historical Project and Task Sessions into them;
- add one current revision allocator per Epoch;
- add stable locally generated `HomeId`; treat hostname/address as routes;
- separate historical Work lookup from current PM snapshot projection;
- move external authority bindings onto the object they mutate.

Use newtypes for every id. Put required DTO fields in Rust/Swift/JSON fixtures with no defaults.

Migration rules:

- a successor chain for one authored Project/Task maps to one Work with multiple Epochs only at terminal restart boundaries;
- nonterminal generations and retries map to Runs/Launches later, never Epochs;
- initial/work-revised directives become Work truth revisions;
- replacement/follow-up prose becomes ordered Steer input in the matching Epoch;
- stale provider session fields are not copied onto Work;
- historical rows remain queryable after their Project disappears from the current roadmap.

Done when:

- a copied dogfood database migrates forward and passes referential checks;
- every Project belongs to exactly one Wave and every Task to exactly one Project;
- the database enforces one open Epoch per Work;
- hostname changes do not change Home identity;
- no production query needs a Session id to locate current Work.

### 2. Replace Session/body lifecycle with Run, Launch, and containment

Implement one controller shared by Wave, Project, and Task:

- `reserve`, `advance`, and `stop` transactions;
- one active Run uniqueness constraint and opaque lease;
- immutable triggers and Run lineage;
- Launch route, state, optional resume token, and containment identity;
- optional Turn and low-level Exec receipts;
- keeper recovery through the same controller;
- bounded SQLite contention policy around every control transaction;
- Task workspace write boundary enforced at Launch construction.

Move Task/Project-specific closure rules and flow selection behind typed boundary inputs, not duplicate process state machines.

Done when:

- the same transition suite runs against Wave, Project, and Task Work;
- reserve-versus-reserve, input-versus-advance, done-versus-input, stop-versus-start, and reap-versus-recovery races have deterministic tests;
- killing the provider at every side-effect boundary leaves either a recoverable Run or an explicit unknown effect, never a second writer;
- Run cannot be terminal while an owned Launch/containment is live;
- fifty concurrent local writers lose no accepted receipt;
- Task providers receive no writable canonical-main path.

### 3. Cut child control to Steer and typed evidence

Replace `ChildCommand`, `ChildDirective`, and their Task/Project variants with:

- one `Steer` append path;
- one Epoch revision allocation;
- immutable `Send` attempts;
- typed decision/approval records;
- typed CI incidents and Run triggers;
- direct reconstruction from canonical Work/domain facts rather than persisted prompt mirrors;
- explicit `interrupt`, `done`, and `abandon` controls;
- authenticated Human or active parent Run request context.

Persist Steer before attempting the provider. Permit at most one live Send attempt for a Steer/Turn. Render all still-outstanding Steers as one ordered seed projection while preserving their individual receipts.

Done when:

- Human→Wave, Wave Run→Project, and Project Run→Task call the same Steer function;
- a Task Run cannot target sibling/parent Work and a stale parent lease cannot target a restarted child Epoch;
- absence of an env var never changes caller identity;
- the wire request cannot submit `Author` or otherwise claim Human provenance;
- callers cannot request live, seed, replace, or retry behavior;
- CI failures enter only through `CiIncident` and reserve one bounded repair Run;
- `ChildCommandKind`, `ChildDirective`, `Replacement`, `FollowUp`, command `Resume`, and command `Decide` have no production references.
- changing any authored Work field changes the next rendered seed without a second copy/update path.

### 4. Make every provider satisfy one reconstruction contract

Define the adapter boundary in outcomes, not capability flags:

```text
send_current(turn, steer) -> Sent | NotSteerable | Unknown | Failed
interrupt(boundary)       -> Ended | Fenced | Unknown
launch(seed, route)       -> Launch
observe(boundary)         -> Progress | Succeeded | Failed | Unknown
```

Codex may live-send. One-shot Claude falls back to seed. OpenCode may live-send only where its observed semantics meet the contract. Opaque TUI emits no Turns. Provider-wide `supports_steer` is insufficient because steerability varies by active Turn kind.

The reconstruction renderer reads current Work truth, outstanding Steers, typed decisions/approvals, flow position, workspace/HEAD, PR/CI/review evidence, and Loopflow-mediated effect receipts. Provider transcripts and summaries are optional hints.

Done when:

- one conformance suite proves equivalent durable outcomes for live, seed-only, persistent-session, and opaque-TUI fakes;
- Codex, Claude, and OpenCode adapters pass the suite;
- losing every resume token still produces a valid next Launch;
- provider/account/model fallback stays inside the current Run;
- interrupted/failed partial Turn usage is retained when reported;
- no correctness branch depends on a native child-session id.

### 5. Collapse waiting, interaction, and status projection

Add typed Wait storage and one status/attention projection used by CLI JSON, Swift, monitoring, and reconciliation.

Replace interactive records with Launch behavior:

- attended interactive flow step starts a TUI Launch behind tmux;
- Swift lists/open launches by `LaunchId`, including when no window is attached;
- only human-routed live TUI Launches derive `AwaitingHuman`;
- non-blocking flow sends the request to the parent and turns its response into a child Steer;
- explicit TUI `done`, handback, or failure records the opaque boundary outcome.

Done when:

- no stored `Blocked`, `Sleeping`, `AwaitingHuman`, or generic `status_reason` drives legal actions;
- every Wait names the exact fact that can wake it;
- clearing a Wait without satisfying/invalidating that fact is impossible through the API;
- closing Swift does not end the TUI Launch; reopening Swift can attach to it;
- killing tmux removes attention and creates recovery evidence;
- parent-routed review never appears in the human attention queue;
- CLI, Swift, and keeper fixtures produce byte-for-byte equivalent status facts.

### 6. Make Turn the sole usage authority

Route provider usage through `ConversationEvent::TurnUsage` into the observed Turn row. Derive every higher-level total by joining Turn → Launch → Run → Epoch → Work.

Keep `provider_account_limits` as a separate latest quota snapshot. Keep raw provider events only as audit artifacts. Remove spend from process/run-event rows and rename trace ids that collide with product Run ids.

Done when:

- `lf usage`, `lf top`, budgets, and monitoring read the same Turn query;
- OpenCode usage cannot be replaced by a synthetic zero event;
- absent, zero, partial, failed, and interrupted usage fixtures remain distinct;
- retry/new Run costs add rather than overwrite;
- summing all Turn rows exactly reproduces every displayed aggregate;
- `boundary_spans` is not a spend authority.

### 7. Purge the old architecture and gate the cutover

Delete old modules, commands, DTOs, schema objects, fixtures, and tests as their replacement passes. Do not retain aliases or deprecated JSON fields.

Run the Mitchell-style review against the resulting public API and persistence shape:

- can the system be explained by the target-contract screen;
- can each 2 a.m. failure identify owner, evidence, and legal next action;
- does any abstraction exist only to support a removed provider/session distinction;
- can another file or table be deleted without losing a distinct fact.

Done when:

- all deletion guards below pass;
- Rust format, clippy, tests, migration tests, DTO round trips, and Swift tests pass;
- provider smoke tests pass where credentials are available without exposing secrets;
- a copied dogfood database survives migration and recovery drills;
- no dual read/write, fallback parser, old enum case, or compatibility DTO remains;
- Rust code is at least 8,000 lines below the 133,974-line baseline, with about 12,000 net lines removed as the working target;
- any shortfall from the deletion target names the retained distinct truth, not schedule pressure.

## Deletion ledger

### Delete as complete concepts

These files implement the Interaction/Handoff split that Launch replaces. They total 4,803 physical lines today. Preserve useful tmux/app-opening mechanics only by moving the smaller remainder behind Launch; none of the old domain names or DTOs survive.

```text
rust/loopflow/src/interactive_handoff.rs
rust/loopflow/src/interaction_review.rs
rust/loopflow/src/task/interactive_rendezvous.rs
rust/loopflow/src/store/interactive_handoffs.rs
rust/loopflow/src/store/sqlite/interactive_handoffs.rs
rust/loopflow/src/store/interaction_reviews.rs
rust/loopflow/src/store/sqlite/interaction_reviews.rs
rust/loopflow/src/lf/commands/handoff.rs
rust/loopflow/src/lf/commands/reviews.rs
rust/loopflow/tests/handoff_tests.rs
rust/loopflow/tests/task_review_authority_tests.rs
swift/Loopflow/Models/InteractiveHandoff.swift
swift/Loopflow/Models/HandoffSurface.swift
swift/LoopflowMac/Services/HandoffSurfaceLauncher.swift
swift/LoopflowTests/HandoffSurfaceLauncherTests.swift
swift/LoopflowTests/HandoffSurfaceTests.swift
tests/fixtures/dto/interactive_handoff_list.json
tests/fixtures/dto/interactive_handoff_attach.json
```

Also remove Handoff/Review branches from `ActiveSessionsView`, `RegistryQuery`, command registration, generated help, DTO fixtures, and snapshots. A smaller `LaunchSurfaceLauncher` may retain generic Warp/terminal opening logic; it consumes a Launch attach descriptor, not a Handoff.

### Replace and substantially shrink

| Current area | Remove | Retain in the new shape |
| --- | --- | --- |
| `child_session.rs` | command/directive/source/effect/state, body generation, successor Session vocabulary | Work/Epoch references move to focused core modules |
| `child_control.rs` | command-specific claim/delivery/interrupt/replacement branches | Steer/Send controller only |
| `store/child_sessions.rs` | duplicated Project/Task lease and lifecycle API | shared Run repository |
| `store/sqlite/child_sessions.rs` | parallel SQL, generation settlement, successor copying, provider fields | shared Run/Launch transactions |
| `project_session/mod.rs` | ProjectSession identity/status/process fields | Project Work plus Epoch-specific domain state |
| `task/mod.rs` | TaskSession identity/status/process and interaction policy | Task Work, PR/CI/flow domain facts |
| `task/runner.rs`, `project_session/runner.rs` | duplicate reserve/activate/settle/reap/command loops | domain boundary selection over shared Run controller |
| `ops/child.rs` | successor routing and copied provider-session state | Work lookup and typed route projection |
| `ops/task.rs`, `ops/project.rs` | Session-shaped control DTOs and lifecycle cross-products | Task/Project closure and external domain operations |
| `task/actions.rs` | action matrix over stored Waiting/Blocked/revoked states | actions derived from one projection |
| `flowloop/wave.rs` | provider-specific inbox/interrupt/session branches | orchestration through Steer and Run controls |
| `lf/commands/runs.rs` | trace id pretending to be product Run, spend reconstruction | product Run/Launch/Turn view |
| `lf/commands/usage.rs`, `lf/commands/top.rs` | `boundary_spans` spend path | Turn aggregation query |

Replace old succession tests with Epoch/recovery tests rather than mechanically porting them:

```text
rust/loopflow/tests/task_session_succession_tests.rs
rust/loopflow/tests/task_successor_resolution_tests.rs
```

### Drop from the live schema

Keep shipped migration files as history, but add one forward migration that copies required facts and drops the old live objects:

```text
project_sessions lifecycle/process columns
task_sessions lifecycle/process columns
project_events/session_id shape
task_events/session_id shape
child_commands
child_directives
interactive_handoffs
interaction_reviews
provider_session_id on Project/Task Work
process_generation, process_pid, process_started_at, process_lease_state,
process_outcome_json, revoked/reaped settlement columns on Session rows
token/cost/provider/model spend columns on run_events
```

Migrate retained domain rows such as Task PRs, CI incidents, Linear observations, and review evidence to stable `TaskId` plus `EpochId` where pursuit-specific. Migrate provider/account pinning to Launch route identity. Do not drop audit history before its durable replacement has been verified on a copied database.

### Symbols and cases that must reach zero production references

```text
ChildCommand
ChildCommandKind
ChildCommandState
ChildCommandEffect
ChildCommandSource
ChildDirective
DirectiveKind
ProjectSessionStatus
TaskSessionStatus
InteractiveHandoff
InteractionReview
InteractionId
HandoffSurface
Replacement
FollowUp
command Resume
command Decide
Blocked
Sleeping
settle
finish(run) -> Continue
HandleId
Actor
Ack
```

Provider adapter internals may still say provider session/thread id. Production Work, Epoch, Run, and public DTOs may not.

## Test matrix

### State and race tests

- reserve × reserve;
- reserve × terminal Epoch transition;
- new input × `advance` to Wait;
- new input × `done` commit;
- live Steer × Turn success;
- interrupt × already-dead executor;
- stop × new reserve;
- reap observation × keeper recovery;
- provider fallback × stop;
- duplicate CI webhook × crash after reserve;
- Wait resolution × duplicate external event;
- fifty SQLite writers × receipt persistence.

Use deterministic barriers around transactions and side-effect boundaries. Do not add sleep-based race tests.

### Provider conformance tests

Each adapter must prove:

- durable-first Steer persistence;
- exact active-boundary correlation where live steer exists;
- typed `NotSteerable` fallback;
- Unknown delivery behavior;
- fixed starting Basis;
- interrupt terminal/fence observation;
- continuation loss reconstruction;
- partial usage preservation;
- containment cleanup.

The fake harnesses are normative. Credentialed provider smoke tests prove the adapter still matches the fake contract but are not the only specification.

### Reconstruction tests

Delete in turn:

- provider transcript;
- resume token;
- provider account availability;
- process receipt tail;
- current PM snapshot row for historical Work.

The renderer must either produce a sufficient seed from domain truth or name the exact typed Wait. It may never silently start from an empty prompt or reuse an incompatible token.

### DTO and projection tests

One fixture set round-trips through Rust, JSON, and Swift for:

- Work/Epoch status;
- Run/Launch/optional Turn;
- Steer/Send receipts;
- typed Wait;
- human attention;
- typed route and lineage boundaries;
- nullable usage.

Every field is required or explicitly optional. No language supplies a wire default.

## Measures

Record after each checkpoint:

| Measure | Baseline | Done |
| --- | ---: | ---: |
| Rust code | 133,974 | ≤125,974; target ≈121,974 |
| Complete old interaction/handoff physical lines | 4,803 | 0 old concept lines |
| Authored-direction domain types | command + directive + review + handoff | 1: Steer |
| Public Run lifecycle verbs | at least reserve/activate/finish/revoke/reap plus runner variants | 3 internal: reserve/advance/stop |
| Stored Work lifecycle states | multiple Session/lease/interaction enums | 3 Epoch states |
| Additive usage authorities | 2 | 1 Turn ledger |
| Provider-independent steering fixtures | fragmented | 4 shapes, one contract |
| Production references to deletion symbols | current | 0 |

Net reduction matters because this architecture deletes duplicate truth. It is not a license to compress readable code or count removed tests without replacing their behavioral proof.
