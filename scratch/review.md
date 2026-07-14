# Center-out architecture review

This is the current review map for PR #872. It replaces the branch history and
earlier open-question ledger with the product contract now expressed in code.
Every former review question has a resolution below; the final section records
the evidence still required at the release boundary.

## Product contract

```text
Human ↔ Wave
          └── Project Session ──┬── Task Session ── worktree ── PR to main
                                └── Task Session ── worktree ── PR to main

Wave ─ ─ ─ root inspection and override ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘
```

- Humans create and talk to Waves.
- Every Project belongs to exactly one Wave.
- Every Task belongs to exactly one Project and its one durable Project Session.
- There is one supervision path: Wave → Project Session → Task Session. A Wave
  can inspect or override a descendant Task as root authority, but that command
  source never becomes a second parent topology.
- Loopflow creates no default Project. `lf task start <free text> --project
  <id>` creates the Linear issue first, ensures its Project Session, then
  launches the Task Session and worktree. `lf task run <issue>` ensures the
  same parent for an existing Linear issue.
- Waves and Project Sessions coordinate from the clean canonical main checkout.
  Only Task Sessions own repository mutations, branches, worktrees, and PRs.
- `lf wt` remains a supported low-level Git primitive. Domain instructions do
  not use it to start roadmap work.
- Wave, Project, and Task are the only durable repeating product lifecycles.
  A provider body or subworker is execution inside one of them, not a fourth
  planning noun. Subworkers inside a Task share that Task's worktree.

## Why this branch became large

The old system split the useful properties across incompatible runtimes:

1. The Wave listener was durable and steerable, but concrete work in the Wave
   checkout could not be isolated or delivered safely.
2. A generic detached loop had a worktree, but its parent could not reliably
   address, steer, interrupt, resume, or observe it while a turn was active.
3. Linear became the sole planning source, so anonymous free-text work and
   local Project mirrors stopped being valid identities.
4. Making the Linear issue plus one durable Task Session the work owner exposed
   generic Runs, stacks, queues, rotation, and arbitrary execution routes as a
   competing lifecycle rather than reusable infrastructure.
5. Deleting that lifecycle exposed the missing Project runtime: something must
   pursue KRs across several Tasks, sleep while they run, and resume from typed
   observations without owning another worktree.
6. Once Project and Task Sessions became the product, `lfd`, its HTTP model,
   legacy ids, compatibility migrations, and the Mac's generic Session
   workspace no longer described a real caller. Pulling those threads reached
   storage, CLI, built-in skills, fixtures, and UI together.

The breadth is therefore one model replacement rather than a collection of
independent features: Human → Wave → Project → Task → PR, with durable control
at every child boundary.

## 1. The three flows

Sources:

- `rust/loopflow/src/engine/builtins/build/flow/wave.yaml`
- `rust/loopflow/src/engine/builtins/build/flow/project.yaml`
- `rust/loopflow/src/engine/builtins/build/flow/task.yaml`

Each domain keeps three skills. A flow is one bounded iteration, not the loop
itself:

```yaml
# wave.yaml
- wave_clarify
- wave_pursue
- wave_mutate

# project.yaml
- project_clarify
- project_pursue
- project_mutate

# task.yaml
- task_clarify
- task_pursue
- task_mutate
```

The harness runs the skills. The domain controller inspects durable state after
the pass and decides what happens next:

| Controller | Repeats when | Waits or idles when | Completes when |
|---|---|---|---|
| Wave | a human wake, cadence, or child observation makes judgment useful | the resident has no current wake | it does not; a Wave is a durable operating context |
| Project Session | current PM or supervised-Task state changed and an open KR remains | supervised Tasks are active | every current KR holds |
| Task Session | the worktree changed and no PR exists yet | interrupted, submitted for review, or awaiting input | the PR merges or the Task is explicitly abandoned |

A Project blocks instead of spinning when a complete iteration changes no PM
or Task state. A Task blocks instead of spinning when its flow produces neither
a PR nor a worktree change. `GOAL.md` supplies Wave intent and policy; there is
no `/goal` file or skill-written loop bit.

## 2. Wave

Source: `rust/loopflow/src/wave/types.rs`

```rust
pub struct Wave {
    pub id: WaveId,
    pub name: String,
    pub repo: String,
    pub created_at: Option<OffsetDateTime>,
    pub parent_wave_id: Option<WaveId>,
}
```

The registry row owns only durable identity, human address, canonical checkout,
and optional chord ancestry. Other Wave truth stays with its real owner:

