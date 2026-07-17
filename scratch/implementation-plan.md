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

## Current implementation review

Review snapshot: `c03c74cd0` plus the paused, uncommitted durable-input attempt
and the revised canonical architecture in `docs/architecture.md` on 2026-07-17.

Disposition: **the adapter and Turn-spend foundations are coherent. The paused
durable-input work is additive reconnaissance, not a checkpoint. Resume only as
a large authoritative replacement and deletion pass.**

The implementation found one sound seam:

- provider-wide `supports_steer` is gone;
- `send_current` reports `Sent | NotSteerable | Failed | Unknown` per active provider Turn;
- Codex correlates `turn/steer` with the expected vendor Turn and waits for the provider response;
- ordinary Steer no longer implies interrupt on Claude or OpenCode;
- trace-only Rust ids moved from `RunId`/`ProcessId` to `TraceId`/`ExecId`, freeing the product names;
- the separate usage slice is deleting spend writes from `run_events` and moving readers toward `agent_turns`.

That is adapter groundwork, not the core cutover. Current state against the phases below:

| Phase | Status | Evidence still missing |
| --- | --- | --- |
| 0. Executable spec | Complete | `docs/architecture.md` fixes current versus target truth, stored nouns, constraints, transitions, side-effect order, races, migration, research, questions, and next vertical slice |
| 1. Work/Epoch/Basis/Home | Uncommitted scaffolding | New ids, tables, migration code, and revision APIs exist in the working tree, but current Work does not traverse them exclusively |
| 2. Run/Launch/containment | Uncommitted bridge | Product Run fencing is being threaded through old child-session stores; Session/body lease stores and duplicated runners remain authoritative |
| 3. Steer/typed control | Uncommitted bridge | Steer/Send storage exists, but `ChildCommand`, `ChildDirective`, replacement/resume/decision variants, ambient authority, and explicit Ack remain |
| 4. Provider reconstruction | Partial | Dynamic Send outcome and controller conformance exist; reconstruction, fallback Launches, durable Send rows, and Basis correlation do not |
| 5. Review/attention/status | Redesigned, not implemented | Review is now an interactive flow interval derived from Launch + attention; `InteractionReview`, Handoff, dispositions, and parent starvation remain in code |
| 6. Turn usage | Store/query complete | Every additive reader uses Turn spend; OpenCode still needs one end-to-end usage producer/parser |
| 7. Purge | Not started | Every named legacy module/schema object remains |

### Findings that change the next implementation step

1. **A confirmed live send is not crash-durable.** `send_current_input` marks the `ChildCommand` accepted, converts the future seed to anonymous `PendingInput::system`, and keeps it only in memory. A crash after `Sent` can lose the later seed. The immutable Steer must remain authoritative; Send is only its receipt.
2. **Live Steer is not completion-fenced.** Task completion checks unincorporated directives, not Steer commands. The active Turn can still complete Work after a live Steer advanced what it should honor. Basis must land with Steer, not as later hardening.
3. **Narrow typed tool responses cannot inherit Steer timing.** The current
   generic `Decide` command records resolution only after provider delivery. On
   a seed-only harness, a tool can wait on prose that waits for its Turn to end.
   Delete generic decision/review machinery; where a tool truly requires a
   typed response, persist and release it before optional notification.
