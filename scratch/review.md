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
    pub status: String,       // idle | running | paused
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
- CLI and lfd projection currently each assemble objective, pause, and listener
  presence. The rule is the same and small; consolidate only if the two
  contracts start to diverge.

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
    pub project: LinearProjectRef,
    pub wave_id: LfdId,
    pub wave: String,
    pub repo: String,
    pub pm_snapshot_synced_at: i64,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: ProjectSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub state_fingerprint: Option<String>,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process: Option<ChildProcess>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

Process generation is embedded in the aggregate:

```rust
pub struct ChildProcess {
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

- `wave_id` and `wave` duplicate Wave identity. The name can drift after a Wave
  rename unless it is explicitly named as a launch-time label.
- `repo` duplicates the owning Wave's canonical repo. It may be a useful launch
  receipt, but its frozen/current semantics are unstated.
- `project.context` and `pm_snapshot_synced_at` are launch context, while the
  runner later reads current PM truth. The field names do not distinguish the
  captured launch receipt from authoritative current Project state.
- `observation_cursor` now names the outbox coordinate it stores. The existing
  SQLite column remains `task_event_cursor` for production migration stability;
  it is an internal storage name rather than the Rust/API contract.
- A public `status` plus optional `process` permits `Completed + Some(process)`
  and `Running + None`. Store methods defend some transitions, but the aggregate
  does not demonstrate the invariant.
- `state_fingerprint: Option<String>` carries an important no-progress decision
  in an opaque string without naming what was fingerprinted.

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
    pub issue: LinearIssueRef,
    pub project: LinearProjectRef,
    pub pm_snapshot_synced_at: i64,
    pub pm_snapshot_warning: Option<String>,
    pub pm_writeback: PmWritebackState,
    pub wave_id: LfdId,
    pub wave: String,
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
    pub process: Option<ChildProcess>,
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

pub struct ChildProcess {
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

- `wave_id`/`wave`, captured `project`, and PM snapshot fields have the same
  frozen-versus-current ambiguity as Project Session.
- Project and Task now use the same `ChildProcess` generation record.
- Process/status contradictions are representable here too.
- `pm_snapshot_warning` is durable session data even though it describes a
  launch-time read warning. Decide whether it is an audit fact or transient UX.
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
`ChildProcess`, commands, directives, decisions, receipts, and the shared
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

- Session-id macros remain independently implemented in Child, Project, and
  Task. They are small but still candidates for a private macro once the error
  vocabulary is reviewed.
- `Resume { message }` combines process lifecycle with optional next-turn
  input. Check every caller for whether it needs one command or an atomic
  resume-plus-directive operation.
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

- Project-supervised Task events may be delivered both to the Project and
  directly to the root Wave. Decide which events are root visibility and which
  should be Project rollups; otherwise Wave Chat can show two narratives.
- `is_wave_observable` and `is_root_wave_observable` put routing policy on the
  event enum. That is compact, but the naming hides the immediate-supervisor
  distinction.
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
    Project(&'a ProjectSession),
    Task(&'a TaskSession),
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

- `ChildTarget` borrows the full aggregate mainly to recover id and event kind.
  After one `ChildRef` exists, test whether a smaller target plus an explicit
  event write is clearer. Do not add a trait or callback framework merely to
  remove a `match`.
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

## First simplification slice — implemented

This slice preserves schemas and behavior:

1. `child_session` owns `SessionSupervisor`, one `ChildRef`,
   `ChildProcess`, child ids, commands, directives, decisions, and generic
   `BoundaryResult<S>`.
2. The observation outbox uses `ChildRef`.
3. Project and Task use `ChildProcess`.
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

## Questions to resolve as we move outward

1. Are Wave name, repo, Linear context, and snapshot warning intentionally
   frozen launch receipts on child Sessions? If yes, name them that way.
2. Which descendant Task events should reach the root Wave directly, and which
   should arrive only through Project interpretation?
3. Can Project Session no-progress detection be represented as typed input
   state rather than an opaque serialized fingerprint?
4. Is `Resume { message }` one atomic user intent, or two operations joined for
   convenience?
5. How does Wave Chat show transport receipt, directive incorporation, decision
   lineage, provider transcript, worktree, and PR without becoming three
   separate consoles?
6. Where should the provider-side exactly-once limitation be visible to an
   operator retrying a command after a crash?

## Review ledger

### Confirmed

- The Project/Task domain split is real and should remain public.
- Task is the sole roadmap runtime that owns worktree and PR delivery.
- Shared steering is durable and provider-neutral, not terminal input.
- Command acceptance and directive incorporation are different facts.
- Event + outbox and Project consume + acknowledge transaction boundaries are
  the correct durability shape.
- `lf wt create` remains available below the domain workflow.
- Generic Wave `Run` is a dead product lifecycle; trace `run_id` is a separate,
  live observability concept. The dead lifecycle is now removed.
- The protected control plane is the canonical checkout on the default branch,
  not every branch that happens to use the canonical checkout path.
- The minimal Wave registry aggregate is six fields; authored policy and live
  runtime state belong elsewhere.

### Implemented code reductions

- One child reference instead of two.
- One child process type instead of Project/Task copies.
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

### Held open

- Frozen launch receipt versus duplicated source-of-truth fields.
- Root Wave visibility versus Project-owned interpretation of Task events.
- Whether status/process invariants warrant a stronger runtime-state type or
  are clearer as explicit validation at persistence boundaries.
