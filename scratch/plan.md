# LOO-966: a stacked Task's parent is one record

## What happened to LOO-959 (read from `~/.lf/loopflow.db` and git, 2026-11-18)

The brief's premise, "stacked on LOO-953 from creation", does not hold. The
record never said LOO-953; nothing rewrote it.

- LOO-959 was created 2026-11-17 18:03 UTC with no `--stack-on`
  (`processes`: only `task run LOO-959 -a claude`).
- The stack came from a steer at 22:18 UTC, quoting Jack: "you can stack onto
  953 i guess", then "Stay stacked on 953: when they land on its branch, sync
  onto it".
- The implement agent stacked by hand: commit `b29fe6222a` "Merge LOO-953
  base: …" (a plain `git merge`, no `lf` process recorded), later `gh pr edit
  --base`, and today `lf sync jack/loo-953-…`. It never ran
  `lf task checkout LOO-959 --stack-on LOO-953`, which already exists
  (`stack_existing_task`, `rust/loopflow-execution/src/ops/task.rs:1254`).
- So `task_checkouts.parent_pr_id` stayed NULL. `lf sync` and `lf pr publish`
  both followed that record correctly: sync merged main, publish passed
  `--base main` on create and on every update (`update_pr`,
  `ops/pr.rs:2010`, always sends `--base`).
- The merges of main (`8abce99`) did not touch the record.

Fixed in kickoff: ran `lf task checkout LOO-959 --stack-on LOO-953` from
LOO-959's worktree. LOO-953's branch matched origin, so nothing was pushed.
`task_checkouts.parent_pr_id` now reads `pr_f3052042…` (LOO-953's PR #1974);
`lf task status LOO-959` prints "stacked on pr_f3052042…"; #1972's base
on GitHub was already LOO-953's branch.

So the record is already single: `task_checkouts.parent_pr_id` → a parent
`task_prs` row. `task_stack` (`ops/task.rs:415`) resolves it for sync
(`ops/sync_execution.rs:85`) and publish (`ops/pr.rs:109`). The defects are
around it:

1. Publish overwrites whatever base the PR has on every update, so any
   disagreement is settled silently in the record's favour.
2. Changing the record on an existing Task doesn't move its PR
   (`stack_existing_task` prints "This Task's checkout and PR are unchanged"),
   so people reach for `gh pr edit --base`, which publish then undoes.
3. Nothing at hand points to the command. Steers, `lf sync <branch>` and the
   agent guide never mention `lf task checkout <ID> --stack-on <parent>` for a
   Task that already exists.
4. `lf task status` names the parent by PR row id (`pr_f3052042…`), not by
   Task; Desktop doesn't show it at all.

## Design

**Source of truth:** `task_checkouts.parent_pr_id`, unchanged. No new column,
no migration.

**Only commands that change the record move the PR's base.**

- `lf task checkout X --stack-on P` (and `lf task run X --stack-on P` on a
  placed Task) records P and retargets X's open PR to P's branch. The
  "checkout and PR are unchanged" line becomes "PR #N now targets <branch>;
  run `lf sync` to merge it."
- Detaching (`lf sync main` on a stacked Task) and the parent merging keep
  retargeting to main as they do today (`record_stack_sync`, `ops/task.rs:486`).
- `lf pr publish` sets the base on create, from the record. On update it reads
  the PR's current base (`baseRefName` from the same `gh pr list` that
  `find_open_pr` already runs) and:
  - same as the record: sends no `--base`;
  - the PR still targets the recorded parent's branch, and that parent has
    merged: retargets to main, as today;
  - anything else: leaves the base, publishes title and body, and prints one
    line naming both and the command for each direction, e.g.
    `PR #1972 targets jack/loo-953-… (LOO-953), but LOO-959 records no stack
    parent. Kept the PR's base. Record it: lf sync LOO-953. Follow the
    record: gh pr edit 1972 --base main`.
    When the PR's base is not an open Task's branch the first command is left
    out.
  The range check (`require_non_task_pr_range_nonempty`) keeps using the
  record's base.

`update_pr` loses its `base` argument. The base moves only through
`retarget_pr`.

**`lf sync <Task>` stacks** (choice made at concept review; see "Passed
over"). When `<onto>` names another open Task, by identifier (`LOO-953`) or by
its branch, `lf sync` is `lf checkout <self> --stack-on <Task>` followed by
`lf sync`: `stack_existing_task` records the parent (and recomputes the fork
when a different parent was recorded) and retargets the open PR, then the
ordinary stacked sync follows the record. This holds for an unstacked Task and
for one stacked on another parent, so the refusal "use the default branch to
detach this Task, or omit the target to sync its parent"
(`ops/sync_execution.rs:80`) goes. `lf sync main` still detaches; `lf sync`
with no target follows the record; `<onto>` that is not an open Task's branch
(a plain ref, or a Task whose PR merged) merges as today with no record change.
Recording before the merge matches `--stack-on` today: `base_commit` stays the
last fork until the sync succeeds, and a failed sync leaves the record on the
new parent, which the next `lf sync` retries.

Why: merging another Task's unmerged branch makes the child depend on it,
whatever the intent. Today that dependency is real in Git and absent from the
record, which is exactly LOO-959. The steer used this verb: "when they land on
its branch, sync onto it". The undo is one command: `lf sync main`.

**Reading the parent.** `lf task status` prints `stacked on LOO-953 (#1974,
jack/loo-953-…)` instead of the row id. `lf task status --json` and Desktop get
the same fact as a required nullable wire field on the Task (`stack_parent:
{identifier, branch, pr_number} | null`), with round-trip fixtures in Rust and
Swift. `pr_number` is null while the parent is unpublished (stacking records
an unpublished parent PR row). Desktop shows "Stacked on LOO-953" beside the
PR line. The wire Task's raw `parent_pr_id` stays for now; nothing a person
reads shows it.

**Teaching.** The `loopflow` builtin skill's Task block gains one line: to
build on another open Task, `lf sync <its ID>`; `lf sync main` undoes it;
never `git merge` a Task's branch or `gh pr edit --base`. `docs/agent-api.md`'s
`--stack-on` paragraph and `docs/work.md` say the same, and the `onto` help in
`rust/loopflow/src/lf/mod.rs` reads "Branch or Task to sync onto; another
open Task stacks this one on it, the default branch detaches".

### Passed over

- `lf sync <Task branch>` only hinting (the kickoff plan). It leaves the
  dependency in Git and out of the record, the state LOO-959 was in, and
  needs a second command to fix it. The case against stacking, syncing a
  sibling to borrow a fix, still makes the child depend on unmerged work; a
  stack says so, and `lf sync main` takes it back out.
- Restacking through sync with a merge that keeps the old parent's commits.
  Going through `stack_existing_task` recomputes the fork, so the stacked sync
  carries only child work onto the new parent.
- Detecting a raw `git merge` of a Task's branch at publish (the branch holds
  another open Task's unmerged head). It would catch LOO-959's first step, but
  it reads Git as a fourth record of the stack. Left as a question.

- Publish adopting the PR's base into the record when it names a Task's
  branch. It would have self-healed LOO-959, but it lets a GitHub edit (or
  GitHub's own retarget) rewrite lf's record, and "moved silently" is what the
  brief rules out. Kept as an open question.
- Publish forcing the record's base with a warning (today's behaviour plus a
  line). That's the move Jack had to undo twice.

## Acceptance check at gate

The fixture from the brief, in `rust/loopflow/tests/sync_tests.rs`, using the
`gh` stand-in from `task_pr_range_tests.rs` extended to answer
`baseRefName` and record `pr edit`:

1. Parent Task INF-123 with a commit; child INF-124 placed unstacked, then
   `lf checkout INF-124 --stack-on INF-123`.
2. `lf pr publish` twice in the child: the create carries `--base parent`; the
   second publish sends no `--base`; the stand-in's PR base is `parent` after
   both.
3. Parent gains a commit; `lf sync` in the child merges it (file present).
4. Disagreement: stand-in sets the child's PR base to `main`; `lf pr publish`
   leaves it, exits 0, and stderr names `gh pr edit … --base parent` and the
   record.
5. Stacking an already-published child retargets its PR (`pr edit --base
   parent` in the log).
6. A second, unstacked published child INF-125 runs `lf sync INF-123`: the
   parent's file is present, `lf task status INF-125` prints `stacked on
   INF-123`, the stand-in logged `pr edit … --base parent`, and a following
   `lf sync main` detaches it (record cleared, `pr edit … --base main`).

Run: `cargo test -p loopflow --test integration sync_tests::` with `LF_*` unset
and `< /dev/null`.

## Absent and error states

- `gh` can't read the PR's base: publish keeps today's path for create, and on
  update sends no `--base` (never guesses); `tracing::warn!` records it.
- Parent PR row missing: `task_stack` already errors with "stack parent … is
  missing"; unchanged.
- Retarget on `--stack-on` fails (no gh login): the record is written, and the
  error names `gh pr edit N --base <branch>` and exits nonzero.

## Exclusions

- `lf wt create` worktrees without a Task (`pr_target`'s upstream reading) keep
  their behaviour; they have no Task record.
- No change to how the parent merging clears the record.
- No migration of other Tasks' records: LOO-959 was the one seen, and it's done.

## What goes

- `update_pr`'s `base` parameter and the unconditional `--base` on update.
- `stack_existing_task`'s "checkout and PR are unchanged" message and the
  assertion on it (`sync_tests.rs:889`).
- `format_task_pr_line`'s raw `pr_…` id in favour of the parent's identifier.
- `task_pr_range_tests.rs:842` asserts `--base main` on "pr edit or pr
  create"; it narrows to create.
- Sync's refusal of an explicit non-default target on a stacked Task, and any
  test asserting it.

## Slices

1. **Publish keeps the PR's base** (teaches the most: whether `gh pr list`'s
   `baseRefName` plus the record gives every case its answer). `GhPr` gains
   `base_ref_name`; update compares, retargets only for a merged recorded
   parent, else prints the line. Fixture steps 1, 2 and 4. Shows: two
   publishes, base stays on the parent, and a disagreement prints both.
2. **Stacking moves the PR; `lf sync <Task>` stacks.** `stack_existing_task`
   retargets an open PR; sync resolves `<onto>` to an open Task (identifier,
   then branch via `task_checkouts_branch`) and calls it before the stacked
   sync. Fixture steps 3, 5 and 6.
3. **The parent is readable.** Status line by identifier; `stack_parent` wire
   field through `TaskRow`, the Desktop Task DTO and fixtures; Desktop label.
4. **Teach it.** `loopflow` builtin skill line and `docs/agent-api.md`.

### Progress (implement, 2026-11-18)

- Slice 1 built: `pr_base_update` / `pr_base_disagreement` in `ops/pr.rs`;
  `update_pr` sends no `--base`. `StackedSync` carries `parent_identifier`,
  `parent_branch` and `parent_merged` (`live_parent_branch()` replaces the old
  `Option`). Outside a Task worktree a differing base still retargets.
- Slice 2 built: `ops::task::open_task_named` (identifier, then branch; not
  self, not done/abandoned, PR not merged) and `ops::task::stack_task`;
  `stack_existing_task` returns `Stacked X on P[; note][; PR #N now targets B]`
  and retargets an Open PR. Sync prints it as a status line (stdout).
- Slice 3, Rust half built: `TaskSnapshot.stack_parent` (required nullable),
  status line, `task_checkout.json`, server schema.
- Slice 4: docs and LOOPFLOW.md were written ahead; goldens regenerated.
  Realign narrowed `docs/ops.md`'s "any other branch merges" line to unstacked
  Tasks, matching the refusal in `ops/sync_execution.rs:90`.
- Fixtures: `sync_tests::sync_onto_an_open_task_stacks_it_moves_its_pr_and_main_detaches_it`,
  `sync_tests::publish_keeps_the_prs_base_and_names_a_disagreement`, unit rows in
  `ops::pr::tests::` and `lf` bin `task_pr_line_names_the_stack_parent_by_task`.

- Slice 3, Desktop half built: `TaskStackParent` decoded on `TaskExecution`
  (required nullable); `TaskStackParentLabel` ("Stacked on LOO-12", help names
  branch and PR) beside the PR link in `WorkSurfaceView`. `task_lifecycle.json`
  gained `stack_parent: null` per phase; Rust's `dto_fixtures::composed_lifecycle_…`
  needs it. Checks: `scripts/test_desktop.sh --filter 'DTOFixtureTests|taskWorkFollowsTheStream'`
  passed (57 tests); `dto_fixtures::composed_lifecycle_preserves_completion_before_graph_arrival` passed.

Remaining:
- `lf sync <Task> --plan` does not resolve the Task; it plans the literal ref (open question).

## Larger than this work (on the page, not planned)

Placement (`branch`, `base_commit`, `parent_pr_id`) is stored on both
`task_checkouts` and the current `task_prs` row, copied by
`task_checkout_to_pr_*` triggers and guarded by `task_pr_placement_*`
("Task PR placement differs from Task"). They cannot disagree today, so this
Task leaves them; folding the PR row's copy away is a staged contract.

## Demo

In LOO-959's worktree: `lf task status LOO-959` prints `stacked on LOO-953
(#1974, …)`; `lf pr publish` updates #1972 and its base stays on LOO-953's
branch (`gh pr view 1972 --json baseRefName`); `lf sync` prints "Merging
origin/jack/loo-953-…". In the fixture, `lf sync INF-123` in an unstacked
child stacks it and moves its PR in one command.

## Docs written ahead (write-docs, 2026-11-18)

- `docs/ops.md` Checkouts: `lf sync LOO-12` example, its output, what sync
  records, never `git merge`/`gh pr edit --base`, publish keeps the base and
  prints the disagreement line.
- `docs/work.md`: one sentence on stacking an existing Task with `lf sync`.
- `docs/agent-api.md`: `lf sync INF-123` in Delegate and a paragraph after `--stack-on`.
- `rust/loopflow-definitions/src/builtins/LOOPFLOW.md`: the Teaching line.
- `rust/loopflow/src/lf/mod.rs`: `onto` help.

The docs promise this output for `lf sync <Task>`, which slice 2 should print:
`Stacked LOO-13 on LOO-12; PR #1972 now targets <branch>` before the usual
"Merging origin/<branch> into <branch>". With no PR, the clause after `;` drops.