4. **The provider fixture was descriptive, not conformance.** Its test asserted literals in `control_contract.json` and never invoked an adapter or controller. **Done (2026-07-17):** the fixture is deleted and the four shapes drive `absorb_commands`/`apply_input`, asserting that each still seeds the next boundary and never interrupts.
5. **Codex outcome cleanup is incomplete. Done (2026-07-17).** A `PendingReply` guard releases the waiter on every terminal path, and rejections are typed from probed live evidence (see `scratch/questions.md`): codex 0.144.5 answers *every* rejection with `-32600`, so only the message separates the expected Turn-boundary race from a Loopflow bug. Timeout, late response, disconnect, mismatched Turn, and explicit rejection are covered; each test was verified to fail against the pre-fix behavior. The `Sent` response shape remains assumed rather than observed.
6. **The usage store/query cutover is done.** `agent_turns` is the only additive grain; Rust, Swift, CLI, Doctor, and monitoring use the shared Turn-spend wire. Cache-only, explicit zero, and absent usage remain distinct.
7. **One table still has two parser/producer paths.** Legacy agent launches update Turn usage through `StreamEvent::Usage`; harness launches use `ConversationEvent::TurnUsage`. Both reach `agent_turns`, but W2-289 remains possible until one normalization function/event owns replacement-versus-accumulation semantics.
8. **Fixed during gate:** `lf top` no longer reads raw Codex session logs or conditionally suppresses persisted Codex Turns. Its throughput chart reads the same `turn_spend_since` query as `lf usage`; live process inspection remains a separate non-spend signal.
9. **Fixed during the usage cutover:** `lf doctor`'s “agent ran but no usage was captured” check moved from `run_events` to terminal Launch/Turn evidence. It reports absent provider usage without converting it to zero, which is how the remaining OpenCode parser gap stays visible.
10. **Project-owned Review is the common dogfood bottleneck.** A deferred interactive Task step creates `InteractionReview`, leaves the Task harness alive but idle, and requires the current Project Session lease to exchange messages and record a disposition. The Project sees it through its normal observation queue, behind its own work, and cannot even start while canonical main is dirty. Authority is preserved by starving the child.
11. **Review is conversation, not decision state.** A Review may be critique, questions, brainstorming, or direction. It is the interval where a flow's current step is interactive and its live Launch owes attention to `User | Parent(WorkRef)`. Steers and Turns occur inside it. Close advances the flow; it carries no approval or changes-requested disposition. There is no Review row or `ReviewId`.
12. **Parent responsiveness is scheduler behavior.** Wave and Project must drain direct User interaction, awaiting child Reviews, and other unblocking child evidence before beginning their own background flow. Child attention may explicitly interrupt a non-steerable background Turn; this does not change plain Steer semantics.
13. **`User` replaces `Human`.** User means an authenticated external Loopflow client, whether a person in Swift/CLI or another system's agent. Internal Loopflow authority remains `Run(RunId)`.
14. **Small checkpoints optimize against the desired simplification.** The current working tree is 136,525 Rust lines and still contains all legacy authority. The next pass must cross the structural cut and delete at least 14,551 net lines to reach the 121,974 target, not preserve additive scaffolding as progress.

No more behavior should be adapted around `ChildCommand`, `InteractionReview`,
or Handoff. The next checkpoint is the large core-control cutover specified in
`docs/architecture.md`: stable Epoch revision, Run/Launch, Steer/Send, fixed
Basis, completion fencing, Review/attention, responsive parent scheduling, and
legacy deletion land together. No intermediate state counts as achieved merely
because it compiles.

## Target contract

### Durable nouns

| Noun | Stored truth | Deliberately absent |
| --- | --- | --- |
| Work | Stable Wave, Project, or Task identity and parentage | Session identity, provider state |
| Epoch | One terminal pursuit of Work: `Open`, `Done`, or `Abandoned` | Retry count, provider generation |
| Basis | `(epoch, rev)` for all prompt-relevant durable input | Separate truth/directive/response cursors |
| Steer | Ordered authored direction from `User` or an active parent `Run` | Replace, resume message, lifecycle command |
| Run | One wake-to-wait execution authority and lease | Provider transcript, process generation |
| Wait | The typed fact required before another useful Run | Blocked/Sleeping lifecycle states |
| Launch | One provider or TUI process lifetime, route, containment, and optional resume token | Work-level provider session |
| Turn | An observed provider exchange, its immutable starting Basis, outcome, and usage | Required recovery floor |
| Send | Immutable attempt to deliver one Steer to one observed Turn | Mutable delivery state on Steer |
| Home | Stable local execution authority identified by `HomeId` | Hostname as identity |