| Truth | Owner |
|---|---|
| Objective, PM binding, provider policy, pause, cadence | `wave/<name>/GOAL.md` |
| Curated memory | `wave/<name>/MEMORY.md` |
| Human thread and turn replay | Wave journal |
| Live turn state | per-Wave listener and resident |
| Project/KR/Task planning truth | Linear, projected atomically into the local PM snapshot |
| Child lifecycle | Project and Task Sessions |

`lf wave <name>` starts the one resident product process for that Wave. The Mac
connects directly to its local listener for chat and invokes `lf --json` for
machine-wide reads. There is no global `lfd` service or HTTP projection.

`WaveId` is an incompatible UUID newtype. Run, process, Project Session, Task
Session, command, directive, and decision ids have their own types. There is no
base `LfId` that permits accidental conversion between domains.

`task_capacity` is gone. Duplicate Task Sessions and duplicate active process
generations remain transactionally impossible; Loopflow currently imposes no
cross-Task concurrency policy.

## 3. Project Session

Sources:

- `rust/loopflow/src/session_context.rs`
- `rust/loopflow/src/project_session/mod.rs`

The planning fields are explicitly one immutable launch receipt:

```rust
pub struct ProjectLaunchReceipt {
    pub project: LinearProjectSnapshot,
    pub pm_snapshot_synced_at: i64,
}

pub struct LinearProjectSnapshot {
    pub id: LinearProjectId,
    pub slug: String,
    pub name: String,
    pub prompt_context: String,
}
```

The runtime aggregate is launch evidence plus lifecycle state:

```rust
pub struct ProjectSession {
    pub id: ProjectSessionId,
    pub launch: ProjectLaunchReceipt,
    pub wave_id: WaveId,
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

It is not a local mirror of its Wave or Linear Project:

- Current Wave name and checkout resolve through `wave_id`.
- Current definition, KRs, and tasks resolve through `launch.project.id` from
  the authoritative PM snapshot.
- The captured prompt context records what seeded the first provider turn. It
  never overwrites current PM truth.
- The Project Session owns no worktree, branch, PR, chat address, memory, or
  cadence.
- `observation_cursor` is the actual outbox coordinate in both Rust and the
  fresh SQLite schema.

`last_state_fingerprint` is a private loop guard over current Project, PM Task,
and supervised Task-Session state. It is deliberately not an operator-facing
diagnostic. Operators inspect the status reason, PM snapshot, Task rows, and
event ledger; persisting another structured state copy would create a second
owner.

`latest_process` is historical generation evidence, not a liveness claim. It
stays after the process exits so the next generation is monotonic and stale
runners can be rejected. Active status requires a generation receipt, while
status/read boundaries separately probe tmux and convert a missing process to
a resumable failure.

## 4. Task Session

Sources:

- `rust/loopflow/src/session_context.rs`
- `rust/loopflow/src/task/mod.rs`

The Task launch receipt proves that planning identity existed before placement:

```rust
pub struct TaskLaunchReceipt {
    pub issue: LinearIssueSnapshot,
    pub project: LinearProjectSnapshot,
    pub pm_snapshot_synced_at: i64,
}
```

```rust
pub struct TaskSession {
    pub id: TaskSessionId,
    pub launch: TaskLaunchReceipt,
    pub pm_writeback: PmWritebackState,
    pub wave_id: WaveId,
    pub project_session_id: ProjectSessionId,
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

`launch.project` is immutable planning evidence. `project_session_id` is the
required runtime parent. The store refuses to reserve or update a Task unless
that Project Session exists and has the same Linear Project and root Wave.

`lf task start` and `lf task run` ensure the Project Session before reserving
the Task. This does not add a blocking Project turn to the small-Task path: a
missing Project Session is durably reserved, then the Task's first consequential
event wakes it through the same outbox used for later supervision. An ambient
`LF_PROJECT_SESSION_ID` must name that exact Session. The owning Wave may still
inspect or override the Task; the command is attributed to the Wave, while Task
observations and routine decisions continue through the Project. A foreign
Wave or unrelated Project is rejected before command persistence.

The aggregate validates relationships that were previously implicit:

- `Starting` and `Running` require `latest_process`.
- `Submitted` and `Merged` require the one `pull_request`.
- pending `CompleteTask` PM writeback is valid only after `Merged`.
- incorporated directive version cannot exceed current direction.
- the referenced Project Session must exist and match both Project and Wave.

`Submitted` is nonterminal. Review and CI repair resume the same Task Session,
provider transcript, and worktree. `Merged` and `Abandoned` are the only
terminal states. Code delivery may be true while Linear completion remains a
visible pending writeback.

## 5. Shared child control

Source: `rust/loopflow/src/child_session.rs`

Project and Task retain separate lifecycle states and event vocabularies. They
share only the mechanics that mean the same thing for both:

```rust
pub enum ChildRef {
    Project(ProjectSessionId),
    Task(TaskSessionId),
}

pub enum ChildCommandKind {
    FollowUp { text: String },
    Steer { text: String },
    Interrupt { replacement: Option<String> },
    Resume { message: Option<String> },
    Decide { decision_id: ChildDecisionId, choice: String, message: Option<String> },
    Abandon { reason: String },
}

pub enum ChildCommandSource {
    Wave(WaveId),
    Project(ProjectSessionId),
    Human,
    Attachment,
    System,
}
```

The public verbs encode intent rather than provider transport:

| Intent | Contract |
|---|---|
| `follow-up` | preserve the current turn; deliver exactly once as the next turn |
| `steer` | change direction now; live-inject when supported, otherwise interrupt and resume |
| `interrupt` | stop the current turn and wait |
| `interrupt --message` | supersede every unaccepted input and make the replacement next |
| `resume [message]` | atomically relaunch the same Session and optionally supply its first input |
| `decide` | resolve one durable child decision through the same receipt path |
| `abandon` | terminal lifecycle action owned by the domain runner, not the harness |

`Resume { message }` remains one command because splitting process launch from
input would create a crash boundary where only half the intent survived.

### Receipt, delivery, and incorporation

Three facts stay separate:

1. The command receipt proves Loopflow durably accepted control intent.
2. `effect` reports how the provider received it: `live_steer`, `next_turn`,
   `replacement`, or `decision`.
3. A versioned directive records whether subsequent child work incorporated
   the new direction and with what summary.

```rust
pub enum ChildCommandState {
    Persisted,
    Claimed,
    Delivering,
    Accepted,
    Failed,
    Superseded,
    Uncertain,
}
```

Provider delivery is the unavoidable ambiguity boundary. Loopflow persists
`Delivering` before provider I/O. A crash before that state permits the next
generation to reclaim the command. A crash after it becomes `Uncertain`, which
is terminal and is never replayed automatically. The receipt tells the operator
to inspect the child transcript before choosing whether to issue new intent.
Wave Chat renders this as a distinct warning, not as failure or acceptance.

The end-of-turn transaction atomically claims pending commands or makes the
generation inactive. A racing command is therefore claimed by the current
generation or observes inactivity and launches the next one; it cannot be left
in a waiting Session by a polling gap.

## 6. Decisions and observations

Sources:

- `rust/loopflow/src/store/sqlite/child_sessions.rs`
- `rust/loopflow/src/project_session/mod.rs`
- `rust/loopflow/src/task/mod.rs`
- `rust/loopflow/src/wave/runtime.rs`

Project and Task events are authoritative in their own ledgers. Appending a
consequential event and its recipient outbox row is one transaction. A Project
consumes a Task observation into its event ledger and acknowledges the outbox
row in one transaction. A Wave appends the typed observation to its journal
before acknowledgement. The uniqueness key `(recipient, source, event_id)`
makes retry and restart idempotent.

The outbox union describes delivery, not an optional parent relationship:

```rust
pub enum ObservationRecipient {
    Wave { wave_id: WaveId },
    Project { session_id: ProjectSessionId },
}

pub struct ObservationOutboxRow {
    pub recipient: ObservationRecipient,
    pub source: ChildRef,
    pub event_id: i64,
    pub payload: ChildEventPayload,
    // timestamps omitted
}
```

Task events target Project recipients. Project events target Wave recipients.
Selected Task events also target the root Wave so the human work map stays
current; this is observation fan-out, not direct Wave supervision.

Routing follows responsibility:

- every consequential Task event reaches its Project Session;
- significant Task status, control, PR, completion, and failure also reach the
  root Wave;
- routine Task decisions stop at the Project boundary until escalated;
- `ProjectEventKind::TaskObserved` is not forwarded as a second narration of
  the same Task event;
- Project conclusions and escalated decisions reach the Wave as Project events.

The observation resolves `ChildCommandSource` from the linked command or
directive. Wave Chat can therefore show Wave, Project, human, attachment, or
system lineage without copying the command ledger into its wire model.

One decision protocol spans both edges. A Task always asks its Project Session.
The Project answers routine choices or emits its own linked request to the Wave.
Exactly one answer resumes the same provider transcript; duplicate resolution
returns the existing result.

## 7. Wave Chat and Task workspace

Sources:

- `rust/loopflow/src/chat/turns.rs`
- `swift/Loopflow/Models/ChatTurn.swift`
- `swift/LoopflowMac/Views/WaveChatView.swift`
- `swift/LoopflowMac/Views/WaveDetailPane.swift`
- `swift/LoopflowMac/Views/TaskWorkspaceView.swift`

The Mac keeps one screen with two projections:

- the ordered Wave thread answers what happened;
- the current work map answers what is true now.

Child motion is a typed activity card, not prose pretending to be a human
message. `ChatTurn` keeps a compact wire shape, but both Rust deserialization
and Swift decoding validate the sum-type invariant: an activity entry cannot
also carry prose, provider items, or a provider body, and it must be a completed
attributed user-side entry. Invalid combinations fail decoding.

Cards distinguish control receipt, uncertain delivery, new direction,
incorporation, decision, PR, completion, and failure. They show the control
source and link to the same Project or Task in the work map. A streamed child
activity triggers an immediate work-map refresh; a 30-second poll repairs
missed notifications without making polling the primary event path.

Selecting a Task opens one Task-shaped workspace:

```text
Task workspace
├── changed files since recorded base
├── per-file patch
├── current file contents
├── multiple embedded Ghostty/tmux shells
└── Open in Warp
```

Git and path semantics remain in `lf`:

```bash
lf task changes INF-123 --json
lf task diff INF-123 [path] --json
lf task file INF-123 <path> --json
```

The reads include committed, staged, unstaged, and untracked changes. File
paths are normalized and canonicalized inside the Task worktree; binary and
oversized results are reported explicitly rather than forced into text JSON.

`TaskTerminalStore` is presentation state keyed by Task Session id. Each
terminal carries the issue, worktree, and tmux name; several shells can coexist
for one Task. The store never owns Task lifecycle or worktree identity, and all
UI-created tmux sessions are cleaned up when the app terminates. The deleted
generic Session workspace is not restored.

## 8. Store, ids, and worktrees

Source: `rust/loopflow/src/store/`

The store is a daemonless SQLite coordination boundary opened directly by
callers. One `001_initial.sql` defines the current schema. Older databases and
clients are intentionally unsupported; dead generic-run, stack, queue,
live-PR, capacity, and compatibility columns are absent rather than neutralized.

The active identity model is semantic:

- `WaveId`, `RunId`, and `ProcessId` are incompatible UUID newtypes;
- Project Session, Task Session, command, directive, and decision ids are
  incompatible prefixed UUID newtypes;
- Linear issue and Project ids are validated domain newtypes;
- worktree naming is a flat author/segment projection, not Wave ancestry or
  worker identity.

No active source or current documentation refers to `LfdId`, `lfd`, `lfdb`,
`LFD_*`, `task_capacity`, `lf serve`, Wave rotation, or obsolete migration
files. Historical release notes and Wave memory remain historical.

## 9. Public API and failure boundaries

The record-first and free-text paths are both explicit:

```bash
lf project start "make releases boring"        # create Linear Project, then run it
lf project run <linear-project-id>

lf task start "add hello world" --project <linear-project-id>
lf task run INF-123
```

Project and Task controls mirror one another where the intent is shared:

```text
status  follow-up  steer  interrupt  receipt  acknowledge
decide  request-decision  wait  resume  attach  abandon
```

The boundaries fail early and name the corrective action:

- Project and Task creation require a registered owning Wave and resolvable
  Linear identity. Registration is checked before a free-text create mutates
  Linear.
- free-text Project and Task creation refreshes the owning Wave's PM snapshot
  after the Linear write and before creating a Session. If refresh fails, the
  error reports the committed Linear id, confirms that no new Session/worktree
  was created, and gives a safe sync-and-run recovery path. Retries reconcile the
  Project title or Task idempotency marker rather than duplicating the record.
- Wave and Project turns require a clean canonical main checkout.
- Task placement happens only after the Linear issue and immutable launch
  receipt are reserved.
- a second `task run` returns the existing Session rather than creating another
  writer;
- foreign Wave or Project control fails before command persistence;
- active status without a live tmux process reconciles to resumable failure;
- submitted/merged state without a PR and pending PM writeback before merge are
  rejected as invalid aggregates;
- Project no-progress and Task no-progress become visible `Blocked` states
  rather than more provider turns.

## Resolved review ledger

| Former review issue | Resolution |
|---|---|
| `Wave` carried `LfdId`, capacity, copied policy, and daemon projection state | `WaveId` in a five-field registry aggregate; authored/runtime truth remains with GOAL, journal, listener, PM, and child Sessions |
| Project Session looked like a local Wave/Linear mirror | one named `ProjectLaunchReceipt`; current Wave and Project truth resolve by typed id |
| captured Wave name and `control_repo` could drift | both removed; the owning Wave row supplies its current name and canonical repo |
| launch context and current PM truth were ambiguous | `launch.project.prompt_context` is immutable evidence; current PM state is always read by `LinearProjectId` |
| observation cursor used a legacy Task-event storage name | Rust and fresh schema both use `observation_cursor` |
| `latest_process` looked live | it is documented as a retained generation receipt; liveness is probed and active-state invariants are validated |
| Project fingerprint hid operator evidence | it remains a private spin guard; status, PM, Tasks, and ledgers are the visible evidence rather than a second state copy |
| Task PR/status relationship was implicit | aggregate validation requires a PR for Submitted/Merged and limits pending PM completion to Merged |
| root ownership and immediate Task control were conflated | `wave_id` records root authority; required `project_session_id` records the only runtime parent; command source records Wave overrides without changing topology |
| Project Sessions were optional for small Tasks | every Task requires the matching Project Session; `task start/run` ensures it before Task reservation |
| small Tasks implied a default or missing Project | there is no default; free-text start requires a Project, creates the Linear Task, ensures its Session, then places work |
| free-text creation could discover a missing Wave after a Linear write | Project/Task start checks the registered Wave before mutation |
| a committed Linear create could disappear behind snapshot refresh failure | start reports the real committed id, creates no new Session/worktree, and supports idempotent retry or explicit sync-and-run |
| shared ids risked collapsing domain identity | concrete newtypes share private mechanics only; no `LfId` base type exists |
| `Resume(message)` mixed process and input concerns | retained as one atomic durable intent so crash recovery cannot split the two halves |
| `Abandon` shared a channel with provider input | retained in the durable command channel, but only the Project/Task runner applies the terminal lifecycle transition |
| Task events could be narrated twice | required Task → Project routing plus a non-forwarded `TaskObserved` wrapper gives the Wave one Task card |
| Wave Chat could not show who changed direction | observations resolve and cards render typed command/directive source lineage |
| a provider crash could replay an already accepted instruction | delivery is persisted before I/O; stale delivery becomes terminal `Uncertain` and is never replayed automatically |
| `ChatTurn` could represent activity mixed with conversation | checked constructors/decoders reject invalid envelopes in Rust and Swift |
| streamed activity could precede a five-second work-map poll | activity now refreshes immediately; a 30-second poll is only repair |
| receipt, direction, and current state read as generic event noise | separate activity kinds render applied/uncertain, directed/incorporated, decisions, and lifecycle with source and links |
| Mac reduction deleted useful terminal multiplexing | Task-scoped Ghostty/tmux tabs and Warp opening are restored without a generic Session hierarchy |
| Swift would need to reconstruct Git state | typed Task `changes`, `diff`, and `file` reads own comparison, path, binary, and size rules |
| `lfd`, old migrations, capacity, and rotation survived in active vocabulary | global daemon/API deleted, schema rebased to `001_initial`, active residue scan is clean |
| flow looping depended on a skill-authored completion bit | controllers loop one bounded three-skill iteration from durable Wave/PM/Task/PR state |
| `lf wt` was either product workflow or forbidden | it remains low-level and supported, but normal Wave/Project/Task instructions omit it |

No architecture or data-model question from the prior review remains open in
this document.

## Release evidence

The deterministic local evidence covers:

- fresh-schema store behavior;
- aggregate invariants and typed-id round trips;
- atomic command boundary settlement and crash recovery;
- provider capability profiles, replacement, decisions, and uncertain delivery;
- typed Task → Project → Wave observation replay;
- Task workspace Git/path behavior;
- Rust/Swift wire fixtures and checked chat envelopes;
- Swift Task workspace and Wave Chat projections.

The side-effecting live Linear → provider → two Task PRs → merge → PM
reconciliation dogfood is not automated. It remains a deliberate manual release
gate because it creates external Linear records, provider spend, worktrees, and
GitHub PRs. That is an evidence boundary, not an unresolved ownership or API
decision.
