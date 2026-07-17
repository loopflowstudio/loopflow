# Loopflow architecture

Working design notes. These record decisions as the architecture is discussed; they are not yet an implementation spec. `scratch/implementation-plan.md` is the authoritative synthesis for the cutover; this file still contains workshop paths that Phase 0 explicitly removes or reconciles.

## Steer

> “ok just plain Steer then. i do like the go-style short names where possible”

> “Keep Steer smaller when possible. I dont really know that we need replace over just a steering message saying ‘do this instead’”

`Steer` is the durable authored input that changes or extends what an agent should do. Human-to-Wave and parent-to-child use the same concept.

Steers are ordered and append-only. “Do this instead” is another Steer. It does not mutate or formally supersede history.

Lifecycle operations remain separate: interrupt, resume, abandon, and automatic CI wake change execution state rather than direction.

### Current concepts hidden in delivery

The current child path contains:

- command — a heterogeneous inbox item containing both direction and lifecycle operations;
- directive — a versioned copy of authoritative working text;
- claim — fencing that assigns an inbox item to one body generation;
- body generation — the disposable provider process allowed to act for a Session;
- effect — currently mixes meaning (`Replacement`) with transport (`LiveSteer`, `NextTurn`);
- delivery — the fallible provider side effect;
- incorporation — proof that the Session understood the direction, distinct from provider delivery;
- runner — the controller choosing a delivery route and enforcing the lease.

### Candidate collapse

Keep the authored object small and immutable:

```rust
struct Steer {
    id: SteerId,
    work: WorkRef,
    epoch: EpochRef,
    seq: u64,
    by: Author,
    text: String,
    issued_at: Timestamp,
}

enum Author {
    Human,
    Run(RunId),
}

enum SendVia {
    Live,
    Seed,
}

struct Send {
    id: SendId,
    steer: SteerId,
    run: RunId,
    turn: TurnId,
    via: SendVia,
    state: SendState,
    error: Option<String>,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
}

enum SendState {
    Sending,
    Sent,
    Failed,
    Unknown,
}

struct Basis {
    work: WorkRef,
    epoch: EpochRef,
    truth_rev: Revision,
    steer_seq: u64,
}

```

A Steer is outstanding until a later root Turn starts from a Basis that includes it and succeeds. A successful live Send means “this Turn may react to the Steer,” not “this Turn incorporated it” and not “globally dequeued.” The next Turn's seed still includes the Steer. Send receipts prevent duplicate live attempts to the same Turn while successful root-Turn completion is the durable incorporation boundary. Claiming is an internal reservation fenced by a Run lease. `LiveSteer` and `NextTurn` become `SendVia::Live` and `SendVia::Seed` receipts. Current `Accepted` becomes `Sent`; it means provider delivery completed, not that the agent incorporated the direction. `Replacement` disappears.

A parent agent authors as its active Run, not merely as a Wave or Project id. Creating the Steer validates the source Run lease and records its Work/Epoch provenance; `if_epoch` fences the child snapshot it acted on. This prevents a stale parent executor from steering a newly restarted child while keeping human and parent direction on the same API.

There is no general Actor concept. `Author` is only Steer provenance. Authorization is request context, never a caller-supplied domain field:

- an explicitly authenticated Human may invoke legal controls on Work at its Home;
- an active Run lease may invoke the same controls only on immediate child Work;
- Linear and GitHub credentials may append only their typed truth/evidence events;
- the Home keeper may mutate only Run recovery state.

Linear, GitHub, timers, attachments, and `System` are not authors. Their typed event, trigger, or channel receipt already says where the fact came from. A missing Run credential is unauthorized, never inferred to mean Human. An environment variable may transport an opaque lease, but environment presence or absence does not choose identity.

Parent authority is one structural check, not an operation matrix: the source lease must be current and `target.parent == source_run.work`. A Wave Run can therefore control its Projects, a Project Run its Tasks, and a Task Run no durable child Work. The child state machine and `if_epoch` still reject illegal transitions. There is no separate reduced set of “agent-safe” child controls.

The active Run lease still fences every transition performed by an executor. It is execution ownership, not steering state. A Send names the exact Turn because one Run may contain many Turns; Run identity alone cannot say which provider interaction received a Steer. Separate Send rows retain retry and uncertainty history without inflating or mutating Steer.

An interrupt used to reach a provider that cannot steer live is not a Steer delivery attempt. The Steer remains `Queued` while Loopflow interrupts the current Turn, then moves to `Sending { via: Seed }` only when the next Turn is about to receive the text. This removes the current false ambiguity where a crash after interrupt but before message delivery produces `Uncertain`.

### Direction without a duplicate directive

The authoritative input to a Run can be projected from:

```text
authored Wave / Project / Task truth for the current epoch
    + ordered Steers from that epoch
    + current execution evidence
```

`Basis` is the compact receipt for exactly which durable direction seeded a Turn: the authored truth revision plus the last ordered Steer included. A Linear title or KR edit advances `truth_rev` and wakes the Work; it does not need to copy the new text into a directive row. A new Steer advances `steer_seq`. Each Turn records one immutable starting Basis. Live delivery never advances it.

A successful root Turn acknowledges its immutable starting Basis. Ack is therefore a derived fact, not an API, table, or independently writable receipt. The provider must report a normal successful Turn boundary; interrupted and failed Turns acknowledge nothing. The captured final root response is already the summary. An explicit `ack(summary)` would add a race and allow the model to assert a newer fact than the controller observed without producing stronger evidence.

Completion is legal only when the latest successful root Turn's Basis equals the Work's current Basis. If authored truth or a Steer changes during a Run, the current Basis moves ahead and the stale Turn cannot complete the Work. A live Steer never advances the active Turn's Basis, so an old tool call cannot accidentally acknowledge newly delivered input. This preserves the real stale-snapshot protection currently provided by directive versions without retaining directive identity or copied text.

Native execution quiescence is a separate completion fence, not part of acknowledgment. A successful root Turn may leave resumable child Handles, but Work cannot become Done while a descendant Turn, background task, or owned process can still mutate the workspace. A completion request made during the Turn is only a proposal tied to that Turn. The controller commits Done only after the root Turn succeeds, its Basis is still current, the owned execution tree is quiescent, domain closure checks pass, and the input-versus-settle transaction finds no newer stimulus. Unknown descendant state blocks or triggers reap; it never permits completion.