`Exec` remains a low-level process receipt beneath Launch. It is useful evidence, not a public control target.

There is no first-class `Actor`, `Ack`, `Handle`, `Body`, `Session`, `Block`,
`Sleep`, `Interaction`, `InteractionId`, Review row, or `ReviewId`. Review is a
projection over interactive flow position, Launch, and attention route.

### One input revision

Each Epoch owns a monotonically increasing `rev`. Any durable fact that changes the context an executor must honor allocates the next revision in the same transaction:

- authored Work truth;
- Steer;
- a typed tool response when a specific tool genuinely requires one;
- typed external evidence selected as execution input.

An execution boundary starts from one immutable `Basis { epoch, rev }`. A live Steer advances current Work Basis but never rewrites the active boundary's Basis. This one cursor replaces copied directives, directive versions, separate incorporation flags, and cross-table completion guesses.

A successful boundary records the Basis it ran from. The latest successfully observed boundary therefore derives the applied Basis; there is no writable Ack API or Ack table. A Steer remains outstanding until a later successful boundary starts at or after its revision.

For structured providers, a Turn is the boundary. For an opaque TUI, the Launch is the boundary because Loopflow cannot see inner Turns. A TUI process exit is not success. `done` or handback must record an explicit Loopflow outcome before the controller treats the boundary as successful.

### Public controls

```rust
steer(work, text, if_basis) -> SteerReceipt
close_review(work, if_basis) -> WorkStatus
interrupt(work, if_run) -> InterruptReceipt
done(run, basis) -> DoneProposal
abandon(work, reason, if_basis) -> EpochReceipt
status(work) -> WorkStatus
```

`steer` never accepts `live`, `replace`, or delivery-policy flags. The receipt may later show `Live` or `Seed` as what happened.

`close_review` is legal only for the current Review's routed User or immediate
parent. It records no disposition; it advances the interactive flow step if
Basis and flow position are still current.

`interrupt` ends the current Turn or opaque Launch boundary. It authors no direction and does not itself end the Run.

`done` proposes terminal Work completion from the current Run. The controller commits `EpochState::Done` only when the proposal Basis is current, the boundary succeeds, domain closure checks pass, no newer input wins the completion transaction, and Run-owned containment is empty.

User authority comes from an authenticated external request. The User may be a
person in Swift/CLI or another system's agent. Parent authority comes from the
active Run lease and the invariant `target.parent == source_run.work`. `Author`
exists only as Steer provenance:

```rust
enum Author {
    User,
    Run(RunId),
}
```

Linear, GitHub, timers, attachments, native subagents, and the keeper are not Authors. They append typed evidence or perform their narrowly owned recovery operation.

The core receives a non-serializable `ControlCtx`, not a caller-authored
identity field. Authenticated client entrypoints construct User context. Agent
entrypoints require the opaque current Run lease. An environment variable may
transport that lease but its absence never selects User authority.

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

### Review, waiting, and attention

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

A Review is the interval where the current flow step is interactive. It contains
ordinary Steers and Turns and derives from the flow position, active Launch,
and `attention: User | Parent(WorkRef)`. At most one is current per Work. Stable
Work routes Steer/close, `LaunchId` opens the surface, and Basis rejects stale
close. No disposition or encoded decision process exists.

Swift is one User client and opens User-routed Launches whether or not it was
open when the Review began. Parent-routed Review enters the immediate parent's
control lane. The child remains Running while its Launch is usable.

Wave and Project drain their control lane before their own pursuit work:
explicit control, open child Reviews oldest-first, other unblocking child
evidence, then background pursue/mutate/cadence. If a child Review arrives
during a non-steerable background Turn, controller policy interrupts that Turn
and seeds the already-durable input next. Parent control runs read-only and do
not depend on canonical main being clean.

### Provider and subagent boundary

A Launch stores provider, model, account, Home, containment identity, and an optional opaque resume token. Compatible continuation may use the token. Incompatible or missing continuation renders one reconstruction seed and starts clean. Provider, model, or account fallback creates another Launch in the same active Run.

