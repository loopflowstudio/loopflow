# Linear-backed, steerable task sessions

## What to build

Make Wave, Project, and Task the core Loopflow lifecycle. A Wave remains a
stable, directly steerable coordinator in its permanent home. Every concrete
change begins as a Linear task and runs in one durable Task Session with an
immutable worktree, its own provider transcript, structured Wave controls, and
one PR to `main`.

The MVP replaces the competing generic-loop, placement, stacking, queue, and
wave-shipping APIs in one migration. It is intentionally an ambitious PR:
shipping half of this API would leave two answers for how work starts and make
every following change pay conversion cost again.

> “The standard way to execute a task should basically be to create the Linear
> task and then just pass the Linear ID to the task.”

> “As standard operating procedure it makes sense for Waves to first create
> tasks or projects and then execute or dispatch a worker on top of it.”

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

## Already true on this branch

The first reduction slice is implementation, not future scope:

- Wave rotation (`lf op next`, wire route, Swift action) is deleted.
- `lfq`, both generic exec doors, the shared exec engine, and subagent token
  plumbing are deleted.
- Loopflow-owned passes launch trusted and unsandboxed in their isolated
  worktrees.
- The focused Rust library suite passes after those deletions.

That slice has already removed 1,356 lines from the working tree; stale-symbol
searches for rotation and `lfq` are clean. Preserve those deletions while the
larger lifecycle replaces the remaining paths.

The rest of this document specifies the large MVP still to build. The old
foreground Wave-epoch design is superseded; do not implement resident-seat
transfer or harness cwd rebinding.

## The demo

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

The equivalent formal CLI path is:

```text
lf task run INF-123
lf task send INF-123 "also name the flag --hello"
lf task wait INF-123
```

## Product model

### Wave

A Wave owns one durable human conversation, memory, cadence, budget, project
selection, and supervision. Its server and worktree path are permanent. Its
home is a control-plane checkout, never a shipping branch or PR.

The Wave may manage zero, one, or several Task Sessions. It stays directly
steerable while they run. Talking to the Wave never silently retargets the
human to a child transcript.

### Project

A Project is one Linear Project inside exactly one Wave/Linear Initiative. It
owns a definition and proof-shaped KRs. It has no permanent worktree, branch,
server, memory, or child project.

Running a Project means asking its Wave to evaluate the KRs and select or create
concrete Linear tasks. All file-writing work then happens through Task
Sessions.

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

