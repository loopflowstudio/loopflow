# Linear-backed, steerable task sessions

## What to build

Finish the terminal-first Wave-to-Task control contract. The broad lifecycle
redesign is already implemented: Linear-backed Task Sessions own immutable
worktrees, resumable provider sessions, durable commands/events, and one PR to
`main`; the competing generic-loop, rotation, stacking, queue, sandbox, and
`lfd` exec paths are gone.

The Task runtime exposed one missing layer: Projects still need a process that
pursues their KRs across many Tasks. The remaining slice therefore has seven
jobs: add durable Project Sessions; deliver typed child observations; add
child-to-supervisor decisions; prove provider behavior with one conformance
suite; dogfood the complete hierarchy; close the turn-boundary command race;
and make durable receipts explicitly waitable. Do not reopen placement,
delivery, remote execution, or Swift UI design while completing this slice.

> “The standard way to execute a task should basically be to create the Linear
> task and then just pass the Linear ID to the task.”

> “As standard operating procedure it makes sense for Waves to first create
> tasks or projects and then execute or dispatch a worker on top of it.”

> “We should benchmark our steering API for tasks against that of Claude,
> Codex, and OpenCode and make sure we are at least as powerful.”

> “The important part here is that the wave server is actually doing the work,
> the wave LM is doing the work, because that is what is directly steerable by
> the UI.”

The last requirement is satisfied through supervision rather than shared
process identity: the Wave LM remains the human-facing mind, creates the task,
starts its worker, sees its state, steers or interrupts it, and decides what to
do with its result. The task worker is a durable child session, not a second
unrelated conversation.

> “I want to avoid work trees moving out from under, especially wave servers,
> but any kind of loop.”

> “For right now my plan is to have almost everything, or literally
> everything, merge directly into main.”

> “We are trying to make LoopFlow work as well as possible for me and no other
> developers.”

## Implementation baseline

Preserve this working implementation rather than redesigning it again:

- `lf task run/start/status/follow-up/steer/interrupt/wait/resume/attach/abandon`
  operate on one durable Task Session per Linear issue.
- Task commands persist `persisted`, `claimed`, `accepted`, `failed`, or
  `superseded` state and record `live_steer`, `next_turn`, or `replacement` as
  the actual effect.
- Codex receives live steering when a turn is active. Claude and OpenCode are
  interrupted and resumed in the same provider session.
- Interrupt-with-replacement transactionally supersedes unaccepted input.
- Process generations reclaim unresolved commands after process death.
- A foreign Wave is refused before command persistence; an unattributed local
  process is the explicit human escape hatch.
- Task state, PM writeback, PR lifecycle, worktree identity, and provider
  session identity survive process restarts.
- Rotation, `lfq`, generic `lfd` execution, sandbox workers, generic loops,
  stacks, queues, and project markdown mirrors are removed.

The focused `cargo test -p loopflow task_` suite currently passes 20 tests.
Those tests prove persistence and local state transitions, not the seven
remaining product behaviors below.

## How Project Sessions fit

The old generic loop mixed two different jobs:

1. isolate code-writing execution in a worktree;
2. repeat judgment until an outcome holds.

Task Sessions now own the first job. Removing generic `lf loop` also removed
the second job for Projects. The current `project` flow is only one bounded
`project_clarify → project_pursue → project_mutate` pass, and `lf project run`
only posts a prose directive to the Wave. It does not durably bind a Project to
a provider session, repeat until its KRs hold, wait without spending tokens, or
resume when a Task changes.

Add a domain-specific Project Session rather than restoring a generic loop:

```text
Human
└── Wave                         permanent mind, chat, memory, cadence
    └── Project Session         bounded KR-pursuit loop, no worktree or PR
        ├── Task Session        implementation worktree + provider + PR
        └── Task Session        implementation worktree + provider + PR
```

The normal control path is `Human → Wave → Project Session → Task Session`.
The Wave remains authorized to inspect or control any descendant Task directly.
A Task launched directly by the Wave simply names the Wave as its supervisor.

A Linear Project remains planning data: definition, KRs, and task membership.
The Project Session is its temporary runtime. Like a Task Session, it may have
a resumable provider transcript and many process generations without giving
the Project Wave semantics. It owns no human chat, permanent memory, cadence,
server, worktree, branch, or PR. A Project requiring those things is promoted
to a Wave.

## Remaining infrastructure slice

### 1. Durable Project Sessions

`lf project start` creates a Linear Project and then starts its one Project
Session. `lf project run <linear-project-id>` starts or returns that same
session. It no longer posts a prose instruction and hopes a later Wave turn
remembers it.

```rust
string_id!(ProjectSessionId, "ps_");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSession {
    pub id: ProjectSessionId,
    pub project_id: LinearProjectId,
    pub project_slug: String,
    pub project_name: String,
    pub wave_id: LfdId,
    pub wave: String,
    pub pm_snapshot_synced_at: i64,
    pub status: ProjectSessionStatus,
    pub status_reason: String,
    pub iteration: u32,
    pub task_event_cursor: i64,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process: Option<SessionProcess>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionSupervisor {
    Wave(LfdId),
    Project(ProjectSessionId),
}
```

Add `supervisor: SessionSupervisor` to `TaskSession`. Existing Task Sessions
migrate to their owning Wave. A Task started from a Project Session captures
that Project Session as its immediate supervisor. Root ownership remains the
Wave, so the Wave may always override the Project Session and steer the Task.
A foreign Wave or unrelated Project Session is refused before persistence.

The Project Session runs as `lf __project <session-id> <generation>` in a named
tmux session using the existing process launcher. It runs from the permanent
Wave home only to read repository and PM context; it must not edit files.
Every concrete file mutation is delegated to a Linear Task and Task Session.
No Project worktree or branch is created.

Public control mirrors the proven Task vocabulary:

```text
lf project run <project-id>
lf project status <project-id>
lf project follow-up <project-id> "..."
lf project steer <project-id> "..."
lf project interrupt <project-id> [--message "..."]
lf project wait <project-id>
lf project resume <project-id> [message]
lf project attach <project-id>
lf project abandon <project-id> --reason "..."
```

`project_clarify → project_pursue → project_mutate` becomes one Project
iteration. After each iteration the runner reads the authoritative PM snapshot
and child Task state:

- every KR holds → `Completed`, emit one completion, stop;
- relevant Tasks are active → `Waiting`, stop the process without losing the
  provider session;
- a decision is outstanding → `Blocked`, wait for the answer;
- open KRs remain and observable PM/Task state changed → begin another
  iteration;
- open KRs remain and the state fingerprint did not change → `Blocked`, report
  the lack of progress to the Wave instead of spinning.

When a supervised Task changes, its typed event advances the Project Session's
event cursor and relaunches the same Project Session if the event can unblock
it. Waiting consumes no provider turns. Repeated wakeups coalesce before launch.
Standing frontier Projects may remain `Waiting` indefinitely; they complete
only when their current proof-shaped KRs all hold.

The second real child-session consumer justifies one shared control primitive.
Refactor Task command persistence into a child-session command envelope rather
than cloning the state machine:

```rust
pub enum ChildSessionRef {
    Project(ProjectSessionId),
    Task(TaskSessionId),
}

pub enum ChildCommandSource {
    Wave(LfdId),
    Project(ProjectSessionId),
    Human,
    Attachment,
    System,
}

pub struct ChildCommand {
    pub id: ChildCommandId,
    pub target: ChildSessionRef,
    pub source: ChildCommandSource,
    pub kind: ChildCommandKind,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub claimed_by_generation: Option<u32>,
    pub accepted_at: Option<OffsetDateTime>,
    pub error: Option<String>,
}
```

Migrate existing `task_commands` rows into this representation. Keep Task and
Project CLI nouns explicit; only their durable delivery machinery is shared.
Do not create a public generic session or loop command.

The database migration adds `project_sessions`, `project_events`, shared
`child_commands`, and `observation_outbox`; it adds required supervisor kind/id
columns to `task_sessions`. Existing Task commands retain ids, state, effect,
generation, timestamps, and errors. Existing Tasks point to their Wave. DTO
fixtures add every required field in Rust and Swift; no wire default hides an
old row or stale client.

### 2. Typed child observations