The epoch qualifier is required by current behavior. Restarting a terminal Project or Task intentionally resets its direction and lifecycle while retaining audit and ingestion history. Without an epoch, old Steers would silently become active input to the new pursuit.

### CI-fix test case

CI-fix is the negative test for Steer. The current `CiFix` command already says it is not input to a live Turn; it is a launch intent whose payload seeds a new Run. A CI failure does not author new direction. It is evidence that the Task's existing mandate is not yet satisfied.

Keep two concepts:

1. `CiIncident` — typed evidence identifying the repository, PR, failed head, and failure set. It owns deduplication and response milestones.
2. Task lifecycle — reconciliation sees an unanswered incident, atomically reserves one Run with the incident as its trigger, and runs the bounded `ci-fix` flow.

```text
observe failed checks
→ ensure CiIncident
→ Task reconciliation finds the unanswered incident
→ reserve one Run triggered by that incident
→ start one bounded ci-fix Turn from the incident evidence
→ wait for new GitHub evidence
```

`CiIncident.trigger_command_id` can collapse into the Run that responded. Reservation, incident response, and Run trigger must be one transaction. A crash after reservation is ordinary Run recovery with the same trigger; a duplicate observation resolves to the same incident. GitHub evidence decides green, merged, or blocked.

No Steer, wake command, or CI-specific delivery queue is needed. If a human separately says “fix CI now,” that authored message is a Steer, but the incident would have woken the Task without it.

### Provider pressure test

The provider APIs support the small public `Steer`, but they reject a static `supports_steer` abstraction.

| Provider surface | Input while active | Interrupt | Continuation |
| --- | --- | --- | --- |
| Codex app-server | `turn/steer` injects into an active regular Turn and requires `expectedTurnId`; review and manual compaction Turns reject it | `turn/interrupt` cooperatively ends the named Turn | `thread/resume` reopens a persisted vendor thread |
| Claude Code Agent SDK (informative) | persistent `stream-json` input queues messages in a long-lived CLI process; callers can interrupt before redirecting | `interrupt()` in streaming mode | persisted session id can resume or fork conversation history |
| Claude Code CLI as Loopflow supports Max accounts | the one-shot `claude -p` process cannot accept another input; Loopflow must queue | Loopflow kills the Turn process | the next process uses `--resume` with the captured session id |
| OpenCode server | `prompt_async` accepts a message asynchronously, but the public contract does not promise incorporation into the model request already in flight; Loopflow should treat it as next-turn delivery | `/session/:id/abort` aborts current execution | the session lives only with that server; Loopflow currently deletes it when the Launch stops |

The two Claude rows share the same Claude Code agent loop. The Agent SDK still spawns a bundled `claude` executable; its extra capability comes from keeping the CLI's `stream-json` protocol open, not from a different model substrate. Anthropic's supported third-party Agent SDK setup requires API-key or cloud-provider authentication and does not permit products to route customers through Claude.ai subscription credentials without approval. Loopflow's personal dogfood path deliberately routes isolated Max account homes through the CLI, so the one-shot row is the required portability floor. The SDK row remains useful evidence for a future API-key deployment, not justification for weakening that floor.

Three consequences follow.

1. **Steering ability is per Turn, not per provider.** Even Codex rejects same-turn input for some active Turn kinds. The controller should attempt `send_current(turn, steer)` against the exact Turn and handle a typed `NotSteerable` result; it should not branch on one provider-wide boolean.
2. **Provider acceptance is not incorporation.** Codex returning the active Turn id, OpenCode returning `204`, or Claude accepting an input into its stream proves only that the provider took responsibility for the message. `Send` records that fact. A later successful root Turn seeded from the resulting Basis proves incorporation.
3. **Loopflow owns the durable queue.** A provider-side queue may optimize delivery, but it cannot become authority unless it exposes durable correlation and replay semantics. Otherwise a disconnect leaves Loopflow unable to distinguish queued, processed, and lost input. The controller always persists first and may try the active Turn; rejection, policy, or a lost Turn-boundary race falls back to Loopflow's next-Turn seed. `interrupt` remains a separate lifecycle operation.

This is simpler than normalizing the provider protocols themselves. The public operation stays `steer(work, text, ...)`; there is no `live` flag and no promise about the delivery route. Only the Launch controller chooses whether to attempt the active Turn. `Send.via` records what actually happened; it is a receipt, not caller intent.

The portable behavior is therefore a contract over durable outcomes:

```text
steer
  persists ordered direction before attempting the provider
  eventually reaches a current or later Turn
  advances the Basis that must be acknowledged before completion

interrupt
  returns only after the current Turn is terminal or fenced
  does not author replacement direction

interrupt + steer
  provides portable preemptive redirection
```

One Steer therefore admits three execution paths without carrying a delivery mode:

| Path | Current Turn | Next Turn | Use |
| --- | --- | --- | --- |
| live steer + finish | receives the Steer when supported and may react immediately, but remains fenced to its original Basis | starts from the Steer and may acknowledge it | normal active-Turn path |
| queued finish | receives nothing new because live delivery was unavailable or lost the Turn-boundary race | starts from the Steer | automatic fallback |
| interrupt + restart | becomes terminal without acknowledging its Basis | starts from the Steer immediately | preemptive redirect |

“Restart” here means another Turn in the same Epoch and ordinarily the same Run. A one-process-per-Turn harness may start another provider process; that does not create a new Work attempt. The Run continues until the Work can sleep, block, finish, or be abandoned.

Live Codex or Claude `stream-json` delivery is an available implementation of `steer` when the exact active Turn accepts it, not a different operation or stronger product semantic. A plain Steer never interrupts and does not promise that no more side effects occur before incorporation; no provider can make that promise once a tool action has begun. A caller that needs the boundary first waits for `interrupt` to fence the current Turn, then calls `steer`; a UI may compose those operations without inventing a replacement message type. The interrupted Turn acknowledges nothing. The replacement Turn starts with the Steer and may succeed from that Basis. This distinction lets Claude, Codex, and OpenCode produce the same observable lifecycle even though one injects, one queues, and one starts another process.