Every Launch runs in a Run-owned process group or tmux containment unit. Run end requires all owned containment to be absent. Provider-native child ids are diagnostics only. Native children use the root Run's effective workspace, tools, and Loopflow authority and are attributed to that Run. A provider mode capable of continuing to mutate after root containment dies is unsupported until it exposes a reliable stop/status fence.

## Behavior changes

These are contract changes, not migration accidents.

| Area | Current behavior | New behavior |
| --- | --- | --- |
| Project/Task identity | Successor Sessions can become the new control target | Stable Work id remains the target; terminal restart creates an Epoch |
| Direction | FollowUp, Steer, Interrupt replacement, Resume message, Decide, CI-fix, and directives share a command ledger | Only authored direction and Review conversation are Steer; narrow tool responses and evidence stay typed; lifecycle uses controls |
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
| Review | InteractiveHandoff and InteractionReview create ids, policies, reviewer generations, dispositions, and terminal states | Interactive flow + Launch + `attention: User | Parent`; Steers carry conversation; close only advances flow |
| User attention | Parent and human routes share state while Swift treats Human specially | User is any authenticated external client; Swift is one client; parent Reviews enter the parent control lane |
| Typed tool response | Decision commands may also carry message text and define review behavior | Only explicitly machine-semantic tool choices stay typed; extra direction is Steer and Review encodes no decision tree |
| Outstanding direction | Individual directives/commands are rendered through delivery state | Records remain individual; renderer emits one ordered seed projection |
| Native subagents | Completion may seek provider-specific descendant evidence | Completion fences the owned executor containment; dormant provider conversations do not block |
| Authority | Ambient environment can determine Human versus parent caller | Authenticated User request or Run lease determines authority; env only transports opaque credentials |
| Home | Address/hostname can become machine identity | Stable `HomeId` is identity; address and reachability are mutable observations |
| Usage | `run_events` and `agent_turns` can both report spend; missing may become zero | Observed Turn is the only additive usage grain; missing remains `None` |
| JSON/API | Session, handoff, review, and boolean reroute shapes leak mechanics | Work, Run, Launch, Wait, Steer, typed route, and receipt DTOs encode domain meaning |
| Historical status | Current PM snapshot may be required to explain old execution | Historical Work/Epoch lookup is durable; current plan projection is a separate join |

Expected compatibility breaks:

- old Session ids no longer control Projects or Tasks;
- `lf handoff` and the current disposition-oriented `lf reviews` disappear;
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
| One monotonic Epoch revision | Truth, directive, tool-response, and completion cursors claim mutually inconsistent freshness | W2-288; directive drift | Race every input kind with `done`; stale proposal always loses |
| Reconstruction renders canonical typed facts, never a copied prompt mirror | PM JSON contains five KRs while the Project seed silently contains four | W2-288 | Change each Work field and prove the next seed reads it directly from the authoritative row/history |
| Immutable Basis on every boundary | Live input retroactively blesses an old tool call or completion | completion-fence risk | Live Steer during Turn cannot advance that Turn's Basis |
| One partial-unique active Run slot plus lease token | Two executors both hold current write authority | PRD-10; duplicate dispatch | Concurrent reserve across processes yields one Run; stale lease mutations fail |
| Run cannot terminate until all owned containment is absent | Work becomes Done while a known writer is still alive | native-subagent/cleanup risk | Spawn background child; `done` remains proposed until containment is empty |
| `stop` owns fence and cleanup in one state machine | A nonexistent process remains permanently revoked, or a dead interrupt parks User-owned work | ENG-3, ENG-4 | Kill before interrupt and during reap; reconciliation reaches one legal Run state and can reserve again |
| Failed Run immutable; new Run requires prior absence | Recovery replays unknown effects or starts beside the old writer | retry side-effect risk | Unknown external effect yields `WaitOn::Effect`; retry before reap is rejected |
| Steer is the only authored direction | CI evidence, lifecycle, decisions, and prose compete in one command enum | ENG-19, ENG-20; PRD-8 | Type-level API has no path to encode CI incident as Steer or replacement as Interrupt |
| Unique Steer revision and immutable Send attempts | Retrying an unknown delivery mutates history or live-sends twice to one Turn | child delivery ambiguity | Unique `(steer, turn, via=live)`; Unknown remains immutable and next seed still contains Steer |
| Typed `RunTrigger::CiIncident` plus incident/head uniqueness | One CI failure falls through into ordinary iterate/gate or launches two repair bodies | ENG-19, ENG-20; PRD-8 | Duplicate webhook and crash-after-reserve produce one incident and one active repair Run |
| One durable flow position advanced in the boundary transaction | A completed review/gate is re-entered, or duplicate reconciliation emits empty serial work | W2-296, W2-297, W2-300 | Reconcile the same evidence repeatedly; flow position advances at most once and then waits/runs from the resulting state |
| Review derived from flow + Launch + attention | A Task is “waiting on Project review” while its live body is idle and the Project is busy or cannot boot | dogfood parent bottleneck; interaction debt | Review needs no row/id/disposition; parent control-lane test services it before background work on every provider shape |
| Parent control lane before background flow | Project/Wave starts another pursue Turn while a child remains synchronously blocked on it | dogfood parent bottleneck | Child Review during live and seed-only background Turns becomes the parent's next handled input; FIFO among child Reviews |
| Read-only parent control Launch | Dirty canonical main prevents a Project from answering a child even though no code write is needed | dogfood safe/blocked report | Dirty-main fixture still permits Review Steer/close and denies repository mutation |
| Typed Wait, derived Running/Waiting | Stored Blocked/Sleeping/status owner contradicts active execution or missing evidence | ENG-3; PRD-6/10 | Property test every Epoch/Run/Wait combination; only legal projections construct |
| One status/attention projection over normalized facts | CLI, Swift, keeper, and roadmap disagree about counts, owner, or legal action | PRD-6 | All surfaces serialize the same projection fixture rather than recalculate it |
| User attention derived from live Launch | A terminal handoff says Work waits on a User while no openable executor exists | interaction debt | Kill tmux: attention disappears and recovery evidence appears; no stored attention flag can remain stale |
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

### Execution policy: one large authority cut

The numbered phases below are proof groups and review aids, not separate
`lf code` run boundaries. The next run owns Phases 1–3 and 5 plus the associated
Phase 7 purge as one interconnected replacement. It may reuse the paused
working-tree research, but it may not preserve a dormant target model beside
Session/body/command authority.

The pass is accepted only when:

- Wave, Project, and Task execute through the new controller;
- Review and parent priority execute through the same Steer/Launch path;
- the old writers, readers, DTOs, commands, and schemas in the deletion ledger
  are removed;
- Rust is at most 121,974 lines, a 12,000-line reduction from the 133,974
  baseline and at least 14,551 lines below the current additive working tree;
- any retained legacy-looking code names a distinct truth and is recorded here
  before acceptance.

Do not commit a types-only, tables-only, dual-write, or compatibility checkpoint
as architectural progress. If the large pass stops, update the design with the
precise blocker and resume the same cut.

### A. Close the current foundation slices

This checkpoint is complete except for the crash/completion criteria that
deliberately belong to the large core-control cut.

- ~~Replace the literal-only provider fixture test with controller tests backed by four fake protocols: live accepted, live rejected, response lost, and opaque TUI.~~
- ~~Test Codex `Sent`, provider rejection, mismatched Turn, timeout, late response, and disconnect. Remove every pending waiter on terminal outcome.~~
- Keep the Loopflow `TurnId` beside the provider Turn id so the future Send row can name both without inferring correlation later.
- Do not mark a confirmed live Steer incorporated or consume its durable source.
- Persist a narrow typed tool response before provider notification when a tool
  genuinely requires machine-readable input. Review conversation remains Steer.
