# Center-out architecture review

This is the one working document for reviewing PR #872. It keeps the relevant
code in view, then records what each type owns, what it accidentally represents,
and which simplifications follow. Excerpts are intentionally selective; source
paths are included for the surrounding implementation.

## Product contract under review

```text
Human ↔ Wave → Project Session → Task Session → PR to main
                  └────────────→ Task Session → PR to main
       └───────────────────────→ Task Session → PR to main
```

- Humans create and talk to Waves.
- Every Project belongs to one Wave.
- Every Task belongs to one Project, even when its Wave supervises it directly.
- Waves choose bets, own chat/memory/cadence, and remain available while their
  children run.
- Project Sessions pursue one Project's KRs across Tasks. They own no worktree,
  branch, PR, permanent memory, cadence, or human conversation.
- Task Sessions deliver one concrete Linear issue through merge or explicit
  abandonment. They are the only domain runtime that owns a worktree.
- `lf wt create` remains a low-level Git primitive. It is not the normal roadmap
  workflow and should not appear in Wave or Task operating instructions.
- Wave, Project, and Task are the only durable repeating product lifecycles.
  Provider workers may cooperate inside a Task worktree; they do not become a
  fourth planning noun or independently acquire worktrees.

All three lifecycles use the same semantic rhythm:

```text
clarify → pursue → mutate → deterministic lifecycle decision
```

The harness runs the skills. The domain controller decides whether the
lifecycle repeats, waits, blocks, completes, or returns to idle. Skills do not
write a loop bit.

## Review route

Review from the center outward:

1. `Wave`, `ProjectSession`, and `TaskSession` aggregates.
2. Shared child identity, commands, directives, decisions, and processes.
3. Atomic command/event/outbox persistence.
4. Provider steering and lifecycle runners.
5. CLI and Wave Chat projections.
6. Worktree, PR, PM, and external-system boundaries.

The test for every layer is the same: does the API name the real product thing,
does one owner hold each piece of state, and can an operator explain a failure
at 2 a.m. without reconstructing implementation history?

## 1. Wave

Source: `rust/loopflow/src/lfd/types/wave.rs`

```rust
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub repo: String,
    pub created_at: Option<OffsetDateTime>,
    pub task_capacity: u32,
    pub parent_wave_id: Option<LfdId>,
}
```

The aggregate is durable placement and identity. Authored intent and machine
policy stay together in `wave/<name>/GOAL.md`:

```rust
pub struct WaveConfig {
    pub crons: Option<Vec<WaveCronDef>>,
    pub task_capacity: Option<u32>,
    pub agent: Option<String>,
    pub skill_agents: Option<HashMap<String, String>>,
    pub pm: Option<WavePmConfig>,
    pub paused: Option<bool>,
}
```

Presentation derives rather than persists current state:

```rust
pub struct WaveSnapshot {
    pub status: WavePresence, // idle | running | paused
    pub paused: bool,         // GOAL policy
    pub goal: String,         // first Objective paragraph
    pub live: bool,           // listener answered /health
    pub endpoint: Option<String>,
    // identity, capacity, child counts, timestamps...
}
```

### What is clear

- A Wave registry row has one durable id, one human name, one canonical
  repository, one Task-process limit, and optional parent identity.
- Parent Wave identity permits promotion into a durable child Wave without
  inventing recursive Projects.
- `GOAL.md` owns objective, pause, provider policy, PM binding, and cadence.
- The listener owns runtime state. `lf status` reports its `loop_state`
  separately from the Wave's idle/running/paused presence.
- The actual authored domain flow is `wave`; the old `ship-roadmap` default is
  gone.

### What is unclear

- `task_capacity` is authored in `GOAL.md` and copied into the registry so
  reservation can remain a transaction. This is deliberate denormalization;
  registration is the reconciliation boundary.
- Existing SQLite columns for the removed generic-run fields remain in the
  production schema. Persistence writes neutral values and no domain API reads
  them. A future table rebuild may remove them, but a migration solely for
  aesthetic purity would add risk without changing ownership.
- `lf status` and the `lfd` HTTP API currently build separate Wave JSON
  responses from the same registry, `GOAL.md`, and listener-health facts. The
  rule is small; if both contracts remain public, share the read assembly when
  they diverge. If the Mac no longer needs the HTTP form, delete that projection
  instead of abstracting over two consumers.

### Reduced in code

- The generic Wave `Run`, `RunStatus`, fork-run store, HTTP endpoints, Wave DTO
  projection, and `lf status` projection are deleted. The independent execution
  trace ledger behind `lf runs` remains.
- CI failure ownership resolves the nonterminal Task Session that owns the PR.
- The obsolete lfd stop endpoint is deleted; it previously returned success
  after finding no generic Run and left the Wave server alive. The real Wave
  listener `/stop` and Mac launcher remain.
- Wave `workers` is now `task_capacity`. Provider workers may collaborate
  inside a Task; Wave capacity limits active Task processes. Zero is no longer
  silently coerced to one.
- Wave registration refreshes authored Task capacity from `GOAL.md`
  before a child can launch. This repairs existing registry rows as well as new
  ones, so `task_capacity: 0` is an enforced policy rather than documentation.
- The obsolete `serialized` compatibility input is deleted. Capacity has one
  name and one numeric meaning.
- The orphaned `live_pr_states` store API and GitHub PR polling path are
  deleted. Task Session status, PR identity, webhooks, and merge reconciliation
  now own the live delivery lifecycle. Historical migration tables remain
  readable but have no current domain type.
- The internal Wave aggregate is reduced from fourteen fields to six. Stale
  `goal`, `metrics`, `direction`, `area`, iteration, pause, and status copies
  are gone.
- The HTTP/CLI contracts derive the displayed objective from `GOAL.md`, pause
  from authored policy, and presence from listener health. `loop_state` remains
  the resident's detailed runtime condition.
- The uncalled HTTP Wave PATCH and DELETE routes are gone. PATCH changed a
  registry cache that authored config would overwrite; DELETE removed only the
  cache while leaving the Wave directory and identity intact.