Investigating Claude's persistent `stream-json` protocol is therefore useful but not architectural gating. If the supported Max-account CLI accepts additional input, Loopflow may use live delivery. If it only queues, the input becomes the next Turn's seed. If interruption is reliable, preemptive redirect gets lower latency. None of those findings change Steer ordering, Basis, completion fencing, or reconstruction.

Delivery remains observable without contaminating incorporation. One Steer may have both a live Send to Turn 7 and a seed Send to Turn 8; successful Turn 8 incorporates its whole Basis, potentially covering several Steers at once. The Steer view projects that receipt history:

```text
Steer 42
  Sent          Turn 7  via Live
  Sent          Turn 8  via Seed
  Incorporated  Turn 8  Basis { steer_seq: 42 }
```

`steer()` returns after durable creation. Status and event streams expose later Sends and the successful root Turn whose Basis covers the Steer's sequence. Callers can observe latency and provider behavior without depending on either route.

### Provider-native subagents

All three substrates also have their own nested-agent model:

- Codex creates persistent child threads with parent links. Its orchestrator can route follow-ups, wait, interrupt, and close those threads.
- Claude Code invokes isolated subagents through the Agent tool. Only their result normally returns to the parent, but custom subagents expose an id that can be resumed inside the same provider session; background tasks can be stopped. Claude Managed Agents makes the same shape explicit as persistent session threads with targeted interrupts.
- OpenCode invokes subagents through its Task tool or an `@agent` message and represents them as navigable child sessions. Its server can list, prompt, and abort sessions by id.

The shared lesson is useful, but these are not another kind of Loopflow Work. A provider-native subagent:

- has no Wave/Project/Task identity or Epoch;
- inherits the parent Run's lease, workspace authority, and completion obligation;
- may disappear with its provider Handle;
- is controlled by the root agent through provider-native messages rather than by Loopflow's public Work API.

When delegation needs independent durable direction, monitoring, sleep/wake, recovery, or human steering, the agent creates child Work through Loopflow. When it only needs temporary context isolation or parallel reasoning, it uses a provider-native subagent. Promoting every vendor child thread into Work would import three unstable orchestration models into the core.

Nested agents still affect internal receipts. A Launch may have a root Handle and a tree of child Handles; captured child Turns and parent/child messages are trace evidence. They do not advance Work Basis independently. The root Run remains responsible for incorporating their results, and completion requires its execution tree to be quiescent. Revoking or reaping a Run must stop the entire owned Handle/process tree, not only the root provider process.

### Portability acceptance suite

Run the same controller tests against every harness. Assert Loopflow outcomes, then separately record which provider mechanism achieved them.

1. **Sleeping Work:** `steer` persists one ordered Steer, reserves one Run, and seeds its first root Turn.
2. **Active additive direction:** Loopflow may deliver live or queue; in both cases receipts report the actual route, a later root Turn starts with the Steer, and Work cannot complete until that Turn succeeds from the still-current Basis.
3. **Preemptive redirect:** `interrupt` + `steer` ends the old Turn and seeds the replacement Turn on every provider.
4. **Turn-boundary race:** whether current-Turn delivery wins or loses its race with completion, the Steer remains input to the next Turn and cannot be acknowledged by the old Turn.
5. **Ambiguous acceptance:** disconnect after a Send begins records `Unknown`; Loopflow never blindly duplicates it.
6. **Ordered burst:** several Steers preserve sequence even if one provider accepts them live and another consumes them together in the next seed.
7. **Non-steerable Codex Turn:** review/compaction rejection degrades to the same queued result as Claude or OpenCode.
8. **Native subagent active:** Work Steer targets the root Handle; completion waits for the root to incorporate the child's result. Direct child-thread steering is provider trace behavior, not a different Work API.
9. **Run revocation with descendants:** root process, provider child sessions, and background tasks all lose write authority before another Run reserves the Epoch.
10. **Lost Handle or provider handoff:** the replacement Launch reconstructs current Basis and outstanding evidence without relying on vendor transcript survival.

This suite is a better compatibility target than identical wire behavior. The providers may differ in latency and transcript shape; Work state, ordering, authority, and completion must not.

### Product sentence

> Each Steer is durable. The active Turn receives it now or a later Turn starts with it. Loopflow records which Turn received it and whether the Work incorporated it.

### Open questions

- Can a failed Send be retried automatically? `Unknown` cannot be, because the provider may already have received it.
- Should a decision response create a Steer linked to the decision, or remain a separate delivery kind?
- On a new Run, should unacknowledged Steers be replayed individually or rendered into one seed projection with one Basis?

## Sleep

> “can a task (or a wave) go into a sleep state?”

Yes. Sleep is the durable condition of live intent with no active Run. The current system already approximates it as Wave `Idle` and Project/Task `Waiting`.

```text
Running --settle--> Sleeping --durable stimulus--> Running
```

A stimulus is stored in its own truthful form:

- a human or parent instruction is a `Steer`;
- failed CI is a `CiIncident`;
- child progress is an observation;
- a scheduled wake is a timer occurrence;
- a manual retry is an explicit wake operation.

The stimulus is not wrapped in a generic wake command. Reconciliation observes it, atomically reserves one Run with that stimulus as the trigger, and launches it. The Run lease prevents two reconcilers from waking the same target twice.

```rust
enum WorkState {
    Open,
    Blocked,
    Done,
    Abandoned,
}
```

Running and Sleeping can be projections rather than stored Work states:

```text
Open + active Run    = Running
Open + no active Run = Sleeping
```

Starting, failed, stalled, lost, and interrupted describe Runs. They do not need to become durable Work states.

`Sleeping` and `Blocked` are different:

- Sleeping has a computable wake condition owned by Loopflow. No human action is currently required.
- Blocked names the external actor or missing fact that must arrive before Loopflow can proceed.

The wake condition and trigger come from domain evidence rather than a free-text status reason. A Task sleeping on CI derives that from its open PR and incident state. A Wave sleeping until its heartbeat derives that from its cadence. The state itself stays small.

The listener, resident, runner, and provider process are runtime observations, not the sleep state. A Wave may be logically sleeping while its lightweight listener remains online. A Task may be sleeping with no process at all. Conversely, a durable `Running` state with no active Run is stranded work, not sleep.

### Consequences

