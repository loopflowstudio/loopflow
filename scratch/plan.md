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
    parent. Kept the PR's base. Record it: lf task checkout LOO-959 --stack-on
    LOO-953. Follow the record: gh pr edit 1972 --base main`.
    When the PR's base is not a Task's branch the first command is left out.
  The range check (`require_non_task_pr_range_nonempty`) keeps using the
  record's base.

`update_pr` loses its `base` argument. The base moves only through
`retarget_pr`.

**`lf sync <ref>`** on an unstacked Task, where `<ref>` is another open Task's
branch, merges as today and then prints the same hint: "LOO-959 is not stacked
on LOO-953, so its PR still targets main. Stack it: lf task checkout LOO-959
--stack-on LOO-953." No record change (see `scratch/questions.md`).

**Reading the parent.** `lf task status` prints `stacked on LOO-953 (#1974,
jack/loo-953-…)` instead of the row id. `lf task status --json` and Desktop get
the same fact as a required nullable wire field on the Task (`stack_parent:
{identifier, branch, pr_number} | null`), with round-trip fixtures in Rust and
Swift. Desktop shows "Stacked on LOO-953" beside the PR line.

**Teaching.** The `loopflow` builtin skill's Task block gains one line:
stack an existing Task with `lf task checkout <ID> --stack-on <parent>`; never
`git merge` a parent or `gh pr edit --base`. `docs/agent-api.md`'s
`--stack-on` paragraph says the same for placed Tasks.

### Passed over

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

## Slices

1. **Publish keeps the PR's base** (teaches the most: whether `gh pr list`'s
   `baseRefName` plus the record gives every case its answer). `GhPr` gains
   `base_ref_name`; update compares, retargets only for a merged recorded
   parent, else prints the line. Fixture steps 1, 2 and 4. Shows: two
   publishes, base stays on the parent, and a disagreement prints both.
2. **Stacking moves the PR; sync hints.** `stack_existing_task` retargets an
   open PR; `lf sync <Task branch>` on an unstacked Task prints the hint.
   Fixture steps 3 and 5.
3. **The parent is readable.** Status line by identifier; `stack_parent` wire
   field through `TaskRow`, the Desktop Task DTO and fixtures; Desktop label.
4. **Teach it.** `loopflow` builtin skill line and `docs/agent-api.md`.

## Demo

In LOO-959's worktree: `lf task status LOO-959` prints `stacked on LOO-953
(#1974, …)`; `lf pr publish` updates #1972 and its base stays on LOO-953's
branch (`gh pr view 1972 --json baseRefName`); `lf sync` prints "Merging
origin/jack/loo-953-…".
