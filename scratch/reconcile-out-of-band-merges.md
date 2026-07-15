# Reconcile out-of-band merges & un-ignored scratch-stash (W2-171)

Out-of-band merges (GitHub auto-merge armed by `lf pr land`, not settled by
`lf pr land -c`) leave a Task in a stranded state that no advertised command
can repair. Three narrow, testable fixes, proven by two dogfood incidents:

- **W2-116 (PR #905)** — Task done; a stray open/draft sibling on the branch
  hid the merge, so completion refused. + scratch stash dirtied the tree.
- **W2-166 (PR #907)** — Task *not* done; reconcile observed the merge
  (`merge_commit` set) but there is **no sequence-2 PR**, the worktree holds
  preserved follow-up edits, and nothing advertised rotates forward.

## User-visible outcome

After an out-of-band merge, a developer or headless runner can, with one
advertised command and no manual git surgery:

1. **Complete a finished Task** — `lf task complete <issue> --summary "..."`
   succeeds even when a stray open/draft sibling PR sits on the branch.
2. **Continue a multi-PR Task** — `lf pr next [slug]` observes the merge,
   settles the merged PR, and rotates the worktree to the next serial PR
   **carrying the preserved follow-up edits forward**, ready to push.
3. Neither is blocked by a scratch stash dirtying the worktree.

## Root causes

1. **Wrong PR chosen during reconcile.** `current_or_merged_pr_for_branch`
   (`ops/pr.rs`) runs `gh pr list --head <branch> --state all` and takes
   `list.into_iter().next()` — the newest PR. A land can leave a stray
   open/draft sibling (W2-116: empty draft #909 beside merged #905). `.next()`
   returns the draft, so `reconcile_task_pr` never sets `merge_commit`, the
   TaskPr row stays unsettled, and `task_complete` (`ops/task.rs:1746`) refuses
   "Task has an open pull request".

2. **No advertised rotate-forward after an observed merge.** `ensure_working_pr`
   (`ops/task.rs:1514`) already does the right thing — reconcile, confirm the
   latest PR settled, create sequence+1 on a fresh branch from `origin/main`,
   push, record the new `TaskPr`. But it is called **only** by the task/project
   runners (`task/runner.rs:383`, `project_session/runner.rs:781`). After an
   out-of-band merge with the worker stopped, there is no `lf` command an
   operator or a re-entering agent can run to advance to PR 2. Worse, its
   `is_clean` guard (`ops/task.rs:1575`) *rejects* rotation when the worktree
   has uncommitted changes — which is exactly the preserved follow-up edits that
   are meant to become PR 2's diff (W2-166).

3. **Scratch stash lands outside the ignored prefix.** `scratch_stash_path`
   (`ops/rebase.rs:272`) builds `.lf/scratch-stash/<branch>-<ts>/`. The ignored
   siblings are `.lf/{log,logs,prompts,journal,summaries,tmp}` — `scratch-stash`
   is **not** ignored. `lf pr land` restores scratch there, dirtying the
   worktree, and `task_complete` (`ops/task.rs:1734`) refuses "Task worktree has
   uncommitted changes". Wave memory already documents the path as
   `.lf/tmp/scratch-stash/` — the code drifted out from under `tmp/`.

## Source of truth

- Merge state: GitHub, read via `gh pr list`. The authoritative *persisted*
  record is the `task_prs` row (`TaskPr`); reconcile projects GitHub's merge
  onto it by setting `merge_commit` and appending a `PrMerged` event, and
  rotation appends a new `TaskPr` + `PrStarted` event. Task events are
  **append-only** — reconcile and rotation only add, never rewrite (existing
  `should_append` guards preserve this).
- Serial position: the `sequence` field on `TaskPr`; the next branch is derived
  from the settled PR's `publication.next_slug` (or the sequence number) and the
  author prefix — unchanged.
- Stash location: derived, host-local scratch under the already-ignored
  `.lf/tmp/` prefix. Not a wire record.

## The fixes

### Fix 1 — prefer the merged PR (ops/pr.rs)

Replace the arbitrary `.next()` with a pure selection that ranks the branch's
PRs: **merged > open/draft > closed**, newest as tiebreak within a rank. A
merged PR is the branch's truth; a stray draft/closed sibling is noise. Extract
`fn select_reconcile_pr(prs: Vec<GhPr>) -> Option<GhPr>` (pure, unit-testable
without `gh`); `current_or_merged_pr_for_branch` parses JSON, calls it, maps the
winner to `PrInfo` as today. Only the reconcile read path changes; `find_open_pr`
/ `current_pr` (open-review flows) are untouched.

### Fix 2 — advertised rotate-forward (`lf pr next`)

**New subcommand** `PrCommand::Next { slug: Option<String> }` →
`lf pr next [slug]`. It:

1. Resolves the Task Session from the current worktree (as other `pr` commands
   do).
2. Calls the Fix-1 reconcile so the out-of-band merge is observed and PR N is
   settled.
3. Rotates to PR N+1 by driving the existing `ensure_working_pr` rotation core,
   **carrying uncommitted follow-up edits forward**: the new branch is created
   from a freshly fetched `origin/<default>` (which already contains merged
   PR N), so the worktree's edits are a clean delta and survive the
   `checkout -b`. `[slug]` names the branch; else the settled PR's stored
   `next_slug`; else the sequence number.
4. Pushes the new branch, records `TaskPr` sequence N+1 (`Working`), appends a
   `PrStarted` event.

**Relaxing the clean-tree guard:** the rotation core gains a `carry_dirty: bool`
parameter (a real behavioral fork between two production callers, not test
scaffolding). The runner keeps its current strict behavior (`false`); `lf pr
next` passes `true`. If the carry hits a genuine checkout conflict, surface it
with the branch name — never silently drop the edits.

**Advertise it:** add `lf pr next` to the runner's Task prompt
(`task/runner.rs:1069`) alongside `land --next` / `abandon` / `task complete`,
so a re-entering agent knows the recovery lever exists.

Error/guard behavior:
- Latest PR not settled (still open) → error: "current PR #N is open; land it
  or wait for merge before `lf pr next`".
- Task Session terminal/completed → nothing to rotate; report it, no-op.
- No Task Session for the worktree → error (as other `pr` commands).

### Fix 3 — stash under the ignored prefix (ops/rebase.rs)

`scratch_stash_path`: `.join("scratch-stash")` → `.join("tmp").join("scratch-stash")`.
One line. Matches the documented, already-gitignored path (chosen over adding
`.lf/scratch-stash/` to `.gitignore` because it converges code with the
documented path and keeps one stash root under `tmp/`).

## End-to-end proof

**Fix 1 (completion):** a TaskPr row is `Open` with a merged PR **and** an
open-draft sibling on the same branch. Reconcile sets `merge_commit` from the
merged PR and settles the row; `active_task_pr` returns `None`; `task_complete`
succeeds. Proof: unit test on `select_reconcile_pr` (merged wins over a newer
draft) + a `reconcile_task_pr` test driving a two-PR branch to a settled,
completable session (mock `gh`; assert row `merge_commit` set, `PrMerged`
appended once, no prior events rewritten).

**Fix 2 (continuation):** a Task Session with settled PR 1 (merged, no PR 2) and
uncommitted follow-up edits in the worktree. `lf pr next follow-up` creates a
`TaskPr` sequence 2 on branch `<author>/<slug>-follow-up` from fresh
`origin/main`, the follow-up edits are present on the new branch, and a
`PrStarted` event is appended. Proof: an `ensure_working_pr`-style test (the
existing `settled_pr_rotates_the_same_worktree_from_fetched_main` at
`ops/task.rs:2740` is the template) extended with a **dirty worktree** and
`carry_dirty = true`, asserting the edits survive onto sequence 2 and the
sequence list is `[1, 2]`.

**Fix 3 (clean tree):** after a reset that stashes `scratch/`, `git status` is
clean — the stash sits under `.lf/tmp/`. Proof: unit test asserting
`scratch_stash_path` is under `.lf/tmp/` and a stashed-and-restored tree leaves
no untracked path outside the ignored prefix.

## Affected surfaces / consumers

- **`lf pr next`** — new advertised command (rotate-forward after merge).
- `lf task complete` — proceeds on an out-of-band merged PR (behavior only
  broadens).
- `lf pr status` / task+project runner reconcile — share `reconcile_task_pr`;
  gain correct merge observation for free. Runner rotation behavior unchanged
  (`carry_dirty = false`).
- `lf pr land` / `lf rebase` — stash path moves under `.lf/tmp/`.
- Runner Task prompt text — advertises `lf pr next`.
- No wire DTO change, no migration: `TaskPr` fields and the `task_prs` schema
  are unchanged.

## Absent / error states

- Branch has **no** PR → reconcile no-ops (unchanged).
- Branch has only an open PR → selection returns it; `lf pr next` refuses with
  "current PR #N is open".
- gh reports merged **without** a merge commit → existing hard error in
  `reconcile_task_pr` (`ops/task.rs:1358`) stands; never silently settle.
- Rotate carry conflict → error naming the branch; edits preserved in the tree.
- `gh` unavailable → `current_or_merged_pr_for_branch` returns `None`; reconcile
  can't observe (unchanged).

## Operational boundary

Reconcile shells one `gh pr list`; selection is in-memory. `lf pr next` adds one
`git fetch` + `checkout -b` + `push` — the same git work the runner already does
on an in-band `land --next`. No new network round-trips beyond that.

## Exclusions

- **Automatic** rotation policy (should the runner also carry dirty edits? relax
  its guard?) → coordinate with **W2-169**; this Task ships the *advertised
  manual* lever and leaves the runner's strict path as-is.
- Broader PR-state ownership / lifecycle redesign → **W2-169**.
- No `--force` on `lf task complete`; no change to the abandon path.
- No cleanup of stray sibling PRs — reconcile ignores them by preferring merged.
- Committed-follow-up-on-the-merged-branch (edits already committed on top of the
  merge, not just uncommitted) is out of scope: the evidence is uncommitted WIP
  ("stopped before push"); `carry_dirty` handles working-tree edits. A committed
  variant, if it appears, is W2-169's rebase-onto-fresh-main concern.