- Sending a Steer to a sleeping Wave, Project, or Task wakes it automatically.
- A CI incident wakes a sleeping Task without becoming a Steer.
- A child observation wakes a sleeping parent.
- A heartbeat or cron wakes a sleeping Wave.
- `resume(message)` splits into an appended Steer, which wakes automatically.
- A bare manual retry remains a lifecycle wake, with the prior failure as its trigger.
- Every Run records why it woke, making launch provenance and deduplication the same fact.

### Open questions

- Does Project share the same `Sleeping` state explicitly? Current behavior says yes.
- Can a target choose to sleep indefinitely with no automatic wake condition, or is that necessarily Blocked?
- Which lightweight process owns timer evaluation when every Wave on a Home is sleeping or offline?

## Work, Run, Turn, and Thread

> “now lets talk about ‘bodies’ and ‘sessions’ — again, what concepts are in this space and can they be condensed?”

### Current concepts

`Session` currently carries several meanings:

- a local durable execution identity beneath a Linear Project or Task;
- the owner of lifecycle state, worktree, PR history, direction, provider choice, and current process generation;
- one terminal attempt in a predecessor/successor chain;
- elsewhere, the provider's resumable conversation identifier.

`Body` also carries several meanings:

- a leased Project or Task runner process generation;
- the provider process and transcript used by that generation;
- one Wave playhead attempt at a flow step;
- the subject of liveness, progress, recovery, handoff, and legal-action projections.

Nearby concepts are generation, lease, process, resident, runner, invocation, pass, turn, agent launch, trace, and provider session. The vocabulary reflects implementation layers more than product boundaries.

### What the code confirms

Three common public execution concepts hold:

- **Work** — the durable Wave, Project, or Task itself.
- **Run** — one bounded activation of Work, from a durable wake cause until the Work sleeps, blocks, or ends.
- **Turn** — one model interaction inside a Run.

**Thread** is different: it is the Wave's durable, journal-backed human conversation, assembled from many Runs and provider processes. It is not a neutral name for a vendor continuation id. Project and Task Turns form private trace/transcript evidence; they do not become public chat Threads. This preserves the settled “one thread per Wave” product model.

An opaque vendor continuation id is a provider **Handle**. A compatible Launch may reuse it, but it is not conversation identity or authoritative memory.

The current outer Project/Task process generation is already wake-to-sleep shaped. One generation claims work, starts one harness, may execute several flow steps and provider Turns, and settles when the Project or Task waits, blocks, completes, or fails. A Wave has the same logical boundary at one pass even though its lightweight resident process survives across passes.

The current inner Wave `BodyProvenance` is a different grain: one harness launch for one flow step. It maps much more closely to a Turn attempt than to the outer Project/Task body generation. This confirms that `Body` currently names two things.

The code also proves one non-public boundary is real: an **Epoch** is one Work pursuit from its start until completion, abandonment, or archival. It spans many sleep/wake Runs. Projects and Tasks visibly need multiple Epochs; a Wave normally has one long-lived Epoch. Starting again creates a fresh Epoch, while recovery may explicitly carry selected direction, workspace, and PR history from the prior Epoch. Epoch is historical scope and stale-write fencing, never a user control target.

This is not the current Task `phase_epoch`, which increments on Kickoff/Iterate/Gate transitions inside one Session. That field is a lifecycle-position revision and should be renamed `phase_rev` if Work Epoch lands; using `epoch` for both would recreate the ambiguity at a smaller scale.

### What the code disproves

The first draft was too aggressive in five places.

1. **Project and Task IDs cannot disappear.** `ProjectSessionId` and `TaskSessionId` are currently the only Loopflow-owned identities. Linear IDs are external bindings. The reduction must promote the local IDs to `ProjectId` and `TaskId`, not replace them with Linear IDs.
2. **Run history alone cannot replace Session successors.** A terminal restart resets direction, lifecycle cursor, provider Handle, workspace/PR sequence, and current status while preserving audit and ingestion dedupe. Recovery additionally chooses what crosses that boundary. Stable Work therefore needs an Epoch record, not merely a counter. The Epoch is not a control target and does not participate in parent routing, but Runs, Steers, PRs, and reviews must name it.
3. **A Run is not a process.** Task and Project currently happen to use one runner process per activation; a Wave reuses a resident across many Runs. Process ids, groups, tmux names, host, and binary provenance are executor receipts under a Run, not Run identity.
4. **An occupied active-Run slot is not enough to report Running.** A reserved Run may never boot; an active Run may be dead, stalled, revoked-but-not-reaped, or unobservable from this Home. Monitoring remains a projection over Work intent, Run lease state, liveness, and progress.
5. **`provider + vendor id` is not a sufficient Handle key.** Codex and Claude resume only through the account that created the vendor transcript; current routing pins that account. A different or unhealthy account deliberately starts a new Handle. OpenCode's current session is server-local and is deleted at harness stop, so it cannot resume across Runs at all. In a decentralized installation, Home locality matters too.

There is also a namespace collision. The current `RunId` groups nested `lf` processes into a diagnostic trace, while `lf runs` presents an `AgentLaunchId` as a run. Neither is the product Run above. If product `Run` is adopted, the current `RunId` should become `TraceId`, `ProcessId` should become `ExecId`, and product-facing `lf runs` should use the new Run record. Agent launches and Turns remain trace detail.

### Stable identity and containment

Wave, Project, and Task are the only durable Work identities. Their ids never change because execution stops, an external title changes, a Project begins another pursuit, or a Task is recovered.

| Identity | Meaning | Parent | Public control target |
| --- | --- | --- | --- |
| `WaveId` | one durable operating context | none | yes |
| `ProjectId` | one measured bet | exactly one `WaveId` | yes |
| `TaskId` | one concrete mandate | exactly one `ProjectId` | yes |
| `EpochRef { work, n }` | one historical pursuit of Work | Work | no; stale-write precondition and history selector |
| `RunId` | one wake-to-sleep ownership period | Epoch | status/interrupt receipt, not direction target |
| `LaunchId`, `TurnId`, `HandleId` | provider execution evidence | Run/Launch | diagnostic only |

`ProjectId` and `TaskId` must be local Loopflow ids. A Linear Project or issue id is an external binding that may be renamed, moved, unavailable, or eventually replaced by another planning system. It cannot be the identity that owns local history. The migration mints one stable local id for each current Project/Task chain; it does not choose one old Session attempt as the conceptual identity.