Replace prose calls to `post_to_named_wave("Task …")` and the current prose
Project directive with one typed, idempotent observation path. Task and Project
event ledgers remain authoritative; supervisors receive durable linked
observations.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildObservation {
    pub source: ChildSessionRef,
    pub label: String,
    pub event_id: i64,
    pub event: ChildEventPayload,
}

pub enum ChildEventPayload {
    Project(ProjectEventKind),
    Task(TaskEventKind),
}

pub enum wave::journal::EventKind {
    // existing variants
    ChildObserved { observation: ChildObservation },
}

pub enum InboxItem {
    Message(PendingMessage),
    Child(ChildObservation),
    Interrupt,
    Skip,
}
```

Use a SQLite outbox so delivery survives a stopped supervisor:

```rust
pub struct ObservationOutboxRow {
    pub supervisor: SessionSupervisor,
    pub source: ChildSessionRef,
    pub event_id: i64,
    pub payload: ChildEventPayload,
    pub delivered_at: Option<OffsetDateTime>,
}
```

Appending a child event and its outbox row is one transaction, unique on
`(supervisor, source, event_id)`. A Project Session consumes supervised Task
observations into its own event ledger before acknowledging them. The Wave
server is still the sole Wave-journal writer; it appends Project observations
and directly supervised Task observations before acknowledging their outbox
rows. Restart or retry cannot duplicate them.

The supervisor receives a typed inbox item, renders a compact structured
prompt, and either wakes once while idle or queues behind the active turn. It
does not masquerade as human speech and does not copy raw tool chatter.

Project these events: command state changes, decision requests/resolutions,
status changes, Tasks started, PR opened, completed, and failed. Fine-grained
progress remains in the child ledger unless explicitly requested by its
supervisor.

### 3. Child-to-supervisor decisions

Add one decision protocol instead of a second approval framework. The first
use is Task plan approval.

```rust
string_id!(ChildDecisionId, "cd_");

pub struct DecisionRequested {
    pub decision_id: ChildDecisionId,
    pub prompt: String,
    pub options: Vec<String>,
}

pub struct DecisionResolved {
    pub decision_id: ChildDecisionId,
    pub choice: String,
    pub message: Option<String>,
}

// TaskEventKind and ProjectEventKind each carry these two payloads.
pub enum ChildCommandKind {
    // existing variants
    Decide {
        decision_id: ChildDecisionId,
        choice: String,
        message: Option<String>,
    },
}
```

A request is durable before the child stops advancing. A Task sends it to its
Project Session when one supervises it, otherwise to the Wave. A Project
Session answers routine Task questions itself; when it needs Wave judgment it
emits its own linked decision request and resumes the Task after the Wave
answers. `lf task decide` and `lf project decide` use the same command receipt
machinery. Foreign supervisors are refused. The runner accepts exactly one
answer, continues the same child/provider transcript, and records a linked
resolution. A duplicate answer returns the existing resolution rather than
starting another provider turn. Human intervention remains possible through
the same explicit unattributed escape hatch.

### 4. Provider conformance suite

Build one parameterized suite at the `Harness` boundary. Exercise Codex,
Claude, and OpenCode through their real adapters with scripted protocol peers;
do not fork provider-control behavior between Project and Task runners or
encode provider names in orchestration logic.

Run the same scenarios for both Project and Task runners where applicable.
Every adapter must pass:

1. live redirect;
2. gentle follow-up;
3. interrupt-and-replacement supersession;
4. completion-boundary delivery;
5. crash recovery at persist, claim, and provider acceptance;
6. owning-Wave/foreign-Wave/human source rules;
7. decision request and response;
8. automatic typed Wave observation;
9. parallel targeting without cross-talk;
10. interrupt, abandon, and resume as distinct lifecycle operations.

Codex should report `live_steer`; Claude and OpenCode should report
`replacement`. Provider-specific live smoke tests may remain opt-in, but the
deterministic conformance suite must run in the normal Rust gate.

### 5. End-to-end dogfood

Exercise the real path once with a deliberately small Linear task:

```text
Wave creates/selects Linear Project
→ lf project run <project>
→ Project Session creates/selects two Linear Tasks
→ Project Session starts and supervises both Task Sessions
→ one Task requests a decision from Project; Project escalates once to Wave
→ Wave steers the Project while the Tasks run
→ Project sleeps until typed Task events wake it
→ Tasks open PRs to main and resume through review
→ both PRs merge and Linear completion reconciles
→ Project verifies the KRs, completes, and emits one typed completion to Wave
```

Capture the Project and Task identifiers, Project/Task Session ids, command
ids/effects, provider session ids before and after resume, PRs, decision chain,
sleep/wake evidence, and final Wave journal event in the PR notes. This is an
explicit side-effecting manual gate, not an automated test.

### 6. Atomic turn-boundary delivery

Remove the correctness dependency on a 200 ms poll and a two-second client
wait. The runner must atomically choose between claiming work and becoming
inactive.

```rust
pub enum BoundaryResult {
    Commands(Vec<ChildCommand>),
    Stopped(ChildSessionRef),
}

async fn claim_commands_or_stop(
    session: &ChildSessionRef,
    generation: u32,
    stopped_status: TaskSessionStatus,
    reason: &str,
) -> StoreResult<BoundaryResult>;
```

In one SQLite transaction, verify the process generation, claim all unresolved
commands, and return them; or, only when none exist, transition the generation
out of the active state. Command insertion then observes either an active
generation that cannot stop without seeing the command or an inactive session
that it must reserve and relaunch. `follow-up` receives the same guarantee as
`steer`; no caller-side sleep is part of correctness.

### 7. Explicitly waitable receipts

Keep command submission and command resolution separate. Submission always
returns the current durable receipt. Add a read/wait API keyed by command id:

```text
lf task receipt cc_123 --json
lf task receipt cc_123 --wait --timeout 30s --json
lf project receipt cc_456 --wait --timeout 30s --json
```

`--wait` returns on `accepted`, `failed`, or `superseded`; timeout returns the
latest truthful nonterminal receipt and a distinct timeout exit status. Waiting
uses store notification/polling and never calls an LM. Existing `steer` and
`interrupt` may retain their convenient short wait, but callers can always
recover and continue waiting by command id. Swift later consumes this same JSON
shape rather than inventing another receipt model.

## The demo

Progressive disclosure keeps the hello-world path small: a Wave may still
create and run one Task directly. A measured multi-Task bet uses the Project
Session automatically; the human does not choose process topology.

Project-sized path:

```text
Human: Make first-run CLI onboarding self-explanatory.

Wave:
  creates/selects Linear Project "First-run onboarding"
  starts Project Session ps_01 and remains available

Project ps_01:
  clarifies proof-shaped KRs
  creates Task INF-123 for the command and INF-124 for docs/tests
  starts both Task Sessions, then sleeps
  wakes on their typed PR/decision events
  verifies the KRs after merge and reports completion to the Wave
```

Small direct path:

From Wave Chat:

```text
Human: Add a hello-world command.

Wave:
  creates/chooses a Linear project
  creates Linear issue INF-123
  starts Task Session ts_01 in loopflow.inf-123
  remains available in the permanent infrastructure wave home

Human: Also name the flag --hello.

Wave:
  sends the instruction to INF-123

Task INF-123:
  updates the active Codex turn, finishes tests, opens one PR to main
  waits through review, handles feedback in the same worktree/session
  reports the merged PR

Wave:
  records the completion in its thread and continues project pursuit
```

The human issues no branch, worktree, placement, tmux, Linear, or PM command.
The Wave never changes cwd or branch. The Task Session is independently
inspectable and controllable throughout.

The equivalent formal CLI paths are:

```text
lf project run <linear-project-id>
lf project steer <linear-project-id> "prioritize the CLI path"
lf project wait <linear-project-id>