The Task Session is the missing runtime concept. It is durable state, not the
tmux process and not the provider's session id. Killing a process does not
delete the Task Session. Resuming does not create a second task.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSession {
    pub id: TaskSessionId,
    pub issue_id: String,         // PmItem.id: canonical Linear UUID
    pub issue_identifier: String, // PmItem.identifier: INF-123
    pub project_id: String,       // PmProject.id
    pub project_slug: String,     // PmProject.slug
    pub wave_id: WaveId,
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
async fn send_input(session_id: TaskSessionId, input: TaskInput) -> Result<SubmissionId>;
async fn interrupt(session_id: TaskSessionId) -> Result<SubmissionId>;
async fn subscribe_status(session_id: TaskSessionId) -> Result<StatusReceiver>;
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
It launches a normal provider harness inside a Loopflow Task Session so Linear
identity, worktree isolation, control, audit, and recovery behave the same on
Codex, Claude, and OpenCode.

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

Task lifecycle code composes this API; it never constructs a `LinearClient`,
queries Linear directly, parses project files, or writes planning rows into
SQLite.

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

lf task run <linear-issue-id>
lf task status <linear-issue-id> [--json]
lf task send <linear-issue-id> <message>
lf task interrupt <linear-issue-id> [--message <message>]
lf task wait <linear-issue-id> [--until submitted|terminal] [--timeout <duration>]
lf task resume <linear-issue-id> [<message>]
lf task attach <linear-issue-id>
lf task abandon <linear-issue-id> --reason <text>
```

The human issue identifier (`INF-123`) is the standard user-facing address;
the canonical Linear UUID is also accepted and is what persistence uses.
Internally, commands resolve either form to the one active Task Session.
Resolution fails loudly if there is none or if corrupted data contains more
than one. The Task Session id appears in JSON, logs, and audit drill-down, but
normal commands do not require it.

`lf task run` returns after the session is durably registered and its worker
has started. It never blocks until completion; `wait` is the explicit blocking
verb. There is no `dispatch` mode because every task is a managed child
session. Parallelism is simply several running tasks.

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

## Commands and events

Task control is structured and durable. Tmux is not the machine protocol.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskCommandKind {
    Message { text: String },
    Interrupt { next_message: Option<String> },
    Resume { message: Option<String> },
    Abandon { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCommand {
    pub id: TaskCommandId,
    pub session_id: TaskSessionId,
    pub source: TaskCommandSource,
    pub kind: TaskCommandKind,
    pub created_at: OffsetDateTime,
    pub claimed_by_generation: Option<ProcessGeneration>,
    pub acknowledged_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskCommandSource {
    Wave(WaveId),
    Human,
    Attachment,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskEventKind {
    Started,
    StatusChanged { from: TaskSessionStatus, to: TaskSessionStatus, reason: String },
    CommandAccepted { command_id: TaskCommandId },
    Progress { summary: String },
    PullRequestOpened { number: u32, url: String },
    Completed { pull_request: PullRequestRef, summary: String },
    Failed { error: String, resumable: bool },
}
```

Commands live until acknowledged. A runner generation claims them in order.
If that process dies, its unacknowledged claims return to pending. Delivery is
at-least-once and command ids are visible in the transcript; a duplicate must
be visible rather than silently lost.

Follow-up messages serialize. If the provider supports mid-turn steering, the
runner injects the message into the active turn. Otherwise it becomes the next
turn's input. `interrupt --message` explicitly stops the current turn and makes
the message next, giving all providers one deterministic redirect operation.

Provider behavior:

- Codex: `turn/steer` reaches the current turn; interrupt uses
  `turn/interrupt`.
- Claude: Loopflow's current per-turn process cannot accept live input; queue
  for the next `--resume` turn, or interrupt and restart.
- OpenCode: the current adapter does not claim live steering; serialize a
  follow-up turn, matching OpenCode's own background `extend` behavior.

## Wave supervision and conversation ownership

The Wave and Task transcripts remain separate.

The Wave journal contains:

- the human's request to the Wave;
- creation of the Linear Project/Task;
- start/status events for the Task Session;
- every Wave→Task command, with task and command ids;
- progress reports the task explicitly sends to the Wave;
- PR, failure, merge, and abandonment events.

The Task transcript contains:

- the focused task directive and assembled context;
- all provider turns, reasoning summaries, tool calls, outputs, and controls;
- review-feedback turns and resumed work.

Raw task tool chatter is not copied into the Wave thread. The Wave can drill
into it when needed, but its normal context uses linked control and result
events. This satisfies the Context project's “act on child reports without
opening the child transcript” proof while preserving auditability.

If the human uses `lf task send` or direct task UI rather than speaking through
the Wave, the command still mirrors into the Wave journal. The Wave therefore
does not forget a consequential instruction merely because it entered through
a task surface.

Task completion queues one structured Wave event. If the Wave is idle, that
event wakes pursuit once. If a Wave turn is active, it is delivered at the next
turn boundary. It never starts a second concurrent Wave turn.

The Wave server tails durable Task events for its Wave with a stored cursor,
folding each event before advancing the cursor. A stopped Wave misses nothing:
events are session history, not the one-hour agent bus. The existing bus stays
appropriate for ephemeral prose reports; Task commands and lifecycle events do
not inherit its retention window.

## Context inheritance

New Task Sessions start focused, not as copies of the whole Wave transcript.
The initial task prompt contains:

- Linear issue id, title, description, and owning Linear Project;
- the Project definition and KRs;
- the PM snapshot `synced_at` and any stale-cache fallback warning;
- the Wave objective and curated memory;
- repository instructions and the selected Task flow/skills;
- the immutable worktree, base commit, delivery target, and completion rules;
- the Wave's explicit delegation message.

Provider/model, permission mode, reasoning settings, and budgets snapshot from
the Wave at launch. Cwd never inherits: it is always the task worktree.

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

The Task Session is recoverable from sqlite plus git plus provider transcript:

1. `lf task status` compares durable status with the recorded process/tmux.
2. A missing process while `Starting` or `Running` becomes `Failed` with a
   resumable reason; it is never silently shown as active.
3. `lf task resume` validates the worktree and branch, releases stale process
   ownership, increments the process generation, and launches the runner.
4. Pending controls are reclaimed from the dead generation.
5. Provider history resumes by provider session id when possible.
6. The Wave receives one failure/recovery event, not a synthetic completion.

Starting and resuming reserve concurrency before spawning, following Codex's
reservation-before-launch pattern. Every terminal process path releases the
slot, including startup failure, interrupt, provider crash, and forced stop.

## Tmux and direct attachment

Task workers may run in named tmux sessions. Tmux owns process lifetime and a
human-facing terminal; it does not own agent semantics.

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
the TTY back through Task commands keeps attachment auditable and provider
neutral. `tmux send-keys` remains an emergency mechanism, not an API used by
the Wave LM.

## Swift and other clients

The MVP exposes stable `lf --json` commands for project/task start, status,
control, and wait. Swift invokes those commands and reads the shared registry.

Required surface behavior:

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

## Large MVP implementation

One PR should deliver the complete independent-task lifecycle:

1. Rebase onto the landed product PM branch and verify the snapshot freshness,
   mutation-refresh, identifier, and init-seeding integration points above.
2. Add the narrow `task_pm` adapter, with removal TODOs only for confirmed
   in-flight PM return-shape/init gaps.
3. Add Linear-backed Project/Task CLI verbs and JSON contracts composed from
   `PmShowResult`, `PmProject`, and `PmItem`.
4. Add `TaskSession`, statuses, commands, events, persistence, reservations,
   and process generations.
5. Add sibling task placement from `main` and one-PR delivery.
6. Add the steerable task runner and provider-neutral control behavior.
7. Add tmux launch, writable attach/control prompt, liveness, resume, wait, and
   cleanup.
8. Fold Task control/result events into the Wave journal and status.
9. Update built-in Wave/Project/Task skills to create records before execution
   and supervise Task Sessions.
10. Move Swift to the CLI JSON contract for the MVP fields it displays.
11. Remove every old public path that competes with the new lifecycle.
12. Migrate the local sqlite schema and dogfood the hello-world path.

Target at most roughly 5,000 added or substantially rewritten implementation
lines, excluding mechanical test fixtures and code deletion. The PR should be
net-negative or close to it because the old generic/stacked paths disappear.

## Code removed or simplified

### Already removed on this branch

- `ops::next` and wave rotation wire/Swift surfaces.
- `lfq` binary and library.
- machine and per-wave generic exec routes.
- shared exec engine, subagent token, and sandbox escape door.

### Remove in the large MVP

- Public `lf loop` and `lf loop --detach` task/project lifecycle.
- `/loops`, `DetachedLoopRequest`, `DetachedLoopResponse`, and generic detached
  launch authorization.
- `flowloop/driver.rs`, `flowloop/pass.rs`, and `flowloop/run.rs` once their
  useful caps/termination behavior lives in Task Sessions.
- `Placement::Stack`, `PlacementRequest::Stack`, child/stack placement plans,
  and public `--stack`, `--child`, `--fork`, and generic `--place` work-start
  flags.
- Stack queue operations, attention projections, repair/inference, DTOs, and
  tests.
- `RunStackStatus`, `stack_position`, `stack_group_id`, `parent_pr_number`,
  `lineage_inferred`, and arbitrary `target_branch` from the active run model.
- `combine_prs` and branch-name-substring PR discovery.
- Direct wave landing, wave PR state, wave-home post-merge reconciliation, and
  remote tracking for wave anchor branches.
- Any Swift session lifecycle or mutation stubs whose only caller was the old
  HTTP/placement model.
- The branch's temporary edits under `wave/*/projects/`; accept the product
  branch's deletion and move those roadmap changes through `lf pm` after its PM
  API lands.
- Documentation, prompts, and tests teaching rotation, generic detached loops,
  stacks, queues, sandbox workers, or task work in wave homes.

### Reduce to one implementation

- One task placement function: Linear issue + `main` → reserved Task Session +
  sibling worktree.
- One planning read shape: `PmShowResult`; Task lifecycle adds no provider
  query DTO or normalized planning tables.
- One planning mutation owner: `lf pm`; Task lifecycle never instantiates a
  `LinearClient` or edits PM snapshots.
- One task process launcher: Task Session → tmux runner.
- One provider control loop behind `Harness`.
- One Task command/event store used by CLI, Wave, tmux attachment, and Swift.
- One completion definition: merged or explicitly abandoned.
- One PR target: `main`.
- One source of roadmap identity: Linear.

The `task_pm` module is a composition seam, not a second PM layer. TODO shims
must name the exact incoming API gap and be deleted when that gap closes; no
shim may read project markdown or support the superseded PM schema.

Manual `lf wt create` may remain as a diagnostic escape hatch only in sibling
mode. It is never how roadmap work starts.

## Explicit deferrals and roadmap owners

These are not compatibility gaps in the MVP. They are extensions to one
settled Task Session model.

| Deferred capability | Roadmap owner | Opening task when pursued |
|---|---|---|
| Several isolated tasks contribute to one PR (`--into`) | Infrastructure / Work Isolation and Integration | Add a durable integration session that applies named task commits serially and resolves conflicts |
| Work begins on an unfinished future base (`--after`) | Infrastructure / Work Isolation and Integration | Add one dependency edge, retain the Task Session while waiting, and resume it to rebase/resolve after the dependency merges |
| Full deletion/drift check for retired vocabulary after the migration | Infrastructure / Technical Architecture | Make architecture docs and a stale-symbol check describe only Wave/Project/Task Sessions |
| Rich direct Task transcript and steer UI | Product / Wave Chat + Auditability | Drill Wave event → Task Session → transcript and send typed Task controls |
| Task list, attach, and status polish in the Mac app | Product / Mac Surface UX | Drive a week without terminal fallback for task supervision |
| Remote Task Sessions and transport | Product / Distributed Computing | Carry the same identity/control/event contract across SSH before adding a proxy |
| Evidence-based focused-context vs history-fork tuning | Intelligence / Context | Compare real Task streaks and token cost before adding a context-fork mode |
| Multiple-task project autonomy and budget scheduling | Product / Loopflow API | Run a Project through several Task Sessions with caps and no escaped work |

This table is a design routing map, not a repository roadmap mirror. After the
product PM change lands, create/update these Projects and Tasks through `lf pm`
so Linear authors them and the SQLite snapshot refreshes. Do not recreate
`wave/*/projects/*.md`.

Do not file an offline Linear identity task now. If Linear availability becomes
a measured source of blocked work, file it under Infrastructure / Developer
Efficiency with the outage evidence.

## Resolutions to the architectural review

1. **Mid-task conversation** — Task turns live in the Task transcript. Human
   messages to the Wave remain in the Wave thread; every Wave→Task or direct
   human→Task command is mirrored there as a linked event. The Wave remembers
   the negotiation without ingesting raw child tool chatter.
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
- No task-specific server is created.
- Task controls are structured and persisted; terminal bytes are not the
  machine API.
- The Wave remains available while tasks run.
- Provider-specific features stay behind Harness capabilities.
- No compatibility aliases preserve rotation, stack, queue, generic loop, or
  sandbox-worker APIs.
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

## Done when: steering and conversation

- While a Codex task turn is active, `lf task send INF-123 "rename the flag"`
  reaches that turn through structured steer and records one accepted command.
- While Claude or OpenCode is active, the same command becomes the next turn's
  input and is shown as queued, never falsely reported as live injection.
- `lf task interrupt INF-123 --message "take the smaller approach"` interrupts
  the current provider turn and delivers the replacement instruction next on
  all three providers.
- A message the human sends directly to the Task is visible as a linked event
  in the Wave journal before the next Wave turn.
- Raw Task tool events do not flood the Wave thread; the Wave can answer what
  it asked the task to do, its latest status, and its result without opening
  the child transcript.
- Task completion wakes an idle Wave exactly once or queues behind an active
  Wave turn; it never starts concurrent Wave turns.
- A Wave can steer Task A while Task B runs, and each command resolves to the
  correct Linear issue/session in N/N targeted tests.

## Done when: process, attachment, and recovery

- `lf task attach INF-123` opens the writable tmux session and text entered at
  the task prompt becomes an audited Task command rather than raw provider
  stdin.
- `/interrupt`, `/status`, and `/detach` work from the attached prompt without
  corrupting the provider protocol.
- Killing the tmux session changes a supposedly running task to a resumable
  failed state with an actionable reason on the next status check.
- `lf task resume INF-123` reuses the same Task Session, worktree, branch, and
  provider history when supported.
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

- `lf project run <id>` wakes the owning Wave with the correct Project/KRs and
  creates no project branch, worktree, server, or child Wave.
- The Wave can create and start Task A, then create and start independent Task
  B without waiting for A.
- Both tasks write only in their own worktrees and open independent PRs to
  `main`.
- `lf task wait INF-123` blocks without polling an LM and returns the terminal
  status; timeout returns current status without changing the task.
- A Project result can be evaluated from its Linear tasks and Wave events; no
  recursive project or local roadmap item is introduced.

## Done when: clients and observability

- CLI text and JSON expose issue id, Task Session id, state reason, worktree,
  provider, process liveness, PR, and latest event.
- The Mac Wave detail shows the same active-task state by invoking `lf --json`;
  it owns no duplicate lifecycle state.
- Every visible `Failed`, `Blocked`, `Waiting`, `Submitted`, and terminal state
  includes a reason and timestamp.
- Wave event → Task Session → raw transcript/worktree/PR is traceable from ids.
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