Containment uses stable ids:

```rust
struct Project {
    id: ProjectId,
    wave: WaveId,
    // authored truth binding and current Epoch
}

struct Task {
    id: TaskId,
    project: ProjectId,
    // authored truth binding and current Epoch
}
```

A Project advancing to another Epoch does not reparent its Tasks. Task observations always route to the stable Project, eliminating current successor-chain routing. An explicit Task move changes its stable `project` relation and records placement history; an external PM mismatch cannot silently reparent live Work. Project movement between Waves likewise requires an explicit quiescent move because it changes Home and authority.

There is no stored generic Work row. `WorkRef` is the typed union accepted by common APIs, while Wave, Project, and Task retain their own authored fields and domain lifecycle policy.

### Epoch transition and carry contract

An Epoch starts only when Work begins a terminal pursuit and ends only as Done or Abandoned. Sleep, wake, block, retry, provider handoff, phase iteration, PR rotation, and Run recovery remain inside the same Epoch.

```text
no Epoch --start--------------------------> Epoch 1 Open
Open <----sleep/wake/block/unblock/Run----> Open or Blocked
Open/Blocked --complete-------------------> Done
Open/Blocked --abandon--------------------> Abandoned
Done/Abandoned --restart fresh------------> Epoch n+1 Open
Abandoned Task --recover prior work-------> Epoch n+1 Open
```

Creating Epoch `n+1` requires Epoch `n` to be terminal and to have no unreaped active Run. `Blocked` is not terminal and never advances the Epoch. A completed Task that is explicitly reopened starts Fresh; it does not acquire a new Task id. Task recovery is the only initial Recover operation because it has demonstrated workspace/PR carry semantics. Project and Wave restart Fresh unless a later domain case earns recovery.

```rust
enum EpochStart {
    Fresh,
    Recover { from: Basis },
}
```

`Basis` is a receipt, so recovery does not copy it into the new Epoch as if it named current direction. Instead, `EpochStart::Recover` makes the prior Basis the new Epoch's seed. Direction at a Basis is reconstructed as:

```text
current authored Work truth at basis.truth_rev
+ Steers inherited through EpochStart::Recover
+ this Epoch's Steers through basis.steer_seq
```

Inherited Steers are the prior Epoch's inherited and locally authored Steers through `from.steer_seq`; the prior authored snapshot is not duplicated. Current Work truth is always rendered from the new Basis revision. Thus a Linear edit made after abandonment is authoritative while the human directions explicitly selected by recovery still survive. A recover chain remains reconstructible through immutable EpochStart references rather than copied directive blobs.

Every new Epoch starts with its own `steer_seq = 0`, no Ack, no Run, and a Basis containing the Work's current `truth_rev`. Ack and Send receipts never cross the boundary. The Recover seed affects prompt reconstruction, not stale-write authority: only a Basis naming the new Epoch can be acknowledged or completed.

| Fact | Fresh | Recover abandoned Task | Why |
| --- | --- | --- | --- |
| stable Work id and parent | retain | retain | Work identity |
| external binding and ingestion dedupe/cursor | retain | retain | Work-owned; never moved between attempts |
| current authored truth | read current | read current | external/authored authority |
| prior Steers | omit | inherit through selected Basis | explicit continuity |
| workspace | create/select fresh | retain after safety check | Task execution continuity |
| PR history | historical only | retain through Workspace | PRs are not re-keyed |
| lifecycle phase/cursor and gate proposal | reset | reset | new pursuit must re-judge current state |
| provider Handles | omit | omit | execution optimization, not Work state |
| old Runs, Sends, Acks, Blocks | historical only | historical only | never grant current authority |
| agent preference | read current Work setting | read current Work setting | routing preference, not attempt state |

The current Task recovery code already validates the important physical precondition: branch, worktree, and PR shape must be safe before ownership moves. The new model changes the write from “re-key every PR row to a successor Session” to “grant the new Epoch authority over the existing Task Workspace.” Historical PR attribution stays intact.

The current `phase_epoch` remains a different counter. Rename it `phase_rev`; it fences transitions within one Epoch and resets when a new Epoch begins.

### Revised model

`Work` is not another stored wrapper. It is the common reference to existing domain objects.

```rust
enum WorkRef {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

struct Epoch {
    work: WorkRef,
    n: u32,
    start: EpochStart,
    state: WorkState,
    basis: Basis,
    workspace: Option<WorkspaceId>,
    active_run: Option<RunId>,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
}

enum EpochStart {
    Fresh,
    Recover {
        from: Basis,
    },
}

struct Workspace {
    id: WorkspaceId,
    task: TaskId,
    home: WaveHome,
    path: PathBuf,
    slug: String,
    created_at: Timestamp,
    retired_at: Option<Timestamp>,
}

struct Run {
    id: RunId,
    epoch: EpochRef,
    trigger: Trigger,
    retry_of: Option<RunId>,
    state: RunState,
    agent: Agent,
    home: WaveHome,
    lease: Lease, // local authority; token never crosses the public wire
    started_at: Timestamp,
    progress_at: Timestamp,
    ended_at: Option<Timestamp>,
    outcome: Option<RunOutcome>,
}

enum RunState {
    Reserved,
    Active,
    Revoked,
    Finished,
}

struct Agent {
    provider: Provider,
    model: Option<Model>,
}

struct Route {
    agent: Agent,
    account: Option<AccountId>,
    home: WaveHome,
}

struct Handle {
    id: HandleId,
    provider: Provider,
    vendor_id: Option<String>,
    parent: Option<HandleId>,
    role: HandleRole,
    created_by: LaunchId,
    account: Option<AccountId>,
    home: WaveHome,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
    end_reason: Option<String>,
}

enum HandleRole {
    Root,
    Worker,
}

struct Launch {
    id: LaunchId,
    run: RunId,
    route: Route,
    root_handle: HandleId,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
    outcome: Option<LaunchOutcome>,
}

struct Turn {
    id: TurnId,
    launch: LaunchId,
    handle: HandleId,
    provider_turn_id: Option<String>,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
    outcome: Option<TurnOutcome>,
}
```

The Work owns durable identity and placement:

- external PM binding, stable Wave/Project ownership, and current Epoch;
- current authored truth and Work-level observation/deduplication cursors;
- complete Epoch, workspace, PR, Steer, and evidence history;
- preferred agent configuration;
- no process or provider transcript fields.

`trigger` is the one durable stimulus whose reconciliation reserved the Run: a Steer, CI incident, child observation, timer occurrence, explicit start, or retry. It is not a generic inbox row. More stimuli may arrive and be consumed by the same Run; each keeps its native receipt and points to the Run or Turn that handled it. This preserves coalescing without rebuilding `ChildCommand` under a broader name.

An Epoch owns per-pursuit state: Open / Blocked / Done / Abandoned, direction Basis, lifecycle position, current workspace/PR selection, and at most one active Run. It advances only when terminal Work begins a new pursuit. Sleeping and waking do not advance it. Previous Epochs stay queryable but cannot receive new controls.

`EpochStart::Recover` makes carry policy explicit. An abandoned Task recovery uses the prior Basis as its direction seed and retains the Workspace and PR history; a completed Task restart uses `Fresh` and a new Workspace. Project restart is fresh. This matches the current distinction between ordinary successors and abandoned recovery without reassigning ownership of historical rows or copying an opaque Session wholesale.

Workspace is Task-only and independently durable. It owns a worktree path and serial PR lineage across Runs and, when explicitly recovered, across Epochs. PRs attach to `(TaskId, WorkspaceId)` and record the Epoch in which they started; recovery grants the new current Epoch authority over the carried Workspace instead of re-keying historical PR rows. A deleted or unavailable worktree changes Workspace availability, never Task identity.

The Run owns execution authority and facts:

- why it woke and which Epoch it may mutate;
- requested agent, actual Wave Home, and runner binary;
- the secret write lease and Work-controller executor receipts;
- progress, start/end, outcome, and retry lineage.

Launch is the internal provider-adapter lifetime beneath a Run. It owns actual provider/model/account routing, provider process receipts, and one root Handle. A Task or Project Run normally has one Launch with several Turns; a Wave Run may have several Launches, often one per flow step. Route selection therefore cannot be a scalar Run field.

The provider may create child Handles beneath that root for native subagents. A captured Turn names the Handle that performed it, so nested work can be inspected without becoming a public lifecycle target. Providers that do not expose a durable child id leave the activity as a nested tool/item receipt rather than inventing one. `AgentLaunch` is already close to the right internal structure and should remain between Run and Turn for diagnostics and capture.

Turns own prompt/context decisions, tool events, token usage, and exact Steer delivery. Work Steers are delivered only to the root Handle; routing further direction to a provider-native child remains the root agent's responsibility.

The Wave Thread remains durable conversation truth in its journal. Wave Turns append to it; human messages and authored reports cross its boundary deliberately. A Project or Task's Turn capture stays private execution evidence rather than acquiring chat semantics.

The Handle is durable only as an execution receipt. One Epoch can accumulate several root and child Handles across subagent work, handoffs, and account failover. `created_by` records where each Handle first appeared; a later Launch can reuse a compatible root Handle without changing that history. Losing access creates a new root Handle from current durable context. No scalar `provider_session_id` on Work should erase earlier Handle lineage.

Resumability should not be a stored bool. It is a current compatibility decision over provider capability, whether the Handle has ended, its account, its Wave Home, and current account availability. Codex or Claude Handle access can become temporarily unavailable; an OpenCode Handle ends when its server is torn down. Recording those facts is more truthful than mutating `resumable`.

Handle disposability is an architectural invariant, not fully true of the code yet. Today only Wave harnesses use `CaptureHandle`; Project and Task runners do not persist the same Turn stream. A Handle can safely remain an optimization only after every authored input, decision, review outcome, lifecycle cursor, consequential tool receipt, and recovery ambiguity needed to reconstruct the next prompt is durable outside the provider transcript.

### What disappears

- `TaskSession` and `ProjectSession` split into stable Task/Project, historical Epoch, and active Run.
- Controls target stable Wave, Project, or Task IDs; Epochs reject stale writes but are not user-facing routing ids.
- Session predecessor/successor routing disappears. Child ownership and observation delivery use stable Project/Task IDs.
- Body generation becomes Run identity. `RunId` plus its secret lease token is the fencing key; an ordinal is display-only.
- `ChildProcessGeneration`, body handoff, and outer body outcome become Run fields/events; inner Wave `BodyProvenance` becomes Launch + Turn trace evidence.
- `BodyObservation` becomes a monitoring projection over Work, Run, liveness, progress, and observability. It may retain nearly the same wire shape while losing the `Body` name.
- `provider_session_id` becomes a root provider Handle; native child sessions become descendant Handles when observable, while Thread remains the Wave's conversation.

### State relationship

```text
Current Epoch Open, no active Run (Sleeping)
    -- trigger arrives --> reserve Run
Current Epoch Open, active Run (Starting / Running / Stalled / Recovering)
    -- settles -------> Run Finished + Epoch Open/Blocked/Done
    -- crashes -------> Run Lost; retry Run with the same trigger
```

A Run is exactly one ownership period. Reservation creates the Run and sets `Epoch.active_run` in one transaction only when the Work still names that current Epoch and its active slot is empty. The holder activates it with the secret lease. Revocation immediately fences writes but deliberately retains the active slot until the executor is reaped; only then may a retry Run reserve it. This preserves the current no-overlap guarantee.

Recovery creates a new Run linked by `retry_of`; it never impersonates the lost executor by incrementing a generation inside the durable Work row. Commands currently claimed by a generation instead record delivery attempts against Run and Turn ids. A crash after delivery begins still yields `Unknown`; the rename does not make distributed side effects transactional.

Retry is not implied by `retry_of`. The final Turn may have changed files, run commands, or affected an external system before its stream disappeared. The current disconnect recovery already distinguishes replay-safe turns from turns with durable side effects. The new model must retain `Safe / Unsafe / Unknown` replay evidence on the Turn or Run outcome; only `Safe` may be driven again automatically. `Unknown` requires resuming the same Handle if that is known safe, handing off with explicit recovery context, or blocking for judgment.

### Monitoring projection

Do not replace `BodyObservation` with another single flattened enum. The code shows two independent axes:

```rust
struct WorkStatus {
    state: WorkState,
    epoch: EpochRef,
    run: Option<RunStatus>,
    block: Option<Block>,
    actions: Vec<Action>,
}

struct RunStatus {
    state: RunState,
    health: RunHealth,
    progress_age: Duration,
    step: Option<String>,
}

enum RunHealth {
    Starting,
    Working,
    Stalled,
    Recovering,
    Dead,
    Unobservable,
}
```

The current Epoch says whether the goal is open, blocked, done, or abandoned. Run says whether current execution is healthy. A failed Run either gets a retry or leaves the Epoch `Blocked` with a typed actor/capability; otherwise `Open + no active Run` would misleadingly look like ordinary sleep. Legal actions derive from both axes.

### Crash and concurrency audit

The collapse works only if these current guarantees survive:

- **Input versus settle:** finishing a Run must atomically claim pending Steers/decisions or clear `Epoch.active_run`. A Steer committed during the boundary is therefore either handled by that Run or remains visible to reserve the next one. The current `claim_*_commands_or_stop_for_lease` transaction already proves the race is real.
- **Input versus no active Run:** creating a wake-producing stimulus should reserve a Run in the same transaction when the active slot is empty. If process launch then fails, the durable reserved Run is recovered; there is no crash gap between “message saved” and “someone will eventually notice.”
- **Non-live Steer:** interrupting a provider is controller policy, not Steer delivery. Keep the Steer queued until `send_input` begins on the next Turn. A crash after interrupt but before send is replay-safe; a crash after send begins is `Unknown`.
- **Revocation versus retry:** a revoked Run retains the active slot until every owned process group is reaped. Stale writes match Work id, epoch, Run id, active lease state, and secret token. A public Run id is not authority.
- **Shared versus owned executors:** reaping is controller-specific beneath the common lease transition. A Task/Project Run owns its runner and provider groups; a Wave Run owns its Launch processes but not the resident that hosts successive Runs. A generic `reap(run)` must dispatch to those ownership receipts, never assume every pid under a Run should die.
- **Nested provider agents:** every native subagent remains inside its parent Run's authority. Reaping must terminate or abandon all known descendant Handles and background tasks before freeing the active slot. A provider child cannot outlive the Run and later mutate its workspace under an expired lease.
- **Epoch change versus stale work:** advancing an Epoch requires a terminal current Epoch and no active Run. Every Run write matches the Epoch it reserved. Old Steers and reviews remain historical and cannot be claimed by the new Epoch.
- **Handle loss versus replay:** loss of a provider Handle never changes Work or Wave Thread identity, but it may change whether a Turn is safely recoverable. The replacement prompt must be reconstructible from durable current-Epoch state, and unsafe ambiguity must block rather than silently replay.
- **Home loss:** `Unobservable` is not `Dead`. A different Home may report the Run but cannot revoke it merely because its local process probe finds nothing. Current `WaveHome` already gives Work one owner and execution address; the simplest decentralized rule is that only that Home may mutate its Runs. Moving Work to another Home must be an explicit quiescent migration, not an opportunistic timeout takeover.

### Minimal API shape

The public controls stay domain-specific and target stable Work:

```rust
steer(work, text, actor, if_epoch) -> Steer
interrupt(work, actor) -> RunStatus
wake(work, actor) -> Run
abandon(work, reason, actor) -> WorkStatus
decide(work, decision, choice, actor) -> Decision
status(work) -> WorkStatus
```

Human and parent-to-child direction use the same `steer` operation. `if_epoch` is optional for a human acting on current Work and required for an automated actor carrying a previously read child snapshot. There is no `replace` operation. A non-live provider may require Loopflow to interrupt the current Turn before delivering the Steer to the next one, but that is controller behavior rather than authored meaning.

Run ownership is a smaller internal protocol:

```rust
reserve(epoch, trigger) -> (Run, Lease)
activate(run, lease, executor)
finish(run, lease, outcome, epoch_update)
revoke(run, outcome)
reap(run)
```

`finish` includes the input-versus-settle boundary transaction. `revoke` fences immediately; `reap` alone frees the active slot. No API updates Work state and process state independently.

Package-scoped names keep receipts short without making them vague: `steer::Send`, `steer::Ack`, `run::Lease`, and `run::Status` can appear as `Send`, `Ack`, `Lease`, and `Status` inside their own modules.

The common API does not imply a central service or even one physical table. The target's `WaveHome` executes the mutation against its local authority: Wave Thread/journal for Wave chat, local SQLite for Project and Task control. Remote callers route to that Home; monitoring elsewhere is a read-only projection. The shared contract is the transaction and receipt shape, not a network coordinator.

### Home keeper

A durable reserved Run still needs something to notice that its launcher died. The existing code has several opportunistic supervisors, while `lfd` describes itself as the one always-running Home daemon and liveness process but does not yet supervise Project/Task execution.

The model needs one Home-local keeper loop:

- scan reserved Runs that missed their boot deadline;
- probe active Runs only from their authoritative Home;
- mark progress deadlines and derive stalled/unobservable health;
- invoke the controller-specific revoke/reap path;
- reserve a safe retry or leave the Epoch typed `Blocked`;
- publish read-only WorkStatus changes for monitoring.

This keeper owns no product judgment and exposes no mutation API. Wave cadence and chat remain with each Wave resident; Task/Project lifecycle remains with their controllers. `lfd` is the natural implementation home because it is already machine-local and always on, but the architectural requirement is the keeper role, not that binary name.

### Migration reality

This is a structural migration, not a type rename. `TaskSession` appears hundreds of times across 42 files, `ProjectSession` across 30, and `provider_session_id` across 50. Rust, SQLite, CLI DTOs, fixtures, and Swift mirrors all encode the current boundaries.

The stored history is transformable:

- group Project Sessions by Linear project id and Task Sessions by Linear issue id;
- mint one stable local Project/Task id per group and order the old Sessions into Epochs;
- rewrite Task parentage to stable Project id while retaining the historical Project Epoch as provenance where useful;
- group reused Task worktree paths into Workspaces, then attach PRs to stable Task + Workspace while preserving their starting Epoch;
- attach reviews, events, Steers, and incidents to stable Work plus Epoch;
- collapse pending observation recipients from Project Session chains to stable Project ids;
- reconstruct finished Runs from `BodyLeaseChanged` events and the current Run from the latest process receipt and lease token;
- backfill Handles from Session process receipts and agent launches where the vendor id is known;
- allow historical Runs or Handles to lack links the old schema never recorded rather than guessing.