- ~~Finish the Turn-spend migration and move `usage`, `top`, `runs`, `doctor`, budgets, and JSON to it.~~
- Normalize legacy stream usage and harness usage through one TurnUsage producer; delete the other accumulation semantics.
- ~~Remove raw Codex log totals from additive `top` accounting.~~
- ~~Move `lf doctor` usage/capture coverage to Launch/Turn evidence before deleting the old run-event check.~~

Done when:

- crash-after-`Sent` still leaves direction available to a later seed;
- a live Steer racing current completion prevents stale completion;
- a seed-only fake blocked in a typed tool request observes the response without
  ending its Turn first;
- ~~the conformance tests execute behavior rather than validate fixture literals~~ **done**;
- ~~Codex has no retained pending waiter after success, rejection, timeout, disconnect, or a late response~~ **done**;
- cache-only, zero, absent, failed, and interrupted Turn usage remain distinct;
- one parser path owns whether usage replaces or accumulates, and every additive total reads only persisted Turn rows;
- `lf doctor` identifies terminal agent Launches whose provider usage is absent without treating absence as zero;
- `cargo fmt --all -- --check`, `cargo clippy -p loopflow --all-targets -- -D warnings`, and the full Rust suite pass together.

The first two criteria require the large core-control cut. Until it completes,
keep the work on this branch rather than adding temporary ChildCommand
incorporation rules.

### 0. Ratify the executable spec — complete 2026-07-17

`docs/architecture.md` now contains one target contract. The earlier
required-Turn Ack, Handle graph, Block/Sleep split, Interaction entity,
`settle`, and Session/body alternatives are gone. Epoch, Run, Launch, Turn,
Wait, Send, completion, authority, side-effect order, and migration are stated
as executable constraints and transition tables.

The four provider shapes already execute through the controller:

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
- narrow typed tool-response records where a tool contract requires them;
- typed CI incidents and Run triggers;
- direct reconstruction from canonical Work/domain facts rather than persisted prompt mirrors;
- explicit `interrupt`, `done`, and `abandon` controls;
- authenticated User or active parent Run request context.

Persist Steer before attempting the provider. Permit at most one live Send attempt for a Steer/Turn. Render all still-outstanding Steers as one ordered seed projection while preserving their individual receipts.

Persist a typed tool response before trying to notify the provider. It allocates
its Epoch revision and directly releases that tool. A same-Turn Send may reduce
latency; later seed text may explain the response; delivery never owns it.
Reviews do not use this path: their conversation is Steer and close advances
the flow without a disposition.

Done when:

- User→Wave, Wave Run→Project, and Project Run→Task call the same Steer function;
- a Task Run cannot target sibling/parent Work and a stale parent lease cannot target a restarted child Epoch;
- absence of an env var never changes caller identity;
- the wire request cannot submit `Author` or otherwise claim User provenance;
- callers cannot request live, seed, replace, or retry behavior;
- a seed-only provider can observe a typed tool response without ending the blocked Turn or receiving prose first;
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

The reconstruction renderer reads current Work truth, outstanding Steers,
narrow typed tool responses, interactive flow/attention position,
workspace/HEAD, PR/CI evidence, and Loopflow-mediated effect receipts. Provider
transcripts and summaries are optional hints.

Done when:

- one conformance suite proves equivalent durable outcomes for live, seed-only, persistent-session, and opaque-TUI fakes;
- the suite invokes the controller and adapter boundary; a fixture that only describes expected rows is insufficient;
- Codex, Claude, and OpenCode adapters pass the suite;
- every Send records the Loopflow Turn and any provider Turn correlation, and terminal outcomes retain no adapter waiter;
- losing every resume token still produces a valid next Launch;
- provider/account/model fallback stays inside the current Run;
- interrupted/failed partial Turn usage is retained when reported;
- no correctness branch depends on a native child-session id.

### 5. Collapse Review, attention, waiting, and parent scheduling

Add typed Wait storage and one status/attention projection used by CLI JSON, Swift, monitoring, and reconciliation.

Replace interactive records with one Review projection:

- current interactive flow + active Launch + `attention: User | Parent` means
  Review is open;
- one Review contains any number of Steers and provider Turns;
- Stable Work routes Steer/close; `LaunchId` opens a User surface; Basis fences
  stale close;
- closing advances the flow with no disposition or approval state;
- User and parent use the same protocol; only attention routing differs;
- parent control lane drains awaiting child Reviews before background work;
- child attention live-steers the current parent Turn when possible and
  explicitly interrupts/restarts background work when not;
- Wave/Project control runs read-only and can respond when canonical main is
  dirty;
- explicit TUI close, handback, or failure records the opaque boundary outcome.

Done when:

- no Review table, Review id, disposition, reviewer generation, stored
  `Blocked`, `Sleeping`, `AwaitingUser`, or generic `status_reason` drives legal
  actions;
- every Wait names the exact fact that can wake it;
- clearing a Wait without satisfying/invalidating that fact is impossible through the API;
- closing Swift does not end the TUI Launch; reopening Swift can attach to it;
- killing tmux removes attention and creates recovery evidence;
- parent-routed Review never appears in the User attention queue;
- Project and Wave never start a background Turn while child Review attention
  is queued;
- seed-only parent harnesses interrupt background work and seed child attention
  next rather than starving it;
- dirty canonical main cannot block Review Steer/close;
- CLI, Swift, and keeper fixtures produce byte-for-byte equivalent status facts.

### 6. Make Turn the sole usage authority

Route provider usage through `ConversationEvent::TurnUsage` into the observed Turn row. Derive every higher-level total by joining Turn → Launch → Run → Epoch → Work.

Keep `provider_account_limits` as a separate latest quota snapshot. Keep raw provider events only as audit artifacts. Remove spend from process/run-event rows and rename trace ids that collide with product Run ids.

**Store landed (2026-07-17).** `run_events` lost its seven spend columns, `agent_turns` is the only additive grain, and `usage`/`top`/`trace` read one `turn_spend_since` join. `PendingUsage`, `record_usage`, `record_result`, `record_agent`, `record_stream_usage`, and `boundary_spans` are gone. Migration `0.11.029_one_spend_grain` drops the columns and `trace_capture_meta`.

The dogfood ledger settled the coverage question empirically rather than by argument — see `scratch/questions.md`. `run_events` held 103 usage rows to `agent_turns`' 779, over the same span, with every one of its 75 usage-bearing processes also holding a captured turn. It was a strict subset carrying ~40% of the spend, and it mis-attributed: the thread-local stamped whichever agent launched last in the process, so one process's claude tokens were reported under `provider = opencode`.

Two consequences worth keeping:

- `lf usage` totals rise (output 1,428,413 → 3,599,965 on the dogfood ledger). That is the reduction paying out, not a regression.
- Absent usage is now honestly `None` everywhere, which makes it invisible. `lf doctor`'s coverage check therefore moved to Launch/Turn grain instead of being deleted with the ledger it read; it names the provider (`opencode 8/8`), which is how W2-289 announces itself.

Still open here:

- one parser must own usage end to end (W2-289) — `StreamEvent::Usage` accumulates and `ConversationEvent::TurnUsage` replaces, and they reach captures through different launch surfaces;
- ~~`lf top` reads a parallel raw Codex total~~ **fixed during gate** — the raw reader is deleted; every provider reaches the one persisted Turn query.
- `lf usage --json` now names the exact Turn, Launch, trace, and exec. A shared Rust/Swift fixture pins that wire, including cache-only and explicit-null measurements; the Mac telemetry dashboard consumes it directly instead of decoding the removed boundary-span shape.

Done when:

- `lf usage`, `lf top`, budgets, and monitoring read the same Turn query;
- OpenCode usage cannot be replaced by a synthetic zero event;
- absent, zero, partial, failed, and interrupted usage fixtures remain distinct;
- a cache-only provider report remains a Turn-spend row;
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
- Rust code is at most 121,974 lines: 12,000 below the 133,974 baseline and
  14,551 below the current additive working tree;
