# Multi-PR Tasks with readable workspaces

## What to build

A Linear Task owns one durable Task Session and one stable worktree, while an
ordered sequence of serial deliveries owns the branches and zero, one, or many
PRs that advance the Task. Task completion is explicit and independent from PR
merge, with `lf pr land -c` retaining a terse one-PR happy path.

> im pretty sure task = multiple Prs is the right model (it also answers the question: what about tasks that arent completed via PR?), but we should have an API that is super well grooved for the 1 pr = 1 task happy case

> and we should find some sort of worktree/branch name strategy that works well with the fact that tasks are still the owners of worktrees and   for now at least are fully serial in their PRs

> i would prefer branch names that are more accessible than the ids, LLMs should be able to adapt to conflicts for non-deterministic algorithms

> lets change it so lf pr land does not complete by default, we just have --complete,-c as a flag you can pass

## The demo

The common case stays short:

```bash
lf task run W2-127 --name release-scoped-migrations
# work
lf pr land -c
```

The worktree is `loopflow.release-scoped-migrations`, the branch is
`jack-heart/release-scoped-migrations`, and merge completes the Task.

A multi-PR Task changes only the landing call:

```bash
lf pr land --next released-upgrade-proof
# after merge the runner rotates the same worktree to fresh origin/main on
# jack-heart/release-scoped-migrations-released-upgrade-proof

# work the next serial delivery
lf pr land --complete
```

`lf task status W2-127 --json` shows both PRs in order, one stable worktree, and
at most one active delivery. A clean investigation Task can finish without a PR:

```bash
lf task complete W2-200 --summary "Root cause and evidence recorded in Linear"
```

## Core model

