# LOO-1094: explicit children on parent completion, reopen abandoned Tasks

## Outcome

Completing a parent never abandons work by surprise. `lf task complete LOO-12`
on a Task with open children refuses, writes nothing, and lists the open
descendant tree (indented so grandchildren that `--children abandon` would
settle are never hidden):

```text
$ lf task complete LOO-12
Error: LOO-12 has 3 open children; say what happens to them:
  LOO-14  Log each retry        #1980 merged
    LOO-17  Retry metrics       unstarted
  LOO-15  Retry budget          #1981 open, at ship-review
  LOO-16  Backoff docs          unstarted
  lf task complete LOO-12 --children rehome    # direct children move up to LOO-10 and stay open
  lf task complete LOO-12 --children abandon   # merged descendants done, the rest abandoned
```

(`to LOO-10` names the nearest open ancestor; with none, "direct children
become top-level Tasks".)

```text
$ lf task complete LOO-12 --children rehome
LOO-12 done
  LOO-14 moved up to LOO-10
  LOO-15 moved up to LOO-10
  LOO-16 moved up to LOO-10
$ lf task complete LOO-12 --children abandon
LOO-12 done
  LOO-14 done: #1980 merged
    LOO-17 abandoned: no merged PR
  LOO-15 abandoned: #1981 open, at ship-review
  LOO-16 abandoned: no merged PR
```

`--children abandon` refuses, writing nothing, while any descendant it would
abandon has live work (a running Graph, headless run, or active turn — not an
idle Task conversation parked at an interactive step), and names each blocker.
`lf task reopen LOO-15` works on an abandoned Task. Desktop and iOS ask the
same question in one dialog.

Steer from Jack Heart, word for word: "Bare `lf task complete` should refuse
when open children exist unless an explicit `--children` option (`rehome` or
`abandon`) is passed. Also design the Desktop and iOS Complete Task
button/swipe UX cleanly for parents with open children so picking between
rehoming and abandoning is natural rather than awkward."

## Concepts and source of truth

Two distinct actions were previously fused into one `completion_request`
mechanism:

1. **Pending completion (`tasks.completion_request` + `TaskEventKind::CompletionRequested`) — leaf / Graph-end only.**
   A Graph reaching `end` (or a bare `lf task complete` on a leaf Task whose PR
   is not yet merged, e.g. a Graph step `- cmd: task complete <ID>`) writes
   `CompletionRequested` so background reconciliation completes the leaf once
   its PR merges. If open children exist when a pending completion settles, it
   clears `completion_request` and returns `WaitingForChildren`. A pending
   completion **never** carries a child policy and **never** abandons or
   rehomes children in the background.
2. **Explicit completion (`lf task complete <ID> [--children rehome|abandon]`) — immediate decision.**
   Never arms a deferred child policy. Preconditions (PR merged or absent,
   explicit `--children` when open children exist, no live execution blocking
   `--children abandon`) are checked before writing; a refusal writes nothing
   to SQLite. Once preconditions hold, completion and child settlement run in
   one immediate SQLite transaction, clearing any stale `completion_request`
   and recording `--reason` on `TaskEventKind::Completed { summary }`.

Why this beats storing `ChildPolicy` on `completion_request`:
- A refused `--children abandon` (live child or unmerged parent PR) cannot
  leave a pending abandon request armed in SQLite that background
  reconciliation later fires silently once the child stops or the PR merges.
- No new `TaskEventKind` variant is needed — zero version skew with released
  `lf`. `TaskEventKind::FamilyCompletionRequested` becomes a read-only
  historical variant with no writers (same as `FollowUp`).
- `ChildPolicy { Rehome, Abandon }` is one `clap::ValueEnum` + serde +
  `JsonSchema` enum in `loopflow-storage` (`sqlite/children.rs`, beside
  `ChildOutcome`), shared by Clap, `Write::TaskComplete`, and storage.

## Design

### Storage (`rust/loopflow-storage/src/sqlite/children.rs`)

- Add `ChildPolicy { Rehome, Abandon }` (`#[serde(rename_all = "snake_case")]`,
  `#[derive(clap::ValueEnum, schemars::JsonSchema, ...)]`) and add
  `ChildOutcome::Rehomed` to `ChildOutcome`.
- Add `CompletedTask` / update `ChildCompletion` so rehomed children carry their
  updated `Task` (with the new `parent_task_id`) and `rehome_destination`
  identifier is available for text output.
- Helper `rehome_destination(conn, parent: &TaskId) -> StoreResult<Option<Task>>`:
  walks `parent_task_id` upward from `parent`'s own parent and returns the
  nearest open (non-deleted, non-terminal) ancestor, or `None` for top-level.
  Never rehomes open children under a closed, abandoned, or deleted grandparent.
  Shared by refusal text, `rehome_children`, and completion output.
- Helper `open_descendant_tree(conn, parent: &TaskId) -> StoreResult<Vec<OpenDescendant>>`:
  walks open descendants recursively in filing order, returning each
  descendant's `Task`, active `TaskPr`, parked `task_graphs.step`, and its own
  open children. Used by both bare-complete refusal formatting and the
  `--children abandon` live guard.
- Add immediate completion entry point `complete_task_now(&self, task: &Task, note: Option<&str>, children: Option<ChildPolicy>) -> StoreResult<ImmediateCompletion>`:
  inside one `TransactionBehavior::Immediate` transaction:
  - Validate task & project.
  - `Done` → no-op (`vec![]`).
  - `Abandoned` → `Err(InvalidData("Task ... is abandoned and cannot be completed"))`.
  - Active PR not merged → `Err(InvalidAuthority("Task requires a verified merge"))`.
  - `children.is_none()` and `open_task_child_count(&tx, &task.id)? > 0` →
    return a refusal variant/error without committing (closes the race where a
    child was filed after pre-check).
  - `record_task_completion(&tx, &task.id, note)` clears `completion_request`
    and `completion_error`, sets `planning_state = 'completed'`, records
    `TaskEventKind::Completed { summary }` (carrying `note` when present), and
    writes `task_summaries` (`Done`).
  - Match `children`:
    - `None` (0 open children) → `vec![]`.
    - `Some(ChildPolicy::Abandon)` → `complete_children(&tx, &task.id)` as today
      (recursive: merged → `Done`, unmerged → `Abandoned`).
    - `Some(ChildPolicy::Rehome)` → `rehome_children(&tx, &task.id)`: resolves
      `dest = rehome_destination(&tx, &task.id)?`, updates each direct open
      child with `Edit::TaskParent(dest.as_ref().map(|t| t.id.as_str().into()))`
      via `planning_write::local` (queueing tracker parent actions in the same
      transaction). Closed children keep their link as history. Returns
      `ChildOutcome::Rehomed` with `children: []` (their own subtrees come
      along untouched).
- Simplify pending completion (`request_completion_in`, `complete_task(&self, task, request)`):
  - Delete `request_family_completion` and `family: bool`.
  - `completion_request` checks only `t.completion_request` and
    `t.planning_state IS 'completed'`.
  - `complete_task(&self, task, request)` is Graph-end / leaf-pending only:
    whenever `open_task_child_count(&tx, &task.id)? > 0`, it clears
    `completion_request` and returns `CompletionSettlement::WaitingForChildren`.
  - Mark `TaskEventKind::FamilyCompletionRequested` as historical (no current
    writers) in `rust/loopflow-storage/src/work/task/mod.rs`.
- `reopen_task` (already updated on this branch): clears `abandoned_at`,
  `abandon_reason`, `completion_request`, `completion_error`; transitions
  `canceled` → `started`/`unstarted`; clears `task_auto`. Verified:
  `cancel_task_retirement` trigger revokes any pending abandonment retirement
  in the same write, and `task_summaries` is historical ("when it last ended",
  not read by `TaskState` or the board, unchanged on done-reopen too).

### Execution (`rust/loopflow-execution/src/ops/task.rs`)

- `task_complete(repo, issue, note, children: Option<ChildPolicy>) -> OpsResult<TaskCompletion>`:
  1. Resolve task; if already `Done`, return `TaskCompletion` with empty
     children. If `Abandoned`, return `"An abandoned Task cannot be completed"`
     writing nothing.
  2. Reconcile cached PR observation (`reconcile_task_pr_observation`).
  3. Check active PR refusal (`task_completion_refusal`):
     - If unmerged PR AND `children.is_none()` AND `open_task_child_count == 0`
       (leaf Task): preserve existing pending-request path
       (`request_task_completion` + `settle_completion`) so Graph steps like
       `- cmd: task complete <ID>` remain retryable without replaying.
     - If unmerged PR AND (`children.is_some()` OR `open_task_child_count > 0`):
       return the unmerged-PR error immediately, **writing nothing** (no armed
       child policy, and no two-step goose chase asking for `--children` before
       saying the PR isn't merged).
  4. If `children.is_none()` and `open_task_child_count > 0`: format the
     open-descendant tree refusal (indented per level, naming each descendant's
     identifier, title, and standing from its PR/step/state — formatted in
     `loopflow-execution` without depending on `loopflow::TaskRow` — plus both
     exact `--children rehome` / `--children abandon` commands naming
     `rehome_destination`) and return `Err`, **writing nothing**.
  5. If `children == Some(ChildPolicy::Abandon)`: run the live-child guard
     before touching SQLite. Walk the full open descendant tree (including
     descendants of merged children, since `complete_children` recurses through
     both done and abandoned children). For every descendant whose active PR is
     not merged, check `live_abandon_blockers(&store.sqlite, &descendant.id)`:
     - Start from `crate::ops::task_execution::unresolved_work(&store.sqlite, &work)`
       (filters out caller's own work and dead OS processes).
     - Exclude an idle held interactive conversation (`OpenWork::Session { held_terminal: Some(t), .. }`
       when `store.sqlite.terminal_retention(t)?.is_none()`), because
       `retirement::close_conversations` cleanly closes idle held conversations
       on abandonment; a running Graph, headless run, or active turn in a held
       terminal (`terminal_retention` is `Some(_)`) **is** a blocker.
     - Any blockers across any unmerged descendant → return error naming each
       blocked descendant and blocker (`"LOO-15 is running: <blocker>; stop it or use --children rehome"`), **writing nothing**.
  6. Call `store.complete_task_now(&task, note, children)` (handles any late
     child-creation race cleanly inside the `IMMEDIATE` transaction).
  7. Run post-commit follow-ups:
     - `ChildOutcome::Rehomed` → `sync_completion` (tracker parent update) only.
     - `ChildOutcome::Done` → `finish_completion`.
     - `ChildOutcome::Abandoned` → `retirement::close_conversations`,
       `sync_completion`, `lifecycle::cleanup_abandoned`.
     - Parent → `finish_completion`.
- `task_reopen`: unchanged apart from dropping any "abandoned" refusal text.

### CLI and wire (`rust/loopflow/src/lf/`)

- `TaskCommand::Complete { issue, children: Option<ChildPolicy>, reason, json }`:
  `--children <rehome|abandon>`, `after_help` explaining refusal when children
  are open, what `rehome` and `abandon` do, and the live-work guard.
- `TaskCommand::Reopen`: help updated to state it works on done and abandoned
  Tasks.
- `write.rs`: `Write::TaskComplete { directory, issue, #[serde(deserialize_with = "Option::deserialize")] children: Option<ChildPolicy> }`;
  update `rust/loopflow/src/server/schema.rs:495`,
  `tests/fixtures/dto/server_schema.json`, and any other DTO fixture carrying
  `task.complete`.
- `mutation.rs::write_child_completions`: print `moved up to LOO-10` /
  `moved to top level` for `ChildOutcome::Rehomed` (using the rehome
  destination resolved on `TaskCompletion`, not a stale pre-edit `parent_task_id`).

### Desktop (`swift/LoopflowMac/`)

- `ServedWrite.taskComplete(directory:issue:children:)` and `TaskChildPolicy`
  enum (`rehome`, `abandon`) in `swift/Loopflow/Services/RegistryQuery.swift`.
- Menu item (`TaskWorkView.swift`): reads `Complete Task…` when children are
  open (was `Complete Task & Settle Children…`), with updated help text.
- `TaskCompleteConfirmation` (`TaskWorkView.swift`):
  - Title: `Complete LOO-12? 3 children are open` (or `1 child is open`),
    counting direct open children like the CLI refusal; the message names
    which descendants Finish marks done.
  - Resolve rehome destination by walking `parentId` up to the nearest open
    ancestor in `model` (`Complete & Move Children to LOO-10`); if none is open
    (or `parentId == nil`), `Complete & Move Children to Top Level`. Matches
    Rust's `rehome_destination`.
  - Buttons:
    - When at least one open descendant would be abandoned:
      1. `Complete & Move Children to LOO-10` (or `… to Top Level`), default,
         non-destructive → `children: .rehome`.
      2. `Complete & Abandon Children`, `.destructive` → `children: .abandon`.
      3. Cancel.
    - When every open descendant already has a merged PR (`abandoned.isEmpty`):
      1. `Complete & Finish Children`, default, non-destructive →
         `children: .abandon` (marks all merged descendants done, abandons 0).
      2. `Complete & Move Children to LOO-10` (or `… to Top Level`),
         non-destructive → `children: .rehome`.
      3. Cancel.
  - Message: explains both choices briefly — where direct children move under
    Move, and which descendants are marked done vs. abandoned under Abandon
    (minus "cannot be reopened").
- `WorkModel.completeTask(_:children:)` and `completeTaskFromInput(_:children:)`
  pass the policy (`nil` for direct leaf completion).
- Reopen already appears for abandoned Tasks (`isCompleted` = `lifecycle.isTerminal`)
  and now succeeds.

### iOS (`swift/LoopflowiOS/TaskListView.swift` & `ViewContent`)

- Add `#[serde(deserialize_with = "Option::deserialize")] pub open_children: Option<u32>`
  to `ViewContent` (`rust/loopflow-storage/src/work/selection.rs`), matching the
  existing Task-only optional fields (`identifier`, `primary_session`,
  `location`): `None` for Waves/Repositories/unavailable items, `Some(count)`
  for Tasks.
- Populate in `rust/loopflow-storage/src/sqlite/selections.rs` using
  `super::children::open_task_child_count(conn, &task_id)?` (indexed by
  `tasks_parent`). Update unit test literal in
  `rust/loopflow/src/lf/commands/open/workspace.rs`, Swift
  `WorkViewPlan.Content` (`openChildren: Int?`, explicit `decode(Int?.self, forKey: .openChildren)`),
  and DTO fixtures (`tests/fixtures/dto/view_plan.json`,
  `subscribe_frames.json`, `server_schema.json`).
- `TaskListView.swift`:
  - Swipe on a leaf (`(task.openChildren ?? 0) == 0`): completes immediately
    with `children: nil` as today.
  - Swipe on a parent (`(task.openChildren ?? 0) > 0`): the row stays; a
    `confirmationDialog` asks `Complete LOO-12? 3 children are open` with
    message `"Move children up to keep them open, or abandon open children (merged ones are marked done)."`
    and buttons `Move Children Up` (`.rehome`), `Abandon Children` (`.destructive`,
    `.abandon`), and Cancel.
  - A refusal (live child, unmerged PR, raced new child) returns via `Notice`.

### Docs

Updated in place for review:
- `docs/work.md` (`## File follow-ups from a Task`, `## Finish`, `## Reference`)
- `docs/surfaces.md` (`### Changes show at once`, `### The backlog`)
- `swift/LoopflowiOS/README.md` (`## Tasks`)

## What goes

- `request_family_completion`, `family: bool` in `completion_request` /
  `request_completion`, and all writers of `TaskEventKind::FamilyCompletionRequested`.
- The silent-abandon default of bare `lf task complete` and `task.complete`
  with `children: null`.
- `reopen_task`'s abandoned refusal (already removed in this checkout).
- Desktop strings `Complete Task & Settle Children…`, `Complete & Settle
  Children`, and "cannot be reopened".
- Tests asserting abandoned Tasks refuse reopen, or that bare complete settles
  children: rewritten to the new behavior, not kept.

Survives: Graph-end `CompletionRequested` waiting for children (and leaf
`- cmd: task complete` pending on unmerged PR); recursive abandon semantics;
`--json` `children` shape (plus `rehomed` outcome).

## Acceptance check (gate)

Storage/execution fixture, headless: open grandparent G → parent P → children A
(merged PR, with unmerged grandchild A1), B (open PR, idle held conversation at
`ship-review`, plus separate test with live Graph / active turn), C (unstarted).

1. `task_complete(P, None)` → error listing A, A1 (indented), B, C and
   suggesting `--children rehome` (naming G) / `--children abandon`; P open;
   no `completion_request` or event written.
2. `task_complete(P, Some(Abandon))` while B has a live Graph (or active turn) →
   error naming B's blocker; P, A, A1, B, C unchanged and **no
   `completion_request` armed**.
3. `task_complete(P, Some(Abandon))` while B has only an idle held conversation
   at `ship-review` → succeeds: P done, A done, A1/B/C abandoned (idle
   conversation closed, not treated as a blocker).
4. `task_complete(P, Some(Rehome))` → P done; A, B, C open with
   `parent_task_id = G` (and with closed/deleted G → `parent_task_id = None`);
   A1 still under A; tracker parent action queued for each direct child.
5. `task_complete(P, Some(Abandon))` or `Some(Rehome)` while P's own PR is
   unmerged → error, P open, **no `completion_request` armed**.
6. `task_reopen` on an abandoned Task → `started`, `abandoned_at` NULL, auto
   off, pending retirement cleared.

Plus: `lf task complete --help` shows `--children`; DTO fixture tests (Rust and
Swift) pass for `task.complete` and `ViewContent`; Desktop view test covers
both dialog button sets (unmerged present vs. all merged).

## Demo

`lf task create` a parent with two children (one with `lf task run` detached),
then `lf task complete <parent>`: refusal lists both; `--children abandon`
names the running one without leaving a pending completion; `--children rehome`
finishes the parent and `lf wave show` lists both children top-level. In
Desktop, right-click the parent → `Complete Task…` → the two-choice dialog.

## Status

Built (pass 1 and the Desktop half of pass 2):
- Storage: `ChildPolicy`, `ChildOutcome::Rehomed`, `OpenDescendant`,
  `ParentCompletion`, `complete_task_now`, `rehome_destination`,
  `open_descendants`; `request_family_completion` and the `family` flag gone;
  pending completion always waits for children; `CompletionSettlement::Settled`
  carries nothing.
- Execution: `task_complete(repo, issue, note, children)`, open-tree refusal
  (counts direct children, lists the whole tree), `live_abandon_blockers`,
  `settle_children`. `--reason` lands on `Completed { summary }`.
- CLI `--children`, `task.complete.children` on the wire, schema fixture,
  rehome text output.
- Desktop: `TaskChildPolicy`, `ServedWrite.taskComplete(children:)`, dialog
  with both button layouts, `WorkModel.rehomeDestination`.
- Tests: `session_cli_tests::a_merged_parent_stays_open_until_it_is_completed_with_its_children`
  (refusal writes nothing, abandon, reopen abandoned, rehome past a closed
  ancestor, top-level rehome, unmerged parent arms nothing);
  `ops::task::tests::abandoning_children_waits_for_live_work_but_not_an_idle_conversation`;
  Swift `TaskCompleteConfirmationTests`, `ServerProtocolTests.taskCompletionNamesWhatHappensToOpenChildren`.

Learned: tracker parent actions for rehome come from `planning_write::local`,
the same path as `lf task update --parent`; no separate queueing needed. A
Task with no tracker object queues none, so the CLI fixture cannot see one.

Built (pass 3, iOS):
- `ViewContent.open_children` (direct open children, `None` for non-Tasks),
  filled in `selections.rs`; Swift `WorkViewPlan.Content.openChildren` with
  explicit `decode(T?.self)` and `encode` of every nullable key (synthesized
  encoding dropped nil keys, which broke the round-trip test once a decode
  required them). DTO fixtures and `server_schema.json` regenerated.
- `TaskFeed.complete`: a swipe on a parent sets `completing` and writes
  nothing; the list's `confirmationDialog` sends `.rehome` / `.abandon`.
  `TaskChildPolicy.question` is the one title for Desktop and the phone.
- Tests: `cli_discovery::desktop_query_plan_reads_backlog_without_execution_or_desktop`
  (`open_children == Some(1)` with one open and one done child),
  `TaskFeedTests.testASwipedParentStaysUntilAskedWhatHappensToItsChildren`,
  `testASwipedLeafSendsNoChildren`; macOS Swift suites for the DTO pass.

Learned: one Rust-owned count is enough for the phone; its dialog names no
destination, so carrying the rehome destination on `ViewContent` would only
serve Desktop, which reads `RoadmapTask`, not view plans. Left as an open
question in `scratch/questions.md`.

Remaining: gate. Desktop's `rehomeDestination` still cannot see deleted
ancestors (says "Top Level" where Rust continues to an open ancestor above a
deleted one).

## Passes

1. **Storage + execution + CLI + docs** (validates the core model first:
   immediate `complete_task_now` without `completion_request`, `rehome_children`
   through `planning_write::local` queueing tracker parent actions in-tx, and
   `live_abandon_blockers` distinguishing active Graphs/turns from idle held
   conversations). Policy enum, `rehome_destination`, descendant refusal, live
   guard, rehome, reopen tests, CLI flag, wire `children`, fixtures, tests 1–6,
   docs.
2. **Desktop** (built): `ServedWrite` field, Desktop dialog (both all-merged and
   unmerged layouts).
3. **iOS**: `ViewContent.open_children` in `selections.rs` + Swift +
   fixtures, iOS confirmation dialog, view tests.

## Exclusions

- No "keep children linked under the done parent" option on completion.
- Rehome moves direct open children only, never into another Wave or Project.
- No change to Graph-end (`CompletionRequested`) waiting-for-children behavior.
- No change to `lf task abandon` on parents (leaves open children linked and
  open; they surface top-level via `task_hierarchy`).
- No iOS reopen / abandoned list UI (not present on iOS today).
- Reopen does not restore closed PRs or deleted branches; the next run opens a
  new PR, as reopen of a done Task does today.