# Direct one-Task fast path
lf task run INF-123
lf task steer INF-123 "also name the flag --hello"
lf task wait INF-123
```

## Product model

### Wave

A Wave owns one durable human conversation, memory, cadence, budget, project
selection, and supervision. Its server and worktree path are permanent. Its
home is a control-plane checkout, never a shipping branch or PR.

The Wave may manage zero, one, or several Project and Task Sessions. It stays
directly steerable while they run. Talking to the Wave never silently
retargets the human to a child transcript.

### Project

A Project is one Linear Project inside exactly one Wave/Linear Initiative. It
owns a definition and proof-shaped KRs. It has no permanent worktree, branch,
server, memory, or child project.

Running a Project creates or resumes its Project Session. That bounded child
repeatedly evaluates the KRs and selects, creates, and supervises concrete
Linear Tasks. All file-writing work happens through Task Sessions.

### Project Session

A Project Session is the durable runtime for pursuing one Linear Project until
its KRs hold or pursuit is explicitly abandoned. It owns a resumable provider
transcript, process generations, commands, child-event cursor, and iteration
state. It owns no worktree, branch, PR, server, chat address, memory, or cadence.

Waiting is a persisted state, not a running process. A Task observation wakes
the same Project Session when new judgment is possible.

### Task

A Task is one Linear issue. The Linear issue is the public identity and must
exist before execution. A task can have one active Task Session; that session
may stop and resume many times while retaining the same worktree and provider
history.

A Task Session owns:

- one immutable worktree and local branch;
- one provider session/transcript at a time;
- structured commands and events;
- one PR targeting `main`;
- lifecycle through review, merge, or explicit abandonment.

### Task Session

The Task Session is durable state, not the tmux process and not the provider's
session id. Killing a process does not delete the Task Session. Resuming does
not create a second task.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSession {
    pub id: TaskSessionId,
    pub issue_id: String,         // PmItem.id: canonical Linear UUID
    pub issue_identifier: String, // PmItem.identifier: INF-123
    pub project_id: String,       // PmProject.id
    pub project_slug: String,     // PmProject.slug
    pub wave_id: WaveId,
    pub supervisor: SessionSupervisor,
    pub pm_snapshot_synced_at: i64,
    pub pm_writeback: PmWritebackState,
    pub status: TaskSessionStatus,
    pub worktree: PathBuf,
    pub branch: BranchName,
    pub base_commit: CommitId,
    pub provider: HarnessKind,
    pub provider_session_id: Option<String>,
    pub process: Option<TaskProcess>,
    pub pull_request: Option<PullRequestRef>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PmWritebackState {
    Current,
    Pending {
        operation: PmWritebackOperation,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmWritebackOperation {
    CompleteTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
```

These PM fields are an immutable launch receipt copied from the shared
`PmShowResult`; they are not a second editable Project/Task model. Current
Project definitions, KRs, and task state always come from the PM snapshot.

`Merged` and `Abandoned` are terminal. `Submitted` is not complete: review
feedback may resume the same worker in the same worktree days later.

## Why structured child sessions

The three relevant agent systems converge on stable child identity plus typed
control, not terminal keystrokes as orchestration.

Codex holds one session-scoped control object across a root and its children.
It addresses children by persistent thread id and exposes send, interrupt,
status subscription, resume, and completion notification. Its shape is the
model, adapted rather than copied:

```rust
async fn send_input(session: ChildSessionRef, input: ChildInput) -> Result<SubmissionId>;
async fn interrupt(session: ChildSessionRef) -> Result<SubmissionId>;
async fn subscribe_status(session: ChildSessionRef) -> Result<StatusReceiver>;
```