The current model in `rust/loopflow/src/task/mod.rs` collapses Task, delivery,
branch, and PR into `TaskSession`: one immutable `branch`, one `base_commit`, one
optional `pull_request`, and terminal `Merged`. Split the real concepts instead.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSession {
    // existing identity, launch receipt, ownership, process, and directive fields
    pub worktree: PathBuf,
    pub workspace_slug: String,
    pub status: TaskSessionStatus,
    // branch, base_commit, and pull_request move to TaskDelivery
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDelivery {
    pub id: TaskDeliveryId,
    pub task_session_id: TaskSessionId,
    pub sequence: u32,
    pub slug: String,
    pub branch: String,
    pub base_commit: String,
    pub status: TaskDeliveryStatus,
    pub after_merge: AfterMerge,
    pub pull_request: Option<PullRequestRef>,
    pub merge_commit: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDeliveryStatus {
    Working,
    Submitted,
    Merged,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AfterMerge {
    Review,
    CompleteTask,
}
```

`TaskSessionStatus` describes the Task process, not a GitHub artifact: keep
`Created`, `Starting`, `Running`, `Waiting`, `Blocked`, `Failed`, and `Abandoned`;
replace terminal `Merged` with terminal `Completed`; remove `Submitted` or stop
emitting it as Task state. An open PR is `TaskDeliveryStatus::Submitted` while the
Task waits.

Expose the aggregate without defaults in the JSON DTO:

```rust
pub struct TaskSessionSnapshot {
    // existing Task fields
    pub worktree: PathBuf,
    pub workspace_slug: String,
    pub deliveries: Vec<TaskDelivery>,
    pub active_delivery: Option<TaskDeliveryId>,
}
```

Update every Rust/Swift fixture or mirror if this snapshot crosses that boundary;
do not leave singular compatibility fields beside the new source of truth.

## Persistence

Add `task_deliveries` with a forward migration created by
`scripts/new_migration.py` after rebasing onto the current released main. Do not
edit the shipped baseline.

Required constraints:

- primary key `id`; foreign key `task_session_id` with cascade delete;
- unique `(task_session_id, sequence)` and globally unique `branch`;
- a partial unique index permits at most one `Working` or `Submitted` delivery
  per Task Session;
- `pr_number` and `pr_url` are both absent or both present;
- merged delivery requires a PR and `merge_commit`;
- sequence is append-only and starts at 1.

Move `branch`, `base_commit`, `pr_number`, and `pr_url` out of `task_sessions` in
the same migration. Migrate every existing Task Session to one sequence-1
delivery, preserving its existing ID-shaped branch as historical truth. Readable
naming applies to newly placed Tasks; do not rename a live worktree or open PR.

Store mutations that settle one delivery and create the next must be one SQLite
transaction. A crash may leave Git ahead of the registry, so reconciliation must
either adopt the uniquely matching checked-out branch or report a precise repair;
it must never create a second active delivery.

## Lifecycle and API

`lf pr open` discovers the owning Task from `LF_TASK_SESSION_ID` or the worktree
path and attaches the PR to the active delivery. It leaves `after_merge=Review`.

`lf pr land` and `lf pr submit` keep their current behavior outside Task
worktrees. Inside one:

- bare `lf pr land` leaves `after_merge=Review`: it lands this delivery, not the
  Task;
- `--complete` / `-c` sets `after_merge=CompleteTask` before arming merge;
- both reject an absent or ambiguous active delivery;
- retry is idempotent and updates the same delivery/PR; re-running with or
  without `-c` before merge updates `after_merge` — the last call wins.

Give `lf pr submit` the same disposition flags so choosing human-gated versus
hands-off merge does not change Task semantics. A submit/land call with no Task
flag always means `Review`.

Any reconciliation surface (`lf task status`, `run`, `resume`, `wait`, and the
Project supervisor) observes PR merge:

1. Mark the delivery `Merged`, recording the merge commit.
2. `CompleteTask`: set Task `Completed`, write the Linear completion, and retain
   pending PM writeback exactly as today if Linear is unavailable.
3. `Review`: the Task stays open. The runner's next iteration boundary opens
   the next delivery; only `-c` or `lf task complete` ends the Task. A PR
   merged manually from GitHub takes the same path — merge is never guessed to
   mean Task completion.

`lf pr abandon` inside a Task worktree settles the active delivery as
`Abandoned` (closing its PR if open). It never completes or abandons the Task
itself; discarding an approach and starting fresh is one settle away.

The runner owns rotation. At every iteration boundary — end of a flow pass,
and on resume after a dead process — it enforces one contract:

1. Task `Completed` or `Abandoned`: stop the loop.
2. Task open and the active delivery settled (`Merged` or `Abandoned`): open
   the next delivery — fetch `origin`, require the stable worktree clean,
   rotate it onto a fresh branch from the fetched default branch, append the
   next `Working` delivery, and push the remote branch.
3. Task open with a `Submitted` delivery: wait on merge as today.

Every iteration therefore starts on an active `Working` delivery; clarify,
pursue, and mutate never reason about rotation. The agent's lifecycle verbs
shrink to `lf pr land [-c]`, `lf pr abandon`, and `lf task complete`. Bare
land means "more work follows" — the loop continues through the merge without
further ceremony, and a PR merged manually from GitHub takes the same path.

Naming the next branch: the settling command takes an optional `--next <slug>`
(`lf pr land --next released-upgrade-proof`) so the agent names the coming
delivery at the moment it knows one is coming. Without it, the runner falls
back to the delivery sequence number — readable, boring, never an opaque ID.
There is no `lf task continue`: rotation is the runner's job, and a human
steering a Task does it through the same settle commands the agent uses.

Open question: whether `task_clarify` / `task_mutate` / `task_pursue` remain
three skills is unsettled. Rotation moving into the runner removes the main
pressure on the split; the delivery doctrine (bare `land` vs `-c`, the
complete/abandon guard) rides where PR publication lives today
(`task_mutate.md`).

`lf task complete` is the PR-independent completion primitive. Placement
creates delivery 1 as `Working`, so every Task has at least one delivery;
"no PR" is the observable claim, not "no deliveries."

- If the active delivery has no PR and no commits beyond base, `complete`
  marks it `Abandoned` in the same transaction and the Task completes. An
  investigation Task therefore finishes with zero PRs and no ceremony.
- An open PR or a dirty worktree rejects `complete`.
- Real unmerged work must be discarded explicitly: `lf pr abandon` settles the
  delivery as `Abandoned`, after which `complete` succeeds. Abandoned
  deliveries — including ones whose PR was closed unmerged — never block
  completion; they remain in the history as the record of the discard.

A changed repository thus cannot evade review silently, but a Task can end
`Completed` with only abandoned deliveries when that is the honest outcome.

Replace events tied to the singular PR with delivery-aware evidence:

```rust
DeliveryStarted { delivery_id, sequence, branch, base_commit }
PullRequestOpened { delivery_id, number, url }
DeliveryMerged { delivery_id, number, url, merge_commit }
Completed { summary }
```

Project and Wave observations should report the Task summary plus the ordered PR
list, not treat the first merge as Task completion.

## Readable naming

IDs remain registry identity only. They do not lead branch or filesystem names.

- `lf task run ISSUE --name <workspace-slug>` accepts the caller's semantic
  2-5-word kebab-case name.
- Without `--name`, derive the initial candidate from the Linear Task title.
- First branch: `<author>/<workspace-slug>`.
- Stable worktree: `<repo>.<workspace-slug>`.
- Continuation branch:
  `<author>/<workspace-slug>-<delivery-slug>`.
- Never use a slash below the first branch name: a surviving
  `author/workspace` ref conflicts with `author/workspace/next` in Git.

On a collision, report the conflicting worktree/branch and its owner. Do not
silently append an opaque ID. The orchestrating LLM retries with a clearer
semantic qualifier such as `session-recovery-locking`; a human can pass the same
`--name`. An issue identifier is an explicit last-resort qualifier, not the
default interface.

All new deliveries base from fetched `origin/main`, never the possibly-ahead
local `main`. This directly prevents unrelated local control-plane commits from
leaking into Task PRs.

## Main code seams

- `rust/loopflow/src/task/mod.rs`: types, invariants, delivery-aware events.
- `rust/loopflow/src/store/sqlite/child_sessions.rs`: normalized persistence and
  atomic delivery transitions.
- `rust/loopflow/src/ops/task.rs`: placement, reconciliation, complete.
- `rust/loopflow/src/task/runner.rs`: the iteration-boundary contract — stop on
  terminal Task, rotate on settled delivery, wait on submitted; observed merge
  is never terminal by itself.
- `rust/loopflow/src/engine/worktrees.rs`: readable placement and safe in-place
  branch rotation from `origin/main`.
- `rust/loopflow/src/ops/land.rs` and PR commands: ambient Task discovery and
  explicit `--complete` disposition.
- `rust/loopflow/src/lf/mod.rs`, CLI rendering, builtin `task_*` skills,
  `flowloop/README.md`, and `engine/builtins/LOOPFLOW.md`: replace the one-PR
  contract everywhere. Also remove any remaining stale `lf op` spellings found
  while touching builtin skills.

## Coordination and implementation slices

This overlaps W2-130, which currently edits `task/mod.rs`, `ops/task.rs`, process
supervision, child-session persistence, and the old baseline migration. Do not
implement both versions independently. First inspect W2-130, land or integrate
its recovery fixes, then rebase this branch onto current `origin/main` and express
all schema work as a new forward migration.

The change is larger than one reviewable PR. Keep one logical Task and land these
serial slices:

1. Delivery model, forward migration, store API, JSON shape, and migrated tests.
2. Readable Task placement plus delivery-aware PR attachment/reconciliation.
3. In-place continuation branch rotation, completion APIs, supervisor behavior,
   docs, and end-to-end tests.

Until slice 1 exists, bootstrapping may require the directly launched agent to
manage those branches through existing `lf` PR/worktree operations. Do not create
parallel implementations or parallel active PRs.

## Constraints

- One Linear Task, one Task Session, one stable worktree, one provider history.
- Zero or many historical deliveries; at most one active branch/PR.
- A merge proves a delivery shipped. It does not inherently prove the Task done.
- Bare `lf pr land` inside a Task lands one delivery and leaves the Task open.
- Only `lf pr land --complete` / `-c` means “complete after this merge.”
- No recursive Tasks, delivery sub-tasks, or PR-as-Task records in Linear.
- No local-main bases, hidden ID suffixes, duplicate singular PR fields, or
  compatibility code paths.
- Preserve exact retry/recovery behavior under process death and SQLite locking.

## Done when

- A one-PR Task uses `lf pr land -c` and closes only after observed merge.
- A two-PR Task keeps one worktree, rotates through two readable serial branches,
  retains both PRs in order, and closes after the second merge.
- A manually merged PR with no disposition leaves the Task open; the runner
  rotates to the next delivery, and only explicit completion ends the Task.
- A clean investigation Task completes with no PR (its empty placement delivery
  auto-abandons); dirty or unmerged work rejects `complete` until explicitly
  abandoned.
- An abandoned delivery (PR closed unmerged or `lf pr abandon`) leaves the Task
  open; the runner starts a fresh delivery from fetched main.
- Concurrent start/reconcile calls produce one Task Session and one active
  delivery.
- Collision tests prove the system asks for a semantic retry rather than exposing
  an opaque ID name.
- `cargo fmt`, `cargo clippy -- -D warnings`, the focused Task/store/CLI suites,
  full `cargo test -p loopflow`, and DTO fixture tests pass.