### Verdict

The reduced aggregate now matches what SQLite must coordinate atomically.
Objective, chat, memory, cadence, and runtime health remain durable, but in
their actual owners rather than as copies on `Wave`.

## 2. Project Session

Sources: `rust/loopflow/src/project_session/mod.rs`,
`rust/loopflow/src/child_session.rs`

```rust
pub enum ProjectSessionStatus {
    Created,
    Starting,
    Running,
    Waiting,
    Blocked,
    Failed,
    Completed,
    Abandoned,
}

pub struct ProjectSession {
    pub id: ProjectSessionId,
    pub project: LinearProjectSnapshot,
    pub wave_id: LfdId,
    pub wave_name: String,
    pub control_repo: String,
    pub pm_snapshot_synced_at: i64,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: ProjectSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub last_state_fingerprint: Option<String>,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub latest_process: Option<ChildProcessGeneration>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

Process generation is embedded in the aggregate:

```rust
pub struct ChildProcessGeneration {
    pub generation: u32,
    pub pid: Option<u32>,
    pub tmux_name: String,
    pub started_at: OffsetDateTime,
}
```

### What is clear

- The Project Session is the durable pursuit runtime for one Linear Project.
- `Completed` and `Abandoned` are terminal; `Waiting`, `Blocked`, and `Failed`
  preserve the same session and provider history.
- It owns iteration/fingerprint state and a child-observation cursor, but no
  worktree or delivery object.
- Directive receipt and directive incorporation are separate facts.

### What is unclear

- `wave_id` is current ownership; `wave_name` is the captured human address used
  to resume and nudge the same Wave. A future Wave rename needs an explicit
  migration rather than silently changing child history.
- `control_repo` captures the canonical Wave checkout used for read-only Project
  turns. It is placement, not another repository source of truth.
- `project.context` and `pm_snapshot_synced_at` are launch context, while the
  runner later reads current PM truth. The field names do not distinguish the
  captured launch receipt from authoritative current Project state.
- `observation_cursor` now names the outbox coordinate it stores. The existing
  SQLite column remains `task_event_cursor` for production migration stability;
  it is an internal storage name rather than the Rust/API contract.
- `latest_process` is a durable generation receipt, not a claim that its tmux
  process is still alive. Retaining it in Waiting, Failed, or terminal states
  is what makes the next generation monotonic and rejects a stale runner.
- An active status still requires a latest generation. Status/read boundaries
  reconcile a missing tmux process to resumable failure rather than trusting a
  persisted PID.
- `last_state_fingerprint` now names its temporal role, but the hash still hides
  which PM/Task input changed from an operator reading the row. The digest is
  deliberately not a public diagnostic: the visible status reason, PM
  snapshot, Task rows, and event ledger are the inspectable evidence. Persisting
  a second structured copy would create another current-state owner.

### Review question

Is a Project Session a captured launch receipt plus a small lifecycle state, or
a local mirror of its Wave and Linear Project? It should be the former. Fields
that are snapshots should say so; current truth should be resolved by id.

## 3. Task Session

Sources: `rust/loopflow/src/task/mod.rs`,
`rust/loopflow/src/child_session.rs`

```rust
pub enum TaskSessionStatus {
    Created,
    Starting,
    Running,
    Waiting,
    Submitted,
    Blocked,
    Failed,
    Merged,
    Abandoned,
}

