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
     - Checkout restoration and finalization both use `task_checkout_ready`.
       The latest `CheckoutInitializing`, `CheckoutReady` or historical
       `PrStarted` decides readiness. The lookup reads the indexed event log
       backwards inside SQLite.
     - Placement that discovers an open PR and publication both use the next
       sequence after the Task's stored PR history.

## Design

- Storage keeps immediate completion and rehoming in one transaction. The
  ancestor walk follows visible parent links, skips completed or abandoned
  ancestors, and ends at a deleted ancestor.
- Reopening and abandonment cleanup share the retirement lock. Reopen
  checks the checkout path and local branch on disk, then commits the
  planning change, PR archival, and checkout removal together.
- Bare parent completion reads the tree before observing the parent's PR.
  A moving child shows `running` alongside its PR. Both preflight and
  transactional refusals report `conflict`.
- CLI, served writes, Desktop, and iOS share the child policy. Desktop names
  the rehome destination and orders the safe button first. iOS uses
  `open_children` to ask before writing.

## Acceptance check (gate)

Gate runs the existing suites alongside these lifecycle checks.

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
`Complete Task…` → a Loopflow sheet listing the open descendant tree with a
radio choice (Move children up vs. Abandon/Finish children), neutral Cancel,
and a green `.success` confirm button (switching to quiet `.destructive`
outline when Abandon is chosen), and reopen an abandoned Task → **Run**
is immediately clickable.

## Placement discovery

A private CLI run reopened a published Task successfully, then checkout failed
with `UNIQUE constraint failed: task_prs.task_id, task_prs.sequence`. The QA
GitHub stand-in still reported an open PR on the branch. Placement can also
discover a newly opened PR on a restored branch. Placement now allocates a discovered PR after
the last stored sequence, using the same indexed lookup as publication.
The remote-branch fixture picked up PR 8 as sequence 2 after PR 7 closed.

## Status

The storage, execution, CLI, Desktop, iOS and documentation changes are built.
The earlier private Machine and iPhone Simulator run is in `scratch/qa.html`.
Gate owns the full Rust and Swift suites and the final app check.

### Verification in this checkout

- Private CLI with local bare origin and GitHub stand-in: Task `08a5f7`
  reported `lf task run 08a5f7` after reopening, checked out the advanced
  `origin/main`, then published PR 101. The stored sequences were
  `(1, 100, historical)` and `(2, 101, current)`. The fixture marked PR
  100 closed after verifying its remote head had been deleted.
- `task_initialization_tests::reopen` passed all three cases, including
  Desktop's Run control, retained work, and placement of a new remote PR.
- The focused parent CLI, live-work, transaction-conflict and documented
  command tests passed.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.
- SQLite used the current-PR index, checkout primary key, Task/event index
  and two Task primary-key lookups on the Machine's 1,622 Tasks and
  75,899 events. With 10,000 synthetic Tasks and 110,000 events, warm median
  statement times were 3.7 µs for checkout readiness, 1.3 µs per ancestor,
  0.8 µs for the next sequence, 1.4 µs for PR archival and 1.5 µs for
  checkout deletion (in-memory, excluding triggers and transaction commit).

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
