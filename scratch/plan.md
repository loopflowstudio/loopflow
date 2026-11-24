# LOO-1094: explicit children on parent completion, reopen abandoned Tasks

## Outcome

Completing a parent never abandons work by surprise. `lf task complete LOO-12`
on a Task with open children refuses, writes nothing, and lists the open
descendant tree (indented so grandchildren that `--children abandon` would
settle are never hidden, and marking actively running descendants so the person
can see blockers before choosing):

```text
$ lf task complete LOO-12
Error: LOO-12 has 3 open children; say what happens to them:
  LOO-14    Log each retry  #1980 merged
    LOO-17  Retry metrics   unstarted
  LOO-15    Retry budget    #1981 open, running
  LOO-16    Backoff docs    unstarted
  lf task complete LOO-12 --children rehome    # direct children move up to LOO-10 and stay open
  lf task complete LOO-12 --children abandon   # merged descendants done, the rest abandoned
```

(`to LOO-10` names the nearest open ancestor along the visible parent chain;
with none — or once a deleted ancestor breaks the chain — "direct children
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

`--children abandon` refuses, writing nothing (and answering `conflict` over
the server), while any descendant it would abandon has live work (a running
Graph, headless run, or active turn — not an idle Task conversation parked at
an interactive step), and names each blocker and `lf task interrupt <child>`.
Desktop and iOS ask the same question in one dialog.

`lf task reopen LOO-15` (and Desktop's **Reopen Task**) brings an abandoned
Task cleanly back to life: its closed PR from the abandoned attempt becomes
history, any dead checkout record whose worktree and local branch were deleted
on abandon is cleared, `lf task status` recommends `run` (not "PR closed
without merge"), Desktop's **Run** / **Start** / **Auto-Ship** controls are
immediately enabled, `lf task run` places a fresh checkout from current main,
and `lf task complete` works if no new PR is needed.

Steer from Jack Heart, word for word: "Bare `lf task complete` should refuse
when open children exist unless an explicit `--children` option (`rehome` or
`abandon`) is passed. Also design the Desktop and iOS Complete Task
button/swipe UX cleanly for parents with open children so picking between
rehoming and abandoning is natural rather than awkward."

## Concepts and source of truth

Three distinct lifecycles must stay separate in storage and execution:

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
   to SQLite and answers `ErrorCode::Conflict` (`OpsError::Refused`). Once
   preconditions hold, completion and child settlement run in one immediate
   SQLite transaction, clearing any stale `completion_request` and recording
   `--reason` on `TaskEventKind::Completed { summary }`.
3. **Reopening an abandoned Task vs. an externally closed PR — two separate states.**
   - While a Task is **open** and someone closes its PR on GitHub without
     merging, that closed PR stays its current PR (`task_prs.historical = 0`,
     `abandoned_at IS NOT NULL`). `lf task status` and Desktop rightly warn
     `"PR closed without merge; reopen the same PR or abandon the Task"`.
   - When a Task itself was **abandoned** (closing its PR and deleting its
     checkout/branch) and someone runs **`lf task reopen`**, that closed PR and
     deleted checkout belong to the **previous, abandoned attempt**, not the
     reopened Task's active delivery:
     - `reopen_task` archives any abandoned current PR (`UPDATE task_prs SET historical=1 WHERE task_id=?1 AND historical=0 AND abandoned_at IS NOT NULL` — already permitted by the `task_pr_history_no_mark` trigger). Its row stays in `task_prs` history (`sequence = 1`), `active_task_pr` becomes `None`, and the next `lf pr submit` allocates `sequence = 2`. Reopening a **done** Task leaves its merged PR untouched (`historical = 0`).
     - `task_reopen` (holding `retirement::lock`) checks whether abandoning
       actually removed the Task's checkout and branch (`!worktree.exists()`
       and `!branch_exists(&repo, &task.branch)`). If both are gone, `reopen_task`
       deletes the stale `task_checkouts` row in the same transaction (after
       computing `has_prior_work`). If cleanup had failed and either the
       worktree or local branch still exists on disk, `task_checkouts` stays
       intact so no local commits are lost.
     - Because the stale `task_checkouts` row is gone when the checkout was
       deleted, `task_worktree_blocker_after` returns `None`, Desktop's **Run**
       / **Start** / **Auto-Ship** buttons enable immediately, and the next
       `lf task run` takes the normal `task.worktree.is_none()` placement path
       (creating a fresh checkout from current `origin/main` on the Task's
       branch).
     - `restore_task_checkout` and `finish_task_checkout` judge whether the
       **current checkout epoch** finished initializing (checking whether
       `CheckoutReady` or `PrStarted` occurred *after* the latest
       `CheckoutInitializing`), rather than scanning all historical events from
       `id > 0`. This lets a reopened Task initialize a fresh checkout cleanly
       while keeping the missing-branch refusal intact for any active checkout,
       and deletes the published-PR-only `let abandoned = pr...` workaround in
       `restore_task_checkout`.

## Design

### Storage (`rust/loopflow-storage/src/sqlite/children.rs`)

Built on `HEAD`:
- `ChildPolicy { Rehome, Abandon }`, `ChildOutcome::Rehomed`, `OpenDescendant`,
  `ParentCompletion`, `complete_task_now`, `open_descendants`,
  `rehome_destination`, `rehome_children`.
- `request_family_completion` and `family: bool` removed; pending completion
  always returns `WaitingForChildren` when children are open;
  `TaskEventKind::FamilyCompletionRequested` marked historical-only.

Remaining fixes for the next pass:
- **`reopen_task(&self, id: &TaskId, note: Option<&str>, clear_abandoned_checkout: bool) -> StoreResult<Task>`**
  (or keep `reopen_task(&self, id, note)` defaulting `clear_abandoned_checkout = false` and add `reopen_task_with_checkout_clear` / update callers — updating the 4 internal callers directly is cleaner and keeps one API):
  Inside the `IMMEDIATE` transaction, when `abandoned` is true:
  1. Compute `target = if has_prior_work(&tx, id)? { "started" } else { "unstarted" }` **before** modifying `task_checkouts`.
  2. Archive any abandoned active PR:
     ```sql
     UPDATE task_prs SET historical=1
     WHERE task_id=?1 AND historical=0 AND abandoned_at IS NOT NULL
     ```
  3. When `clear_abandoned_checkout` is true, delete the stale checkout row:
     ```sql
     DELETE FROM task_checkouts WHERE task_id=?1
     ```
     (Triggers `cancel_task_retirement_checkout_delete` and advances
     `store_revisions` automatically.)
  4. Clear `abandoned_at`, `abandon_reason`, `completion_request`,
     `completion_error`; set `planning_state` via `task_state::decide_in`;
     delete `task_auto`; record the reopened progress event.
  5. When reopening a **done** Task (`abandoned == false`), neither `task_prs`
     nor `task_checkouts` is touched.
- **`rehome_destination_in`**: stop walking upward if an ancestor row is
  soft-deleted (`planning_deleted_at IS NOT NULL`) and return `Ok(None)`. A
  deleted Task already breaks the visible tree in `task_hierarchy` and Desktop,
  so rehoming never teleports children under an invisible grandparent, and Rust
  matches Desktop's ancestor walk.

### Execution (`rust/loopflow-execution/src/ops/task.rs`, `task_run.rs`, `error.rs`, `engine/mod.rs`)

Built on `HEAD`:
- `task_complete(repo, issue, note, children)` with pre-flight checks,
  `open_children_refusal`, `live_abandon_blockers`, `settle_children`,
  `OpsError::Refused(Error::conflict(...))` wired into `Error::written`.

Remaining fixes for the next pass:
- **`task_reopen` (`ops/task.rs`)**:
  Under `retirement::lock(&task.id)`, determine `clear_abandoned_checkout`:
  true iff the Task is currently abandoned (`task_work_status == Abandoned`)
  AND `task.worktree` is `Some(path)` where `!path.exists()` AND either
  `task.branch.is_empty()` or `!crate::git::worktrees::branch_exists(&repo, &task.branch)?`.
  Pass `clear_abandoned_checkout` into `store.sqlite.reopen_task(...)`.
- **`restore_task_checkout` & `finish_task_checkout` (`ops/task.rs`)**:
  - Replace the full-history `.any(|e| CheckoutReady | PrStarted)` scan in both
    functions with a shared helper `current_checkout_ready(events: &[TaskEvent]) -> bool`
    that scans `events` in reverse and returns `true` if `CheckoutReady` or
    `PrStarted` appears after the latest `CheckoutInitializing` (and `false` if
    `CheckoutInitializing` appears first or none exists).
  - Delete the QA workaround (`let abandoned = pr.as_ref().is_some_and(|pr| pr.abandoned_at.is_some());`
    and its `if !abandoned` branches) in `restore_task_checkout`. When an
    abandoned Task's checkout & branch were deleted, `reopen_task` already
    cleared `task_checkouts`, so `prepare_task` runs the normal placement path
    and creates a fresh checkout from current `origin/main`. When a retained
    local branch or worktree survived abandonment, `restore_task_checkout`
    restores it cleanly.
- **`complete_task_now` error mapping in `task_complete` (`ops/task.rs:190`)**:
  Map `StoreError::InvalidData(msg) | StoreError::InvalidAuthority(msg)` from
  `complete_task_now` to `refused(msg)` (`OpsError::Refused(Error::conflict(msg))`),
  so raced refusals (e.g., a child filed between pre-check and transaction)
  still answer `conflict`, never `outcome_unknown`.
- **`open_children_refusal` (`ops/task.rs:221-241`)**:
  Include `"running"` in `standing` whenever `descendant.state == TaskState::Moving`
  (e.g. `#1981 open, running`, or `running` when there is no PR), so running
  children that will block `--children abandon` are visible in the bare refusal.
- **Abandoned refusal copy**:
  - `rust/loopflow-execution/src/ops/task_run.rs:23`: change
    `"Task is abandoned; recover it before running a Graph"` to
    `"Task is abandoned; reopen it before running a Graph"`.
  - `rust/loopflow-execution/src/ops/task.rs:1356` (`validate_preparation_status`):
    change `"Task {} is abandoned; inspect its retained history with `lf task status {}`"` to
    `"Task {} is abandoned; `lf task reopen {}` reopens it without moving its Graph"`
    (matching `WorkStatus::Done`).

### CLI, wire, Desktop, iOS, and docs

Built on `HEAD`:
- CLI `--children <rehome|abandon>`, `after_help`, `TaskCommand::Reopen` help,
  rehome text output (`moved up to LOO-10` / `moved to top level`).
- Wire `Write::TaskComplete { directory, issue, children }` and
  `ViewContent.open_children: Option<u32>`, Swift `TaskChildPolicy`,
  `ServedWrite.taskComplete`, `WorkViewPlan.Content`, DTO fixtures.
- Desktop `TaskCompleteConfirmation` (safe-first button ordering for both
  all-merged and unmerged cases), `WorkModel.rehomeDestination`, `TaskActionItems`.
- iOS `TaskFeed.complete` + `confirmationDialog` before writing.
- Docs: `docs/work.md`, `docs/surfaces.md`, `swift/LoopflowiOS/README.md`.
  Note: update the one sentence in `docs/work.md` (`"starts again from the Task's base"`)
  to `"starts a fresh checkout from main"` if `prepare_task` places fresh from
  `origin/main` when the abandoned checkout was deleted.

## What goes

Already removed on `HEAD`:
- `request_family_completion`, `family: bool`, writers of `FamilyCompletionRequested`.
- Silent-abandon default of bare `lf task complete` / `task.complete` with `children: null`.
- `reopen_task`'s abandoned refusal and Desktop's "cannot be reopened" / "Settle Children" copy.

Removed in this pass:
- The `let abandoned = pr.as_ref().is_some_and(...)` workaround in
  `restore_task_checkout` (`ops/task.rs:1416-1449`) — replaced by archiving the
  abandoned PR and clearing the deleted `task_checkouts` row in `reopen_task`
  plus epoch-scoped `current_checkout_ready`.

Survives: Graph-end `CompletionRequested` waiting for children (and leaf
`- cmd: task complete` pending on unmerged PR); recursive abandon semantics;
`--json` `children` shape; `"PR closed without merge; reopen the same PR or abandon the Task"`
for an **open** Task whose PR was closed externally on GitHub; preserved
checkouts/branches when `cleanup_abandoned` failed before reopen.

## Acceptance check (gate)

All existing checks on `HEAD` pass, plus the reopened-abandoned lifecycle checks:

1. **Bare refusal + running indicator:** `task_complete(P, None)` refuses with
   `ErrorCode::Conflict`, writes nothing, lists indented descendants, and marks
   a child with an open PR AND a running Graph as `#101 open, running`.
2. **Live-work guard:** `task_complete(P, Some(Abandon))` refuses (`conflict`),
   writes nothing when a descendant has a live Graph or active turn, and
   succeeds when a descendant has only an idle held conversation at `ship-review`.
3. **Rehome across closed & deleted ancestors:** `task_complete(P, Some(Rehome))`
   skips closed ancestors to reach an open grandparent, stops at a deleted
   ancestor (landing top-level, matching Desktop), keeps closed children linked,
   and queues tracker parent updates.
4. **Raced `complete_task_now` refusal:** returns `OpsError::Refused(Error::conflict(...))`,
   never `OutcomeUnknown`.
5. **Reopen abandoned Task end-to-end (published PR + deleted checkout/branch):**
   - Abandon `LOO-15` (PR `#7` closed with `abandoned_at`, worktree & branch deleted).
   - `lf task reopen LOO-15`:
     - PR `#7` is marked `historical = 1` (still in `task_prs`, `active_task_pr` is `None`).
     - Stale `task_checkouts` row is deleted (`task.worktree` is `None`).
     - `lf task status LOO-15` and Desktop `RoadmapTask`: `actions.recommended == Some(Run)`
       (NOT `"PR closed without merge"`), `run_control.unavailable == None`
       (Desktop Run/Start/Auto-Ship enabled, NO missing-worktree blocker).
     - `lf task complete LOO-15` succeeds (no dead-PR refusal) — test separately.
     - `lf task run LOO-15` (or `lf task checkout LOO-15`) creates a fresh
       checkout from current `origin/main` and writes a new `CheckoutReady`
       event (no lingering `CheckoutInitializing` blocker).
     - Subsequent `lf pr submit` creates `sequence = 2` PR cleanly.
6. **Reopen abandoned Task with retained worktree or local branch:**
   - When `cleanup_abandoned` failed and the worktree (or local branch) still
     exists on disk, `task_reopen` archives any abandoned PR (`historical = 1`)
     but **keeps** `task_checkouts` intact so local commits are preserved.
7. **Open Task with externally closed PR (regression check):**
   - Without `task_reopen`, an open Task whose PR has `abandoned_at != None`
     still reports `"PR closed without merge; reopen the same PR or abandon the Task"`.
8. **Swift & DTO checks:** `TaskCompleteConfirmationTests`, `TaskFeedTests`,
   `ServerProtocolTests`, `WorkSelectionTests`, and full Rust suites in CI.

## Demo

`lf task create` a parent with two children (one with `lf task run` detached),
then `lf task complete <parent>`: refusal lists both (marking the running one
`running`); `--children abandon` names the running one and `lf task interrupt`
without writing; `--children rehome` finishes the parent and `lf wave show`
lists both children top-level. Abandon a started child, `lf task reopen` it,
confirm `lf task status` recommends `run` (not "PR closed without merge") and
`lf task run` starts fresh from main. In Desktop, right-click the parent →
`Complete Task…` → the two-choice dialog, and reopen an abandoned Task → **Run**
is immediately clickable.

## Status

Built on `HEAD` (`4f0a3df928`):
- Storage/execution/CLI/Desktop/iOS/docs for explicit parent completion
  (`ChildPolicy`, `complete_task_now`, `rehome_children`, `open_descendants`,
  `live_abandon_blockers`, `OpsError::Refused`, `ViewContent.open_children`,
  Desktop `TaskCompleteConfirmation`, iOS swipe confirmation dialog, initial
  `reopen_task` unblocking).
- Verified in QA (`scratch/qa.html`) on a private Machine + iPhone SE Simulator.

Remaining (concept-review pass → implement → gate):
1. Fix reopened-abandoned Task lifecycle cleanly: archive abandoned PR
   (`historical = 1`), clear dead `task_checkouts` when worktree & branch are
   gone on disk, scope `restore_task_checkout`/`finish_task_checkout` to the
   current checkout epoch (`CheckoutInitializing`), delete QA's `restore_task_checkout`
   workaround, update abandoned refusal copy (`task_run.rs`, `validate_preparation_status`)
   and `docs/work.md` reopen wording.
2. Map `complete_task_now` refusal errors (`InvalidData`/`InvalidAuthority`) to
   `OpsError::Refused(Error::conflict(...))`.
3. Show `running` alongside PR in `open_children_refusal` when `state == Moving`.
4. Stop `rehome_destination_in` at deleted ancestors (`planning_deleted_at IS NOT NULL`).
5. Add tests covering 1–7 above (including Desktop `RoadmapTask` / `task_run_control`
   and `task_actions` after reopening an abandoned Task) and run gate.

## Passes

1. **Storage + execution + CLI + docs** (built).
2. **Desktop** (built).
3. **iOS** (built).
4. **Concept-review alignment pass (next `implement` pass):** reopened-abandoned
   PR archival + dead checkout clearing + checkout-epoch check, `complete_task_now`
   conflict mapping, `running` standing in refusal tree, deleted-ancestor stop
   in `rehome_destination_in`, tests, and doc line update.
5. **Gate**: Swift `TaskCompleteConfirmationTests`, `TaskFeedTests`,
   `ServerProtocolTests` and the full Rust suites in CI; acceptance check; demo.

## Exclusions

- No "keep children linked under the done parent" option on completion.
- Rehome moves direct open children only, never into another Wave or Project.
- No change to Graph-end (`CompletionRequested`) waiting-for-children behavior.
- No change to `lf task abandon` on parents (leaves open children linked and
  open; they surface top-level via `task_hierarchy`).
- No iOS reopen / abandoned list UI (not present on iOS today).
- Reopen does not restore closed PRs or deleted branches; the next run opens a
  new PR, as reopen of a done Task does today.
- Phone `NoticeView` single-line truncation and `lf task interrupt` on `cmd:`
  steps are separate pre-existing issues outside this Task.