See Codex's
[`AgentControl`](https://github.com/openai/codex/blob/main/codex-rs/core/src/agent/control.rs),
[`send_input` handler](https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/handlers/multi_agents/send_input.rs),
and [spawn/resume implementation](https://github.com/openai/codex/blob/main/codex-rs/core/src/agent/control/spawn.rs).
Codex also snapshots active runtime policy into a child; Loopflow adopts that
explicit snapshot but deliberately substitutes the task worktree for inherited
cwd. See
[`apply_spawn_agent_runtime_overrides`](https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/handlers/multi_agents_common.rs).

OpenCode creates a child Session with `parentID`, uses `task_id` to continue
the same history, serializes follow-up work through `extend`, and exposes wait
and cancel. Loopflow adopts the stable id and serialized-follow-up behavior,
but persists it: OpenCode's experimental background registry is process-local.
See OpenCode's
[`TaskTool`](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/tool/task.ts)
and
[`BackgroundJob`](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/background-job.ts).

Claude Code independently reaches the same product shape: a subagent has a
stable id and separate transcript, `SendMessage` can redirect or resume it,
and worktree isolation is launch metadata. Agent Teams use a lead, independent
sessions, shared task state, and a mailbox. Tmux/iTerm panes are display modes,
not the messaging protocol. See
[Claude subagents](https://code.claude.com/docs/en/sub-agents) and
[Agent Teams](https://code.claude.com/docs/en/agent-teams).

Loopflow should not delegate its lifecycle to any provider's subagent feature.
It launches normal provider harnesses inside Loopflow Project and Task Sessions
so Linear identity, supervision, control, audit, and recovery behave the same
on Codex, Claude, and OpenCode. Only Task Sessions add worktree isolation and
delivery.

## Incoming PM API contract

The product worktree changes the PM boundary this design builds on:

- Linear is the only durable author of Initiatives, Projects, definitions,
  KRs, and Tasks.
- SQLite stores one atomic `PmShowResult` snapshot per canonical repository and
  Wave. It is a daemonless read model, not an authoring surface.
- `lf pm show` reads that snapshot through `PmRefresh::{Auto, Force, Never}`.
- `lf pm task create/update/done/move` and `lf pm project
  create/update/archive` write Linear, then refresh the affected snapshot
  before returning.
- `lf pm init` owns only the Wave→Initiative binding, with snapshot seeding
  expected as part of settling the first-read cliff.
- `wave/<wave>/projects/*.md` is deleted. Roadmap changes go through `lf pm`.

| Before product PM change | After product PM change |
|---|---|
| Project definitions/KRs live or mirror under `wave/*/projects/` | Project definitions/KRs live only in Linear Project content |
| `pm init` may seed/migrate Projects | `pm init` establishes only the Initiative binding; snapshot seeding is the remaining first-read policy decision |
| Reads reconstruct from files or query provider-shaped paths | `pm show` exports one typed SQLite `PmShowResult` |
| Sync diagnoses/reconciles several local representations | `pm sync` explicitly replaces one Wave snapshot from Linear |
| Callers may perform provider reads during their own workflow | CLI, Wave, Task lifecycle, and Swift consume the snapshot |
| A mutation and local read state can drift independently | Every successful PM mutation refreshes the affected snapshot before returning |

Project and Task lifecycle code compose this API; they never construct a
`LinearClient`, query Linear directly, parse project files, or write planning
rows into SQLite.

The intended read modes are:

```rust
enum PmRefresh {
    Auto,  // fresh cache; bounded refresh when stale; cached soft-stale fallback
    Force, // explicit network refresh
    Never, // deterministic cache-only read
}
```

The product checkout is currently mid-transition. Before implementing this
design, rebase onto its landed PM contract and verify four integration points:

1. `PmShowOptions.refresh` is honored by `pm_show`, not merely declared.
2. CLI `--sync` and `--no-sync` map to `Force` and `Never`.
3. `PmItem` includes Linear's human identifier (`INF-123`) as well as its UUID.
4. `pm init` leaves a readable snapshot, or the lifecycle owns one temporary
   post-init sync.
5. A PM mutation preserves its committed provider result when the following
   snapshot refresh fails. The current `write Linear; refresh?; return result`
   shape must not collapse “issue created, cache refresh failed” into an error
   that discards the created UUID.

Use one narrow function module while those APIs settle; do not introduce a PM
provider trait or compatibility model:

```rust
mod task_pm {
    pub fn load_wave(repo: &Path, wave: &str, refresh: PmRefresh)
        -> OpsResult<PmShowResult>;

    pub fn create_and_load_task(
        repo: &Path,
        wave: &str,
        project: &str,
        title: &str,
        notes: &str,
    ) -> OpsResult<(PmShowResult, PmItem)>;

    pub fn complete_task(
        repo: &Path,
        wave: &str,
        item_id: &str,
        pr: &str,
    ) -> OpsResult<()>;
}
```

`create_and_load_task` may temporarily call `pm_update`, then load
`PmShowResult` with `Never` and find the returned UUID because the incoming
mutation result returns only an id. Mark that bridge
`TODO(product-pm): return the refreshed PmItem from task creation`, and remove
it once the PM mutation API returns the created item. If `pm init` lands
without snapshot seeding, a single `ensure_snapshot_after_init` bridge may call
`pm sync`; mark it for removal when init owns that invariant. These shims adapt
an in-flight internal API only—there is no compatibility path for project
files or old PM commands.

Do not shim an ambiguous committed mutation by matching title or creating
again. The PM API must return either a refreshed item or a structured committed
outcome such as:

```rust
pub struct PmMutationResult<T> {
    pub value: T,
    pub snapshot: PmSnapshotWrite,
}

pub enum PmSnapshotWrite {
    Refreshed { synced_at: i64 },
    Pending { error: String },
}
```

When creation is committed but refresh is pending, `task start` retains the
real UUID, reports that no execution was placed, and retries refresh/resolution
for that same item. It never issues a second create.

## Public API

### Formal record-first API

```text
lf project run <linear-project-id>
lf project status <linear-project-id> [--json]
lf project follow-up <linear-project-id> <message>
lf project steer <linear-project-id> <message>
lf project interrupt <linear-project-id> [--message <message>]
lf project decide <linear-project-id> <decision-id> <choice> [--message <message>]
lf project wait <linear-project-id> [--until waiting|terminal] [--timeout <duration>]
lf project resume <linear-project-id> [<message>]
lf project attach <linear-project-id>
lf project receipt <command-id> [--wait] [--timeout <duration>]
lf project abandon <linear-project-id> --reason <text>

lf task run <linear-issue-id>
lf task status <linear-issue-id> [--json]
lf task follow-up <linear-issue-id> <message>
lf task steer <linear-issue-id> <message>
lf task interrupt <linear-issue-id> [--message <message>]
lf task decide <linear-issue-id> <decision-id> <choice> [--message <message>]
lf task wait <linear-issue-id> [--until submitted|terminal] [--timeout <duration>]
lf task resume <linear-issue-id> [<message>]
lf task attach <linear-issue-id>
lf task receipt <command-id> [--wait] [--timeout <duration>]
lf task abandon <linear-issue-id> --reason <text>
```

The human issue identifier (`INF-123`) is the standard user-facing address;
the canonical Linear UUID is also accepted and is what persistence uses.
Internally, commands resolve either form to the one active Task Session.
Resolution fails loudly if there is none or if corrupted data contains more
than one. The Task Session id appears in JSON, logs, and audit drill-down, but
normal commands do not require it.

`lf project run` and `lf task run` return after their session is durably
registered and its process has started. Neither blocks until completion;
`wait` is the explicit blocking verb. There is no `dispatch` mode because both
are managed child sessions. Parallel implementation is several Task Sessions;
Project Sessions coordinate them and sleep when only child progress can change
the answer.

For a task with no existing session, `task run` loads the owning Wave's
`PmShowResult` in `Auto` mode, resolves one open `PmItem` and its `PmProject`,
and records the snapshot's `synced_at` in the launch receipt. A fresh snapshot
is network-free; a soft-stale refresh failure may use the labeled cached
snapshot; a hard-stale failure stops before placement. Controls for an existing
Task Session use its persisted ids and never refresh PM merely to send a
message or resume its provider process.

### Natural-language wrappers

```text
lf project start "make releases boring"
lf task start "add a hello-world command" --project <linear-project-id>
```

`start` means create the Linear record, then invoke the formal `run` path with
the returned id. It is not an overloaded identifier parser. If record creation
fails, no Task Session, worktree, branch, or provider process is created.

If Linear creation commits but snapshot refresh fails, the PM result retains
the UUID and a durable PM mutation receipt. `start` creates no worktree or
provider process; it reports the committed issue and resumes from that receipt
after sync. A retry reuses the committed UUID rather than issuing another
create. If the provider cannot make create idempotent directly, a local
request-id receipt belongs inside the PM mutation layer as a temporary shim,
not inside Task placement.

The wrapper calls `lf pm task create` semantics, which write Linear and refresh
the snapshot before returning. It then resolves the returned UUID from that
fresh snapshot with `Never`; it does not make another opportunistic network
read. `project start` follows the same write-then-refresh contract through
`lf pm project create`.

A Wave normally calls the same operations itself. The human's natural-language
request is not a hidden local task; the Wave creates the record first and
receives its id before starting execution.

### Example JSON

```json
{
  "issue_id": "5ed…",
  "issue_identifier": "INF-123",
  "project_id": "8ab…",
  "project_slug": "work-isolation",
  "pm_snapshot_synced_at": 1783728000,
  "pm_writeback": { "state": "current" },
  "session_id": "ts_01J...",
  "wave": "infrastructure",
  "status": "running",
  "worktree": "/Users/jack/src/loopflow.inf-123",
  "delivery": { "kind": "pull_request", "base": "main" },
  "provider": "codex"
}
```

Swift and automation invoke `lf --json`; they do not reimplement lifecycle
mutations through `lfd` HTTP.

## Lifecycle

```text
Linear issue
  → Created
  → Starting          reservation + worktree + process launch
  → Running           provider turn active
  ↔ Waiting           idle, external wait, or next instruction
  ↔ Submitted         PR open; review can resume Running
  ↔ Blocked/Failed    same session may resume
  → Merged            terminal; cleanup allowed
  → Abandoned         terminal only through explicit command
```

Starting is transactional in intent:

1. Load the owning Wave's `PmShowResult` through the selected freshness mode.
2. Resolve exactly one open `PmItem`, its `PmProject`, and owning Wave from that
   snapshot; persist their ids plus `synced_at` as the launch receipt.
3. Reserve the issue and concurrency slot in sqlite.
4. Record `TaskSession { status: Created }`.
5. Create one sibling worktree from current `main`; persist its exact base.
6. Transition to `Starting` and launch `lf __task <session-id>` in tmux.
7. The task runner attaches by session id, persists its provider session id,
   and transitions to `Running`.
8. Only then does `lf task run` report success.

Failure unwinds only resources created by this attempt. The durable record says
which step failed. No second caller can race through the reservation and create
a second worktree.

## Project runner

The Project runner owns repeated judgment, not file-writing execution.

```rust
pub async fn run_project_session(
    store: SharedStore,
    session_id: ProjectSessionId,
    generation: u32,
) -> Result<()>;

struct ProjectRunner {
    session: ProjectSession,
    harness: Box<dyn Harness>,
    commands: ChildCommandReceiver,
    observations: ChildObservationReceiver,
    events: ProjectEventWriter,
}
```

The runner resumes the same provider session across process generations. One
iteration executes the authored `project` flow's clarify, pursue, and mutate
steps against one captured Project id. The Project seed carries the current PM
definition/KRs, supervised Task summaries, pending decisions, and Wave context.
It never relies on `scratch/<branch>.md` for Project identity or KRs.

After the iteration, deterministic state inspection—not an LM-written loop
bit—chooses `Completed`, `Waiting`, `Blocked`, or another iteration. A state
fingerprint covers Project definition/KRs, filed Task ids/states, supervised
Task Session statuses, decisions, and the last consumed child event. Repeating
without a changed fingerprint blocks and reports; it never spins provider
turns merely because a KR remains open.

The Project runner may invoke only coordination lifecycle operations as part
of normal work: PM reads/writes and Project/Task commands. Repository edits,
commits, branches, PRs, and tests belong inside a Task Session. This is a
prompted trust boundary, not a sandbox; the conformance test proves the normal
Project workflow creates no Project worktree or branch.

The built-in skills reflect the ownership boundary:

- `wave_pursue` selects Projects, starts/steers Project Sessions, handles
  escalated decisions, and may run a small Task directly.
- `project_clarify` reads/writes the exact Linear Project named by its session;
  it never uses `scratch/<branch>.md` as the KR source.
- `project_pursue` creates/selects Tasks and supervises their Task Sessions; it
  never starts another Project or edits repository files.
- `project_mutate` evaluates evidence and reports its judgment; the runner,
  not an LM-authored loop bit, owns repeat/wait/block/complete mechanics.
- `task_*` skills remain focused on one issue, one worktree, and one PR.

## Task runner

Replace the generic headless pass loop with one inbox-aware task runner using
the existing Harness contract.

```rust
pub async fn run_task_session(
    store: SharedStore,
    session_id: TaskSessionId,
) -> Result<()>;

struct TaskRunner {
    session: TaskSession,
    harness: Box<dyn Harness>,
    controls: TaskControlReceiver,
    events: TaskEventWriter,
}
```

The runner has one cwd for its lifetime: `session.worktree`. Every task-flow
pass, tool call, test, review revision, and resume uses that checkout. The
provider process may stop while the PR waits, but the Task Session and
worktree remain.

The runner evaluates the configured Task flow. Flows remain customizable step
policy; they cannot redefine Task identity, worktree ownership, control,
delivery, or completion.

The current `flowloop/wave.rs` contains the honest control behavior worth
sharing: provider event handling, live-steer capability checks, queued
next-turn input, interruption, timeout, and usage. Extract the smallest common
runner core or call common functions directly. Do not create a factory trait or
a generic orchestration framework solely to make tests easier.

## Child commands and domain events

Project and Task control share one structured, durable command state machine.
Tmux is not the machine protocol. Domain events stay distinct because a
Project pursuing KRs and a Task shipping a PR are different things.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildCommandKind {
    FollowUp { text: String },
    Steer { text: String },
    Interrupt { replacement: Option<String> },
    Resume { message: Option<String> },
    Decide { decision_id: ChildDecisionId, choice: String, message: Option<String> },
    Abandon { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildCommandState {
    Persisted,
    Claimed,
    Accepted,
    Failed,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildCommandEffect {
    LiveSteer,
    NextTurn,
    Replacement,
    Decision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildCommand {
    pub id: ChildCommandId,
    pub target: ChildSessionRef,
    pub source: ChildCommandSource,
    pub kind: ChildCommandKind,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub created_at: OffsetDateTime,
    pub claimed_by_generation: Option<ProcessGeneration>,
    pub accepted_at: Option<OffsetDateTime>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildCommandSource {
    Wave(WaveId),
    Project(ProjectSessionId),
    Human,
    Attachment,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskEventKind {
    Started,
    StatusChanged { from: TaskSessionStatus, to: TaskSessionStatus, reason: String },
    CommandChanged {
        command_id: ChildCommandId,
        state: ChildCommandState,
        effect: Option<ChildCommandEffect>,
        error: Option<String>,
    },
    DecisionRequested {
        decision_id: ChildDecisionId,
        prompt: String,
        options: Vec<String>,
    },
    Progress { summary: String },
    PullRequestOpened { number: u32, url: String },
    Completed { pull_request: PullRequestRef, summary: String },
    Failed { error: String, resumable: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectEventKind {
    Started,
    StatusChanged {
        from: ProjectSessionStatus,
        to: ProjectSessionStatus,
        reason: String,
    },
    CommandChanged {
        command_id: ChildCommandId,
        state: ChildCommandState,
        effect: Option<ChildCommandEffect>,
        error: Option<String>,
    },
    TaskObserved {
        task_session_id: TaskSessionId,
        task_event_id: i64,
        event: Box<TaskEventKind>,
    },
    TaskStarted { task: TaskSessionId, issue_identifier: String },
    DecisionRequested {
        decision_id: ChildDecisionId,
        prompt: String,
        options: Vec<String>,
    },
    DecisionResolved {
        decision_id: ChildDecisionId,
        choice: String,
        message: Option<String>,
    },
    IterationCompleted { iteration: u32, summary: String },
    Completed { summary: String },
    Failed { error: String, resumable: bool },
}
```

Commands live until accepted, failed, or superseded. A runner generation
claims them in order. If that process dies, unresolved claims return to pending.
Every operation returns a receipt from this durable state; `live` is never
inferred merely because a Codex process happened to be running.

The three instruction verbs have provider-independent intent:

- `follow-up` preserves the current turn and becomes exactly the next turn.
- `steer` changes direction now: inject live when supported, otherwise
  interrupt and resume the same provider session with the instruction next.
- `interrupt --message` stops the current turn, supersedes every unaccepted
  instruction, and makes the replacement next. A bare interrupt stops and
  leaves the Task waiting.

The runner performs one final durable command claim before ending a turn. A
command racing the boundary is either accepted before exit or automatically
resumes the same Task Session; it cannot remain stranded in a waiting Task.

Provider behavior:

- Codex: `turn/steer` reaches the current turn; interrupt uses
  `turn/interrupt`.
- Claude: Loopflow's current per-turn process cannot accept live input;
  `steer` interrupts and resumes, while `follow-up` waits for the boundary.
- OpenCode: the current adapter does not claim live steering; `steer` aborts
  and resumes, while `follow-up` serializes like OpenCode background `extend`.

This is the minimum parity floor established in the dated capability matrix
and black-box scenarios in `scratch/research.md`. Loopflow deliberately omits
peer-to-peer teammate chat and nested teams; neither is necessary for a Wave to
control a Task. It must match the stronger parent controls: Codex’s addressed
interrupt-plus-input and structured submission receipt, OpenCode’s serialized
extensions/wait/cancel, and Claude’s lead-owned plan decision and automatic
updates.

Child judgment is part of steering. A Task or Project may publish
`DecisionRequested` and wait without losing its provider transcript. The
immediate supervisor answers with `Decide`; the child continues in the same
session. Plan approval is the first use. Provider permission prompts may map
into the same protocol later only when the supervisor has enough context to
decide safely; the MVP must not build a second generic approvals system.

## Supervision and conversation ownership

Wave, Project, and Task transcripts remain separate.

The Wave journal contains:

- the human conversation;
- creation and control of Project Sessions;
- linked Project decisions, status, blockers, and completion;
- linked events from Tasks the Wave started directly;
- explicit Wave overrides of descendant Tasks.

The Project event ledger and transcript contain:

- the captured Linear Project definition and KRs;
- Project iterations and the commands they issued;
- linked observations from supervised Tasks;
- decisions made locally or escalated to the Wave;
- the evidence used to declare KRs held or remain blocked.

The Task event ledger and transcript contain the focused implementation
directive, provider turns, tool activity, review revisions, commands, PR, and
completion evidence.

Raw child tool chatter is never copied upward. Each supervisor receives linked
typed state changes and summaries and can drill into the child transcript when
needed. The Wave can still answer which Projects and Tasks it started, what
they are doing, what they need, and what completed without loading every child
turn.

The primary path is Wave→Project→Task. Wave→Task remains valid for small direct
work and for root-owner override. Direct human Project/Task control is an
explicit escape hatch marked `Human`. A process carrying another
`LFD_WAVE_ID`, or a Project Session unrelated to the target Task, is refused
before command persistence.

Child completion queues one structured observation to its immediate
supervisor. If the supervisor is inactive, the durable observation relaunches
it once. If its turn is active, delivery waits for the next boundary. It never
starts a second concurrent turn for the same session. Project completion then
queues one structured observation to the Wave.

The SQLite observation outbox bridges durable child ledgers to supervisors. A
stopped Project or Wave misses nothing. The agent bus remains appropriate for
ephemeral prose; lifecycle commands, decisions, and observations do not inherit
its retention window.

## Context inheritance

New Project Sessions start from the owning Wave's objective and curated memory,
the authoritative Linear Project definition/KRs, filed Task state, and the
Wave's explicit pursuit directive. They do not copy the full human transcript.
Their provider session persists the Project's bounded working conversation
until completion or abandonment; it is not promoted into Wave memory
automatically.

New Task Sessions start focused, not as copies of the whole Wave transcript.
The initial task prompt contains:

- Linear issue id, title, description, and owning Linear Project;
- the Project definition and KRs;
- the PM snapshot `synced_at` and any stale-cache fallback warning;
- the Wave objective and curated memory;
- repository instructions and the selected Task flow/skills;
- the immutable worktree, base commit, delivery target, and completion rules;
- the immediate supervisor's explicit delegation message.

Provider/model, permission mode, reasoning settings, and budgets snapshot from
the launching supervisor. A Task cwd never inherits: it is always the task
worktree. A Project Session runs in the Wave home but has no file-writing
mission.

These planning fields come from one `PmShowResult` read. Prompt assembly does
not issue per-Project or per-Task Linear calls. Resuming an existing Task
Session uses its durable launch receipt and transcript; review feedback does
not implicitly refresh PM unless it performs a PM mutation.

Persist the provider session id after the provider announces it. Resume the
same provider history when supported; otherwise rebuild a bounded task prompt
from the durable Task transcript. Full Wave-history forks are deferred until
real runs show focused context is insufficient.

## PM identity, snapshots, and failure policy

Linear identity remains mandatory, but a live Linear network call does not.

- `project run` resolves an existing `PmProject` from `PmShowResult`.
- `task run` resolves an existing open `PmItem` from `PmShowResult`.
- Natural-language `start` wrappers create the record through the PM mutation
  API, which refreshes the snapshot before Task Session placement.
- Every task belongs to exactly one known Project and Wave before execution.
- A known task in an acceptable cached snapshot may start while Linear is
  unreachable; the snapshot is the official local read model.
- A missing snapshot, unknown item, hard-stale refresh failure, or failed PM
  mutation stops before worktree creation.

This is the precise fail-closed boundary: Loopflow never invents an unlinked
local task, but it does not make cached, already-identified work depend on a
fresh SaaS round trip. The error reports snapshot age and whether refresh was
skipped, attempted with cached fallback, or required and failed.

If real Linear outages materially block creation, that evidence can justify a
locally minted formal identity plus forward sync later. Do not build it
speculatively or backfill anonymous work.

Credential refresh and bounded retry happen inside the PM mutation/refresh
path before surfacing failure. A retry of `task start` after an ambiguous
network response must query through PM reconciliation/idempotency before
creating another issue. Task lifecycle code must not reach around `lf pm` to
perform that check directly against Linear.

## Project execution

```text
lf project run <linear-project-id>
```

This loads `PmShowResult` in `Auto`, resolves the Project by canonical id (or
its unique snapshot slug), appends its snapshot-backed definition/KRs and a
durable project directive to the owning Wave's inbox, and wakes the Wave when
its server is live. If the Wave is stopped, the command reports `queued` and
the next `lf serve` consumes it. It does not read a project file, spawn a
Project Worker, or create a project worktree.

The Wave may then:

1. decide an existing task is next and call `lf task run <id>`;
2. create a new Linear task and run it;
3. steer or interrupt active task sessions;
4. wait when evidence is external;
5. close or renew KRs once task results arrive.

A Project can therefore advance many tasks without becoming a branch or a
second durable mind.

## Worktrees and delivery

MVP delivery has one rule: every Task Session opens at most one PR to `main`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDelivery {
    pub base_branch: MainBranch,
    pub pull_request: Option<PullRequestRef>,
}
```

There is no delivery enum yet because the MVP has one delivery mode. Do not
encode deferred stacks or integration targets as unused variants.

Task worktrees are siblings created from current `main`. One Task Session is
the sole writer. Names are human hints; sqlite links issue, session, branch,
and path. Starting, submitting, merging, or deleting another task never moves,
renames, rebranches, or removes a live session's worktree.

Wave homes never own PRs and never accept file edits intended for delivery.
Remove the direct-wave escape hatch and its anchor-branch landing/reconcile
logic. Before each Wave turn, a clean Wave home may fast-forward to current
`main`; if it is dirty, the Wave reports the invariant failure instead of
resetting user work. A turn already in progress never sees its checkout change.

## Review, merge, and cleanup

Opening a PR transitions the session to `Submitted`; it does not end the Task.

- Review feedback sends a Task command and resumes the same provider session
  in the same worktree.
- CI failure does the same.
- A sleeping provider/tmux process may be relaunched without changing the Task
  Session id.
- The task becomes `Merged` only after its PR is observed merged into `main`.
- On merge, the lifecycle calls the PM `task done` mutation with the PR URL;
  that mutation comments the link, completes the Linear issue, and refreshes
  the Wave snapshot.
- If PM finalization fails after code has merged, delivery remains truthfully
  `Merged` and `pm_writeback` becomes `Pending { CompleteTask, error }`. The
  Wave receives an attention item and retries through the PM API; it never
  edits the SQLite snapshot or claims Linear is already complete.
- Cleanup may remove the worktree and local branch only after `Merged`.
- `Abandoned` requires an explicit reason. Before cleanup, every commit must be
  reachable from a pushed branch or recorded recovery ref.
- `Failed` and `Blocked` retain the worktree and remain resumable.

This makes the PR's afterlife part of the original Task rather than a new task
or an unowned branch. PM writeback failure does not resurrect delivered code or
pin a safe-to-remove worktree, but the Task is not reported fully reconciled
until `pm_writeback` returns to `Current`.

## Crash recovery

Both child-session kinds recover from SQLite plus provider transcript; Tasks
also recover from git:

1. `lf task status` compares durable status with the recorded process/tmux.
2. A missing process while `Starting` or `Running` becomes `Failed` with a
   resumable reason; it is never silently shown as active.
3. `lf task resume` validates the worktree and branch, releases stale process
   ownership, increments the process generation, and launches the runner.
4. Pending controls are reclaimed from the dead generation.
5. Provider history resumes by provider session id when possible.
6. The immediate supervisor receives one failure/recovery event, not a
   synthetic completion.

Project recovery performs the same generation checks without git/worktree
validation, then resumes from its PM snapshot, child-event cursor, and state
fingerprint.

Starting and resuming reserve concurrency before spawning, following Codex's
reservation-before-launch pattern. Every terminal process path releases the
slot, including startup failure, interrupt, provider crash, and forced stop.

## Tmux and direct attachment

Project and Task workers may run in named tmux sessions. Tmux owns process
lifetime and a human-facing terminal; it does not own agent semantics.

`lf task attach INF-123` attaches read-write. The task runner exposes a tiny
control prompt whose input calls the same durable Task command functions:

```text
task INF-123> focus on the parser
task INF-123> /interrupt fix the parser before touching docs
task INF-123> /status
task INF-123> /detach
```

Text is never written directly into a provider's stdin. Codex owns a JSON-RPC
pipe, OpenCode uses HTTP, and Claude runs with null stdin in Loopflow. Mapping
the TTY back through child commands keeps Project and Task attachment auditable
and provider neutral. `tmux send-keys` remains an emergency mechanism, not an
API used by a supervisor.

## Swift and other clients

The MVP exposes stable `lf --json` commands for project/task start, status,
control, and wait. Swift invokes those commands and reads the shared registry.

Required surface behavior:

- `lf status --json` exposes Project Sessions and their supervisor/child links
  alongside Task Sessions.
- Swift DTOs decode the required Project Session fields and may render a
  passive active-session row; this slice adds no Project control surface.
- Wave detail shows active Task Sessions and their Linear ids/status.
- Opening a Task shows its worktree, PR, latest event, and attach action.
- Existing Wave Chat continues to steer the Wave, which may control children.
- No Swift-owned process, placement, task lifecycle, or HTTP mutation model is
  added.

Rich direct task transcript/steering UI is deferred. The CLI and attach path
must prove the lifecycle first.

## Trust boundary

Loopflow-owned workers are trusted local processes with authority in their own
worktrees. Provider sandboxing stays disabled. The worktree boundary prevents
concurrent writers; it is not a security sandbox.

Standard execution enters through `lf`. There is no `lfq`, generic `/v0/exec`,
or task execution API in machine `lfd`. Remote execution may later transport
the same typed Task controls over SSH or a narrow receiver, but local MVP does
not keep a speculative proxy.

## Remaining implementation order

Complete the seven remaining behaviors without reopening the shipped Task
identity, placement, or delivery model:

1. Introduce `ProjectSession`, its event ledger/process generations, and
   `TaskSession.supervisor`; migrate existing Tasks to Wave supervision.
2. Refactor Task commands into the shared child-command envelope. In the same
   migration, make boundary settlement atomic and add receipt read/wait so both
   runners inherit one delivery guarantee.
3. Implement the Project runner and replace prose `project run` delivery with
   idempotent Project Session start/resume. Prove waiting stops the process and
   preserves the provider session.
4. Add the typed observation outbox and supervisor inbox path. Replace every
   prose Task notification and carry Task→Project→Wave completion without loss
   or duplication.
5. Add decision request/response at both supervision edges.
6. Build the provider-neutral conformance suite and fix adapters until all ten
   scenarios pass for Project and Task runners on Codex, Claude, and OpenCode.
7. Run the full repository gate and Mitchell Hashimoto-style ownership/failure
   review, then perform the explicit live two-Task Project dogfood and record
   its evidence.

This is infrastructure-first. Swift receives only the DTO update and passive
Project Session projection needed to keep `lf status --json` honest; its richer
Project/Task inspector and event rendering follow after the terminal contract
is proven. No new server, child-specific HTTP service, remote transport,
placement option, or delivery mode belongs in this slice.

## Code removed or simplified

The broad reduction is complete: rotation, `lfq`, generic exec/loop APIs,
detached-loop DTOs, stack placement and delivery, queue reconciliation,
`combine_prs`, wave landing, sandbox execution, project markdown mirrors, and
their Swift surfaces are gone. Treat their absence as an invariant.

The remaining slice should delete or reduce:

- every prose `post_to_named_wave("Task …")` notification once typed Task
  observations own delivery;
- the prose-only `project run` directive once Project Session creation owns the
  lifecycle;
- the two-second command-resolution wait as a correctness mechanism;
- duplicated boundary checks split between runner and control caller;
- Task-only command storage after Project proves the shared child-command
  concept;
- provider-name branches in conformance behavior, if any appear—the Harness
  capability contract owns the difference;
- any decision-specific approval path that duplicates the Task command/event
  state machine;
- test-only production abstractions added to script providers; protocol peers
  belong under test configuration instead.

### Reduce to one implementation

- One task placement function: Linear issue + `main` → reserved Task Session +
  sibling worktree.
- One planning read shape: `PmShowResult`; Task lifecycle adds no provider
  query DTO or normalized planning tables.
- One planning mutation owner: `lf pm`; Task lifecycle never instantiates a
  `LinearClient` or edits PM snapshots.
- One child process launcher used by Project and Task Sessions.
- One provider control loop behind `Harness`, parameterized by domain policy
  rather than provider name.
- One child command/receipt store; separate Project and Task event vocabularies.
- One durable observation outbox used at Task→Project, Task→Wave, and
  Project→Wave boundaries.
- One Task completion definition: merged or explicitly abandoned; one Project
  completion definition: every current KR observably holds.
- One PR target: `main`.
- One source of roadmap identity: Linear.

The `task_pm` module is a composition seam, not a second PM layer. TODO shims
must name the exact incoming API gap and be deleted when that gap closes; no
shim may read project markdown or support the superseded PM schema.

Manual `lf wt create` may remain as a diagnostic escape hatch only in sibling
mode. It is never how roadmap work starts.

## Explicit deferrals and roadmap owners

These are not compatibility gaps in the MVP. They are extensions to the
settled Wave→Project Session→Task Session hierarchy.

| Deferred capability | Roadmap owner | Opening task when pursued |
|---|---|---|
| Several isolated tasks contribute to one PR (`--into`) | Infrastructure / Work Isolation and Integration | Add a durable integration session that applies named task commits serially and resolves conflicts |
| Work begins on an unfinished future base (`--after`) | Infrastructure / Work Isolation and Integration | Add one dependency edge, retain the Task Session while waiting, and resume it to rebase/resolve after the dependency merges |
| Full deletion/drift check for retired vocabulary after the migration | Infrastructure / Technical Architecture | Make architecture docs and a stale-symbol check describe only Wave/Project/Task Sessions |
| Rich child transcript and steer UI | Product / Wave Chat + Auditability | Drill Wave event → Project Session → Task Session → transcript and send typed controls |
| Task list, attach, and status polish in the Mac app | Product / Mac Surface UX | Drive a week without terminal fallback for task supervision |
| Remote Task Sessions and transport | Product / Distributed Computing | Carry the same identity/control/event contract across SSH before adding a proxy |
| Evidence-based focused-context vs history-fork tuning | Intelligence / Context | Compare real Task streaks and token cost before adding a context-fork mode |

This table is a design routing map, not a repository roadmap mirror. After the
product PM change lands, create/update these Projects and Tasks through `lf pm`
so Linear authors them and the SQLite snapshot refreshes. Do not recreate
`wave/*/projects/*.md`.

Do not file an offline Linear identity task now. If Linear availability becomes
a measured source of blocked work, file it under Infrastructure / Developer
Efficiency with the outage evidence.

## Resolutions to the architectural review

1. **Child conversation** — Project and Task turns live in their own
   transcripts. Human messages remain in the Wave thread; every Wave→Project,
   Project→Task, Wave override, or direct human command is linked through typed
   events. Supervisors remember the negotiation without ingesting raw child
   tool chatter.
2. **Linear in the hot path** — identity is fail-closed, network access is not.
   Creation uses the PM mutation API and must reach Linear; an existing task
   may launch from the bounded-freshness SQLite snapshot. Missing/unknown or
   unusably stale PM state stops before placement. No offline authoring or
   backfill system is added.
3. **`--after` conflicts** — deferred. When built, the original Task Session
   and worktree remain waiting and resume to perform/resolve the rebase; no
   anonymous submission process handles conflicts.
4. **`--into` integration** — deferred. When built, a durable integration
   session owns serial application and conflict resolution; disjoint work is
   not assumed.
5. **Wave anchor reconciliation** — deleted. Wave homes never ship. They may
   fast-forward only between Wave turns while clean.
6. **PR review lifetime** — opening a PR is `Submitted`, not done. Review,
   CI repair, and merge remain the original Task Session's responsibility and
   retain its worktree.

## Constraints

- A Wave path and branch never change for its lifetime.
- A Task Session path and branch never change for its lifetime; its initial
  base commit remains recorded even if normal review work later rebases it.
- One live writer owns one worktree.
- Every file-writing task has a Linear issue before its worktree exists.
- Every planning read comes from the canonical-repo `PmShowResult`; a task
  worktree never creates its own PM snapshot namespace.
- Every planning mutation goes through `lf pm` semantics and refreshes the
  affected snapshot; Task lifecycle code never writes planning state locally.
- No repository Project mirror or `wave/*/projects/` compatibility reader is
  introduced.
- Every independent task produces zero or one PR, always targeting `main`.
- Projects own no worktree or branch.
- One Linear Project resolves to at most one non-abandoned Project Session.
- No Project- or Task-specific server is created.
- Child controls are structured and persisted; terminal bytes are not the
  machine API.
- The Wave remains available while Project and Task Sessions run.
- Provider-specific features stay behind Harness capabilities.
- No compatibility aliases preserve rotation, stack, queue, generic loop, or
  sandbox-worker APIs.
- Project pursuit is the only new loop; it is a domain lifecycle, not a public
  generic runner or user-authored process primitive.
- No generic multi-product execution platform is extracted.

## Done when: hello world and record identity

- From a newly served Wave, one human message “add a hello-world command”
  creates or selects one Linear Project, creates one Linear issue, starts one
  Task Session/worktree, and eventually opens one PR to `main`.
- The human runs zero explicit PM, branch, placement, worktree, server, tmux,
  or task commands in that workflow.
- `lf task run INF-123 --json` reports the same issue/session/worktree visible
  in `lf task status INF-123 --json` and the Wave status.
- `INF-123` and the canonical Linear UUID resolve to the same `PmItem` and Task
  Session; the identifier is carried by `PmShowResult`, not reconstructed from
  branch names or task titles.
- Retrying `task start` after an ambiguous Linear response or committed-create
  snapshot failure reuses the PM mutation receipt/UUID and creates no duplicate
  issue.
- A committed create followed by refresh failure reports the real issue UUID,
  creates no worktree/provider process, and can continue after `pm sync`.
- With Linear unavailable after credential refresh/retry, `task start` exits
  nonzero, the Wave stays steerable, and no branch, worktree, session, or
  provider process appears.
- With Linear unavailable but an acceptable snapshot containing INF-123,
  `task run INF-123` starts from that snapshot and records its age/fallback in
  the launch receipt.
- A missing snapshot, unknown task, or hard-stale refresh failure stops before
  reservation and gives an actionable `lf pm sync --wave <wave>` instruction.
- A successful `task start` observes the item in the mutation-refreshed
  snapshot without a second network refresh.
- An issue belonging to no known Project/Wave is refused before placement.
- A second simultaneous `task run INF-123` receives the existing session or a
  clear already-running result; it never creates a second writer.

## Done when: Project pursuit

- `lf project run <id> --json` creates one `ps_…` record, starts one Project
  process, returns the Linear Project/Wave ids, and creates no branch, worktree,
  server, PR, or child Wave.
- Repeating `project run` returns or resumes the same Project Session; two
  concurrent calls never create two Project processes or provider sessions.
- One Project iteration runs clarify, pursue, and mutate against the exact
  captured Linear Project. KRs and tasks come from the PM snapshot, never a
  local Project markdown file or ambient guess.
- The Project Session creates or selects concrete Linear Tasks and starts every
  file-writing change through `lf task run`; the Project process leaves the
  repository tree clean.
- When its Tasks are active, the Project Session becomes `Waiting`, exits its
  process, and consumes zero provider turns. A relevant Task event relaunches
  the same session/provider transcript exactly once.
- When open KRs remain but one complete iteration changes no PM, Task,
  decision, or observation state, the Project becomes `Blocked` and reports
  why instead of spinning.
- When all KRs observably hold, the Project becomes `Completed`, emits one
  completion to the Wave, and does not relaunch on duplicate Task events.
- `project resume` after process death keeps the Project Session id, Project
  id, event cursor, iteration, and provider session when the adapter supports
  resume. Unacknowledged commands and observations are reclaimed.
- A Task started by a Project records that Project Session as supervisor. The
  owning Wave can override it; another Wave or Project cannot.
- Promoting a Project remains a separate authored migration into a Wave. A
  normal Project Session gains no chat address, memory, cadence, or permanent
  home.

## Done when: steering, observations, and decisions

- In a deterministic race test, command insertion pauses after the runner's
  last ordinary poll. Boundary settlement then either claims that command in
  the current generation or marks the session inactive and causes insertion to
  reserve the next generation. Repeating this for `follow-up`, `steer`, and
  `interrupt --message` leaves zero unresolved commands in a waiting session.
- Killing a runner after persistence and after claim lets the next generation
  reclaim the command. Killing it after provider acceptance never replays the
  accepted command.
- `lf task receipt <id> --wait --timeout 30s --json` and `lf project receipt`
  return the same command
  id, state, effect, generation, accepted timestamp, and error stored in SQLite.
  A timeout is distinguishable from failure and can be resumed by running the
  same command again.
- Every consequential child event appears once in its source ledger and once
  in its immediate supervisor with the same `(source, event_id)`. Restarting a
  Task, Project, or Wave and retrying delivery creates no duplicate observation.
- An idle supervisor wakes once for a child observation. An active supervisor
  completes its current turn before consuming it. No observation starts
  concurrent turns, and no raw child tool event enters a parent conversation.
- A Task requests plan approval from its Project. The Project rejects with
  feedback, receives a revision, escalates one linked decision to the Wave, and
  then approves the Task while both provider transcripts remain continuous.
  Duplicate and foreign-supervisor answers are refused or idempotently return
  the existing resolution.
- The normal Rust gate runs all ten conformance scenarios against Codex,
  Claude, and OpenCode adapters. Codex proves `live_steer`; Claude/OpenCode prove
  interrupt-and-resume `replacement`; every provider proves FIFO follow-up,
  replacement supersession, recovery, ownership, decisions, observations,
  targeting, and lifecycle distinction.
- One live Linear-backed Project Session supervises two Task Sessions from
  creation through steering, decision, sleep/wake, PR review/resume, merge, PM
  reconciliation, KR verification, and one typed Project completion in the
  Wave. The PR notes contain the linked ids and receipts needed to audit the
  hierarchy.

## Done when: process, attachment, and recovery

- `lf task attach INF-123` and `lf project attach <id>` open their writable
  tmux control session; entered text becomes an audited child command rather
  than raw provider stdin.
- `/interrupt`, `/status`, and `/detach` work from the attached prompt without
  corrupting the provider protocol.
- Killing the tmux session changes a supposedly running task to a resumable
  failed state with an actionable reason on the next status check.
- `lf task resume INF-123` reuses the same Task Session, worktree, branch, and
  provider history; `lf project resume <id>` reuses the same Project Session,
  event cursor, and provider history when supported.
- Unacknowledged commands owned by a dead process generation are reclaimed;
  acknowledged commands are not silently lost.
- Starting/resuming beyond the Wave's configured concurrency fails before
  process launch and leaks no reservation.
- Every startup, interrupt, provider crash, timeout, merge, and abandonment
  path releases process capacity.

## Done when: review, delivery, and cleanup

- Every MVP Task PR targets `main`; no queue, parent PR, stack group, wave
  branch, or inferred target participates.
- A submitted PR retains its Task Session and worktree.
- Review feedback three days later resumes the same Task Session and updates
  the same PR.
- CI failure can be sent to the same session and repaired without creating a
  new task or worktree.
- A merged PR transitions the session to `Merged`, emits one completion event,
  then permits worktree/branch cleanup.
- Merge finalization calls the PM task-done mutation with the PR URL and the
  refreshed `PmShowResult` shows the item complete.
- If that PM mutation fails after merge, delivery remains `Merged`, cleanup is
  safe, `pm_writeback` is visibly pending, and a later retry reconciles Linear
  without recreating the Task Session.
- Failed and blocked sessions are never cleaned automatically.
- Abandonment requires a reason and refuses cleanup until commits are pushed
  or otherwise reachable.
- Starting, merging, or cleaning Task B never changes Task A's path, branch,
  checkout, process, or provider session.
- A Wave home never opens a PR. Attempting to submit from a Wave anchor gives a
  direct instruction to create/run a Linear task.

## Done when: project and parallel workflows

- `lf project run <id>` starts the Project Session under the owning Wave with
  the correct Project/KRs and creates no project branch, worktree, server, or
  child Wave.
- The Project Session can create and start Task A, then create and start
  independent Task B without waiting for A.
- Both tasks write only in their own worktrees and open independent PRs to
  `main`.
- `lf task wait INF-123` blocks without polling an LM and returns the terminal
  status; timeout returns current status without changing the task.
- `lf project wait <id>` blocks without polling an LM and returns waiting,
  blocked, failed, completed, or abandoned state as requested.
- A Project result is evaluated from its Linear KRs, supervised Task events,
  and merged PR evidence; no recursive Project or local roadmap item is
  introduced.

## Done when: clients and observability

- CLI text and JSON expose Project/issue id, Project/Task Session id, supervisor,
  state reason, provider, process liveness, latest event, and—only for Tasks—
  worktree and PR.
- The Mac Wave detail passively lists active Project Sessions alongside the
  existing Task projection. Rich Project/Task controls and observation
  rendering remain the later UI; it reads these same `lf --json` shapes and
  owns no duplicate lifecycle state.
- Every visible `Failed`, `Blocked`, `Waiting`, `Submitted`, and terminal state
  includes a reason and timestamp.
- Wave event → Project Session → Task Session → raw transcript/worktree/PR is
  traceable from ids.
- Provider usage and cost attach to the Task Session and roll up to the Wave.

## Done when: deletion and simplification

- `rg` finds no `lfq`, `/v0/exec`, wave rotation, `/waves/{wave_id}/next`,
  generic detached-loop route, public `--detach`, public stack/child placement,
  stack queue, or `combine_prs` product surface.
- The active `Run`/Task model has no stack position/group/status, parent PR,
  lineage inference, or arbitrary target branch fields.
- Rust has one steerable provider-loop implementation used by Task Sessions;
  the old blocking pass loop is deleted.
- Swift has no execution/placement/session lifecycle that competes with `lf`.
- Built-in skills and docs name only Wave, Project, Task, Task Session, and
  provider harness where those concepts are exercised.
- The final diff is within the roughly 5,000-line implementation budget and
  shows substantial deletion of the old lifecycle.

## Verification

```text
cargo fmt --check
cargo test -p loopflow
cargo clippy -p loopflow --all-targets -- -D warnings
swift test --package-path swift
scripts/test.py --rust --python --swift
```

Add focused end-to-end tests for create/run, concurrent reservation, live and
queued steer, interrupt-and-message, crash/resume, submitted-review-resume,
merge cleanup, PM create-refresh-run composition, identifier/UUID resolution,
fresh/soft-stale/hard-stale snapshot behavior, post-merge PM writeback retry,
Wave journal folding, JSON round trips, and stale-symbol deletion.

The known headless Loopflow UI runner hang remains “unproven,” not a regression
signal. Perform the required simulated operational review before handoff:
simple interfaces, one owner per state, failure reasons visible at 2 a.m., and
no abstraction retained solely for hypothetical flexibility.