The dangerous case is an old runner writing while tables are rebuilt. The migration must first quiesce and reap every active Project/Task process and stop Wave residents, or refuse with an actionable list. Dual-write compatibility would preserve the very model being removed. After migration, restart current open Epochs from durable state under the new Run protocol.

The trace rename can preserve SQLite column names initially while changing their typed meaning at the API boundary: current trace `RunId -> TraceId`, `ProcessId -> ExecId`. Do not infer product Run linkage from timestamps when the old data did not record it.

### Test cases

- A Steer wakes sleeping Work and is sent or acknowledged by the resulting Run.
- A CI incident causes one Task Run; duplicate observation cannot reserve another.
- Provider handoff ends Run A and starts Run B on the same Task, worktree, and PR history.
- Losing a provider Handle does not lose Work or the Wave Thread; the retry Run rebuilds context.
- Recovering an abandoned Task starts an Epoch seeded from the prior Basis and retains its Workspace and PR history.
- Restarting a completed Task starts a fresh Epoch and cannot replay old Steers, reviews, or lifecycle cursors.
- A Task created under Project epoch 1 still reports to the same stable Project after the Project advances to epoch 2; no successor route is required.
- A revoked Run rejects every stale write and prevents a retry until its executor is reaped.
- A reserved Run that never boots is detected and recovered without presenting Work as healthy Running.
- A Wave resident hosts several sequential Runs without making the resident process itself the Run.
- Codex, Claude, and OpenCode native subagents remain nested Handle/Turn evidence under one Run; none becomes independently steerable Work by accident.
- Wave, Project, and Task monitoring use the same Work + active Run projection while retaining different domain loops.

### Open questions

- Should Epoch appear in public receipts for precise historical lookup, or remain an internal stale-write fence?
- Does the current provider Handle derive from the last compatible Launch, or does Epoch keep a convenience pointer backed by Handle history?
- Is the existing owner-plus-address `WaveHome` stable enough to key Run and Handle locality, or does Home migration require a separate durable id?
- Can any Work own multiple active Runs? The current supervision model says no; delegation creates child Work instead.
- Can `Agent` remain exactly the requested provider/model pair and `Route` exactly its account/Home resolution, with provider fallback always starting another Run?

## Boundary of the first core redesign

The first redesign should settle every fact that can change identity, authority, lifecycle, or write safety. It should not absorb every system that happens to mention Runs or providers.

### Resolve before the design is ratified

1. **Stable identity and containment.** Fix `WaveId`, `ProjectId`, `TaskId`, `EpochRef`, `RunId`, `LaunchId`, `TurnId`, `HandleId`, and `WorkspaceId`; state which are public control targets and which are historical receipts.
2. **Epoch transition and carry policy.** Write the exact Fresh/Recover transitions for completed, abandoned, blocked, and migrated Work. Specify what may cross an Epoch boundary: Basis, workspace, PR lineage, lifecycle cursor, Handle, and evidence.
3. **Authority.** Define how Human, active Run, Linear ingestion, GitHub ingestion, and System prove identity. Eliminate ambient environment as caller authority. Specify `if_epoch` and lease checks for every mutation.
4. **Direction and completion.** Define how authored truth becomes `truth_rev`, how a Turn's final Basis is recorded, what `ack(summary)` proves, and whether completion without an explicit Ack is ever valid.
5. **Run reservation and settlement.** Pin the atomic transitions for stimulus versus sleep, input versus settle, reserve versus boot failure, revoke versus reap, and retry versus unsafe side effects.
6. **State and ownership projection.** Define WorkState, RunState, RunHealth, typed Block, and legal actions together. Every non-progressing state must name the actor or durable condition that can advance it.
7. **Provider control contract.** Test current-Turn Send, next-Turn Send, interrupt, Handle resume, lost connection, and ambiguous acceptance against Codex, Claude, and OpenCode. Keep provider capabilities dynamic and local to the Launch/Turn.
8. **Home authority and keeper.** State the one-writer Home invariant, what remote Homes may observe, and which keeper repairs reserved/dead Runs. Exact fleet transport can remain open.
9. **Durable reconstruction floor.** Name the minimum Turn and execution evidence required to start another Launch without its provider Handle. Without this, Handle is secretly authoritative state.

These are one knot. Deferring any of them would let the new nouns describe states the implementation cannot safely enter or recover from.

### Define only the boundary in this design

- **Wave, Project, and Task lifecycle policy:** retain their separate domain transition tables and show where each reserves/finishes a Run. Do not redesign KR selection, gate policy, or cadence in the common runtime document.
- **Workspace, PR, review, and CI:** define ownership, Epoch carry, and typed triggers. Keep GitHub polling, serial-PR policy, and repair-flow details in Task designs.
- **Decisions and approvals:** keep typed responses distinct from Steer and show how they wake Work. Their UI and provider-specific approval protocols can follow.
- **Execution evidence:** define the Run → Launch → Turn spine and receipt requirements. Migrating usage, spend, trace retention, and lineage stores can be separate reductions.
- **Agent API:** fix the semantic operations, IDs, status, errors, and stale-write behavior. CLI grammar, HTTP transport, Swift DTOs, and compatibility migration follow from that contract.
- **Persistence:** specify transactions and invariants, not a grand replacement store. SQLite tables and repository boundaries can be designed once the aggregate boundaries hold.

### Keep out of the first push

- provider account/profile ownership and authentication ceremonies;
- model routing, fallback policy, quota balancing, and provider event normalization beyond the control receipts above;
- spend/usage parser consolidation and historical trace retention;
- full Linear/GitHub reconciliation, webhook ingestion, and external-team ownership;
- repository-scoped Wave lookup aliases and durable machine naming, provided stable UUID identity is preserved;
- SQLite contention tuning or a local write broker, except for proving the required transactions can be executed safely;
- Mac/UI monitoring presentation, transcript rendering, website, and user documentation;
- generic Flow, actor, event, or workflow frameworks.

Those are worthwhile projects, but none should be allowed to add another identity, state machine, command queue, or execution noun while the core is being replaced.