- no shortfall is accepted as a completed core cutover.

## Deletion ledger

### Delete as complete concepts

These files implement the Interaction/Handoff decision aggregate that Review
projection and Launch replace. They total 4,803 physical lines today. Preserve
useful tmux/app-opening mechanics only by moving the smaller remainder behind
Launch; none of the old domain names, ids, dispositions, or DTOs survive.

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

Also remove Handoff/InteractionReview branches from `ActiveSessionsView`,
`RegistryQuery`, command registration, generated help, DTO fixtures, and
snapshots. A smaller `LaunchSurfaceLauncher` may retain generic Warp/terminal
opening logic. Review itself is derived from flow + Launch + attention; it has
no store module or id.

### Replace and substantially shrink

| Current area | Remove | Retain in the new shape |
| --- | --- | --- |
| `child_session.rs` | command/directive/source/effect/state, body generation, successor Session vocabulary | Work/Epoch references move to focused core modules |
| `child_control.rs` | command-specific claim/delivery/interrupt/replacement branches | Steer/Send controller only |
| `store/child_sessions.rs` | duplicated Project/Task lease and lifecycle API | shared Run repository |
| `store/sqlite/child_sessions.rs` | parallel SQL, generation settlement, successor copying, provider fields | shared Run/Launch transactions |
| `project_session/mod.rs` | ProjectSession identity/status/process fields | Project Work plus Epoch-specific domain state |
| `task/mod.rs` | TaskSession identity/status/process, interaction policy, review disposition | Task Work, PR/CI/flow domain facts and derived Review attention |
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

Migrate retained domain rows such as Task PRs, CI incidents, and Linear
observations to stable `TaskId` plus `EpochId` where pursuit-specific. Review
conversation becomes Steers/Turns and current interactive flow position; do
not copy dispositions or an interaction aggregate. Migrate provider/account
pinning to Launch route identity. Do not drop audit history before its durable
replacement has been verified on a copied database.

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
InteractionReviewId
InteractionReviewStatus
InteractionReviewDisposition
InteractionId
HandoffSurface
AwaitingHuman
Author::Human
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
- confirmed live Send × controller crash before the next seed;
- live Steer × stale current-Turn completion;
- typed tool response × seed-only provider blocked in a waiting tool call;
- child Review × parent background Turn, live steer accepted;
- child Review × parent background Turn, interrupt and seed fallback;
- child Review FIFO × parent background flow resumption;
- Review close × newer Basis or flow position;
- dirty canonical main × parent Review response;
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
- explicit provider rejection, timeout, late reply, mismatched Turn, and disconnect cleanup;
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
- User attention and parent control-lane routing;
- typed route and lineage boundaries;
- nullable usage.

Every field is required or explicitly optional. No language supplies a wire default.

## Measures

Record after each checkpoint:

| Measure | Baseline | Review snapshot | Done |
| --- | ---: | ---: | ---: |
| Rust code | 133,974 | 136,525 additive working tree | ≤121,974 |
| Named legacy child/control/interaction modules | 12,002 physical lines | 12,002 | 0 old concept lines |
| Complete old interaction/handoff physical lines | 4,803 | 4,803 | 0 old concept lines |
| Authored-direction domain types | command + directive + review + handoff | unchanged | 1: Steer; Review is derived |
| Public Run lifecycle verbs | at least reserve/activate/finish/revoke/reap plus runner variants | unchanged | 3 internal: reserve/advance/stop |
| Stored Work lifecycle states | multiple Session/lease/interaction enums | unchanged | 3 Epoch states |
| Additive usage authorities | 2 | 1 Turn ledger and query; parser normalization remains | 1 Turn ledger |
| Executable provider-independent steering shapes | fragmented | 4 shapes through the controller | 4 shapes, one contract |
| Files containing core deletion symbols | 31 | 31 | 0 |

Net reduction matters because this architecture deletes duplicate truth. It is not a license to compress readable code or count removed tests without replacing their behavioral proof.