pub struct TaskSession {
    pub id: TaskSessionId,
    pub issue: LinearIssueSnapshot,
    pub project: LinearProjectSnapshot,
    pub pm_snapshot_synced_at: i64,
    pub pm_writeback: PmWritebackState,
    pub wave_id: LfdId,
    pub wave_name: String,
    pub supervisor: SessionSupervisor,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: TaskSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub worktree: PathBuf,
    pub branch: String,
    pub base_commit: String,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub latest_process: Option<ChildProcessGeneration>,
    pub pull_request: Option<PullRequestRef>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

```rust
pub enum PmWritebackState {
    Current,
    Pending {
        operation: PmWritebackOperation,
        error: String,
    },
}

pub struct ChildProcessGeneration {
    pub generation: u32,
    pub pid: Option<u32>,
    pub tmux_name: String,
    pub started_at: OffsetDateTime,
}
```

### What is clear

- Linear issue, Project, Wave, immutable placement, provider transcript, PR,
  and post-merge PM reconciliation have one durable owner.
- `Submitted` is explicitly nonterminal; review and CI return to the same Task.
- `Merged` and `Abandoned` are the only terminal states.
- Delivery can be true while PM writeback remains visibly pending.
- `supervisor` separates immediate control from root Wave ownership.

### What is unclear

- `wave_id` is root ownership; `wave_name`, captured `project`, issue text, and
  PM timestamp are explicit launch facts used to resume the same work.
- Project and Task use the same `ChildProcessGeneration` receipt. The field is
  `latest_process` internally while Serde keeps `process` on the established
  CLI wire shape.
- `PullRequestRef` records only number and URL; PR lifecycle is inferred from
  Task status. This is simple if one invariant owns every transition, but the
  relationship should be explicit in the lifecycle review.

### Review question

The Task aggregate is closest to the desired product model. Use it as the test
case for shared child mechanics, but do not make Task the namespace owner of
concepts that Projects use equally.

## 4. Supervision and child identity

Source: `rust/loopflow/src/child_session.rs`

```rust
pub enum SessionSupervisor {
    Wave { wave_id: LfdId },
    Project { session_id: ProjectSessionId },
}

pub enum ChildRef {
    Project(ProjectSessionId),
    Task(TaskSessionId),
}

```

`ChildRef` now targets commands and directives and identifies Project/Task
sources in observations. Its Serde shape is the one durable representation.

### Verdict

There is now one child-session reference. `SessionSupervisor`, `ChildRef`,
`ChildProcessGeneration`, commands, directives, decisions, receipts, and the shared
boundary result live in `child_session`. Project and Task retain their statuses,
aggregates, events, and lifecycle policy.

This is a concrete simplification, not a generic session framework. It needs no
trait, provider registry, factory, or public `lf child` command.

## 5. Commands, directives, and decisions

Source: `rust/loopflow/src/child_session.rs`

```rust
pub enum ChildCommandKind {
    FollowUp { text: String },
    Steer { text: String },
    Interrupt { replacement: Option<String> },
    Resume { message: Option<String> },
    Decide {
        decision_id: ChildDecisionId,
        choice: String,
        message: Option<String>,
    },
    Abandon { reason: String },
}

pub enum ChildCommandState {
    Persisted,
    Claimed,
    Accepted,
    Failed,
    Superseded,
}

pub enum ChildCommandEffect {
    LiveSteer,
    NextTurn,
    Replacement,
    Decision,
}

pub enum ChildCommandSource {
    Wave(LfdId),
    Project(ProjectSessionId),
    Human,
    Attachment,
    System,
}
```

```rust
pub struct ChildDirective {
    pub id: ChildDirectiveId,
    pub target: ChildRef,
    pub version: u32,
    pub kind: DirectiveKind,
    pub text: String,
    pub source: ChildCommandSource,
    pub command_id: Option<ChildCommandId>,
    pub issued_at: OffsetDateTime,
    pub applied_at: Option<OffsetDateTime>,
    pub incorporated_at: Option<OffsetDateTime>,
    pub incorporated_summary: Option<String>,
}

pub struct ChildCommand {
    pub id: ChildCommandId,
    pub target: ChildRef,
    pub source: ChildCommandSource,
    pub kind: ChildCommandKind,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub created_at: OffsetDateTime,
    pub claimed_by_generation: Option<u32>,
    pub accepted_at: Option<OffsetDateTime>,
    pub error: Option<String>,
}
```

### What is clear

- `follow-up`, `steer`, and `interrupt` encode different human intent rather
  than exposing provider-specific transport.
- The receipt says what was durably accepted and how it was applied.
- A versioned directive separately proves whether later child work incorporated
  the instruction. Provider acceptance alone is not treated as compliance.
- Source attribution preserves the Wave → Project → Task responsibility chain
  and keeps direct human intervention explicit.
- Decisions reuse durable commands and events instead of introducing a second
  approval system.
- Shared commands now live in `child_session`; imports state the actual
  Project/Task dependency instead of making Task their false owner.

### What is unclear

- Session, command, decision, and directive ids remain distinct newtypes while
  one private macro owns their prefix + UUID parsing and formatting.
- `Resume { message }` is one atomic user intent. The same durable command both
  forces a stopped nonterminal Session to launch and, when present, becomes its
  first next-turn input. Splitting it into `follow-up` plus process launch would
  reintroduce a boundary where only one half could survive.
- `Abandon` is a lifecycle terminal command, while the other variants mostly
  describe provider input. Keeping one durable command channel is defensible,
  but the runner—not the harness—must visibly own this distinction.

## 6. Domain events and the observation outbox

Sources: `rust/loopflow/src/project_session/mod.rs`,
`rust/loopflow/src/task/mod.rs`,
`rust/loopflow/src/lfdb/sqlite/child_sessions.rs`

Project and Task retain distinct event vocabularies. The shared envelope is:

```rust
pub enum ChildEventPayload {
    Project { event: ProjectEventKind },
    Task { event: TaskEventKind },
}

pub struct ObservationOutboxRow {
    pub id: i64,
    pub supervisor: SessionSupervisor,
    pub source: ChildRef,
    pub event_id: i64,
    pub payload: ChildEventPayload,
    pub delivered_at: Option<OffsetDateTime>,
}
```

Task event and supervisor observation are committed together:

```rust
let transaction = conn.transaction()?;
transaction.execute("INSERT INTO task_events ...", params![...])?;
let event_id = transaction.last_insert_rowid();
if kind.is_wave_observable() {
    insert_observation(
        &transaction,
        &session.supervisor,
        &ChildRef::Task(session_id.clone()),
        event_id,
        &ChildEventPayload::Task { event: kind.clone() },
        created_at,
    )?;
}
transaction.commit()?;
```

Project consumption and acknowledgement are also one transaction:

```rust
if !exists {
    transaction.execute("INSERT INTO project_events ...", params![...])?;
}
transaction.execute(
    "UPDATE observation_outbox SET delivered_at=?1 WHERE id=?2 ...",
    params![now, observation.id],
)?;
transaction.execute(
    "UPDATE project_sessions
     SET task_event_cursor=MAX(task_event_cursor, ?1) ...",
    params![observation.id, now, project_session_id.as_str()],
)?;
transaction.commit()?;
```

### What is clear

- The child ledger is authoritative; delivery to a stopped supervisor survives.
- Task → Project acknowledgement cannot commit without the Project observation.
- Domain events stay distinct even though delivery mechanics are shared.
- Raw provider/tool chatter does not become human speech in the Wave journal.
- The outbox uses the same `ChildRef` as commands and directives.

### What is unclear

- Routing is deliberate: every consequential Task event reaches its immediate
  supervisor; significant delivery/control state also reaches the root Wave;
  routine Task decisions remain at the Project boundary until explicitly
  escalated. `ProjectEventKind::TaskObserved` is never sent onward, preventing a
  duplicate Project narration of the same Task event.
- Wave Chat activity does not yet carry `ChildCommandSource`. The durable
  command/directive can distinguish Wave, Project, human, attachment, and
  system control, but the card currently shows only outcome/effect. The UI
  therefore cannot answer “who changed this direction?” without a drill-down.
- The Rust/API field is now `observation_cursor`; the legacy SQLite column name
  is intentionally left in place until a schema rebuild earns a migration.

## 7. Atomic turn-boundary settlement

Sources: `rust/loopflow/src/child_session.rs`,
`rust/loopflow/src/lfdb/child_sessions.rs`,
`rust/loopflow/src/lfdb/sqlite/child_sessions.rs`

Task and Project now expose the same typed result:

```rust
pub enum BoundaryResult<S> {
    Commands(Vec<ChildCommand>),
    Stopped(S),
}

pub async fn claim_task_commands_or_stop(...)
    -> StoreResult<BoundaryResult<TaskSession>>;
pub async fn claim_project_commands_or_stop(
    ...
) -> StoreResult<BoundaryResult<ProjectSession>>;
```

Both SQLite implementations validate the generation, claim unresolved commands,
return them when present, or transition the session out of an active status in
the same transaction.

### Verdict

The guarantee and API now match. The generic parameter shares only the
state-machine result; each domain still chooses its own stopped status, reason,
events, and next action.

## 8. Provider-neutral steering

Source: `rust/loopflow/src/child_control.rs`

```rust
pub(crate) enum ChildTarget<'a> {
    Project(&'a ProjectSessionId),
    Task(&'a TaskSessionId),
}
```

The module owns command claiming, live steering, interrupt-and-replace,
decision delivery, and receipt settlement. It branches on `Harness`
capabilities, not provider names, then records the outcome in the appropriate
domain event ledger.

### What is clear

- Provider transport is below durable Loopflow intent.
- Project and Task runners keep their lifecycle policy.
- Codex live steer and Claude/OpenCode interrupt-and-resume can report different
  effects without exposing different public commands.

### What is unclear

- `ChildTarget` is the small borrowed counterpart of persisted `ChildRef`. It
  carries only the typed Session id needed to verify command ownership and
  append the correct domain event; provider control no longer borrows either
  full aggregate.
- The remaining exact-once limit must stay documented: a process can die after
  provider acceptance but before the local receipt records acceptance. Local
  transactions cannot solve provider-side idempotency.

## 9. Public CLI

Source: `rust/loopflow/src/lf/mod.rs`

Project and Task intentionally mirror one another:

```text
project: start run status follow-up steer interrupt receipt acknowledge
         decide request-decision wait resume attach abandon promote

task:    start run status follow-up steer interrupt receipt acknowledge
         decide request-decision wait resume attach abandon
```

The record-first and free-text APIs are both explicit:

```text
lf project run <linear-project-id>
lf project start "make releases boring" --wave infrastructure

lf task run INF-123
lf task start "add hello-world" --project <linear-project-id>
```

Low-level worktrees remain available:

```rust
pub enum WtCommand {
    /// Create a low-level sibling worktree
    Create { name: String, plan: bool },
    Switch { name: String },
    List { /* ... */ },
    Prune { /* ... */ },
    // ...
}
```

### Verdict

- Keep explicit `project` and `task` nouns. A generic public child/session
  command would erase the product distinction to save parser duplication.
- Keep free text as `start = create Linear record, then run the same lifecycle`.
- Keep `lf wt create` below Task. Omit it from the normal Wave/Project/Task
  instructions; document it only as an advanced Git primitive if users need it.
- Review `run` versus `resume` carefully: `run` is idempotent create-or-return
  for a domain session; `resume` starts another process generation of the same
  session. Error messages should teach that distinction consistently.

## 10. Data ownership map

| State | Owner |
|---|---|
| Wave id, name, repo, parent, Task capacity cache | Wave registry |
| Wave objective, cadence, pause, provider/PM policy | `GOAL.md` |
| Wave chat and memory observations | Wave journal / `MEMORY.md` |
| Wave listener and resident condition | live Wave runtime |
| Project definition, KRs, Task membership | Linear |
| Local planning reads | atomic Wave PM snapshot |
| Project pursuit lifecycle | Project Session + Project event ledger |
| Task delivery lifecycle | Task Session + Task event ledger |
| Worktree, branch, base, PR | Task Session only |
| Child instructions and receipts | `child_commands` |
| Current direction and incorporation proof | `child_directives` |
| Supervisor delivery | observation outbox |
| Provider conversation continuity | provider session id on domain Session |
| Provider/tmux process lifetime | shared child process generation |
| Low-level ad hoc worktree | Git/`lf wt`, outside the roadmap hierarchy |

The shipping boundary follows state, not directory alone. The canonical
checkout on its default branch is the Wave/Project control plane and refuses
PR delivery. A sibling worktree—or an explicitly selected feature branch in
the canonical checkout—remains a valid low-level Git surface. This preserves
`lf wt` without teaching Waves to ship from their homes.

## 11. Wave Chat

Sources: `rust/loopflow/src/chat/turns.rs`,
`rust/loopflow/src/wave/{journal,runtime}.rs`,
`swift/Loopflow/Models/{ChatTurn,WaveWorkMap}.swift`,
`swift/LoopflowMac/Views/{WaveChatView,WaveDetailPane}.swift`

Wave Chat combines two projections with different jobs:

```text
child event ledger
  → observation outbox
  → Wave journal TaskObserved / ProjectObserved
  → ChatTurn.activity
  → Wave SSE
  → linked activity card                 historical motion

Linear PM snapshot + Project/Task Sessions
  → lf status <wave> --json
  → WaveWorkMap
  → Project/Task list + inspector         current truth
```

The historical card carries a human-facing address and a durable session id:

```rust
pub struct ChildControlActivity {
    pub id: String,
    pub subject: ChildActivitySubject,
    pub subject_id: String, // Project slug or Linear issue identifier
    pub session_id: String,
    pub kind: ChildActivityKind,
    pub title: String,
    pub summary: String,
    pub directive_version: Option<u32>,
    pub command_id: Option<String>,
    pub effect: Option<ChildCommandEffect>,
    pub decision_id: Option<String>,
    pub options: Vec<String>,
}
```

The current-state side remains domain-shaped:

```swift
public struct WaveProjectWork {
    public let project: ProjectPlanningSnapshot
    public let runtime: ProjectRuntimeSnapshot?
    public let directive: WorkDirectiveSnapshot?
    public let nextMove: WorkNextMove
    public let tasks: [WaveTaskWork]
}
```

### What is clear

- The transcript answers “what happened?” and preserves ordering across human,
  Wave, Project, and Task motion.
- The work map answers “what is true now?” from Linear plus durable Sessions.
- Project slugs and Task identifiers are the normal human addresses. Canonical
  Linear ids and Session ids remain available for disambiguation and audit.
- Clicking an activity selects the same object in the work map.
- Decision buttons compose a message to the Wave; they do not let the Mac app
  mutate a child behind the Wave's back. Human → Wave conversation ownership
  therefore remains intact.
- Command acceptance and directive incorporation render as separate activity
  cards. The former carries `ChildCommandEffect`; the latter carries the
  directive version and incorporation summary. One card never claims both.
- Routine Task decisions stop at their immediate Project. Significant Task
  state, PR, completion, and failure events also reach the root Wave directly.
  The Project's internal `TaskObserved` wrapper is not delivered again, so the
  Wave does not receive a duplicate roll-up card.

### What is unclear

- `ChatTurn` represents ordinary human turns, assistant turns, and child
  activity with `role` plus optional `body` and `activity`. Invalid combinations
  are representable. A discriminated `ThreadEntry` may express the product more
  directly, but that wire migration should follow real UI behavior rather than
  precede it.
- The work map polls every five seconds while the transcript streams. This is
  operationally simple, but selection can briefly point at an event whose
  current session row has not appeared in the next poll.
- Significant Project-supervised Task events are intentionally visible at both
  responsibility levels: the Project consumes them to pursue KRs and the Wave
  sees the Task outcome directly. The UI should show one Task card, not invent a
  second Project narration unless the Project itself reaches a new conclusion.
- Transport receipt, semantic directive incorporation, and current lifecycle
  state are three facts. The UI currently exposes all three, but the visual
  hierarchy still needs real dogfood: direction should read as an instruction
  awaiting/incorporating evidence, not as generic event noise.

### Verdict

Keep one screen and two projections. Do not create a separate Project console
or Task chat surface yet. Make the work map the durable inspector and the Wave
transcript the place where humans direct the system and receive consequential
motion. Join them through typed ids; do not merge current state into historical
turns or make the transcript poll lifecycle truth.

## First simplification slice — implemented

This slice preserves schemas and behavior:

1. `child_session` owns `SessionSupervisor`, one `ChildRef`,
   `ChildProcessGeneration`, child ids, commands, directives, decisions, and generic
   `BoundaryResult<S>`.
2. The observation outbox uses `ChildRef`.
3. Project and Task use `ChildProcessGeneration`.
4. Both boundary methods return `BoundaryResult<S>`.
5. `TaskSessionStatus`, `ProjectSessionStatus`, both aggregates, both event
   enums, and both runners remain domain-specific.
6. The Rust/API field is `observation_cursor`; the existing SQLite column stays
   stable.
7. Migration, round-trip, persistence, runner, steering, and delivery tests
   pass without changing database values. The full Rust suite passes 1,291
   tests; clippy and `git diff --check` are clean.

This removes false ownership and duplicate representations. It does not create
a generic lifecycle, generic runner, public `child` noun, or test-only trait.

## Second simplification slice — implemented

This slice removes the lifecycle that competed with Project and Task Sessions:

1. Generic Wave `Run`/`ForkRun` types, persistence APIs, lfd routes, DTOs, and
   CLI/Swift/Python projections are deleted. The independent trace ledger used
   by `lf runs` remains.
2. GitHub CI failures resolve the nonterminal Task Session that owns the PR.
3. The obsolete lfd Wave stop route is deleted; the Wave listener remains the
   single lifecycle owner.
4. Wave `workers` is renamed `task_capacity` across authored config and public
   contracts. `task_capacity: 0` prevents launch, and Wave registration applies
   authored capacity to both new and existing registry rows.
5. The stale `serialized` compatibility input and uncalled live-PR cache are
   deleted. Task Session delivery owns PR state.

The slice deletes 2,497 lines while adding 265. The repository gate passes:
49 Python tests, 1,286 Rust tests with 3 intentional skips, 59 website tests
with 3 intentional skips, 84 Swift tests, Swift multiplatform boundaries,
clippy, format, and `git diff --check`.

## Third simplification slice — implemented

This slice gives each Wave fact one owner:

1. `Wave` contains only durable registry identity, placement, Task capacity,
   parent, and creation time.
2. Objective and policy are read from `GOAL.md`; live status is derived from
   listener health. The resident's `loop_state` remains a separate fact.
3. The false `ship-roadmap` default and copied goal/metrics/direction/area,
   iteration, pause, and status fields are deleted from Rust and wire mirrors.
4. Wave PATCH/DELETE HTTP routes are deleted because they mutated only part of
   the Wave's actual identity.
5. Legacy SQLite columns remain migration-compatible but are no longer exposed
   as domain state.
6. The low-level `lf wt create` smoke path is restored. Worktrees remain a
   supported Git primitive below Task; only domain instructions stop presenting
   them as the normal roadmap workflow.

The slice deletes 754 lines while adding 299, including this review. The gate
passes 49 Python tests, 1,283 Rust tests with 3 intentional skips, 59 website
tests with 3 intentional skips, 84 Swift tests, the CLI smoke test, the signed
macOS build-for-testing, clippy, format, Swift boundaries, and `git diff --check`.

## Fourth simplification slice — implemented

The app now uses the same Wave presence model as the Rust contract:

1. `WaveStatus` contains only `running`, `paused`, and `idle`.
2. Removed `waiting` and `failed` from Wave rows, sorting, summary state, and
   tests. Waiting/failure belong to Project/Task Sessions; a resident failure is
   `WaveLoopState.failed` in Wave Chat.
3. `WaveSnapshot.status` decodes directly as `WaveStatus`; an unknown server
   value is a contract error instead of silently becoming idle.
4. UI comments now describe registry queries, Wave journal streaming, and the
   work map without the removed generic-run/daemon model.
5. The app's `Wave` and `WaveViewModel` are immutable query snapshots. The dead
   `Wave.goal` copy is removed; objective remains in WavePlan and WaveWorkMap,
   where the UI actually renders it.

The focused Swift gate passes 83 tests.

## Fifth simplification slice — implemented

The child aggregate vocabulary now exposes launch semantics directly:

1. One private macro implements all prefixed UUID newtypes while Project,
   Task, command, directive, and decision ids remain incompatible Rust types.
2. Project and Task Sessions name the captured Wave address `wave_name`.
3. Project Session names its stable read-only checkout `control_repo`.
4. Project no-progress state is `last_state_fingerprint`, making clear that it
   compares the completed prior iteration with current PM/Task input.
5. Serde names and SQLite columns stay `wave`, `repo`, and
   `state_fingerprint`, preserving stored values and existing wire consumers.

The focused Rust build and format checks pass.

## Sixth simplification slice — implemented

The Wave work map and child-activity wire types now carry domain facts rather
than stringly or speculative shapes:

1. Project and Task runtime status fields use their Rust enums and matching
   exhaustive Swift enums. JSON remains the same snake-case values.
2. Swift `WorkController` is `.wave(id)` or `.project(sessionId)`; impossible
   kind/id combinations no longer decode.
3. Removed generic Task `delivery { kind, base, pr_* }`. The one real variable
   is `pull_request: { number, url } | null`; one PR to `main` remains a Task
   lifecycle invariant rather than repeated payload data.
4. Child control effects stay `ChildCommandEffect` through Rust, JSON, and
   Swift. Wave Chat can distinguish live steer, next turn, replacement, and
   decision without parsing strings.
5. The child-activity fixture now represents a real command-acceptance event.
   Transport acceptance and directive incorporation are tested as distinct
   facts instead of one impossible combined card.

Rust format/build, the focused activity mapping test, all 83 Swift tests, and
`git diff --check` pass.

## Seventh simplification slice — implemented

The latest runtime receipt and displayed direction now use their actual domain
types:

1. `ChildProcess` is now `ChildProcessGeneration`, and each aggregate stores it
   as `latest_process`. A completed Session retaining this record is valid
   recovery history, not an apparently live process.
2. Active status plus current tmux liveness remains the definition of a live
   process. Missing liveness is reconciled to a visible resumable failure.
3. Work-map directive kind is `DirectiveKind` in Rust and
   `WorkDirectiveKind` in Swift instead of an unconstrained string.
4. Wave presence is `WavePresence` in Rust and `WaveStatus` in Swift. The old
   Rust fixture could still construct a nonexistent `waiting` Wave; it no
   longer can.
5. Standalone `project status` and `task status` snapshots also carry their
   domain status enums internally. Their JSON values remain unchanged.
6. The standalone Task snapshot no longer repeats the constant
   `delivery: { kind: pull_request, base: main }`. Its status plus optional PR
   are the complete variable delivery state.
7. Existing database columns and JSON field names remain stable.

Rust test compilation, format, all 83 Swift tests, and `git diff --check` pass.

## Eighth simplification slice — implemented

Task launch state no longer carries a never-populated freshness warning:

1. Both Project and Task Sessions capture `pm_snapshot_synced_at`, the durable
   fact needed to audit which Linear snapshot seeded the child.
2. Auto reads already reject hard-stale snapshots. Soft-stale fallback is
   reported by the PM read operation; it is not copied as a Task-only string
   that production always set to `None`.
3. The legacy SQLite warning column stays neutral until a table rebuild earns
   the migration risk.
4. Rust test targets compile with the reduced aggregate and snapshot API.

## Ninth simplification slice — implemented

Shared provider control now depends on child identity, not aggregate shape:

1. `ChildTarget` borrows only `ProjectSessionId` or `TaskSessionId`.
2. Command target verification and domain-event routing remain exhaustive.
3. Project/Task lifecycle, PM context, worktree state, and delivery state cannot
   leak into the provider steering core by convenience.
4. No trait, callback registry, or generic child lifecycle was introduced.

Rust build, format, and `git diff --check` pass.

## Tenth simplification slice — implemented

Captured planning context has a neutral owner and an honest name:

1. `session_context` owns `LinearIssueId`, `LinearProjectId`,
   `LinearIssueSnapshot`, and `LinearProjectSnapshot`.
2. Project Session no longer imports its Project representation from the Task
   module.
3. The snapshots explicitly mean immutable launch facts used to resume a child;
   current Project/KR/Task truth still comes from the Wave PM snapshot.
4. The Task and Project modules now start with the ownership contract that must
   survive after this scratch review is deleted.
5. The governance scan skill reads `lf status` and domain Sessions instead of
   detached loops, Wave branches, queue state, and local item files.

The full Rust suite, clippy, format, and `git diff --check` pass. `scratch/`
contains only this review.

## Eleventh simplification slice — implemented

The outward command contract now follows user intent at the stopped boundary:

1. A bare `interrupt` against an inactive Project or Task is accepted without
   launching a provider process solely to stop it again.
2. `interrupt --message` still relaunches because the replacement must reach
   the same durable Session.
3. Task decision help now says “immediate supervisor”; a Project-supervised
   Task does not bypass its Project and address the Wave directly.
4. Project steer help states that inactive Sessions relaunch when needed.

The stopped-interrupt regression test and clippy pass.

## Questions to resolve as we move outward

1. How does Wave Chat show transport receipt, directive incorporation, decision
   lineage, provider transcript, worktree, and PR without becoming three
   separate consoles?
2. How should activity cards expose durable control source—Wave, Project,
   human, attachment, or system—without duplicating the command ledger?
3. Where should the provider-side exactly-once limitation be visible to an
   operator retrying a command after a crash?
4. Should the ordered Wave thread eventually use a discriminated entry enum for
   human messages, Wave turns, and child activity, or is the current optional
   activity field the smaller honest wire contract?

## Review ledger

### Confirmed

- The Project/Task domain split is real and should remain public.
- Task is the sole roadmap runtime that owns worktree and PR delivery.
- Shared steering is durable and provider-neutral, not terminal input.
- Command acceptance and directive incorporation are different facts.
- Resume-with-message is one durable command so relaunch and first input cannot
  split across a crash boundary.
- Event + outbox and Project consume + acknowledge transaction boundaries are
  the correct durability shape.
- `lf wt create` remains available below the domain workflow.
- Generic Wave `Run` is a dead product lifecycle; trace `run_id` is a separate,
  live observability concept. The dead lifecycle is now removed.
- The protected control plane is the canonical checkout on the default branch,
  not every branch that happens to use the canonical checkout path.
- The minimal Wave registry aggregate is six fields; authored policy and live
  runtime state belong elsewhere.
- Wave Chat needs one historical stream and one current work projection, not a
  separate console for each domain runtime.

### Implemented code reductions

- One child reference instead of two.
- One explicitly historical child-process generation type instead of
  Project/Task copies or an ambiguous live-process field.
- One boundary result shape instead of enum versus tuple.
- Shared child types moved out of the Task namespace.
- The observation cursor now matches its stored id.
- Generic Wave Run types, store APIs, HTTP/CLI projections, and clients removed.
- CI failure routing now resolves Task Session PR ownership.
- Wave `workers` renamed to `task_capacity`; zero capacity is honored.
- Authored Task capacity is refreshed into the registry at Wave serve time.
- Stale generic-Run PR cache and `serialized` compatibility input removed.
- Copied Wave objective/policy/runtime fields and the false default flow removed.
- Unowned HTTP Wave mutation routes removed.
- Low-level `lf wt create` smoke coverage restored after the domain-instruction
  cleanup accidentally changed it to a no-op listing.
- Swift Wave presence reduced to running/paused/idle; resident failure and
  child blockers remain in their actual runtime owners.
- Prefixed child ids share one implementation; child launch fields now name
  captured Wave placement and prior-iteration state explicitly.
- Work-map statuses, supervisors, PRs, and control effects now use the smallest
  valid domain types across Rust and Swift.
- Work-map directive kind is typed through Rust and Swift.
- Wave presence is exhaustive on both sides of the wire.

### Held open

- Wave Chat control-source lineage and visual hierarchy.

## Product reset design — no `lfd`

> “I’m not sure we need lfd at all right now.”
>
> “We might want to figure out what we want to actually use in the product
> right now or next/first.”
>
> “Aggressively reduce and perhaps eliminate lfd as a notion. I do think we’ll
> have one, but we shouldn’t build around it until we actually need it.”

### What the product uses now

```text
Human / Mac
    ├── lf commands ─────────────────────┐
    └── Wave Chat ── live Wave listener  │
                                         ▼
                              local SQLite store
                                  │          │
                         Project Session  Task Session
                                               │
                                      worktree + PR to main
```

- `lf` is the one machine-wide command and JSON API.
- The local store coordinates durable Wave, Project Session, Task Session,
  command, event, PM-snapshot, credential, and trace state without a daemon.
- `lf serve <wave>` is the resident product process. Its per-Wave local HTTP
  listener exists because the Mac and resident need live messages and SSE.
- Project and Task Sessions are explicitly launched child processes. They use
  the store; they do not call a global server.
- The Mac reads current state by invoking `lf --json` and connects directly to
  the selected Wave listener for chat.
- `lf auth` runs provider/browser authentication directly. Linear refreshes its
  OAuth grant before PM access. `gh` and provider CLIs own their own auth.
- Task runner/status already reconcile open or merged PR state through `gh`.
  Merge correctness does not require a webhook daemon.

### What to delete

Delete the `lfd` binary and the assumption that Loopflow needs one global HTTP
service. This includes:

- install/start/stop/status service plumbing and self-hosted `lfd` deployment;
- the `/v0` HTTP router, DTOs, auth middleware, read APIs, remote smoke clients,
  and active docs;
- GitHub webhook ingress and its radio-command bridge;
- the background provider-token refresh loop (refresh on use instead);
- global health/metrics for a process that no longer exists;
- remote-execution claims with no Project/Task transport behind them;
- `lfd`-specific config, session token, onboarding, observability, and security
  shells that have no remaining caller.

The read API has no current product consumer. The only non-test `/v0/waves`
caller is a private-host verification probe; it can disappear with that host
surface. Repos, sessions, attention, flows, catalog, and providers have no
non-test caller. Tests and docs that exercise an otherwise unused API do not
justify keeping it.

### What to keep, under its real owner

Deleting the daemon does not mean deleting useful libraries that happen to sit
under `lfd/` today:

| Current owner | Surviving owner |
|---|---|
| `lfd::pm` | PM domain module used by `lf pm`, Waves, Projects, Tasks, and Mac snapshots |
| `lfdb` | local `store` module; default `~/.lf/loopflow.db`, override `LF_DB_PATH` |
| `lfd::types::Wave` | Wave domain |
| provider tokens and credential types | provider-auth/store boundary |
| repo identity and edges | store/repository domain if still consumed |
| path/token redaction helpers | root security module only when a live caller remains |

Do not move an orphan merely to remove the namespace. Prove a non-test caller
or delete it. `Summary`, legacy chat-message/memory tables, generic control
sessions, and attention records each get the same audit before receiving a new
home or a new typed id.

Rename daemon-derived ambient variables with no compatibility aliases:

```text
LFD_WAVE_ID       → LF_WAVE_ID
LFD_SESSION_ID    → LF_SESSION_ID
LFD_CHANNEL       → LF_CHANNEL
LFD_DB_PATH       → LF_DB_PATH
```

Project/Task-specific environment variables retain their domain names.

### What comes next

1. Finish the local Human → Wave → Project → Task → PR product and Wave Chat
   inspector/steering UX on this smaller runtime.
2. Dogfood real multi-Task Projects and learn which continuous machine-level
   behavior is missing.
3. Only then design a remote transport or machine agent around named Project,
   Task, auth, and observation operations. Do not resurrect arbitrary exec or a
   generic read API as the starting point.

### Constraints

- Keep the per-Wave listener; it is part of Wave residency, not `lfd`.
- Keep daemonless SQLite transactions and one-writer worktree ownership.
- Remove webhook acceleration without weakening truth: runner/status/wait must
  still observe merge and reconcile Linear.
- On-use token refresh is the correctness path. Losing pre-emptive background
  refresh may make the first PM operation slower, but not incorrect.
- No compatibility aliases, old database upgrade path, or dormant remote API.
- Active docs, install scripts, release automation, Python, Swift, fixtures,
  and log categories must describe the surviving product.

### Done when

- The Rust crate has no `lfd` binary or module and no `lfdb` module name.
- `rg '\blfd\b|LFD_'` finds only deliberately retained historical release
  artifacts, if any; no active source, test, script, or documentation depends
  on them.
- `lf`, Wave serve/chat, PM, Project Session, Task Session, auth, trace, and
  fresh-store gates pass with no global daemon running.
- The repository is net-negative by a meaningful margin.

## Next reduction design — typed identity and a clean registry

> “This should probably be LfId now right?”
>
> “Get rid of LfdId anywhere? Or make all the Id types subclass LfId?”
>
> “Definitely the kind of thing I want you to find and clear up.”

### What to build

Delete `LfdId` as a domain type. Give each durable concept an incompatible UUID
newtype, rename the branch/worktree naming type that currently occupies
`WaveId`, remove the unused Wave Task-capacity policy, and replace the historical
SQLite migration chain with one schema describing only the product that exists
now.

This is an infrastructure-only reduction. JSON and SQLite continue to carry the
same UUID strings, but old databases and old clients are explicitly unsupported.

### The demo

`rg '\bLfdId\b|task_capacity|stack_parent|StackParentOpen' rust/loopflow/src`
returns no product code. A fresh `loopflow.db` opens, Wave/Project/Task status and
control tests pass, `lf wt list` is flat, and `lf rebase --plan` targets main
unless the caller supplies `--onto`.

### Data structures

One private macro shares mechanics without sharing identity:

```rust
uuid_id!(WaveId);
uuid_id!(ControlSessionId);
uuid_id!(AttentionId);
uuid_id!(SummaryId);
uuid_id!(ChatMessageId);
uuid_id!(RunId);
uuid_id!(ProcessId);
```

Each generated type owns `new`, `parse`, `as_str`, `Display`, `FromStr`, Serde,
and SQLite conversion. There is no public `LfId` base type: Rust newtypes are
the type boundary, and the macro is the only shared implementation. Existing
prefixed Project/Task/command/directive/decision ids stay unchanged.

The mapping is semantic:

```rust
pub struct Wave {
    pub id: WaveId,
    pub parent_wave_id: Option<WaveId>,
    pub name: String,
    pub repo: String,
    pub created_at: Option<OffsetDateTime>,
}

pub struct Session {
    pub id: ControlSessionId,
    pub wave_id: WaveId,
    pub run_id: Option<RunId>,
    pub parent_session_id: Option<ControlSessionId>,
    // ...
}

pub struct TraceCaptureContext {
    pub run_id: RunId,
    pub process_id: ProcessId,
    // ...
}
```

Wave references on Project Sessions, Task Sessions, supervisors, command
sources, summaries, attention, chat, and store APIs all use `WaveId`.
Attention, summary, chat-message, control-session, trace-run, and process
identities use their corresponding types. Raw database row decoding constructs
the exact type at the storage boundary.

The existing `engine::identity::WaveId` is not a Wave id. It becomes a flat
`WorktreeName`: one author plus one `WorktreeSegment`, projected as
`user/name` remotely and `<repo>.<name>` locally. Delete recursive chain,
timestamp, worker, subwave, parent, depth, and lineage APIs. Consequences:

- `lf wt list` is flat; it does not reconstruct a tree from dotted names.
- `lf wt switch` matches exact branch, sibling suffix, or flat worktree name.
- placement has no `parent_branch` or `stack_depth` fields.
- rebase has no `StackParentOpen`, `RebaseOntoParent`, `stack_parent`,
  `merged_parent`, or name-derived parent fork point. Its implicit base is
  `origin/<default-branch>`; `--onto` is the only alternate base.
- branch filtering does not infer Wave ownership from a branch name. Remove a
  `--wave` filter if it has no remaining source of true Wave identity.

Wave Task capacity disappears completely:

```rust
pub struct Wave {
    // no task_capacity
}
```

Remove it from `GOAL.md` parsing, registration, Rust/Python/Swift DTOs, fixtures,
docs, and launch errors. Task creation still transactionally reserves one Task
Session and one current process generation, so duplicate writers remain
impossible. There is no cross-Task concurrency limit until measured demand
justifies one.

### SQLite baseline

Replace the ordered repair history with one `001_initial.sql` built from the
current live store contract. Delete every later migration and every migration
test whose only purpose is upgrading an older schema. The baseline must omit
dead generic-run, queue, stack, live-PR cache, legacy Wave, and Task-capacity
columns/tables rather than recreating them with neutral values.

Keep `schema_migrations` only if the store still needs a marker for future
forward migrations. Opening an existing incompatible database should fail with
one direct instruction to delete/recreate it; do not attempt repair, silently
accept an unknown migration history, or retain compatibility columns. Fresh
in-memory and on-disk database tests are the contract.

### Key constraints

- Preserve UUID string serialization; this is a type refactor, not a new id
  encoding.
- Do not introduce `LfId`, a trait, dynamic dispatch, or conversion paths
  between unrelated ids.
- Do not give `WaveId` implicit conversions to run/session/process ids.
- Keep Project/Task/child prefixed ids in their existing domain modules unless
  moving them removes a real dependency cycle.
- `lf wt` remains a supported low-level primitive, but none of its naming state
  represents Wave, Project, Task, worker, or supervision identity.
- Delete compatibility code instead of adapting it to the baseline.

### Done when

```text
cargo fmt --check
cargo test -p loopflow
cargo clippy -p loopflow --all-targets -- -D warnings
swift test --package-path swift
uv run pytest python/tests
git diff --check
```

Fresh-schema tests prove every live store path. Compile-time type separation is
demonstrated by APIs accepting concrete ids; do not add compile-fail test
infrastructure solely for this refactor. `scratch/review.md` remains the only
scratch file.
